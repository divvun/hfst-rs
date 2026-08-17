//! Disk-backed coaccessibility analysis and stable survivor rewriting.
//!
//! The source spool is scanned into bounded reverse-edge sort runs. Disk-backed
//! reachability and rank indexes then preserve ascending state renumbering and
//! within-state arc order without retaining an input-sized map in memory.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::io::{Seek, SeekFrom};

use super::*;

const TRIMMED_STATES_FILE: &str = "trimmed-states.bin";
const TRIMMED_ARCS_FILE: &str = "trimmed-arcs.bin";
const FINAL_STATES_FILE: &str = "final-states.bin";
const REVERSE_OFFSETS_FILE: &str = "reverse-offsets.bin";
const REACHABLE_QUEUE_FILE: &str = "coaccessible-queue.bin";
const LIVE_BITS_FILE: &str = "coaccessible-bits.bin";
const LIVE_RANKS_FILE: &str = "coaccessible-ranks.bin";
const REVERSE_EDGE_BYTES: u64 = 8;
const PAGE_BYTES: usize = 64 * 1024;
const OPERATIONAL_FLOOR_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SORT_CHUNK_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PAGE_CACHE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_QUEUE_CACHE_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const MERGE_FAN_IN: u64 = 32;

#[derive(Debug)]
pub(super) struct PendingSpill {
    pub(super) scratch_path: PathBuf,
    pub(super) spool_path: PathBuf,
    pub(super) state_count: u64,
    pub(super) arc_count: u64,
    pub(super) final_count: u64,
    pub(super) metadata: FstMetadata,
    pub(super) tracked_memory_cap_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TrimBudget {
    pub(super) io_buffer_bytes: usize,
    pub(super) sort_records: usize,
    pub(super) page_cache_pages: usize,
    pub(super) queue_records: usize,
}

impl TrimBudget {
    fn new(cap: Option<u64>) -> Result<Self> {
        // A small fixed operating envelope is necessary even when the caller
        // deliberately selects a zero-byte construction cap. No allocation in
        // this envelope scales with the input: scalable sets, queues, offsets,
        // and ranks remain on disk.
        let envelope = cap
            .unwrap_or(MAX_SORT_CHUNK_BYTES)
            .max(OPERATIONAL_FLOOR_BYTES);
        let io_bytes = (envelope / 256).clamp(1, 64 * 1024);
        let io_buffer_bytes = usize::try_from(io_bytes).context("I/O buffer size overflow")?;
        let sort_bytes = envelope
            .saturating_sub(io_bytes.saturating_mul(2))
            .clamp(REVERSE_EDGE_BYTES, MAX_SORT_CHUNK_BYTES);
        let sort_records = usize::try_from(sort_bytes / REVERSE_EDGE_BYTES)
            .context("reverse-edge sort chunk does not fit this platform")?
            .max(1);
        let cache_bytes = (envelope / 2)
            .min(MAX_PAGE_CACHE_BYTES)
            .max(u64::try_from(PAGE_BYTES).context("page size does not fit u64")?);
        let page_cache_pages = usize::try_from(
            cache_bytes / u64::try_from(PAGE_BYTES).context("page size does not fit u64")?,
        )
        .context("page-cache size does not fit this platform")?
        .max(1);
        let queue_bytes = (envelope / 4)
            .min(MAX_QUEUE_CACHE_BYTES)
            .max(u64::try_from(size_of::<StateId>()).context("state-id size does not fit u64")?);
        let queue_records = usize::try_from(
            queue_bytes
                / u64::try_from(size_of::<StateId>()).context("state-id size does not fit u64")?,
        )
        .context("queue-cache size does not fit this platform")?
        .max(1);
        Ok(Self {
            io_buffer_bytes,
            sort_records,
            page_cache_pages,
            queue_records,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ReverseEdge {
    pub(super) target: StateId,
    pub(super) source: StateId,
}

pub(super) fn reverse_run_path(scratch: &Path, generation: u64, index: u64) -> PathBuf {
    scratch.join(format!("reverse-run-{generation}-{index}.bin"))
}

pub(super) fn create_new_file(path: &Path, what: &str) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "creating {what} {} (scratch disk may be full)",
                path.display()
            )
        })
}

fn flush_and_sync(writer: &mut BufWriter<File>, path: &Path, what: &str) -> Result<()> {
    writer.flush().with_context(|| {
        format!(
            "flushing {what} {} (scratch disk may be full)",
            path.display()
        )
    })?;
    writer.get_ref().sync_data().with_context(|| {
        format!(
            "syncing {what} {} (scratch disk may be full)",
            path.display()
        )
    })
}

pub(super) fn write_reverse_edge(
    writer: &mut impl Write,
    edge: ReverseEdge,
    path: &Path,
) -> Result<()> {
    writer
        .write_all(&edge.target.to_le_bytes())
        .and_then(|()| writer.write_all(&edge.source.to_le_bytes()))
        .with_context(|| {
            format!(
                "writing reverse compose edge to {} (scratch disk may be full)",
                path.display()
            )
        })
}

fn read_reverse_edge(reader: &mut impl Read, path: &Path) -> Result<ReverseEdge> {
    Ok(ReverseEdge {
        target: read_u32(reader, path, "reverse-edge target")?,
        source: read_u32(reader, path, "reverse-edge source")?,
    })
}

pub(super) fn read_reverse_edge_opt(
    reader: &mut impl Read,
    path: &Path,
) -> Result<Option<ReverseEdge>> {
    let mut bytes = [0_u8; REVERSE_EDGE_BYTES as usize];
    let read = reader
        .read(&mut bytes[..1])
        .with_context(|| format!("reading reverse compose edge from {}", path.display()))?;
    if read == 0 {
        return Ok(None);
    }
    read_exact(reader, &mut bytes[1..], path, "reverse-edge record")?;
    Ok(Some(ReverseEdge {
        target: StateId::from_le_bytes(bytes[..4].try_into().expect("four target bytes")),
        source: StateId::from_le_bytes(bytes[4..].try_into().expect("four source bytes")),
    }))
}

fn write_sorted_run(
    scratch: &Path,
    run_index: u64,
    chunk: &mut Vec<ReverseEdge>,
    budget: TrimBudget,
) -> Result<()> {
    chunk.sort_unstable();
    let path = reverse_run_path(scratch, 0, run_index);
    let file = create_new_file(&path, "reverse-edge sort run")?;
    let mut writer = BufWriter::with_capacity(budget.io_buffer_bytes, file);
    for &edge in chunk.iter() {
        write_reverse_edge(&mut writer, edge, &path)?;
    }
    flush_and_sync(&mut writer, &path, "reverse-edge sort run")?;
    chunk.clear();
    Ok(())
}

fn generate_reverse_runs(pending: &PendingSpill, budget: TrimBudget) -> Result<(u64, PathBuf)> {
    let file = File::open(&pending.spool_path).with_context(|| {
        format!(
            "opening compose scratch spool {} for reverse-edge sorting",
            pending.spool_path.display()
        )
    })?;
    let mut reader = BufReader::with_capacity(budget.io_buffer_bytes, file);
    let mut magic = [0_u8; SPOOL_MAGIC.len()];
    read_exact(&mut reader, &mut magic, &pending.spool_path, "header")?;
    ensure!(
        &magic == SPOOL_MAGIC,
        "invalid compose scratch header in {}",
        pending.spool_path.display()
    );

    let finals_path = pending.scratch_path.join(FINAL_STATES_FILE);
    let finals_file = create_new_file(&finals_path, "compose final-state stream")?;
    let mut finals_writer = BufWriter::with_capacity(budget.io_buffer_bytes, finals_file);
    let mut chunk = Vec::new();
    chunk
        .try_reserve_exact(budget.sort_records)
        .context("reserving bounded reverse-edge sort chunk")?;
    let mut run_count = 0_u64;
    let mut seen_arcs = 0_u64;
    let mut seen_finals = 0_u64;

    for source_index in 0..pending.state_count {
        let source = StateId::try_from(source_index)
            .context("compose scratch source id exceeds rustfst StateId")?;
        match read_u8(&mut reader, &pending.spool_path, "final tag")? {
            0 => {}
            1 => {
                let _weight = read_u32(&mut reader, &pending.spool_path, "final weight")?;
                finals_writer
                    .write_all(&source.to_le_bytes())
                    .with_context(|| {
                        format!(
                            "writing compose final-state stream {} (scratch disk may be full)",
                            finals_path.display()
                        )
                    })?;
                seen_finals = seen_finals
                    .checked_add(1)
                    .context("compose final-state count overflow")?;
            }
            tag => bail!("invalid final tag {tag} for compose state {source}"),
        }
        let state_arcs = read_u64(&mut reader, &pending.spool_path, "arc count")?;
        for _ in 0..state_arcs {
            let tr = read_transition(&mut reader, &pending.spool_path)?;
            ensure!(
                u64::from(tr.nextstate) < pending.state_count,
                "compose scratch arc targets missing state {}",
                tr.nextstate
            );
            chunk.push(ReverseEdge {
                target: tr.nextstate,
                source,
            });
            seen_arcs = seen_arcs
                .checked_add(1)
                .context("compose reverse-edge count overflow")?;
            if chunk.len() == budget.sort_records {
                write_sorted_run(&pending.scratch_path, run_count, &mut chunk, budget)?;
                run_count = run_count
                    .checked_add(1)
                    .context("compose reverse-edge run count overflow")?;
            }
        }
    }
    ensure!(
        seen_arcs == pending.arc_count,
        "compose scratch arc count changed: expected {}, found {seen_arcs}",
        pending.arc_count
    );
    ensure!(
        seen_finals == pending.final_count,
        "compose scratch final count changed: expected {}, found {seen_finals}",
        pending.final_count
    );
    let mut trailing = [0_u8; 1];
    ensure!(
        reader.read(&mut trailing).with_context(|| format!(
            "checking compose scratch spool {}",
            pending.spool_path.display()
        ))? == 0,
        "compose scratch spool {} has trailing data",
        pending.spool_path.display()
    );
    if !chunk.is_empty() || run_count == 0 {
        write_sorted_run(&pending.scratch_path, run_count, &mut chunk, budget)?;
        run_count = run_count
            .checked_add(1)
            .context("compose reverse-edge run count overflow")?;
    }
    flush_and_sync(
        &mut finals_writer,
        &finals_path,
        "compose final-state stream",
    )?;
    Ok((run_count, finals_path))
}

struct ReverseRunReader {
    path: PathBuf,
    reader: BufReader<File>,
}

fn merge_reverse_run_group(
    scratch: &Path,
    input_generation: u64,
    start: u64,
    end: u64,
    output_path: &Path,
    budget: TrimBudget,
) -> Result<()> {
    let mut readers = Vec::new();
    readers
        .try_reserve_exact(usize::try_from(end - start).context("merge fan-in overflow")?)
        .context("reserving reverse-edge merge readers")?;
    for index in start..end {
        let path = reverse_run_path(scratch, input_generation, index);
        let file = File::open(&path)
            .with_context(|| format!("opening reverse-edge sort run {}", path.display()))?;
        readers.push(ReverseRunReader {
            path,
            reader: BufReader::with_capacity(budget.io_buffer_bytes, file),
        });
    }
    let output = create_new_file(output_path, "merged reverse-edge run")?;
    let mut writer = BufWriter::with_capacity(budget.io_buffer_bytes, output);
    let mut heap = BinaryHeap::new();
    for (index, input) in readers.iter_mut().enumerate() {
        if let Some(edge) = read_reverse_edge_opt(&mut input.reader, &input.path)? {
            heap.push(Reverse((edge, index)));
        }
    }
    while let Some(Reverse((edge, input_index))) = heap.pop() {
        write_reverse_edge(&mut writer, edge, output_path)?;
        let input = &mut readers[input_index];
        if let Some(next) = read_reverse_edge_opt(&mut input.reader, &input.path)? {
            heap.push(Reverse((next, input_index)));
        }
    }
    flush_and_sync(&mut writer, output_path, "merged reverse-edge run")?;
    drop(writer);
    drop(readers);
    for index in start..end {
        let path = reverse_run_path(scratch, input_generation, index);
        fs::remove_file(&path)
            .with_context(|| format!("removing reverse-edge sort run {}", path.display()))?;
    }
    Ok(())
}

pub(super) fn merge_reverse_runs(
    scratch: &Path,
    mut run_count: u64,
    budget: TrimBudget,
) -> Result<PathBuf> {
    let mut generation = 0_u64;
    while run_count > 1 {
        let next_generation = generation
            .checked_add(1)
            .context("reverse-edge merge generation overflow")?;
        let mut output_index = 0_u64;
        let mut start = 0_u64;
        while start < run_count {
            let end = start.saturating_add(MERGE_FAN_IN).min(run_count);
            let output_path = reverse_run_path(scratch, next_generation, output_index);
            if end - start == 1 {
                let input_path = reverse_run_path(scratch, generation, start);
                fs::rename(&input_path, &output_path).with_context(|| {
                    format!(
                        "carrying reverse-edge sort run {} to {}",
                        input_path.display(),
                        output_path.display()
                    )
                })?;
            } else {
                merge_reverse_run_group(scratch, generation, start, end, &output_path, budget)?;
            }
            output_index = output_index
                .checked_add(1)
                .context("reverse-edge merged-run count overflow")?;
            start = end;
        }
        generation = next_generation;
        run_count = output_index;
    }
    Ok(reverse_run_path(scratch, generation, 0))
}

fn build_reverse_offsets(
    pending: &PendingSpill,
    sorted_edges_path: &Path,
    budget: TrimBudget,
) -> Result<PathBuf> {
    let edge_file = File::open(sorted_edges_path).with_context(|| {
        format!(
            "opening sorted reverse-edge stream {}",
            sorted_edges_path.display()
        )
    })?;
    let mut edges = BufReader::with_capacity(budget.io_buffer_bytes, edge_file);
    let offsets_path = pending.scratch_path.join(REVERSE_OFFSETS_FILE);
    let offsets_file = create_new_file(&offsets_path, "compose reverse-offset index")?;
    let mut offsets = BufWriter::with_capacity(budget.io_buffer_bytes, offsets_file);
    let mut next = read_reverse_edge_opt(&mut edges, sorted_edges_path)?;
    let mut previous = None;
    let mut edge_index = 0_u64;

    for target_index in 0..pending.state_count {
        offsets
            .write_all(&edge_index.to_le_bytes())
            .with_context(|| {
                format!(
                    "writing compose reverse-offset index {} (scratch disk may be full)",
                    offsets_path.display()
                )
            })?;
        while next.is_some_and(|edge| u64::from(edge.target) == target_index) {
            let edge = next.expect("edge checked above");
            ensure!(
                u64::from(edge.source) < pending.state_count,
                "reverse compose edge has missing source {}",
                edge.source
            );
            if let Some(previous) = previous {
                ensure!(
                    previous <= edge,
                    "reverse compose edge stream {} is not sorted",
                    sorted_edges_path.display()
                );
            }
            previous = Some(edge);
            edge_index = edge_index
                .checked_add(1)
                .context("reverse compose edge index overflow")?;
            next = read_reverse_edge_opt(&mut edges, sorted_edges_path)?;
        }
        if let Some(edge) = next {
            ensure!(
                u64::from(edge.target) > target_index
                    && u64::from(edge.target) < pending.state_count,
                "reverse compose edge has invalid target {}",
                edge.target
            );
        }
    }
    offsets
        .write_all(&edge_index.to_le_bytes())
        .with_context(|| {
            format!(
                "writing compose reverse-offset terminator {} (scratch disk may be full)",
                offsets_path.display()
            )
        })?;
    ensure!(
        next.is_none(),
        "reverse compose edge stream {} contains an out-of-range target",
        sorted_edges_path.display()
    );
    ensure!(
        edge_index == pending.arc_count,
        "reverse compose edge count changed: expected {}, found {edge_index}",
        pending.arc_count
    );
    flush_and_sync(&mut offsets, &offsets_path, "compose reverse-offset index")?;
    Ok(offsets_path)
}

#[derive(Debug)]
struct CachedPage {
    file_page: u64,
    valid_len: usize,
    bytes: Vec<u8>,
    dirty: bool,
    last_used: u64,
}

#[derive(Debug)]
struct PageCache {
    file: File,
    path: PathBuf,
    logical_len: u64,
    writable: bool,
    max_pages: usize,
    clock: u64,
    pages: Vec<CachedPage>,
    locations: HashMap<u64, usize>,
}

impl PageCache {
    fn new(
        file: File,
        path: PathBuf,
        logical_len: u64,
        writable: bool,
        max_pages: usize,
    ) -> Result<Self> {
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(max_pages)
            .context("reserving bounded compose page cache")?;
        let mut locations = HashMap::new();
        locations
            .try_reserve(max_pages)
            .context("reserving bounded compose page index")?;
        Ok(Self {
            file,
            path,
            logical_len,
            writable,
            max_pages: max_pages.max(1),
            clock: 0,
            pages,
            locations,
        })
    }

