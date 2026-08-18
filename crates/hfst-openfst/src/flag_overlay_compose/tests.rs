use super::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rustfst::algorithms::compose::compose;
use rustfst::algorithms::tr_compares::{ILabelCompare, OLabelCompare};
use rustfst::algorithms::{connect, isomorphic, tr_sort};
use rustfst::fst_traits::{ExpandedFst, MutableFst};

const LEFT_FLAG: Label = 10;
const RIGHT_FLAG: Label = 20;
const REGULAR: Label = 30;

static NEXT_SCRATCH_DIR: AtomicU64 = AtomicU64::new(0);

struct TestScratchDir(PathBuf);

impl TestScratchDir {
    fn new() -> Result<Self> {
        let sequence = NEXT_SCRATCH_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "hfst-overlay-pair-store-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn is_empty(&self) -> Result<bool> {
        Ok(std::fs::read_dir(&self.0)?.next().is_none())
    }
}

impl Drop for TestScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn state(fst: &mut StdVectorFst) -> StateId {
    fst.add_state()
}

fn one_state_final() -> Result<StdVectorFst> {
    let mut fst = StdVectorFst::new();
    let start = state(&mut fst);
    fst.set_start(start)?;
    fst.set_final(start, TropicalWeight::one())?;
    Ok(fst)
}

fn sort_left(fst: &mut StdVectorFst) {
    tr_sort(fst, OLabelCompare {});
}

fn sort_right(fst: &mut StdVectorFst) {
    tr_sort(fst, ILabelCompare {});
}

fn add_self_loops(fst: &mut StdVectorFst, labels: &[Label]) -> Result<()> {
    for state in 0..fst.num_states() as StateId {
        for label in labels {
            fst.add_tr(state, Tr::new(*label, *label, TropicalWeight::one(), state))?;
        }
    }
    Ok(())
}

fn materialize(lazy: &FlagOverlayComposeFst) -> Result<StdVectorFst> {
    let mut result: StdVectorFst = lazy.inner.compute()?;
    connect(&mut result)?;
    Ok(result)
}

fn materialize_pair_store_variants(
    left: Arc<StdVectorFst>,
    right: Arc<StdVectorFst>,
    overlay: FlagOverlay,
) -> Result<StdVectorFst> {
    let lazy = compose_flag_overlay_lazy(Arc::clone(&left), Arc::clone(&right), overlay.clone())?;
    let actual = materialize(&lazy)?;

    let scratch = TestScratchDir::new()?;
    let spilled_lazy = compose_flag_overlay_lazy_with_store(
        left,
        right,
        overlay,
        Some(ComposeStateStoreConfig::new(0, scratch.path())),
    )?;
    assert!(
        !scratch.is_empty()?,
        "a zero-byte pair-state cap must create scratch storage"
    );
    let spilled = materialize(&spilled_lazy)?;
    drop(spilled_lazy);
    assert!(
        scratch.is_empty()?,
        "dropping the lazy compose FST must clean pair-state scratch"
    );
    assert_eq!(
        spilled, actual,
        "forced-spill and in-memory pair stores changed exact state or arc ordering"
    );
    Ok(actual)
}

fn ordinary_compose(left: &StdVectorFst, right: &StdVectorFst) -> Result<StdVectorFst> {
    let mut left = left.clone();
    let mut right = right.clone();
    sort_left(&mut left);
    sort_right(&mut right);
    let mut result = compose::<TropicalWeight, _, _, StdVectorFst>(&left, &right)?;
    connect(&mut result)?;
    Ok(result)
}

fn explicit_overlay_compose(
    left: &StdVectorFst,
    right: &StdVectorFst,
    overlay: &FlagOverlay,
) -> Result<StdVectorFst> {
    let mut left = left.clone();
    let mut right = right.clone();
    add_self_loops(&mut left, overlay.left_self_loops())?;
    add_self_loops(&mut right, overlay.right_self_loops())?;
    ordinary_compose(&left, &right)
}

fn arc_count(fst: &StdVectorFst) -> Result<usize> {
    (0..fst.num_states() as StateId)
        .try_fold(0, |count, state| Ok(count + fst.get_trs(state)?.len()))
}

fn assert_overlay_matches_explicit(
    mut left: StdVectorFst,
    mut right: StdVectorFst,
    overlay: FlagOverlay,
) -> Result<StdVectorFst> {
    let expected = explicit_overlay_compose(&left, &right, &overlay)?;
    sort_left(&mut left);
    sort_right(&mut right);
    let left = Arc::new(left);
    let right = Arc::new(right);
    let left_arcs = arc_count(&left)?;
    let right_arcs = arc_count(&right)?;

    let actual = materialize_pair_store_variants(Arc::clone(&left), Arc::clone(&right), overlay)?;

    assert_eq!(arc_count(&left)?, left_arcs, "left operand was mutated");
    assert_eq!(arc_count(&right)?, right_arcs, "right operand was mutated");
    assert!(
        isomorphic(&actual, &expected)?,
        "virtual overlay differs from explicit self-loop insertion"
    );
    Ok(actual)
}

