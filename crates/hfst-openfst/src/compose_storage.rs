//! Bounded, single-touch materialization for lazy composition products.
//!
//! Accounting here covers the state and transition buffers owned by this
//! module. [`ComposeMemoryPlan`] partitions a caller's allowance before the
//! rustfst pair interner and this materializer receive their disjoint caps.
//! Loaded operands, allocator retention, and the final returned `VectorFst`
//! are outside that accounting, so the allowance is not a process RSS limit.
//! Spill trimming keeps every input-scaled work structure on disk. Its sort
//! chunks and page/frontier caches fit the materializer cap when that cap is at
//! least 16 MiB; smaller caps use a fixed operating envelope of at most 16 MiB
//! so even a zero-byte allowance can make forward progress.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use rustfst::fst_properties::FstProperties;
use rustfst::fst_traits::{AllocableFst, Fst, MutableFst};
use rustfst::semirings::Semiring;
use rustfst::{StateId, SymbolTable, Trs};

use crate::{StdTransition, StdVectorFst, TropicalWeight};

const SPOOL_MAGIC: &[u8; 8] = b"HFSTCSP1";
const SPOOL_FILE: &str = "states.bin";
const TRIMMED_STATES_MAGIC: &[u8; 8] = b"HFSTCTS1";
const TRIMMED_ARCS_MAGIC: &[u8; 8] = b"HFSTCTA1";
static SCRATCH_NONCE: AtomicU64 = AtomicU64::new(0);

/// One composition's optional memory allowance after policy partitioning.
///
/// A bounded plan leaves 10% of the exact allowance for allocator metadata,
/// page cache, and short-lived producer allocations. The remaining tracked
/// bytes are split 3:1 in favor of the pair interner, whose random-access hot
/// set benefits more from memory than the sequentially spilled product. Any
/// integer remainder also goes to the pair interner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComposeMemoryPlan {
    Unbounded,
    Bounded {
        allowance_bytes: u64,
        tracked_cap_bytes: u64,
        pair_interner_cap_bytes: u64,
        materializer_cap_bytes: u64,
    },
}

impl ComposeMemoryPlan {
    pub fn from_allowance(allowance_bytes: Option<u64>) -> Self {
        let Some(allowance_bytes) = allowance_bytes else {
            return Self::Unbounded;
        };

        // Compute floor(allowance * 9 / 10) without overflowing at u64::MAX.
        let quotient = allowance_bytes / 10;
        let remainder = allowance_bytes % 10;
        let tracked_cap_bytes = quotient * 9 + remainder * 9 / 10;
        let materializer_cap_bytes = tracked_cap_bytes / 4;
        let pair_interner_cap_bytes = tracked_cap_bytes - materializer_cap_bytes;
        Self::Bounded {
            allowance_bytes,
            tracked_cap_bytes,
            pair_interner_cap_bytes,
            materializer_cap_bytes,
        }
    }

    pub fn allowance_bytes(self) -> Option<u64> {
        match self {
            Self::Unbounded => None,
            Self::Bounded {
                allowance_bytes, ..
            } => Some(allowance_bytes),
        }
    }

    pub fn tracked_cap_bytes(self) -> Option<u64> {
        match self {
            Self::Unbounded => None,
            Self::Bounded {
                tracked_cap_bytes, ..
            } => Some(tracked_cap_bytes),
        }
    }

    pub fn pair_interner_cap_bytes(self) -> Option<u64> {
        match self {
            Self::Unbounded => None,
            Self::Bounded {
                pair_interner_cap_bytes,
                ..
            } => Some(pair_interner_cap_bytes),
        }
    }

    pub fn materializer_cap_bytes(self) -> Option<u64> {
        match self {
            Self::Unbounded => None,
            Self::Bounded {
                materializer_cap_bytes,
                ..
            } => Some(materializer_cap_bytes),
        }
    }
}