    fn page_slot(&mut self, file_page: u64) -> Result<usize> {
        self.clock = self.clock.wrapping_add(1);
        if let Some(&slot) = self.locations.get(&file_page) {
            self.pages[slot].last_used = self.clock;
            return Ok(slot);
        }

        let slot = if self.pages.len() < self.max_pages {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(PAGE_BYTES)
                .context("reserving compose page-cache page")?;
            bytes.resize(PAGE_BYTES, 0);
            self.pages.push(CachedPage {
                file_page,
                valid_len: 0,
                bytes,
                dirty: false,
                last_used: self.clock,
            });
            self.pages.len() - 1
        } else {
            let (slot, _) = self
                .pages
                .iter()
                .enumerate()
                .min_by_key(|(_, page)| page.last_used)
                .expect("page cache always has an eviction candidate");
            self.flush_page(slot)?;
            self.locations.remove(&self.pages[slot].file_page);
            slot
        };

        let page_bytes = u64::try_from(PAGE_BYTES).context("page size does not fit u64")?;
        let page_start = file_page
            .checked_mul(page_bytes)
            .context("compose page offset overflow")?;
        ensure!(
            page_start < self.logical_len || self.logical_len == 0,
            "compose page offset {page_start} exceeds {}",
            self.path.display()
        );
        let valid_len = usize::try_from((self.logical_len - page_start).min(page_bytes))
            .context("compose page length does not fit this platform")?;
        let page = &mut self.pages[slot];
        page.file_page = file_page;
        page.valid_len = valid_len;
        page.bytes.fill(0);
        page.dirty = false;
        page.last_used = self.clock;
        self.file
            .seek(SeekFrom::Start(page_start))
            .with_context(|| format!("seeking compose page file {}", self.path.display()))?;
        read_exact(
            &mut self.file,
            &mut page.bytes[..valid_len],
            &self.path,
            "page-cache page",
        )?;
        self.locations.insert(file_page, slot);
        Ok(slot)
    }

