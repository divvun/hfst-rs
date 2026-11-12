//! Path extraction takes memory and time linear in the length of the path it
//! walks.
//!
//! Each backend's walker counts how often every state occurs on the current
//! path in one table the whole recursion shares, and lends the callback its
//! own path; `hfst fst2strings` follows that path by appending each pair to
//! the strings it filters on. Upstream copies two count maps and the path at
//! every state, and the tool rebuilds its strings at every state, so a path n
//! states long holds memory and takes time quadratic in n: about 7 GB at
//! 20,000 states. These walk chains of two lengths and bound, per state and as
//! the chain grows fourfold, the heap a walk holds at its peak and the bytes
//! it allocates in all. Each copy of a path or its strings is an allocation,
//! so the bytes allocated count the copying, where a timer on a loaded machine
//! would not.

use allocation_counter::AllocationInfo;
use hfst::backend::{AlgebraBackend, Backend, LookupBackend};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPath, Symbol};
use hfst::hfst_extract_strings::{ExtractStringsCb, RetVal};
use hfst::hfst_transducer::HfstTransducer;
use hfst::tools::fst2strings::{PathQuery, extract_paths, extract_paths_ol};
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

#[cfg(feature = "foma")]
use hfst::backend_foma::FomaTransducer;

/// The longer chain; the shorter is a quarter of it.
const STATES: u32 = 20_000;

/// The most heap a walk may hold at once per state of the chain. A path
/// entry is two symbols, and the walk holds the path and each state's sorted
/// arcs. Copying the count maps per call holds tens of thousands of bytes per
/// state at these lengths.
const PEAK_PER_STATE: u64 = 1024;

/// The most bytes a walk may allocate in all per state of the chain. Copying
/// the path per call allocates hundreds of thousands per state at 20,000
/// states.
const ALLOCATED_PER_STATE: u64 = 2048;

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
    fn operator_call(&mut self, path: &HfstTwoLevelPath, is_final: bool) -> RetVal {
        if is_final {
            self.finals += 1;
            self.longest = self.longest.max(path.second.len());
        }
        RetVal::new(true, true)
    }
}

/// What `walk` allocated over chains of `STATES / 4` and `STATES` states,
/// checked to grow with the chain and not with its square.
fn assert_linear(tag: &str, walk: impl Fn(u32) -> AllocationInfo) {
    let short = STATES / 4;
    let [low, high] = [short, STATES].map(&walk);
    for (n, info) in [(short, low), (STATES, high)] {
        assert!(
            info.bytes_max <= PEAK_PER_STATE * u64::from(n),
            "{tag}: a chain of {n} states held {} bytes at its peak, over \
             {PEAK_PER_STATE} per state",
            info.bytes_max
        );
        assert!(
            info.bytes_total <= ALLOCATED_PER_STATE * u64::from(n),
            "{tag}: a chain of {n} states allocated {} bytes, over \
             {ALLOCATED_PER_STATE} per state",
            info.bytes_total
        );
    }
    assert!(
        high.bytes_max < low.bytes_max * 6 && high.bytes_total < low.bytes_total * 6,
        "{tag}: growing the chain from {short} to {STATES} states took the peak \
         from {} to {} bytes and the bytes allocated from {} to {}",
        low.bytes_max,
        high.bytes_max,
        low.bytes_total,
        high.bytes_total
    );
}

/// What a backend's walk over a chain of `n` states allocates.
fn walk_backend<B: Backend>(n: u32) -> AllocationInfo {
    let b = B::from_basic_owned(chain(n)).expect("the chain converts");
    let mut found = Count::default();
    let info = allocation_counter::measure(|| b.extract_paths_cb(&mut found, -1));
    assert_eq!(found.finals, 1, "the chain has one path");
    assert_eq!(found.longest, n as usize, "the path runs the whole chain");
    info
}

/// The query that filters on every string fst2strings builds, letting the
/// chain's one path through.
fn filtering() -> PathQuery {
    PathQuery {
        in_prefix: "aa".to_string(),
        out_prefix: "bb".to_string(),
        in_exclude: "ab".to_string(),
        out_exclude: "ba".to_string(),
        max_in_length: STATES + 1,
        ..PathQuery::default()
    }
}

/// What `hfst fst2strings`'s search over a chain of `n` states allocates.
fn walk_tool<B: AlgebraBackend>(n: u32) -> AllocationInfo {
    let t = HfstTransducer::<B>::new_from_basic_owned(chain(n)).expect("the chain converts");
    let query = filtering();
    let mut found = 0;
    let info = allocation_counter::measure(|| {
        extract_paths(&t, &query, &mut |p| found += p.second.len(), &mut |_| {})
            .expect("the chain is acyclic");
    });
    assert_eq!(found, n as usize, "the one path runs the whole chain");
    info
}

fn walk_tool_ol<B: LookupBackend>(n: u32) -> AllocationInfo {
    let t = HfstTransducer::<B>::new_from_basic_owned(chain(n)).expect("the chain converts");
    let query = filtering();
    let mut found = 0;
    let info = allocation_counter::measure(|| {
        extract_paths_ol(&t, &query, &mut |p| found += p.second.len(), &mut |_| {})
            .expect("the chain is acyclic");
    });
    assert_eq!(found, n as usize, "the one path runs the whole chain");
    info
}

// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.extract-paths-fn+2/test]
// [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.extract-paths-fn+2/test]
// [spec:hfst:sem:hfst-extract-strings.hfst.extract-strings-cb.operator-fn+1/test]
#[test]
fn every_backend_walks_long_chains_in_linear_cost() {
    hfst::depth::on_compile_stack(|| {
        assert_linear("tropical", walk_backend::<StdVectorFst>);
        assert_linear(
            "optimized lookup",
            walk_backend::<Transducer<WeightedTables>>,
        );
        #[cfg(feature = "foma")]
        assert_linear("foma", walk_backend::<FomaTransducer>);
    });
}

// [spec:hfst:sem:hfst-fst2strings.callback.operator-fn+1/test]
#[test]
fn fst2strings_filters_long_chains_in_linear_cost() {
    hfst::depth::on_compile_stack(|| {
        assert_linear("tropical", walk_tool::<StdVectorFst>);
        assert_linear(
            "optimized lookup",
            walk_tool_ol::<Transducer<WeightedTables>>,
        );
        #[cfg(feature = "foma")]
        assert_linear("foma", walk_tool::<FomaTransducer>);
    });
}