/// Storage policy for a single composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeStorageConfig {
    /// This materializer's already-partitioned tracked cap. `None` is
    /// unbounded. The caller must not pass the original user allowance here.
    pub tracked_memory_cap_bytes: Option<u64>,
    /// Parent directory for the operation-owned scratch directory.
    pub scratch_dir: PathBuf,
}

impl ComposeStorageConfig {
    pub fn bounded(tracked_memory_cap_bytes: u64, scratch_dir: impl Into<PathBuf>) -> Self {
        Self {
            tracked_memory_cap_bytes: Some(tracked_memory_cap_bytes),
            scratch_dir: scratch_dir.into(),
        }
    }

    pub fn unbounded(scratch_dir: impl Into<PathBuf>) -> Self {
        Self {
            tracked_memory_cap_bytes: None,
            scratch_dir: scratch_dir.into(),
        }
    }
}

/// A product retained in memory or in operation-owned scratch storage.
#[derive(Debug)]
pub enum ComposeArtifact {
    InMemory(StdVectorFst),
    Scratch(SpilledFst),
}

impl ComposeArtifact {
    /// Callers should first drop the producer and its operands, then call
    /// [`Self::prepare_for_reload`] when spilled. This method also prepares an
    /// unprepared spill as a safe fallback. Reloading allocates only survivor
    /// states, but that returned FST is not charged to the materializer cap.
    pub fn into_vector_fst(self) -> Result<StdVectorFst> {
        match self {
            Self::InMemory(fst) => Ok(fst),
            Self::Scratch(spilled) => spilled.into_vector_fst(),
        }
    }

    pub fn is_spilled(&self) -> bool {
        matches!(self, Self::Scratch(_))
    }

    /// Whether this artifact has already received rustfst-connect-equivalent
    /// accessible/coaccessible trimming.
    ///
    /// This is deliberately provenance, rather than an inference from FST
    /// property bits. It becomes true only after both trimmed scratch streams
    /// have been flushed and synced successfully.
    pub fn is_externally_trimmed(&self) -> bool {
        matches!(self, Self::Scratch(spilled) if spilled.is_externally_trimmed())
    }

    /// Durably trims a scratch artifact after its lazy producer and operands
    /// have been dropped. In-memory artifacts are intentionally left for the
    /// caller's ordinary `connect` path.
    pub fn prepare_for_reload(&mut self) -> Result<bool> {
        match self {
            Self::InMemory(_) => Ok(false),
            Self::Scratch(spilled) => {
                spilled.ensure_externally_trimmed()?;
                Ok(true)
            }
        }
    }
}

/// A dense state/transition stream stored beneath the scratch parent.
#[derive(Debug)]
pub struct SpilledFst {
    scratch: ScratchDir,
    data: Option<SpilledData>,
}

#[derive(Debug)]
enum SpilledData {
    Pending(PendingSpill),
    Trimmed(TrimmedSpill),
    Poisoned { original_state_count: u64 },
}

#[derive(Debug)]
struct TrimmedSpill {
    state_path: PathBuf,
    arc_path: PathBuf,
    state_count: u64,
    metadata: FstMetadata,
}

impl SpilledFst {
    pub fn scratch_dir(&self) -> &Path {
        self.scratch.path()
    }

    pub fn state_count(&self) -> u64 {
        // Pending artifacts report the accessible product size; after durable
        // preparation this reports the coaccessible survivor count.
        match self.data.as_ref().expect("live spilled artifact has data") {
            SpilledData::Pending(pending) => pending.state_count,
            SpilledData::Trimmed(trimmed) => trimmed.state_count,
            SpilledData::Poisoned {
                original_state_count,
            } => *original_state_count,
        }
    }

    pub fn is_externally_trimmed(&self) -> bool {
        matches!(self.data.as_ref(), Some(SpilledData::Trimmed(_)))
    }

    pub fn into_vector_fst(mut self) -> Result<StdVectorFst> {
        self.ensure_externally_trimmed()?;
        let fst = self.read_vector_fst()?;
        self.scratch.cleanup()?;
        Ok(fst)
    }