    fn flush_page(&mut self, slot: usize) -> Result<()> {
        let page = &mut self.pages[slot];
        if !page.dirty {
            return Ok(());
        }
        ensure!(
            self.writable,
            "attempted to write a read-only compose page cache"
        );
        let offset = page
            .file_page
            .checked_mul(u64::try_from(PAGE_BYTES).context("page size does not fit u64")?)
            .context("compose page offset overflow")?;
        self.file
            .seek(SeekFrom::Start(offset))
            .with_context(|| format!("seeking compose page file {}", self.path.display()))?;
        self.file
            .write_all(&page.bytes[..page.valid_len])
            .with_context(|| {
                format!(
                    "writing compose page file {} (scratch disk may be full)",
                    self.path.display()
                )
            })?;
        page.dirty = false;
        Ok(())
    }

    fn read_byte(&mut self, offset: u64) -> Result<u8> {
        ensure!(
            offset < self.logical_len,
            "compose page read offset {offset} exceeds {}",
            self.path.display()
        );
        let page_bytes = u64::try_from(PAGE_BYTES).context("page size does not fit u64")?;
        let slot = self.page_slot(offset / page_bytes)?;
        let within = usize::try_from(offset % page_bytes)
            .context("compose in-page offset does not fit this platform")?;
        Ok(self.pages[slot].bytes[within])
    }

