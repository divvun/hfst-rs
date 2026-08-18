//! Focused owned composition, spill parity, and cleanup tests.

use super::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use hfst_openfst::rustfst::algorithms::isomorphic;

static NEXT_SCRATCH_DIR: AtomicU64 = AtomicU64::new(0);

struct TestScratchDir(PathBuf);

impl TestScratchDir {
    fn new() -> Self {
        for _ in 0..128 {
            let sequence = NEXT_SCRATCH_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "hfst-owned-compose-test-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create compose test scratch parent: {error}"),
            }
        }
        panic!("cannot allocate a unique compose test scratch parent")
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn is_empty(&self) -> bool {
        std::fs::read_dir(&self.0)
            .expect("read compose test scratch parent")
            .next()
            .is_none()
    }
}

impl Drop for TestScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn symbol_table() -> Arc<SymbolTable> {
    let mut symbols = SymbolTable::empty();
    symbols.add_symbol(internal_epsilon);
    symbols.add_symbol(internal_unknown);
    symbols.add_symbol(internal_identity);
    symbols.add_symbol("a");
    symbols.add_symbol("b");
    symbols.add_symbol("c");
    symbols.add_symbol("d");
    symbols.add_symbol("e");
    Arc::new(symbols)
}

fn one_arc(
    symbols: Arc<SymbolTable>,
    input: &str,
    output: &str,
    arc_weight: f32,
    final_weight: f32,
) -> StdVectorFst {
    let mut fst = StdVectorFst::new();
    let start = fst.add_state();
    let final_state = fst.add_state();
    fst.set_start(start).unwrap();
    fst.set_final(final_state, TropicalWeight::new(final_weight))
        .unwrap();
    fst.add_tr(
        start,
        StdTransition::new(
            symbols.get_label(input).unwrap(),
            symbols.get_label(output).unwrap(),
            TropicalWeight::new(arc_weight),
            final_state,
        ),
    )
    .unwrap();
    fst.set_input_symbols(symbols);
    fst
}

fn successful_and_dead_branches(symbols: Arc<SymbolTable>) -> (StdVectorFst, StdVectorFst) {
    let mut left = StdVectorFst::new();
    let left_start = left.add_state();
    let left_dead = left.add_state();
    let left_final = left.add_state();
    left.set_start(left_start).unwrap();
    left.set_final(left_final, TropicalWeight::new(0.5))
        .unwrap();
    left.add_tr(
        left_start,
        StdTransition::new(
            symbols.get_label("a").unwrap(),
            symbols.get_label("b").unwrap(),
            TropicalWeight::new(0.25),
            left_dead,
        ),
    )
    .unwrap();
    left.add_tr(
        left_start,
        StdTransition::new(
            symbols.get_label("a").unwrap(),
            symbols.get_label("d").unwrap(),
            TropicalWeight::new(0.75),
            left_final,
        ),
    )
    .unwrap();
    left.set_input_symbols(Arc::clone(&symbols));

    let mut right = StdVectorFst::new();
    let right_start = right.add_state();
    let right_dead = right.add_state();
    let right_final = right.add_state();
    right.set_start(right_start).unwrap();
    right
        .set_final(right_final, TropicalWeight::new(1.0))
        .unwrap();
    right
        .add_tr(
            right_start,
            StdTransition::new(
                symbols.get_label("b").unwrap(),
                symbols.get_label("e").unwrap(),
                TropicalWeight::new(1.25),
                right_dead,
            ),
        )
        .unwrap();
    right
        .add_tr(
            right_start,
            StdTransition::new(
                symbols.get_label("d").unwrap(),
                symbols.get_label("c").unwrap(),
                TropicalWeight::new(1.5),
                right_final,
            ),
        )
        .unwrap();
    right.set_input_symbols(symbols);
    (left, right)
}