    fn ensure_externally_trimmed(&mut self) -> Result<()> {
        if self.is_externally_trimmed() {
            return Ok(());
        }
        let data = self
            .data
            .take()
            .expect("live spilled artifact has pending data");
        let SpilledData::Pending(pending) = data else {
            bail!("compose scratch artifact is unavailable after a previous trim failure")
        };
        let original_state_count = pending.state_count;
        match trim_scratch_spool(pending) {
            Ok(trimmed) => {
                self.data = Some(SpilledData::Trimmed(trimmed));
                Ok(())
            }
            Err(error) => {
                self.data = Some(SpilledData::Poisoned {
                    original_state_count,
                });
                Err(error)
            }
        }
    }

    fn read_vector_fst(&self) -> Result<StdVectorFst> {
        let Some(SpilledData::Trimmed(trimmed)) = self.data.as_ref() else {
            bail!("compose scratch artifact was not durably trimmed");
        };
        let state_file = File::open(&trimmed.state_path).with_context(|| {
            format!(
                "opening trimmed compose state spool {}",
                trimmed.state_path.display()
            )
        })?;
        let arc_file = File::open(&trimmed.arc_path).with_context(|| {
            format!(
                "opening trimmed compose arc spool {}",
                trimmed.arc_path.display()
            )
        })?;
        let mut state_reader = BufReader::new(state_file);
        let mut arc_reader = BufReader::new(arc_file);
        let mut magic = [0_u8; TRIMMED_STATES_MAGIC.len()];
        read_exact(&mut state_reader, &mut magic, &trimmed.state_path, "header")?;
        ensure!(
            &magic == TRIMMED_STATES_MAGIC,
            "invalid trimmed compose state header in {}",
            trimmed.state_path.display()
        );
        read_exact(&mut arc_reader, &mut magic, &trimmed.arc_path, "header")?;
        ensure!(
            &magic == TRIMMED_ARCS_MAGIC,
            "invalid trimmed compose arc header in {}",
            trimmed.arc_path.display()
        );

        let count = usize::try_from(trimmed.state_count)
            .context("compose scratch state count does not fit this platform")?;
        let mut fst = StdVectorFst::new();
        fst.reserve_states(count);
        fst.add_states(count);
        for index in 0..count {
            let state = StateId::try_from(index)
                .context("compose scratch state id exceeds rustfst StateId")?;
            let final_tag = read_u8(&mut state_reader, &trimmed.state_path, "final tag")?;
            let final_bits = read_u32(&mut state_reader, &trimmed.state_path, "final weight bits")?;
            match final_tag {
                0 => {}
                1 => {
                    fst.set_final(state, TropicalWeight::new(f32::from_bits(final_bits)))
                        .with_context(|| format!("restoring final weight for state {state}"))?;
                }
                tag => bail!("invalid final tag {tag} for compose state {state}"),
            }

            let arc_count = read_u64(&mut state_reader, &trimmed.state_path, "arc count")?;
            let arc_count =
                usize::try_from(arc_count).context("arc count does not fit this platform")?;
            fst.reserve_trs(state, arc_count)
                .with_context(|| format!("reserving arcs for compose state {state}"))?;
            for _ in 0..arc_count {
                let tr = read_transition(&mut arc_reader, &trimmed.arc_path)?;
                ensure!(
                    u64::from(tr.nextstate) < trimmed.state_count,
                    "compose scratch arc targets missing state {}",
                    tr.nextstate
                );
                fst.add_tr(state, tr)
                    .with_context(|| format!("restoring arc for state {state}"))?;
            }
        }

        let mut trailing = [0_u8; 1];
        ensure!(
            state_reader.read(&mut trailing).with_context(|| format!(
                "checking trimmed compose state spool {}",
                trimmed.state_path.display()
            ))? == 0,
            "trimmed compose state spool {} has trailing data",
            trimmed.state_path.display()
        );
        ensure!(
            arc_reader.read(&mut trailing).with_context(|| format!(
                "checking trimmed compose arc spool {}",
                trimmed.arc_path.display()
            ))? == 0,
            "trimmed compose arc spool {} has trailing data",
            trimmed.arc_path.display()
        );
        trimmed.metadata.apply(&mut fst)?;
        Ok(fst)
    }
}