    fn write_byte(&mut self, offset: u64, value: u8) -> Result<()> {
        ensure!(
            self.writable,
            "attempted to write a read-only compose page cache"
        );
        let page_bytes = u64::try_from(PAGE_BYTES).context("page size does not fit u64")?;
        let slot = self.page_slot(offset / page_bytes)?;
        let within = usize::try_from(offset % page_bytes)
            .context("compose in-page offset does not fit this platform")?;
        self.pages[slot].bytes[within] = value;
        self.pages[slot].dirty = true;
        Ok(())
    }

    fn read_u32(&mut self, offset: u64) -> Result<u32> {
        ensure!(offset.is_multiple_of(4), "unaligned compose rank read");
        let page_bytes = u64::try_from(PAGE_BYTES).context("page size does not fit u64")?;
        let within = usize::try_from(offset % page_bytes)
            .context("compose in-page rank offset does not fit this platform")?;
        ensure!(
            within + 4 <= PAGE_BYTES,
            "compose rank crosses a cache page"
        );
        let slot = self.page_slot(offset / page_bytes)?;
        Ok(u32::from_le_bytes(
            self.pages[slot].bytes[within..within + 4]
                .try_into()
                .expect("four rank bytes"),
        ))
    }

    fn flush_and_sync(&mut self) -> Result<()> {
        for slot in 0..self.pages.len() {
            self.flush_page(slot)?;
        }
        self.file.sync_data().with_context(|| {
            format!(
                "syncing compose page file {} (scratch disk may be full)",
                self.path.display()
            )
        })
    }
}

#[derive(Debug)]
struct DiskBitSet {
    cache: PageCache,
    state_count: u64,
    count: u64,
}

impl DiskBitSet {
    fn create(path: &Path, state_count: u64, max_pages: usize) -> Result<Self> {
        let byte_len = state_count
            .checked_add(7)
            .context("compose live-bit length overflow")?
            / 8;
        let file = create_new_file(path, "compose coaccessibility bitset")?;
        file.set_len(byte_len).with_context(|| {
            format!(
                "sizing compose coaccessibility bitset {} (scratch disk may be full)",
                path.display()
            )
        })?;
        Ok(Self {
            cache: PageCache::new(file, path.to_path_buf(), byte_len, true, max_pages)?,
            state_count,
            count: 0,
        })
    }