fn ordinary_reference(mut left: StdVectorFst, mut right: StdVectorFst) -> StdVectorFst {
    let input_symbols = Arc::clone(left.input_symbols().unwrap());
    left.set_output_symbols(Arc::clone(&input_symbols));
    right.set_input_symbols(Arc::clone(&input_symbols));
    algorithms::ArcSortOutput(&mut left);
    algorithms::ArcSortInput(&mut right);

    let mut result = StdVectorFst::new();
    algorithms::Compose(&left, &right, &mut result);
    algorithms::Connect(&mut result);
    result.set_input_symbols(input_symbols);
    result
}

#[test]
fn owned_compose_matches_all_memory_plans() {
    let symbols = symbol_table();
    let left = one_arc(Arc::clone(&symbols), "a", "b", 0.25, 0.5);
    let right = one_arc(Arc::clone(&symbols), "b", "c", 0.75, 1.0);
    let expected = ordinary_reference(left.clone(), right.clone());
    let scratch = TestScratchDir::new();

    let unbounded =
        TropicalWeightTransducer::try_compose_owned(left.clone(), right.clone(), None, None)
            .unwrap();
    assert!(isomorphic(&unbounded, &expected).unwrap());
    assert!(Arc::ptr_eq(unbounded.input_symbols().unwrap(), &symbols));
    assert!(scratch.is_empty(), "unbounded compose created scratch");

    // 200 bytes becomes a 135-byte pair cap and 45-byte product cap:
    // the start pair fits, while the second pair and first state+arc force
    // the two stores to migrate independently. Zero forces both at once.
    for allowance in [0, 200] {
        let bounded = TropicalWeightTransducer::try_compose_owned_with_memory_plan(
            left.clone(),
            right.clone(),
            None,
            hfst_openfst::compose_storage::ComposeMemoryPlan::from_allowance(Some(allowance)),
            scratch.path().to_path_buf(),
            false,
        )
        .unwrap();
        assert_eq!(
            bounded, unbounded,
            "{allowance}-byte compose changed exact state or arc ordering"
        );
        assert!(Arc::ptr_eq(bounded.input_symbols().unwrap(), &symbols));
        assert!(
            scratch.is_empty(),
            "{allowance}-byte compose left pair or product scratch behind"
        );
    }
}

#[test]
fn unbounded_compose_ignores_scratch_parent() {
    let symbols = symbol_table();
    let left = one_arc(Arc::clone(&symbols), "a", "b", 0.25, 0.5);
    let right = one_arc(symbols, "b", "c", 0.75, 1.0);
    let scratch = TestScratchDir::new();
    let not_a_directory = scratch.path().join("not-a-directory");
    std::fs::File::create(&not_a_directory).unwrap();

    TropicalWeightTransducer::try_compose_owned_with_memory_plan(
        left,
        right,
        None,
        hfst_openfst::compose_storage::ComposeMemoryPlan::Unbounded,
        not_a_directory,
        false,
    )
    .expect("unbounded compose must not touch its scratch parent");
}

#[test]
fn external_trim_matches_connect() {
    let symbols = symbol_table();
    let (left, right) = successful_and_dead_branches(Arc::clone(&symbols));
    let unbounded =
        TropicalWeightTransducer::try_compose_owned(left.clone(), right.clone(), None, None)
            .unwrap();
    let scratch = TestScratchDir::new();
    let trimmed = TropicalWeightTransducer::try_compose_owned_with_memory_plan(
        left,
        right,
        None,
        hfst_openfst::compose_storage::ComposeMemoryPlan::from_allowance(Some(0)),
        scratch.path().to_path_buf(),
        false,
    )
    .unwrap();

    assert_eq!(trimmed, unbounded);
    assert_eq!(trimmed.num_states(), 2);
    let start = trimmed.start().unwrap();
    assert_eq!(trimmed.get_trs(start).unwrap().len(), 1);
    assert!(Arc::ptr_eq(trimmed.input_symbols().unwrap(), &symbols));
    assert!(scratch.is_empty(), "external trim left scratch behind");
}