/// Materializes a start-reachable product whose IDs are dense from zero.
///
/// Every state is expanded exactly once. The traversal never calls
/// states_iter, num_states, or num_trs, so it works with rustfst's NullCache.
/// Outgoing target IDs extend the numeric frontier without an explicit queue.
///
/// Only this module's buffers are charged to the materializer cap. The caller
/// must give the rustfst pair interner its separate cap from the same
/// [`ComposeMemoryPlan`].
pub fn materialize_fst<F>(fst: &F, config: &ComposeStorageConfig) -> Result<ComposeArtifact>
where
    F: Fst<TropicalWeight> + ?Sized,
{
    let metadata = FstMetadata::capture(fst);
    let Some(start) = metadata.start else {
        let mut empty = StdVectorFst::new();
        metadata.apply(&mut empty)?;
        return Ok(ComposeArtifact::InMemory(empty));
    };
    ensure!(
        start == 0,
        "bounded compose materialization requires start state 0, got {start}"
    );

    let mut storage = Storage::Memory(MemoryBuffer::default());
    let mut state_index = 0_usize;
    let mut discovered = 1_usize;
    while state_index < discovered {
        let state =
            StateId::try_from(state_index).context("compose product exceeded StateId capacity")?;
        let trs = fst
            .get_trs(state)
            .with_context(|| format!("expanding compose state {state}"))?;
        for tr in trs.trs() {
            let target_count = usize::try_from(u64::from(tr.nextstate) + 1)
                .context("compose target does not fit this platform")?;
            discovered = discovered.max(target_count);
        }
        let final_weight = fst
            .final_weight(state)
            .with_context(|| format!("computing final weight for compose state {state}"))?;
        storage = storage.append(final_weight, trs.trs(), config)?;
        state_index = state_index
            .checked_add(1)
            .context("compose state frontier overflow")?;
    }
    storage.finish(metadata, config)
}

#[derive(Clone, Debug)]
struct FstMetadata {
    start: Option<StateId>,
    properties: FstProperties,
    input_symbols: Option<Arc<SymbolTable>>,
    output_symbols: Option<Arc<SymbolTable>>,
}

impl FstMetadata {
    fn capture<F: Fst<TropicalWeight> + ?Sized>(fst: &F) -> Self {
        Self {
            start: fst.start(),
            properties: fst.properties(),
            input_symbols: fst.input_symbols().cloned(),
            output_symbols: fst.output_symbols().cloned(),
        }
    }

    fn apply(&self, fst: &mut StdVectorFst) -> Result<()> {
        if let Some(start) = self.start {
            fst.set_start(start)
                .with_context(|| format!("restoring compose start state {start}"))?;
        }
        if let Some(symbols) = &self.input_symbols {
            fst.set_input_symbols(Arc::clone(symbols));
        }
        if let Some(symbols) = &self.output_symbols {
            fst.set_output_symbols(Arc::clone(symbols));
        }
        fst.set_properties(self.properties);
        Ok(())
    }

    fn after_connect(&self, has_states: bool) -> Self {
        if self
            .properties
            .contains(FstProperties::ACCESSIBLE | FstProperties::COACCESSIBLE)
        {
            return self.clone();
        }
        let preserved = FstProperties::ACCEPTOR
            | FstProperties::I_DETERMINISTIC
            | FstProperties::O_DETERMINISTIC
            | FstProperties::NO_EPSILONS
            | FstProperties::NO_I_EPSILONS
            | FstProperties::NO_O_EPSILONS
            | FstProperties::I_LABEL_SORTED
            | FstProperties::O_LABEL_SORTED
            | FstProperties::UNWEIGHTED
            | FstProperties::ACYCLIC
            | FstProperties::INITIAL_ACYCLIC
            | FstProperties::TOP_SORTED
            | FstProperties::UNWEIGHTED_CYCLES;
        Self {
            start: has_states.then_some(0),
            properties: (self.properties & preserved)
                | FstProperties::ACCESSIBLE
                | FstProperties::COACCESSIBLE,
            input_symbols: self.input_symbols.clone(),
            output_symbols: self.output_symbols.clone(),
        }
    }
}

