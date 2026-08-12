//! Native THFST backend — makes `ImplementationType::THFST_TYPE` a real,
//! usable transducer implementation, backed by the in-tree weighted
//! optimized-lookup engine under a distinct stream identity.
//!
//! Unlike foma, this module is NOT feature-gated: THFST is always on. It has
//! no C++ HFST ancestor — these rules are authored greenfield against the
//! contract in `docs/spec/port/back-ends/thfst/thfst-backend.md`. This node
//! (`thfst.seam`) makes the type constructible and convertible; the on-disk
//! directory format (read_dir/write_dir) lands in the next node (`thfst.io`).

use crate::backend::{Backend, LookupBackend};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::{
    HfstOneLevelPaths, HfstTwoLevelPaths, ImplementationType, StringVector,
};
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_symbol_defs::StringSet;
use crate::transducer::{Transducer, WeightedTables};

/// The backend's transducer handle: the in-memory weighted optimized-lookup
/// engine under a distinct stream identity (`THFST_TYPE`). It implements
/// `Backend` and `LookupBackend` by delegating to the inner engine, but NOT
/// `AlgebraBackend`: THFST is a lookup-tier citizen exactly like HFST_OL/OLW.
// [spec:hfst:def:thfst-backend.thfst-transducer]
// [spec:hfst:sem:thfst-backend.thfst-transducer]
pub struct ThfstTransducer(pub(crate) Transducer<WeightedTables>);

impl ThfstTransducer {
    /// Wrap an optimized-lookup (weighted) engine as a THFST handle — the O(1)
    /// table move behind `into_thfst()` / the OLW->THFST `from_any` arm.
    pub fn from_ol(t: Transducer<WeightedTables>) -> Self {
        ThfstTransducer(t)
    }

    /// Recover the inner optimized-lookup engine — the O(1) table move behind
    /// `into_olw()` / the THFST->OLW `from_any` arm.
    pub fn into_ol(self) -> Transducer<WeightedTables> {
        self.0
    }

    /// Serialize this THFST transducer to a `X.thfst/` directory (the three
    /// member files `alphabet`, `index`, `transition`) — a thin wrapper over
    /// [`crate::thfst_io::write_dir`] on the inner weighted OL engine. Creates
    /// `dir` and its parents if absent. Against the reference producer
    /// (`thfst-tools`) the `index`/`transition` files are byte-identical and
    /// the `alphabet` is semantically equal.
    // [spec:hfst:def:thfst-backend.write-dir-fn]
    // [spec:hfst:sem:thfst-backend.write-dir-fn]
    pub fn write_dir(&self, dir: &std::path::Path) -> crate::error::Result<()> {
        crate::thfst_io::write_dir(&self.0, dir)
    }

    /// Load a THFST transducer from a `X.thfst/` directory — a thin wrapper
    /// over [`crate::thfst_io::read_dir`]. The directory must contain all three
    /// member files (else `NotTransducerStream`); the OL header THFST does not
    /// store is synthesized (both symbol counts = key_table length, table sizes
    /// = the exact record counts, weighted = true, all property flags false).
    // [spec:hfst:def:thfst-backend.read-dir-fn]
    // [spec:hfst:sem:thfst-backend.read-dir-fn]
    pub fn read_dir(dir: &std::path::Path) -> crate::error::Result<Self> {
        Ok(ThfstTransducer(crate::thfst_io::read_dir(dir)?))
    }
}

impl Backend for ThfstTransducer {
    const TYPE: ImplementationType = ImplementationType::THFST_TYPE;

    /// THFST is weighted-only per spec (a logically-unweighted source
    /// serializes with 0.0 weights), so the stream tag is unconditional —
    /// unlike the OLW backend, which peeks the header weightedness.
    fn stream_type(&self) -> ImplementationType {
        ImplementationType::THFST_TYPE
    }

    /// THFST has no byte-stream serialization: the directory itself is the
    /// container (see `.directory-format`); serialization goes through the
    /// directory hook, not this byte-stream arm.
    fn write(&self, _os: &mut dyn std::io::Write, _hfst_format: bool) -> crate::error::Result<()> {
        crate::bail!(
            StreamCannotBeWritten,
            "THFST has no byte-stream serialization; write to a .thfst directory instead"
        )
    }

    /// The directory-format serialization hook — overrides the erroring
    /// default so the generic `HfstOutputStream::write<B: Backend>` reaches the
    /// `.thfst` directory writer without a runtime downcast. Delegates to the
    /// inherent `write_dir`.
    // [spec:hfst:def:thfst-backend.stream-io]
    // [spec:hfst:sem:thfst-backend.stream-io]
    fn write_to_dir(&self, dir: &std::path::Path) -> crate::error::Result<()> {
        self.write_dir(dir)
    }

    // Every remaining method delegates to the inner `Transducer<WeightedTables>`
    // `Backend` impl body-for-body via fully-qualified trait calls (the inner
    // engine also carries inherent methods of the same names that would
    // otherwise shadow the trait).
    fn empty() -> Self {
        ThfstTransducer(<Transducer<WeightedTables> as Backend>::empty())
    }
    fn copy(&self) -> crate::error::Result<Self> {
        Ok(ThfstTransducer(Backend::copy(&self.0)?))
    }
    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        Backend::to_basic(&self.0)
    }
    fn from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(ThfstTransducer(
            <Transducer<WeightedTables> as Backend>::from_basic(net)?,
        ))
    }
    fn get_alphabet(&self) -> StringSet {
        Backend::get_alphabet(&self.0)
    }
    fn is_cyclic(&self) -> bool {
        Backend::is_cyclic(&self.0)
    }
    fn number_of_states(&self) -> u32 {
        Backend::number_of_states(&self.0)
    }
    fn number_of_arcs(&self) -> u32 {
        Backend::number_of_arcs(&self.0)
    }
    fn has_weights(&self) -> bool {
        Backend::has_weights(&self.0)
    }
    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        Backend::insert_to_alphabet(&mut self.0, symbol)
    }
    fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        Backend::is_infinitely_ambiguous(&self.0)
    }
    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        Backend::extract_paths_cb(&self.0, callback, cycles);
    }
    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        Backend::extract_paths_fd_cb(&self.0, callback, cycles, filter_fd);
    }
}

// Explicit delegating impl — the `ol_lookup_backend!` macro only generates the
// `LookupBackend` impl for `Transducer<$tables>`, so THFST forwards each method
// to the inner engine's `LookupBackend` impl via fully-qualified trait calls.
impl LookupBackend for ThfstTransducer {
    fn lookup_fd_str(&mut self, s: &str, limit: isize, time_cutoff: f64) -> HfstOneLevelPaths {
        LookupBackend::lookup_fd_str(&mut self.0, s, limit, time_cutoff)
    }
    fn lookup_fd_strvec(
        &mut self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstOneLevelPaths {
        LookupBackend::lookup_fd_strvec(&mut self.0, s, limit, time_cutoff)
    }
    fn lookup_fd_pairs_str(
        &mut self,
        s: &str,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstTwoLevelPaths {
        LookupBackend::lookup_fd_pairs_str(&mut self.0, s, limit, time_cutoff)
    }
    fn is_lookup_infinitely_ambiguous_str(&mut self, s: &str) -> bool {
        LookupBackend::is_lookup_infinitely_ambiguous_str(&mut self.0, s)
    }
    fn is_lookup_infinitely_ambiguous_strvec(&mut self, s: &StringVector) -> bool {
        LookupBackend::is_lookup_infinitely_ambiguous_strvec(&mut self.0, s)
    }
}