#[test]
fn overlay_validation_handles_canonical_and_invalid_labels() {
    let overlay = FlagOverlay::new(vec![8, 7, 8], vec![10, 9], true).unwrap();
    assert_eq!(overlay.left_self_loops(), &[7, 8]);
    assert_eq!(overlay.right_self_loops(), &[9, 10]);
    assert!(overlay.enforces_left_before_right());

    assert!(FlagOverlay::new(vec![EPS_LABEL], vec![], false).is_err());
    assert!(FlagOverlay::new(vec![], vec![NO_LABEL], false).is_err());
    assert!(FlagOverlay::new(vec![7], vec![7], false).is_err());
}

#[test]
fn empty_overlay_is_identical_to_ordinary_composition() -> Result<()> {
    let mut left = StdVectorFst::new();
    let l0 = state(&mut left);
    let l1 = state(&mut left);
    left.set_start(l0)?;
    left.set_final(l1, TropicalWeight::new(0.25))?;
    left.add_tr(l0, Tr::new(7, REGULAR, TropicalWeight::new(0.5), l1))?;

    let mut right = StdVectorFst::new();
    let r0 = state(&mut right);
    let r1 = state(&mut right);
    right.set_start(r0)?;
    right.set_final(r1, TropicalWeight::new(0.75))?;
    right.add_tr(r0, Tr::new(REGULAR, 9, TropicalWeight::new(1.25), r1))?;

    assert_overlay_matches_explicit(left, right, FlagOverlay::default())?;
    Ok(())
}

#[test]
fn virtual_right_loop_works_when_listing_left() -> Result<()> {
    // left priority 1 > right priority 0: rustfst enumerates the right FST
    // and asks matcher1 for NO_LABEL. The left flag must be listed there.
    let mut left = StdVectorFst::new();
    let l0 = state(&mut left);
    let l1 = state(&mut left);
    left.set_start(l0)?;
    left.set_final(l1, TropicalWeight::new(0.75))?;
    left.add_tr(l0, Tr::new(7, LEFT_FLAG, TropicalWeight::new(1.25), l1))?;
    let right = one_state_final()?;

    let actual = assert_overlay_matches_explicit(
        left,
        right,
        FlagOverlay::new(vec![], vec![LEFT_FLAG], false)?,
    )?;
    assert_eq!(arc_count(&actual)?, 1);
    Ok(())
}

#[test]
fn virtual_right_loop_works_for_direct_query() -> Result<()> {
    // left priority 1 <= right priority 2: rustfst enumerates the left FST
    // and queries matcher2 directly for LEFT_FLAG.
    let mut left = one_state_final()?;
    left.add_tr(0, Tr::new(7, LEFT_FLAG, TropicalWeight::new(1.25), 0))?;
    let mut right = one_state_final()?;
    right.add_tr(0, Tr::new(71, 71, TropicalWeight::one(), 0))?;
    right.add_tr(0, Tr::new(72, 72, TropicalWeight::one(), 0))?;

    assert_overlay_matches_explicit(
        left,
        right,
        FlagOverlay::new(vec![], vec![LEFT_FLAG], false)?,
    )?;
    Ok(())
}

#[test]
fn virtual_left_loop_works_when_listing_right() -> Result<()> {
    // left priority 0 <= right priority 1: rustfst enumerates the left FST
    // and asks matcher2 for NO_LABEL. The right flag must be listed there.
    let left = one_state_final()?;
    let mut right = StdVectorFst::new();
    let r0 = state(&mut right);
    let r1 = state(&mut right);
    right.set_start(r0)?;
    right.set_final(r1, TropicalWeight::new(0.5))?;
    right.add_tr(r0, Tr::new(RIGHT_FLAG, 9, TropicalWeight::new(2.0), r1))?;

    let actual = assert_overlay_matches_explicit(
        left,
        right,
        FlagOverlay::new(vec![RIGHT_FLAG], vec![], false)?,
    )?;
    assert_eq!(arc_count(&actual)?, 1);
    Ok(())
}

#[test]
fn virtual_left_loop_works_for_direct_query() -> Result<()> {
    // left priority 2 > right priority 1: rustfst enumerates the right FST
    // and queries matcher1 directly for RIGHT_FLAG.
    let mut left = one_state_final()?;
    left.add_tr(0, Tr::new(71, 71, TropicalWeight::one(), 0))?;
    left.add_tr(0, Tr::new(72, 72, TropicalWeight::one(), 0))?;
    let mut right = one_state_final()?;
    right.add_tr(0, Tr::new(RIGHT_FLAG, 9, TropicalWeight::new(2.0), 0))?;

    assert_overlay_matches_explicit(
        left,
        right,
        FlagOverlay::new(vec![RIGHT_FLAG], vec![], false)?,
    )?;
    Ok(())
}