#[derive(Debug)]
enum Storage {
    Memory(MemoryBuffer),
    Scratch(ScratchWriter),
}

impl Storage {
    fn append(
        self,
        final_weight: Option<TropicalWeight>,
        trs: &[StdTransition],
        config: &ComposeStorageConfig,
    ) -> Result<Self> {
        match self {
            Self::Memory(mut memory) => {
                if memory.try_append(final_weight, trs, config.tracked_memory_cap_bytes)? {
                    Ok(Self::Memory(memory))
                } else {
                    let mut scratch = ScratchWriter::from_buffer(&config.scratch_dir, &memory)?;
                    scratch.write_state(final_weight, trs)?;
                    Ok(Self::Scratch(scratch))
                }
            }
            Self::Scratch(mut scratch) => {
                scratch.write_state(final_weight, trs)?;
                Ok(Self::Scratch(scratch))
            }
        }
    }

    fn finish(
        self,
        metadata: FstMetadata,
        config: &ComposeStorageConfig,
    ) -> Result<ComposeArtifact> {
        match self {
            Self::Scratch(scratch) => Ok(ComposeArtifact::Scratch(
                scratch.finish(metadata, config.tracked_memory_cap_bytes)?,
            )),
            Self::Memory(memory) if memory.conversion_fits(config.tracked_memory_cap_bytes)? => {
                Ok(ComposeArtifact::InMemory(memory.into_vector_fst(metadata)?))
            }
            Self::Memory(memory) => {
                let scratch = ScratchWriter::from_buffer(&config.scratch_dir, &memory)?;
                Ok(ComposeArtifact::Scratch(
                    scratch.finish(metadata, config.tracked_memory_cap_bytes)?,
                ))
            }
        }
    }
}

#[derive(Debug)]
struct BufferedState {
    final_weight: Option<TropicalWeight>,
    trs: Vec<StdTransition>,
}

#[derive(Debug, Default)]
struct MemoryBuffer {
    states: Vec<BufferedState>,
    arc_capacity: usize,
}

