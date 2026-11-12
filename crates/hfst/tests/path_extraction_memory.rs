//! Path extraction holds memory linear in the length of the path it walks.
//!
//! Each backend's walker counts how often every state occurs on the current
//! path in one table that the whole recursion shares. Upstream passes two such
//! maps by value, a copy per call, so a path n states long holds n
//! copies of up to n entries: about 7 GB at 20,000 states. This walks one long
//! chain in every backend, at two lengths, and bounds the heap the walk adds
//! at its peak, per state and as the chain grows fourfold.
//!
//! The walk hands the callback a copy of the path at every state, so its time
//! is quadratic in the chain's length; a debug build walks shorter chains.

use hfst::backend::Backend;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPath, Symbol};
use hfst::hfst_extract_strings::{ExtractStringsCb, RetVal};
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;
use peak_alloc::PeakAlloc;

#[cfg(feature = "foma")]
use hfst::backend_foma::FomaTransducer;

#[global_allocator]
static HEAP: PeakAlloc = PeakAlloc;

/// The longer chain each backend walks; the shorter is a quarter of it.
const STATES: u32 = if cfg!(debug_assertions) {
    4_000
} else {
    20_000
};

/// The most heap a walk may add per state of the chain. A path entry is two
/// symbols, and the walk holds the path, the copy it hands the callback and
/// each state's sorted arcs. Copying the maps per call takes tens of thousands
/// of bytes per state at these lengths.
const BYTES_PER_STATE: usize = 1024;

/// `n` arcs `a:b` in a row, the last state final.
fn chain(n: u32) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for s in 0..n {
        let tr = HfstBasicTransition::new_symbols(
            s + 1,
            Symbol::from("a"),
            Symbol::from("b"),
            0.0,
            net.coder_mut(),
        );
        net.add_transition(s, &tr, true);
    }
    net.set_final_weight(n, &0.0);
    net
}

/// Counts the final paths and their length, keeping none of them.
#[derive(Default)]
struct Count {
    finals: usize,
    longest: usize,
}

impl ExtractStringsCb for Count {
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        if is_final {
            self.finals += 1;
            self.longest = self.longest.max(path.second.len());
        }
        RetVal::new(true, true)
    }
}

/// The most heap the walk over `b` held beyond what was live before it.
fn walk_peak<B: Backend>(b: &B, n: u32) -> usize {
    let mut found = Count::default();
    let before = HEAP.current_usage();
    HEAP.reset_peak_usage();
    b.extract_paths_cb(&mut found, -1);
    let peak = HEAP.peak_usage().saturating_sub(before);
    assert_eq!(found.finals, 1, "the chain has one path");
    assert_eq!(found.longest, n as usize, "the path runs the whole chain");
    peak
}

/// Walk chains of `STATES / 4` and `STATES` states in backend `B` and check
/// the peak grows with the chain, not with its square.
fn assert_linear<B: Backend>(tag: &str) {
    let peaks: Vec<(u32, usize)> = [STATES / 4, STATES]
        .into_iter()
        .map(|n| {
            let b = B::from_basic_owned(chain(n)).unwrap_or_else(|e| panic!("{tag}: {e}"));
            (n, walk_peak(&b, n))
        })
        .collect();
    let [(short, short_peak), (long, long_peak)] = peaks[..] else {
        unreachable!("two lengths")
    };
    for (n, peak) in [(short, short_peak), (long, long_peak)] {
        assert!(
            peak <= BYTES_PER_STATE * n as usize,
            "{tag}: a chain of {n} states held {peak} bytes at its peak, \
             over {BYTES_PER_STATE} per state"
        );
    }
    assert!(
        long_peak < short_peak * 6,
        "{tag}: growing the chain from {short} to {long} states took the peak \
         from {short_peak} to {long_peak} bytes"
    );
}

// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.extract-paths-fn+1/test]
// [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.extract-paths-fn+1/test]
#[test]
fn every_backend_walks_long_chains_in_linear_memory() {
    hfst::depth::on_compile_stack(|| {
        assert_linear::<StdVectorFst>("tropical");
        assert_linear::<Transducer<WeightedTables>>("optimized lookup");
        #[cfg(feature = "foma")]
        assert_linear::<FomaTransducer>("foma");
    });
}