    fn insert(&mut self, state: StateId) -> Result<bool> {
        ensure!(
            u64::from(state) < self.state_count,
            "compose coaccessibility state {state} is out of range"
        );
        let byte_offset = u64::from(state) / 8;
        let mask = 1_u8 << (state % 8);
        let byte = self.cache.read_byte(byte_offset)?;
        if byte & mask != 0 {
            return Ok(false);
        }
        self.cache.write_byte(byte_offset, byte | mask)?;
        self.count = self
            .count
            .checked_add(1)
            .context("compose survivor count overflow")?;
        Ok(true)
    }

    fn finish(mut self) -> Result<LiveBits> {
        self.cache.flush_and_sync()?;
        Ok(LiveBits {
            path: self.cache.path.clone(),
            state_count: self.state_count,
            count: self.count,
        })
    }
}

#[derive(Clone, Debug)]
struct LiveBits {
    path: PathBuf,
    state_count: u64,
    count: u64,
}

fn read_offset_pair(file: &mut File, path: &Path, state: StateId) -> Result<(u64, u64)> {
    let offset = u64::from(state)
        .checked_mul(u64::try_from(size_of::<u64>()).context("offset size does not fit u64")?)
        .context("compose reverse-offset lookup overflow")?;
    file.seek(SeekFrom::Start(offset))
        .with_context(|| format!("seeking compose reverse-offset index {}", path.display()))?;
    let start = read_u64(file, path, "reverse-offset start")?;
    let end = read_u64(file, path, "reverse-offset end")?;
    Ok((start, end))
}

struct ReachQueue {
    path: PathBuf,
    memory: Vec<StateId>,
    memory_limit: usize,
    writer: BufWriter<File>,
    reader: BufReader<File>,
    disk_written: u64,
    disk_read: u64,
}

impl ReachQueue {
    fn create(path: &Path, budget: TrimBudget) -> Result<Self> {
        drop(create_new_file(path, "compose coaccessibility queue")?);
        let writer_file = OpenOptions::new()
            .append(true)
            .open(path)
            .with_context(|| format!("opening compose queue {} for append", path.display()))?;
        let reader_file = File::open(path)
            .with_context(|| format!("opening compose queue {} for reading", path.display()))?;
        let mut memory = Vec::new();
        memory
            .try_reserve_exact(budget.queue_records)
            .context("reserving bounded compose reachability queue")?;
        Ok(Self {
            path: path.to_path_buf(),
            memory,
            memory_limit: budget.queue_records,
            writer: BufWriter::with_capacity(budget.io_buffer_bytes, writer_file),
            reader: BufReader::with_capacity(budget.io_buffer_bytes, reader_file),
            disk_written: 0,
            disk_read: 0,
        })
    }

    fn push(&mut self, state: StateId) -> Result<()> {
        if self.memory.len() < self.memory_limit {
            self.memory.push(state);
            return Ok(());
        }
        self.writer
            .write_all(&state.to_le_bytes())
            .with_context(|| {
                format!(
                    "writing compose coaccessibility queue {} (scratch disk may be full)",
                    self.path.display()
                )
            })?;
        self.disk_written = self
            .disk_written
            .checked_add(1)
            .context("compose coaccessibility queue length overflow")?;
        Ok(())
    }

    fn pop(&mut self) -> Result<Option<StateId>> {
        if let Some(state) = self.memory.pop() {
            return Ok(Some(state));
        }
        if self.disk_read == self.disk_written {
            return Ok(None);
        }
        self.writer.flush().with_context(|| {
            format!(
                "flushing compose coaccessibility queue {} (scratch disk may be full)",
                self.path.display()
            )
        })?;
        while self.memory.len() < self.memory_limit && self.disk_read < self.disk_written {
            self.memory
                .push(read_u32(&mut self.reader, &self.path, "queued state")?);
            self.disk_read = self
                .disk_read
                .checked_add(1)
                .context("compose coaccessibility queue position overflow")?;
        }
        Ok(self.memory.pop())
    }