impl MemoryBuffer {
    fn try_append(
        &mut self,
        final_weight: Option<TropicalWeight>,
        trs: &[StdTransition],
        cap: Option<u64>,
    ) -> Result<bool> {
        let next_arc_capacity = self
            .arc_capacity
            .checked_add(trs.len())
            .context("compose arc-capacity accounting overflow")?;
        if exceeds_cap(
            bytes_for::<BufferedState>(self.states.len() + 1)?,
            bytes_for::<StdTransition>(next_arc_capacity)?,
            cap,
        ) {
            return Ok(false);
        }

        if self.states.len() == self.states.capacity() {
            self.states
                .try_reserve_exact(1)
                .context("reserving compose state buffer")?;
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(trs.len())
            .context("reserving compose arc buffer")?;
        let actual_arc_capacity = self
            .arc_capacity
            .checked_add(owned.capacity())
            .context("compose arc-capacity accounting overflow")?;
        if exceeds_cap(
            bytes_for::<BufferedState>(self.states.capacity())?,
            bytes_for::<StdTransition>(actual_arc_capacity)?,
            cap,
        ) {
            return Ok(false);
        }

        owned.extend_from_slice(trs);
        self.arc_capacity = actual_arc_capacity;
        self.states.push(BufferedState {
            final_weight,
            trs: owned,
        });
        Ok(true)
    }

    fn conversion_fits(&self, cap: Option<u64>) -> Result<bool> {
        let buffered_state_bytes = bytes_for::<BufferedState>(self.states.capacity())?;
        let transition_bytes = bytes_for::<StdTransition>(self.arc_capacity)?;
        // VectorFstState is private. BufferedState has the same sized payload
        // on supported targets and conservatively charges the handoff overlap.
        let output_state_bytes = bytes_for::<BufferedState>(self.states.len())?;
        let total = buffered_state_bytes
            .checked_add(transition_bytes)
            .and_then(|bytes| bytes.checked_add(output_state_bytes));
        Ok(match cap {
            None => total.is_some(),
            Some(cap) => total.is_some_and(|bytes| bytes <= cap),
        })
    }

    fn into_vector_fst(self, metadata: FstMetadata) -> Result<StdVectorFst> {
        let mut fst = StdVectorFst::new();
        fst.reserve_states(self.states.len());
        for state in self.states {
            let state_id = fst.add_state();
            if let Some(weight) = state.final_weight {
                fst.set_final(state_id, weight)
                    .with_context(|| format!("restoring final weight for state {state_id}"))?;
            }
            fst.reserve_trs(state_id, state.trs.len())
                .with_context(|| format!("reserving arcs for state {state_id}"))?;
            for tr in state.trs {
                fst.add_tr(state_id, tr)
                    .with_context(|| format!("restoring arc for state {state_id}"))?;
            }
        }
        metadata.apply(&mut fst)?;
        Ok(fst)
    }
}

fn exceeds_cap(first: u64, second: u64, cap: Option<u64>) -> bool {
    match cap {
        None => first.checked_add(second).is_none(),
        Some(cap) => first.checked_add(second).is_none_or(|bytes| bytes > cap),
    }
}

fn bytes_for<T>(capacity: usize) -> Result<u64> {
    u64::try_from(capacity)
        .context("capacity does not fit u64")?
        .checked_mul(u64::try_from(size_of::<T>()).context("element size does not fit u64")?)
        .context("compose memory accounting overflow")
}

#[derive(Debug)]
struct ScratchWriter {
    writer: BufWriter<File>,
    scratch: ScratchDir,
    spool_path: PathBuf,
    state_count: u64,
    arc_count: u64,
    final_count: u64,
}

impl ScratchWriter {
    fn from_buffer(parent: &Path, buffer: &MemoryBuffer) -> Result<Self> {
        let scratch = ScratchDir::create(parent)?;
        let spool_path = scratch.path().join(SPOOL_FILE);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&spool_path)
            .with_context(|| format!("creating compose scratch spool {}", spool_path.display()))?;
        let mut writer = Self {
            writer: BufWriter::new(file),
            scratch,
            spool_path,
            state_count: 0,
            arc_count: 0,
            final_count: 0,
        };
        writer.write_bytes(SPOOL_MAGIC, "header")?;
        for state in &buffer.states {
            writer.write_state(state.final_weight, &state.trs)?;
        }
        // Existing memory is released only after the copied prefix is durable
        // enough for subsequent reads.
        writer.flush("committing buffered compose states")?;
        Ok(writer)
    }