#[test]
// [spec:hfst:req:virtual-flag-algebra.materialized-reference/test]
// [spec:hfst:req:virtual-flag-algebra.backend-core/test]
fn true_epsilon_interleaving_matches_materialized_loops() -> Result<()> {
    let mut left = StdVectorFst::new();
    let l0 = state(&mut left);
    let l1 = state(&mut left);
    let l2 = state(&mut left);
    left.set_start(l0)?;
    left.set_final(l2, TropicalWeight::new(0.25))?;
    left.add_tr(l0, Tr::new(7, EPS_LABEL, TropicalWeight::new(0.5), l1))?;
    left.add_tr(l1, Tr::new(8, LEFT_FLAG, TropicalWeight::new(1.5), l2))?;

    let mut right = StdVectorFst::new();
    let r0 = state(&mut right);
    let r1 = state(&mut right);
    right.set_start(r0)?;
    right.set_final(r1, TropicalWeight::new(0.75))?;
    right.add_tr(r0, Tr::new(EPS_LABEL, 9, TropicalWeight::new(2.0), r1))?;

    assert_overlay_matches_explicit(
        left,
        right,
        FlagOverlay::new(vec![], vec![LEFT_FLAG], false)?,
    )?;
    Ok(())
}

fn restriction(labels: &[Label]) -> Result<StdVectorFst> {
    let mut restriction = StdVectorFst::new();
    let clear = state(&mut restriction);
    let saw_right = state(&mut restriction);
    restriction.set_start(clear)?;
    restriction.set_final(clear, TropicalWeight::one())?;
    restriction.set_final(saw_right, TropicalWeight::one())?;

    for label in labels {
        match *label {
            LEFT_FLAG => {
                restriction.add_tr(clear, Tr::new(*label, *label, TropicalWeight::one(), clear))?
            }
            RIGHT_FLAG => {
                restriction.add_tr(
                    clear,
                    Tr::new(*label, *label, TropicalWeight::one(), saw_right),
                )?;
                restriction.add_tr(
                    saw_right,
                    Tr::new(*label, *label, TropicalWeight::one(), saw_right),
                )?;
            }
            _ => {
                restriction.add_tr(clear, Tr::new(*label, *label, TropicalWeight::one(), clear))?;
                restriction.add_tr(
                    saw_right,
                    Tr::new(*label, *label, TropicalWeight::one(), clear),
                )?;
            }
        }
    }
    Ok(restriction)
}

#[test]
// [spec:hfst:req:virtual-flag-algebra.materialized-reference/test]
fn both_sided_overlay_orders_left_before_right() -> Result<()> {
    let mut left = one_state_final()?;
    left.add_tr(
        0,
        Tr::new(LEFT_FLAG, LEFT_FLAG, TropicalWeight::new(0.5), 0),
    )?;
    left.add_tr(0, Tr::new(REGULAR, REGULAR, TropicalWeight::new(0.75), 0))?;

    let mut right = one_state_final()?;
    right.add_tr(
        0,
        Tr::new(RIGHT_FLAG, RIGHT_FLAG, TropicalWeight::new(1.0), 0),
    )?;
    right.add_tr(0, Tr::new(REGULAR, REGULAR, TropicalWeight::new(1.25), 0))?;

    let overlay = FlagOverlay::new(vec![RIGHT_FLAG], vec![LEFT_FLAG], true)?;

    // Independent reference: physically add the loops, compose the left
    // operand with the same two-state restriction HFST builds, then perform
    // an ordinary composition.
    let mut explicit_left = left.clone();
    let mut explicit_right = right.clone();
    add_self_loops(&mut explicit_left, overlay.left_self_loops())?;
    add_self_loops(&mut explicit_right, overlay.right_self_loops())?;
    let restricted_left = ordinary_compose(
        &explicit_left,
        &restriction(&[LEFT_FLAG, RIGHT_FLAG, REGULAR])?,
    )?;
    let expected = ordinary_compose(&restricted_left, &explicit_right)?;

    sort_left(&mut left);
    sort_right(&mut right);
    let actual = materialize_pair_store_variants(Arc::new(left), Arc::new(right), overlay)?;
    assert!(isomorphic(&actual, &expected)?);

    let start = actual.start().expect("non-empty composition");
    let start_trs = actual.get_trs(start)?;
    let saw_right = start_trs
        .iter()
        .find(|tr| tr.ilabel == RIGHT_FLAG && tr.olabel == RIGHT_FLAG)
        .expect("right-origin flag from clear state")
        .nextstate;
    assert_ne!(saw_right, start);
    assert!(start_trs.iter().any(|tr| tr.ilabel == LEFT_FLAG));

    let saw_right_trs = actual.get_trs(saw_right)?;
    assert!(
        !saw_right_trs.iter().any(|tr| tr.ilabel == LEFT_FLAG),
        "a left-origin flag cannot immediately follow a right-origin flag"
    );
    assert!(
        saw_right_trs
            .iter()
            .any(|tr| { tr.ilabel == REGULAR && tr.olabel == REGULAR && tr.nextstate == start })
    );
    Ok(())
}