    fn finish(mut self) -> Result<()> {
        ensure!(
            self.memory.is_empty() && self.disk_read == self.disk_written,
            "compose coaccessibility queue was not fully consumed"
        );
        flush_and_sync(
            &mut self.writer,
            &self.path,
            "compose coaccessibility queue",
        )?;
        drop(self.writer);
        drop(self.reader);
        fs::remove_file(&self.path)
            .with_context(|| format!("removing compose queue {}", self.path.display()))
    }
}

fn reverse_reachable(
    pending: &PendingSpill,
    sorted_edges_path: &Path,
    offsets_path: &Path,
    finals_path: &Path,
    budget: TrimBudget,
) -> Result<LiveBits> {
    let live_path = pending.scratch_path.join(LIVE_BITS_FILE);
    let mut live = DiskBitSet::create(&live_path, pending.state_count, budget.page_cache_pages)?;
    let queue_path = pending.scratch_path.join(REACHABLE_QUEUE_FILE);
    let mut queue = ReachQueue::create(&queue_path, budget)?;
    let finals_file = File::open(finals_path).with_context(|| {
        format!(
            "opening compose final-state stream {}",
            finals_path.display()
        )
    })?;
    let mut finals = BufReader::with_capacity(budget.io_buffer_bytes, finals_file);
    for _ in 0..pending.final_count {
        let state = read_u32(&mut finals, finals_path, "final state")?;
        ensure!(
            u64::from(state) < pending.state_count,
            "compose final state {state} is out of range"
        );
        if live.insert(state)? {
            queue.push(state)?;
        }
    }
    let mut trailing = [0_u8; 1];
    ensure!(
        finals.read(&mut trailing).with_context(|| format!(
            "checking compose final-state stream {}",
            finals_path.display()
        ))? == 0,
        "compose final-state stream {} has trailing data",
        finals_path.display()
    );
    drop(finals);

    let mut offsets = File::open(offsets_path).with_context(|| {
        format!(
            "opening compose reverse-offset index {}",
            offsets_path.display()
        )
    })?;
    let sorted_file = File::open(sorted_edges_path).with_context(|| {
        format!(
            "opening sorted reverse-edge stream {}",
            sorted_edges_path.display()
        )
    })?;
    let mut sorted_edges = BufReader::with_capacity(budget.io_buffer_bytes, sorted_file);
    while let Some(target) = queue.pop()? {
        let (start, end) = read_offset_pair(&mut offsets, offsets_path, target)?;
        ensure!(
            start <= end && end <= pending.arc_count,
            "invalid reverse-edge range {start}..{end} for compose state {target}"
        );
        let byte_offset = start
            .checked_mul(REVERSE_EDGE_BYTES)
            .context("reverse-edge seek offset overflow")?;
        sorted_edges
            .seek(SeekFrom::Start(byte_offset))
            .with_context(|| {
                format!(
                    "seeking sorted reverse-edge stream {}",
                    sorted_edges_path.display()
                )
            })?;
        for _ in start..end {
            let edge = read_reverse_edge(&mut sorted_edges, sorted_edges_path)?;
            ensure!(
                edge.target == target,
                "reverse-offset index for state {target} points at state {}",
                edge.target
            );
            if live.insert(edge.source)? {
                queue.push(edge.source)?;
            }
        }
    }
    queue.finish()?;
    live.finish()
}

fn build_live_ranks(
    pending: &PendingSpill,
    live: &LiveBits,
    budget: TrimBudget,
) -> Result<PathBuf> {
    ensure!(
        live.state_count == pending.state_count,
        "compose live-bit state count changed"
    );
    let bits_file = File::open(&live.path).with_context(|| {
        format!(
            "opening compose coaccessibility bitset {}",
            live.path.display()
        )
    })?;
    let mut bits = BufReader::with_capacity(budget.io_buffer_bytes, bits_file);
    let ranks_path = pending.scratch_path.join(LIVE_RANKS_FILE);
    let ranks_file = create_new_file(&ranks_path, "compose survivor-rank index")?;
    let mut ranks = BufWriter::with_capacity(budget.io_buffer_bytes, ranks_file);
    let mut byte = 0_u8;
    let mut count = 0_u64;
    for state in 0..pending.state_count {
        if state % 8 == 0 {
            byte = read_u8(&mut bits, &live.path, "coaccessibility byte")?;
        }
        let rank =
            StateId::try_from(count).context("compose survivor rank exceeds rustfst StateId")?;
        ranks.write_all(&rank.to_le_bytes()).with_context(|| {
            format!(
                "writing compose survivor-rank index {} (scratch disk may be full)",
                ranks_path.display()
            )
        })?;
        if byte & (1_u8 << (state % 8)) != 0 {
            count = count
                .checked_add(1)
                .context("compose survivor-rank overflow")?;
        }
    }
    ensure!(
        count == live.count,
        "compose survivor count changed: expected {}, found {count}",
        live.count
    );
    flush_and_sync(&mut ranks, &ranks_path, "compose survivor-rank index")?;
    Ok(ranks_path)
}

#[derive(Debug)]
struct LiveRankIndex {
    bits: PageCache,
    ranks: PageCache,
    state_count: u64,
}

impl LiveRankIndex {
    fn open(live: &LiveBits, ranks_path: &Path, max_pages: usize) -> Result<Self> {
        let bit_len = live
            .state_count
            .checked_add(7)
            .context("compose live-bit length overflow")?
            / 8;
        let rank_len = live
            .state_count
            .checked_mul(4)
            .context("compose survivor-rank file length overflow")?;
        let bit_file = File::open(&live.path).with_context(|| {
            format!(
                "opening compose coaccessibility bitset {}",
                live.path.display()
            )
        })?;
        let rank_file = File::open(ranks_path).with_context(|| {
            format!(
                "opening compose survivor-rank index {}",
                ranks_path.display()
            )
        })?;
        let bit_pages = (max_pages / 2).max(1);
        let rank_pages = max_pages.saturating_sub(bit_pages).max(1);
        Ok(Self {
            bits: PageCache::new(bit_file, live.path.clone(), bit_len, false, bit_pages)?,
            ranks: PageCache::new(
                rank_file,
                ranks_path.to_path_buf(),
                rank_len,
                false,
                rank_pages,
            )?,
            state_count: live.state_count,
        })
    }