    fn write_state(
        &mut self,
        final_weight: Option<TropicalWeight>,
        trs: &[StdTransition],
    ) -> Result<()> {
        match final_weight {
            None => self.write_bytes(&[0], "final tag")?,
            Some(weight) => {
                self.write_bytes(&[1], "final tag")?;
                self.write_bytes(&weight.value().to_bits().to_le_bytes(), "final weight")?;
                self.final_count = self
                    .final_count
                    .checked_add(1)
                    .context("compose scratch final-state count overflow")?;
            }
        }
        self.write_bytes(
            &u64::try_from(trs.len())
                .context("arc count does not fit u64")?
                .to_le_bytes(),
            "arc count",
        )?;
        for tr in trs {
            self.write_bytes(&tr.ilabel.to_le_bytes(), "input label")?;
            self.write_bytes(&tr.olabel.to_le_bytes(), "output label")?;
            self.write_bytes(&tr.weight.value().to_bits().to_le_bytes(), "arc weight")?;
            self.write_bytes(&tr.nextstate.to_le_bytes(), "arc target")?;
        }
        self.arc_count = self
            .arc_count
            .checked_add(u64::try_from(trs.len()).context("arc count does not fit u64")?)
            .context("compose scratch total arc count overflow")?;
        self.state_count = self
            .state_count
            .checked_add(1)
            .context("compose scratch state count overflow")?;
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8], what: &str) -> Result<()> {
        let path = self.spool_path.display().to_string();
        self.writer.write_all(bytes).with_context(|| {
            format!("writing {what} to compose scratch spool {path} (scratch disk may be full)")
        })
    }

    fn flush(&mut self, what: &str) -> Result<()> {
        let path = self.spool_path.display().to_string();
        self.writer
            .flush()
            .with_context(|| format!("{what} at {path} (scratch disk may be full)"))
    }

    fn finish(
        mut self,
        metadata: FstMetadata,
        tracked_memory_cap_bytes: Option<u64>,
    ) -> Result<SpilledFst> {
        self.flush("flushing compose scratch spool")?;
        self.writer.get_ref().sync_data().with_context(|| {
            format!(
                "syncing compose scratch spool {} (scratch disk may be full)",
                self.spool_path.display()
            )
        })?;
        let Self {
            writer,
            scratch,
            spool_path,
            state_count,
            arc_count,
            final_count,
        } = self;
        drop(writer);
        let scratch_path = scratch.path().to_path_buf();
        Ok(SpilledFst {
            scratch,
            data: Some(SpilledData::Pending(PendingSpill {
                scratch_path,
                spool_path,
                state_count,
                arc_count,
                final_count,
                metadata,
                tracked_memory_cap_bytes,
            })),
        })
    }
}

mod trim;

use trim::{PendingSpill, trim_scratch_spool};

#[derive(Debug)]
struct ScratchDir(Option<PathBuf>);

impl ScratchDir {
    fn create(parent: &Path) -> Result<Self> {
        for _ in 0..128 {
            let sequence = SCRATCH_NONCE.fetch_add(1, Ordering::Relaxed);
            let clock = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = parent.join(format!(
                ".hfst-compose.{}.{}.scratch",
                std::process::id(),
                clock ^ u128::from(sequence)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(Some(path))),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("creating compose scratch directory {}", path.display())
                    });
                }
            }
        }
        bail!(
            "could not create compose scratch directory in {}",
            parent.display()
        )
    }

    fn path(&self) -> &Path {
        self.0
            .as_deref()
            .expect("live scratch directory has a path")
    }

    fn cleanup(&mut self) -> Result<()> {
        let Some(path) = self.0.clone() else {
            return Ok(());
        };
        match fs::remove_dir_all(&path) {
            Ok(()) => {
                self.0 = None;
                Ok(())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.0 = None;
                Ok(())
            }
            Err(error) => Err(error)
                .with_context(|| format!("removing compose scratch directory {}", path.display())),
        }
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _cleanup_result = fs::remove_dir_all(path);
        }
    }
}

fn read_transition(reader: &mut impl Read, path: &Path) -> Result<StdTransition> {
    let ilabel = read_u32(reader, path, "input label")?;
    let olabel = read_u32(reader, path, "output label")?;
    let weight = f32::from_bits(read_u32(reader, path, "arc weight")?);
    let nextstate = read_u32(reader, path, "arc target")?;
    Ok(StdTransition::new(ilabel, olabel, weight, nextstate))
}

fn read_u8(reader: &mut impl Read, path: &Path, what: &str) -> Result<u8> {
    let mut bytes = [0_u8; 1];
    read_exact(reader, &mut bytes, path, what)?;
    Ok(bytes[0])
}

fn read_u32(reader: &mut impl Read, path: &Path, what: &str) -> Result<u32> {
    let mut bytes = [0_u8; size_of::<u32>()];
    read_exact(reader, &mut bytes, path, what)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read, path: &Path, what: &str) -> Result<u64> {
    let mut bytes = [0_u8; size_of::<u64>()];
    read_exact(reader, &mut bytes, path, what)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_exact(reader: &mut impl Read, bytes: &mut [u8], path: &Path, what: &str) -> Result<()> {
    reader.read_exact(bytes).with_context(|| {
        format!(
            "reading {what} from compose scratch spool {}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests;