    fn map(&mut self, old_state: StateId) -> Result<Option<StateId>> {
        ensure!(
            u64::from(old_state) < self.state_count,
            "compose state {old_state} exceeds survivor index"
        );
        let byte = self.bits.read_byte(u64::from(old_state) / 8)?;
        if byte & (1_u8 << (old_state % 8)) == 0 {
            return Ok(None);
        }
        let rank_offset = u64::from(old_state)
            .checked_mul(4)
            .context("compose survivor-rank lookup overflow")?;
        Ok(Some(self.ranks.read_u32(rank_offset)?))
    }
}

fn write_trimmed_transition(
    writer: &mut impl Write,
    tr: &StdTransition,
    path: &Path,
) -> Result<()> {
    writer
        .write_all(&tr.ilabel.to_le_bytes())
        .and_then(|()| writer.write_all(&tr.olabel.to_le_bytes()))
        .and_then(|()| writer.write_all(&tr.weight.value().to_bits().to_le_bytes()))
        .and_then(|()| writer.write_all(&tr.nextstate.to_le_bytes()))
        .with_context(|| {
            format!(
                "writing trimmed compose arc spool {} (scratch disk may be full)",
                path.display()
            )
        })
}

fn write_trimmed_state(
    writer: &mut impl Write,
    final_bits: Option<u32>,
    arc_count: u64,
    path: &Path,
) -> Result<()> {
    let tag = u8::from(final_bits.is_some());
    writer
        .write_all(&[tag])
        .and_then(|()| writer.write_all(&final_bits.unwrap_or(0).to_le_bytes()))
        .and_then(|()| writer.write_all(&arc_count.to_le_bytes()))
        .with_context(|| {
            format!(
                "writing trimmed compose state spool {} (scratch disk may be full)",
                path.display()
            )
        })
}

fn create_trimmed_writers(
    scratch: &Path,
    budget: TrimBudget,
) -> Result<(PathBuf, BufWriter<File>, PathBuf, BufWriter<File>)> {
    let state_path = scratch.join(TRIMMED_STATES_FILE);
    let arc_path = scratch.join(TRIMMED_ARCS_FILE);
    let state_file = create_new_file(&state_path, "trimmed compose state spool")?;
    let arc_file = create_new_file(&arc_path, "trimmed compose arc spool")?;
    let mut states = BufWriter::with_capacity(budget.io_buffer_bytes, state_file);
    let mut arcs = BufWriter::with_capacity(budget.io_buffer_bytes, arc_file);
    states
        .write_all(TRIMMED_STATES_MAGIC)
        .with_context(|| format!("writing trimmed compose header to {}", state_path.display()))?;
    arcs.write_all(TRIMMED_ARCS_MAGIC)
        .with_context(|| format!("writing trimmed compose header to {}", arc_path.display()))?;
    Ok((state_path, states, arc_path, arcs))
}

fn write_empty_trimmed_spool(scratch: &Path, budget: TrimBudget) -> Result<(PathBuf, PathBuf)> {
    let (state_path, mut states, arc_path, mut arcs) = create_trimmed_writers(scratch, budget)?;
    flush_and_sync(&mut states, &state_path, "trimmed compose state spool")?;
    flush_and_sync(&mut arcs, &arc_path, "trimmed compose arc spool")?;
    Ok((state_path, arc_path))
}

fn rewrite_verbatim_spool(
    pending: &PendingSpill,
    budget: TrimBudget,
) -> Result<(PathBuf, PathBuf)> {
    let source_file = File::open(&pending.spool_path).with_context(|| {
        format!(
            "opening compose scratch spool {} for verbatim rewrite",
            pending.spool_path.display()
        )
    })?;
    let mut source = BufReader::with_capacity(budget.io_buffer_bytes, source_file);
    let mut magic = [0_u8; SPOOL_MAGIC.len()];
    read_exact(&mut source, &mut magic, &pending.spool_path, "header")?;
    ensure!(
        &magic == SPOOL_MAGIC,
        "invalid compose scratch header in {}",
        pending.spool_path.display()
    );
    let (state_path, mut states, arc_path, mut arcs) =
        create_trimmed_writers(&pending.scratch_path, budget)?;
    let mut seen_arcs = 0_u64;
    let mut seen_finals = 0_u64;
    for state_index in 0..pending.state_count {
        let state = StateId::try_from(state_index)
            .context("compose scratch state id exceeds rustfst StateId")?;
        let final_bits = match read_u8(&mut source, &pending.spool_path, "final tag")? {
            0 => None,
            1 => {
                seen_finals = seen_finals
                    .checked_add(1)
                    .context("compose final-state count overflow")?;
                Some(read_u32(&mut source, &pending.spool_path, "final weight")?)
            }
            tag => bail!("invalid final tag {tag} for compose state {state}"),
        };
        let arc_count = read_u64(&mut source, &pending.spool_path, "arc count")?;
        for _ in 0..arc_count {
            let tr = read_transition(&mut source, &pending.spool_path)?;
            ensure!(
                u64::from(tr.nextstate) < pending.state_count,
                "compose scratch arc targets missing state {}",
                tr.nextstate
            );
            write_trimmed_transition(&mut arcs, &tr, &arc_path)?;
            seen_arcs = seen_arcs
                .checked_add(1)
                .context("compose scratch total arc count overflow")?;
        }
        write_trimmed_state(&mut states, final_bits, arc_count, &state_path)?;
    }
    ensure!(
        seen_arcs == pending.arc_count,
        "compose scratch arc count changed: expected {}, found {seen_arcs}",
        pending.arc_count
    );
    ensure!(
        seen_finals == pending.final_count,
        "compose scratch final count changed: expected {}, found {seen_finals}",
        pending.final_count
    );
    let mut trailing = [0_u8; 1];
    ensure!(
        source.read(&mut trailing).with_context(|| format!(
            "checking compose scratch spool {}",
            pending.spool_path.display()
        ))? == 0,
        "compose scratch spool {} has trailing data",
        pending.spool_path.display()
    );
    flush_and_sync(&mut states, &state_path, "trimmed compose state spool")?;
    flush_and_sync(&mut arcs, &arc_path, "trimmed compose arc spool")?;
    Ok((state_path, arc_path))
}

fn rewrite_trimmed_spool(
    pending: &PendingSpill,
    live: &LiveBits,
    ranks_path: &Path,
    budget: TrimBudget,
) -> Result<(PathBuf, PathBuf)> {
    let source_file = File::open(&pending.spool_path).with_context(|| {
        format!(
            "opening compose scratch spool {} for survivor rewrite",
            pending.spool_path.display()
        )
    })?;
    let mut source_reader = BufReader::with_capacity(budget.io_buffer_bytes, source_file);
    let mut magic = [0_u8; SPOOL_MAGIC.len()];
    read_exact(
        &mut source_reader,
        &mut magic,
        &pending.spool_path,
        "header",
    )?;
    ensure!(
        &magic == SPOOL_MAGIC,
        "invalid compose scratch header in {}",
        pending.spool_path.display()
    );
    let (state_path, mut states, arc_path, mut arcs) =
        create_trimmed_writers(&pending.scratch_path, budget)?;
    let mut index = LiveRankIndex::open(live, ranks_path, budget.page_cache_pages)?;
    let mut written_states = 0_u64;

    for old_index in 0..pending.state_count {
        let old_state = StateId::try_from(old_index)
            .context("compose scratch state id exceeds rustfst StateId")?;
        let final_bits = match read_u8(&mut source_reader, &pending.spool_path, "final tag")? {
            0 => None,
            1 => Some(read_u32(
                &mut source_reader,
                &pending.spool_path,
                "final weight",
            )?),
            tag => bail!("invalid final tag {tag} for compose state {old_state}"),
        };
        let arc_count = read_u64(&mut source_reader, &pending.spool_path, "arc count")?;
        let new_source = index.map(old_state)?;
        if let Some(new_source) = new_source {
            ensure!(
                u64::from(new_source) == written_states,
                "compose survivor ranks are not ascending at old state {old_state}"
            );
            let mut live_arc_count = 0_u64;
            for _ in 0..arc_count {
                let mut tr = read_transition(&mut source_reader, &pending.spool_path)?;
                ensure!(
                    u64::from(tr.nextstate) < pending.state_count,
                    "compose scratch arc targets missing state {}",
                    tr.nextstate
                );
                if let Some(new_target) = index.map(tr.nextstate)? {
                    tr.nextstate = new_target;
                    write_trimmed_transition(&mut arcs, &tr, &arc_path)?;
                    live_arc_count = live_arc_count
                        .checked_add(1)
                        .context("trimmed compose arc count overflow")?;
                }
            }
            write_trimmed_state(&mut states, final_bits, live_arc_count, &state_path)?;
            written_states = written_states
                .checked_add(1)
                .context("trimmed compose state count overflow")?;
        } else {
            for _ in 0..arc_count {
                let tr = read_transition(&mut source_reader, &pending.spool_path)?;
                ensure!(
                    u64::from(tr.nextstate) < pending.state_count,
                    "compose scratch arc targets missing state {}",
                    tr.nextstate
                );
            }
        }
    }
    ensure!(
        written_states == live.count,
        "compose survivor count changed: expected {}, wrote {written_states}",
        live.count
    );
    let mut trailing = [0_u8; 1];
    ensure!(
        source_reader.read(&mut trailing).with_context(|| format!(
            "checking compose scratch spool {}",
            pending.spool_path.display()
        ))? == 0,
        "compose scratch spool {} has trailing data",
        pending.spool_path.display()
    );
    flush_and_sync(&mut states, &state_path, "trimmed compose state spool")?;
    flush_and_sync(&mut arcs, &arc_path, "trimmed compose arc spool")?;
    Ok((state_path, arc_path))
}

fn remove_scratch_file(path: &Path, what: &str) -> Result<()> {
    fs::remove_file(path).with_context(|| format!("removing {what} {}", path.display()))
}

pub(super) fn trim_scratch_spool(pending: PendingSpill) -> Result<TrimmedSpill> {
    let budget = TrimBudget::new(pending.tracked_memory_cap_bytes)?;
    if pending
        .metadata
        .properties
        .contains(FstProperties::ACCESSIBLE | FstProperties::COACCESSIBLE)
    {
        let (state_path, arc_path) = rewrite_verbatim_spool(&pending, budget)?;
        remove_scratch_file(&pending.spool_path, "untrimmed compose spool")?;
        return Ok(TrimmedSpill {
            state_path,
            arc_path,
            state_count: pending.state_count,
            metadata: pending.metadata,
        });
    }
    if pending.final_count == 0 {
        let (state_path, arc_path) = write_empty_trimmed_spool(&pending.scratch_path, budget)?;
        remove_scratch_file(&pending.spool_path, "untrimmed compose spool")?;
        return Ok(TrimmedSpill {
            state_path,
            arc_path,
            state_count: 0,
            metadata: pending.metadata.after_connect(false),
        });
    }

    let (run_count, finals_path) = generate_reverse_runs(&pending, budget)?;
    let sorted_edges_path = merge_reverse_runs(&pending.scratch_path, run_count, budget)?;
    let offsets_path = build_reverse_offsets(&pending, &sorted_edges_path, budget)?;
    let live = reverse_reachable(
        &pending,
        &sorted_edges_path,
        &offsets_path,
        &finals_path,
        budget,
    )?;
    ensure!(
        live.count != 0,
        "compose product has final states but no coaccessible states"
    );
    let ranks_path = build_live_ranks(&pending, &live, budget)?;
    let (state_path, arc_path) = rewrite_trimmed_spool(&pending, &live, &ranks_path, budget)?;

    // The survivor streams are durable now. Only at this point may the
    // artifact claim external trimming provenance.
    remove_scratch_file(&pending.spool_path, "untrimmed compose spool")?;
    remove_scratch_file(&sorted_edges_path, "sorted reverse-edge stream")?;
    remove_scratch_file(&offsets_path, "compose reverse-offset index")?;
    remove_scratch_file(&finals_path, "compose final-state stream")?;
    remove_scratch_file(&live.path, "compose coaccessibility bitset")?;
    remove_scratch_file(&ranks_path, "compose survivor-rank index")?;

    Ok(TrimmedSpill {
        state_path,
        arc_path,
        state_count: live.count,
        metadata: pending.metadata.after_connect(true),
    })
}
