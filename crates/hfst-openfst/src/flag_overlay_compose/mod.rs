//! Lazy composition with virtual flag-diacritic self-loops.
//!
//! HFST's `-F` composition normally inserts every flag missing from one
//! operand as a zero-weight self-loop at every state of the other operand.
//! That eagerly costs `states * missing_flags` transitions.  This module
//! presents those loops through custom rustfst matchers instead, so a loop is
//! created only when composition actually asks for that label at that state.

use std::sync::Arc;

use anyhow::{Result, bail};
use rustfst::algorithms::compose::compose_filters::{ComposeFilter, ComposeFilterBuilder};
use rustfst::algorithms::compose::filter_states::{
    FilterState, IntegerFilterState, PairFilterState,
};
use rustfst::algorithms::compose::matchers::{
    IterItemMatcher, MatchType, Matcher, MatcherFlags, REQUIRE_PRIORITY, SortedMatcher,
};
use rustfst::algorithms::compose::{
    ComposeFst, ComposeFstOpOptions, ComposeFstOpState, ComposeStateStoreConfig, ComposeStateTuple,
};
use rustfst::algorithms::lazy::NullCache;
use rustfst::fst_properties::FstProperties;
use rustfst::fst_traits::{CoreFst, Fst};
use rustfst::semirings::{Semiring, TropicalWeight};
use rustfst::{EPS_LABEL, Label, NO_LABEL, StateId, Tr, Trs, TrsVec};

use crate::StdVectorFst;

type FstHandle = Arc<StdVectorFst>;
type BaseMatcher = SortedMatcher<TropicalWeight, StdVectorFst, FstHandle>;
type BaseMatcherIter = <BaseMatcher as Matcher<TropicalWeight, StdVectorFst, FstHandle>>::Iter;

/// The virtual loops needed to reproduce HFST flag harmonization.
///
/// `left_self_loops` are labels logically inserted at every state of the left
/// operand; `right_self_loops` are inserted on the right.  The two sets must be
/// disjoint (the both-sided `-F` path obtains that property by renaming the
/// operands' flag features to `_1` and `_2`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// [spec:hfst:req:virtual-flag-algebra.backend-core]
pub struct FlagOverlay {
    left_self_loops: Arc<[Label]>,
    right_self_loops: Arc<[Label]>,
    ordering_epsilon_labels: Arc<[Label]>,
    enforce_left_before_right: bool,
    flags_as_epsilon: bool,
}

impl FlagOverlay {
    /// Builds a validated overlay. Duplicate labels within one side are
    /// harmless and are canonicalized away.
    pub fn new(
        mut left_self_loops: Vec<Label>,
        mut right_self_loops: Vec<Label>,
        enforce_left_before_right: bool,
    ) -> Result<Self> {
        canonicalize_labels(&mut left_self_loops)?;
        canonicalize_labels(&mut right_self_loops)?;

        if left_self_loops
            .iter()
            .any(|label| right_self_loops.binary_search(label).is_ok())
        {
            bail!("flag overlay label sets must be disjoint");
        }

        Ok(Self {
            left_self_loops: Arc::from(left_self_loops),
            right_self_loops: Arc::from(right_self_loops),
            ordering_epsilon_labels: Arc::from([]),
            enforce_left_before_right,
            flags_as_epsilon: false,
        })
    }

    /// Treats real flag events and the virtual loops as the one-sided epsilon
    /// moves used by HFST's `flag-is-epsilon` composition mode.
    // [spec:hfst:req:virtual-flag-algebra.special-compose]
    pub fn with_flags_as_epsilon(mut self) -> Self {
        self.flags_as_epsilon = true;
        self
    }

    /// Marks encoded labels whose logical left-output label is epsilon.
    ///
    /// Intersection encodes an input/output pair as one acceptor label before
    /// running the product.  An encoded `x:epsilon` transition is numerically
    /// non-epsilon, but it must not reset HFST's two-sided flag-order state.
    pub fn with_ordering_epsilon_labels(mut self, mut labels: Vec<Label>) -> Result<Self> {
        canonicalize_labels(&mut labels)?;
        if labels
            .iter()
            .any(|label| self.left_contains(*label) || self.right_contains(*label))
        {
            bail!("flag labels cannot also be ordering-epsilon labels");
        }
        self.ordering_epsilon_labels = Arc::from(labels);
        Ok(self)
    }

    pub fn left_self_loops(&self) -> &[Label] {
        &self.left_self_loops
    }

    pub fn right_self_loops(&self) -> &[Label] {
        &self.right_self_loops
    }

    pub fn enforces_left_before_right(&self) -> bool {
        self.enforce_left_before_right
    }

    pub fn flags_are_epsilon(&self) -> bool {
        self.flags_as_epsilon
    }

    fn left_contains(&self, label: Label) -> bool {
        self.left_self_loops.binary_search(&label).is_ok()
    }

    fn right_contains(&self, label: Label) -> bool {
        self.right_self_loops.binary_search(&label).is_ok()
    }

    fn is_ordering_epsilon(&self, label: Label) -> bool {
        self.ordering_epsilon_labels.binary_search(&label).is_ok()
    }
}

fn canonicalize_labels(labels: &mut Vec<Label>) -> Result<()> {
    if labels
        .iter()
        .any(|label| *label == EPS_LABEL || *label == NO_LABEL)
    {
        bail!("flag overlay labels cannot be epsilon or NO_LABEL");
    }
    labels.sort_unstable();
    labels.dedup();
    Ok(())
}

/// Scans the real transitions for true epsilons plus a small set of labels.
///
/// rustfst asks a matcher for `NO_LABEL` when it elects to enumerate the
/// opposite FST.  Returning the real flag transitions here is what makes the
/// virtual overlay correct whichever side wins rustfst's priority decision.
struct ListedMatcherIter {
    base: BaseMatcherIter,
    trs: TrsVec<TropicalWeight>,
    position: usize,
    match_type: MatchType,
    listed_labels: Arc<[Label]>,
}

impl Iterator for ListedMatcherIter {
    type Item = IterItemMatcher<TropicalWeight>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.base.next() {
            return Some(item);
        }
        while let Some(tr) = self.trs.get(self.position) {
            self.position += 1;
            let label = match self.match_type {
                MatchType::MatchInput => tr.ilabel,
                MatchType::MatchOutput => tr.olabel,
                MatchType::MatchBoth | MatchType::MatchNone | MatchType::MatchUnknown => {
                    unreachable!("validated overlay matcher type")
                }
            };
            if label != EPS_LABEL && self.listed_labels.binary_search(&label).is_ok() {
                return Some(IterItemMatcher::Tr(tr.clone()));
            }
        }
        None
    }
}

/// Enumerates the logical epsilon moves of the flag-as-epsilon mode without
/// adding them to either operand. Real flags on this matcher are returned with
/// their original label so the filter can classify the event before rewriting
/// it to epsilon. The two synthetic groups represent this operand's missing
/// loops and the opposite operand's missing loops, respectively.
struct EpsilonMatcherIter {
    base: BaseMatcherIter,
    trs: TrsVec<TropicalWeight>,
    position: usize,
    match_type: MatchType,
    listed_labels: Arc<[Label]>,
    loop_labels: Arc<[Label]>,
    own_virtual_position: usize,
    opposite_virtual_position: usize,
    state: StateId,
}

impl Iterator for EpsilonMatcherIter {
    type Item = IterItemMatcher<TropicalWeight>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.base.next() {
            return Some(item);
        }
        while let Some(tr) = self.trs.get(self.position) {
            self.position += 1;
            let label = match self.match_type {
                MatchType::MatchInput => tr.ilabel,
                MatchType::MatchOutput => tr.olabel,
                MatchType::MatchBoth | MatchType::MatchNone | MatchType::MatchUnknown => {
                    unreachable!("validated overlay matcher type")
                }
            };
            if label != EPS_LABEL && self.listed_labels.binary_search(&label).is_ok() {
                return Some(IterItemMatcher::Tr(tr.clone()));
            }
        }
        if let Some(label) = self.loop_labels.get(self.own_virtual_position) {
            self.own_virtual_position += 1;
            return Some(IterItemMatcher::Tr(Tr::new(
                *label,
                *label,
                TropicalWeight::one(),
                self.state,
            )));
        }
        if let Some(label) = self.listed_labels.get(self.opposite_virtual_position) {
            self.opposite_virtual_position += 1;
            let tr = match self.match_type {
                MatchType::MatchInput => {
                    Tr::new(NO_LABEL, *label, TropicalWeight::one(), self.state)
                }
                MatchType::MatchOutput => {
                    Tr::new(*label, NO_LABEL, TropicalWeight::one(), self.state)
                }
                MatchType::MatchBoth | MatchType::MatchNone | MatchType::MatchUnknown => {
                    unreachable!("validated overlay matcher type")
                }
            };
            return Some(IterItemMatcher::Tr(tr));
        }
        None
    }
}

enum OverlayMatcherIter {
    Base(BaseMatcherIter),
    BaseThenVirtual {
        base: BaseMatcherIter,
        virtual_tr: Option<IterItemMatcher<TropicalWeight>>,
    },
    Listed(ListedMatcherIter),
    Epsilon(EpsilonMatcherIter),
    One(Option<IterItemMatcher<TropicalWeight>>),
}

impl Iterator for OverlayMatcherIter {
    type Item = IterItemMatcher<TropicalWeight>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Base(iter) => iter.next(),
            Self::BaseThenVirtual { base, virtual_tr } => base.next().or_else(|| virtual_tr.take()),
            Self::Listed(iter) => iter.next(),
            Self::Epsilon(iter) => iter.next(),
            Self::One(item) => item.take(),
        }
    }
}

/// A sorted matcher that overlays exact, unit-weight self-loops lazily.
#[derive(Clone, Debug)]
struct OverlayMatcher {
    base: BaseMatcher,
    match_type: MatchType,
    loop_labels: Arc<[Label]>,
    listed_labels: Arc<[Label]>,
    flags_as_epsilon: bool,
}

impl OverlayMatcher {
    fn with_overlay(
        fst: FstHandle,
        match_type: MatchType,
        loop_labels: Arc<[Label]>,
        listed_labels: Arc<[Label]>,
        flags_as_epsilon: bool,
    ) -> Result<Self> {
        if !matches!(match_type, MatchType::MatchInput | MatchType::MatchOutput) {
            bail!("overlay matcher requires input or output matching");
        }
        let base = BaseMatcher::new(fst, match_type)?;
        Ok(Self {
            base,
            match_type,
            loop_labels,
            listed_labels,
            flags_as_epsilon,
        })
    }

    fn virtual_loop(&self, state: StateId, label: Label) -> Tr<TropicalWeight> {
        match self.match_type {
            MatchType::MatchInput => Tr::new(label, NO_LABEL, TropicalWeight::one(), state),
            MatchType::MatchOutput => Tr::new(NO_LABEL, label, TropicalWeight::one(), state),
            MatchType::MatchBoth | MatchType::MatchNone | MatchType::MatchUnknown => {
                unreachable!("validated overlay matcher type")
            }
        }
    }

    fn is_flag_label(&self, label: Label) -> bool {
        self.loop_labels.binary_search(&label).is_ok()
            || self.listed_labels.binary_search(&label).is_ok()
    }
}

impl Matcher<TropicalWeight, StdVectorFst, FstHandle> for OverlayMatcher {
    type Iter = OverlayMatcherIter;

    fn new(fst: FstHandle, match_type: MatchType) -> Result<Self> {
        Self::with_overlay(fst, match_type, Arc::from([]), Arc::from([]), false)
    }

    fn iter(&self, state: StateId, label: Label) -> Result<Self::Iter> {
        if self.flags_as_epsilon {
            if label == NO_LABEL {
                return Ok(OverlayMatcherIter::Epsilon(EpsilonMatcherIter {
                    base: self.base.iter(state, NO_LABEL)?,
                    trs: self.base.fst().as_ref().get_trs(state)?,
                    position: 0,
                    match_type: self.match_type,
                    listed_labels: Arc::clone(&self.listed_labels),
                    loop_labels: Arc::clone(&self.loop_labels),
                    own_virtual_position: 0,
                    opposite_virtual_position: 0,
                    state,
                }));
            }
            if self.is_flag_label(label) {
                return Ok(OverlayMatcherIter::One(Some(IterItemMatcher::EpsLoop)));
            }
        }
        if label == NO_LABEL {
            return Ok(OverlayMatcherIter::Listed(ListedMatcherIter {
                base: self.base.iter(state, NO_LABEL)?,
                trs: self.base.fst().as_ref().get_trs(state)?,
                position: 0,
                match_type: self.match_type,
                listed_labels: Arc::clone(&self.listed_labels),
            }));
        }

        let base = self.base.iter(state, label)?;
        if self.loop_labels.binary_search(&label).is_ok() {
            Ok(OverlayMatcherIter::BaseThenVirtual {
                base,
                virtual_tr: Some(IterItemMatcher::Tr(self.virtual_loop(state, label))),
            })
        } else {
            Ok(OverlayMatcherIter::Base(base))
        }
    }

    fn final_weight(&self, state: StateId) -> Result<Option<TropicalWeight>> {
        self.base.final_weight(state)
    }

    fn match_type(&self, test: bool) -> Result<MatchType> {
        self.base.match_type(test)
    }

    fn flags(&self) -> MatcherFlags {
        self.base.flags()
    }

    fn priority(&self, state: StateId) -> Result<usize> {
        if self.flags_as_epsilon && self.match_type == MatchType::MatchInput {
            Ok(REQUIRE_PRIORITY)
        } else {
            self.base.priority(state)
        }
    }

    fn fst(&self) -> &FstHandle {
        self.base.fst()
    }
}

type OverlayFilterState = PairFilterState<IntegerFilterState, IntegerFilterState>;

const FLAG_ORDER_CLEAR: StateId = 0;
const FLAG_ORDER_SAW_RIGHT: StateId = 1;

#[derive(Clone, Debug)]
struct OverlayComposeFilterBuilder {
    matcher1: Arc<OverlayMatcher>,
    matcher2: Arc<OverlayMatcher>,
    overlay: FlagOverlay,
}

impl OverlayComposeFilterBuilder {
    fn with_overlay(
        fst1: FstHandle,
        fst2: FstHandle,
        matcher1: Option<OverlayMatcher>,
        matcher2: Option<OverlayMatcher>,
        overlay: FlagOverlay,
    ) -> Result<Self> {
        let matcher1 = match matcher1 {
            Some(matcher) => matcher,
            None => OverlayMatcher::new(fst1, MatchType::MatchOutput)?,
        };
        let matcher2 = match matcher2 {
            Some(matcher) => matcher,
            None => OverlayMatcher::new(fst2, MatchType::MatchInput)?,
        };
        Ok(Self {
            matcher1: Arc::new(matcher1),
            matcher2: Arc::new(matcher2),
            overlay,
        })
    }
}

impl
    ComposeFilterBuilder<
        TropicalWeight,
        StdVectorFst,
        StdVectorFst,
        FstHandle,
        FstHandle,
        OverlayMatcher,
        OverlayMatcher,
    > for OverlayComposeFilterBuilder
{
    type IM1 = OverlayMatcher;
    type IM2 = OverlayMatcher;
    type CF = OverlayComposeFilter;

    fn new(
        fst1: FstHandle,
        fst2: FstHandle,
        matcher1: Option<OverlayMatcher>,
        matcher2: Option<OverlayMatcher>,
    ) -> Result<Self> {
        Self::with_overlay(fst1, fst2, matcher1, matcher2, FlagOverlay::default())
    }

    fn build(&self) -> Result<Self::CF> {
        Ok(OverlayComposeFilter {
            matcher1: Arc::clone(&self.matcher1),
            matcher2: Arc::clone(&self.matcher2),
            overlay: self.overlay.clone(),
            sequence_state: IntegerFilterState::new(FLAG_ORDER_CLEAR),
            flag_order: IntegerFilterState::new(FLAG_ORDER_CLEAR),
            all_eps_left: false,
            no_eps_left: false,
        })
    }
}

#[derive(Clone, Debug)]
struct OverlayComposeFilter {
    matcher1: Arc<OverlayMatcher>,
    matcher2: Arc<OverlayMatcher>,
    overlay: FlagOverlay,
    sequence_state: IntegerFilterState,
    flag_order: IntegerFilterState,
    all_eps_left: bool,
    no_eps_left: bool,
}

impl
    ComposeFilter<
        TropicalWeight,
        StdVectorFst,
        StdVectorFst,
        FstHandle,
        FstHandle,
        OverlayMatcher,
        OverlayMatcher,
    > for OverlayComposeFilter
{
    type FS = OverlayFilterState;

    fn start(&self) -> Self::FS {
        Self::FS::new((
            IntegerFilterState::new(FLAG_ORDER_CLEAR),
            IntegerFilterState::new(FLAG_ORDER_CLEAR),
        ))
    }

    fn set_state(&mut self, s1: StateId, _s2: StateId, filter_state: &Self::FS) -> Result<()> {
        self.sequence_state = filter_state.state1().clone();
        self.flag_order = filter_state.state2().clone();
        let fst1 = self.matcher1.fst().as_ref();
        let trs = fst1.get_trs(s1)?;
        let transition_count = trs.len();
        let mut epsilon_count = fst1.num_output_epsilons(s1)?;
        let virtual_count = if self.overlay.flags_as_epsilon {
            epsilon_count += trs
                .trs()
                .iter()
                .filter(|tr| {
                    self.overlay.left_contains(tr.olabel) || self.overlay.right_contains(tr.olabel)
                })
                .count();
            self.overlay.left_self_loops.len()
        } else {
            0
        };
        self.all_eps_left = transition_count + virtual_count == epsilon_count + virtual_count
            && !fst1.is_final(s1)?;
        self.no_eps_left = epsilon_count + virtual_count == 0;
        Ok(())
    }

    fn filter_tr(
        &mut self,
        arc1: &mut Tr<TropicalWeight>,
        arc2: &mut Tr<TropicalWeight>,
    ) -> Result<Self::FS> {
        let mut logical_left_output = arc1.olabel;
        let mut left_flag = false;
        let mut right_flag = false;

        if self.overlay.flags_as_epsilon {
            let synthetic_left = arc1.ilabel == EPS_LABEL && arc1.olabel == NO_LABEL;
            if synthetic_left && arc2.ilabel == NO_LABEL && self.overlay.left_contains(arc2.olabel)
            {
                // Missing right-origin loop on the left: f:f becomes f:eps.
                logical_left_output = arc2.olabel;
                right_flag = true;
                arc1.ilabel = arc2.olabel;
                arc1.olabel = EPS_LABEL;
                arc2.olabel = EPS_LABEL;
            } else if arc2.ilabel == NO_LABEL
                && arc2.olabel == EPS_LABEL
                && (self.overlay.left_contains(arc1.olabel)
                    || self.overlay.right_contains(arc1.olabel))
            {
                // A real left-output flag participates as an epsilon while its
                // original label still drives the left-path ordering state.
                logical_left_output = arc1.olabel;
                left_flag = self.overlay.right_contains(logical_left_output);
                right_flag = self.overlay.left_contains(logical_left_output);
                arc1.olabel = EPS_LABEL;
            } else if synthetic_left
                && (self.overlay.left_contains(arc2.ilabel)
                    || self.overlay.right_contains(arc2.ilabel))
            {
                // A real or virtual right-input flag is a right-only epsilon
                // move and therefore does not alter the left-path ordering.
                logical_left_output = NO_LABEL;
                arc2.ilabel = EPS_LABEL;
            } else if synthetic_left {
                logical_left_output = NO_LABEL;
            }
        } else {
            let left_output = arc1.olabel;
            let direct_virtual_right = arc2.olabel == NO_LABEL
                && arc2.ilabel == left_output
                && self.overlay.right_contains(left_output);
            let listed_virtual_right = arc2.ilabel == NO_LABEL
                && arc2.olabel == EPS_LABEL
                && self.overlay.right_contains(left_output);
            let direct_virtual_left = arc1.ilabel == NO_LABEL
                && arc1.olabel == arc2.ilabel
                && self.overlay.left_contains(arc1.olabel);
            let listed_virtual_left = arc1.ilabel == EPS_LABEL
                && arc1.olabel == NO_LABEL
                && self.overlay.left_contains(arc2.ilabel);

            debug_assert!(!(direct_virtual_left && direct_virtual_right));
            debug_assert!(!(listed_virtual_left && listed_virtual_right));

            if direct_virtual_right {
                arc2.olabel = arc2.ilabel;
            }
            if listed_virtual_right {
                arc2.ilabel = left_output;
                arc2.olabel = left_output;
            }
            if direct_virtual_left {
                arc1.ilabel = arc1.olabel;
            }
            if listed_virtual_left {
                arc1.ilabel = arc2.ilabel;
                arc1.olabel = arc2.ilabel;
            }
            logical_left_output = arc1.olabel;
            left_flag = self.overlay.right_contains(logical_left_output);
            right_flag = self.overlay.left_contains(logical_left_output);
        }

        let sequence_state = if arc1.olabel == NO_LABEL {
            if self.all_eps_left {
                IntegerFilterState::new_no_state()
            } else if self.no_eps_left {
                IntegerFilterState::new(0)
            } else {
                IntegerFilterState::new(1)
            }
        } else if arc2.ilabel == NO_LABEL {
            if self.sequence_state != IntegerFilterState::new(0) {
                IntegerFilterState::new_no_state()
            } else {
                IntegerFilterState::new(0)
            }
        } else if arc1.olabel == EPS_LABEL {
            IntegerFilterState::new_no_state()
        } else {
            IntegerFilterState::new(0)
        };
        if sequence_state == IntegerFilterState::new_no_state() {
            return Ok(Self::FS::new_no_state());
        }

        let next_flag_order = if !self.overlay.enforce_left_before_right {
            FLAG_ORDER_CLEAR
        } else if left_flag && *self.flag_order.state() == FLAG_ORDER_SAW_RIGHT {
            return Ok(Self::FS::new_no_state());
        } else if right_flag {
            FLAG_ORDER_SAW_RIGHT
        } else if logical_left_output != EPS_LABEL
            && logical_left_output != NO_LABEL
            && !self.overlay.is_ordering_epsilon(logical_left_output)
        {
            FLAG_ORDER_CLEAR
        } else {
            *self.flag_order.state()
        };

        Ok(Self::FS::new((
            sequence_state,
            IntegerFilterState::new(next_flag_order),
        )))
    }

    fn filter_final(
        &self,
        _weight1: &mut TropicalWeight,
        _weight2: &mut TropicalWeight,
    ) -> Result<()> {
        Ok(())
    }

    fn matcher1(&self) -> &OverlayMatcher {
        &self.matcher1
    }

    fn matcher2(&self) -> &OverlayMatcher {
        &self.matcher2
    }

    fn matcher1_shared(&self) -> &Arc<OverlayMatcher> {
        &self.matcher1
    }

    fn matcher2_shared(&self) -> &Arc<OverlayMatcher> {
        &self.matcher2
    }

    fn properties(&self, inprops: FstProperties) -> FstProperties {
        inprops
    }
}

type OverlayOpState = ComposeFstOpState<ComposeStateTuple<OverlayFilterState>>;
type InnerOverlayComposeFst = ComposeFst<
    TropicalWeight,
    StdVectorFst,
    StdVectorFst,
    FstHandle,
    FstHandle,
    OverlayMatcher,
    OverlayMatcher,
    OverlayComposeFilterBuilder,
    NullCache<TropicalWeight>,
>;

/// A one-shot lazy composition FST backed by [`NullCache`].
///
/// The inputs are owned through `Arc` so this concrete rustfst `ComposeFst`
/// satisfies the `'static` bounds on rustfst's `Fst` implementation.  The
/// transition vectors inside `StdVectorFst` are themselves Arc-backed, so an
/// upstream caller can wrap already-owned operands without copying their arcs.
///
/// `NullCache` intentionally does not support `states_iter()` or `num_trs()`.
/// Consumers should begin at `start()`, call `get_trs(state)` exactly once, and
/// discover the dense target state IDs from those transitions.  This is the
/// contract used by the bounded/spilling materializer.
#[derive(Debug)]
pub struct FlagOverlayComposeFst {
    inner: InnerOverlayComposeFst,
}

impl FlagOverlayComposeFst {
    pub fn new(fst1: FstHandle, fst2: FstHandle, overlay: FlagOverlay) -> Result<Self> {
        Self::new_with_state_store(fst1, fst2, overlay, None)
    }

    /// Builds a lazy overlay composition with an optional bounded pair-state
    /// store. When `state_store` is `None`, rustfst's ordinary unbounded
    /// in-memory interner is retained for compatibility.
    pub fn new_with_state_store(
        fst1: FstHandle,
        fst2: FstHandle,
        overlay: FlagOverlay,
        state_store: Option<ComposeStateStoreConfig>,
    ) -> Result<Self> {
        let matcher1 = OverlayMatcher::with_overlay(
            Arc::clone(&fst1),
            MatchType::MatchOutput,
            Arc::clone(&overlay.left_self_loops),
            Arc::clone(&overlay.right_self_loops),
            overlay.flags_as_epsilon,
        )?;
        let matcher2 = OverlayMatcher::with_overlay(
            Arc::clone(&fst2),
            MatchType::MatchInput,
            Arc::clone(&overlay.right_self_loops),
            Arc::clone(&overlay.left_self_loops),
            overlay.flags_as_epsilon,
        )?;
        let filter_builder = OverlayComposeFilterBuilder::with_overlay(
            Arc::clone(&fst1),
            Arc::clone(&fst2),
            Some(matcher1.clone()),
            Some(matcher2.clone()),
            overlay,
        )?;
        let options: ComposeFstOpOptions<
            OverlayMatcher,
            OverlayMatcher,
            OverlayComposeFilterBuilder,
            OverlayOpState,
        > = ComposeFstOpOptions::new(
            Some(matcher1),
            Some(matcher2),
            Some(filter_builder),
            state_store.map(OverlayOpState::new_spillable).transpose()?,
        );
        let inner = InnerOverlayComposeFst::new_with_options_and_cache(
            fst1,
            fst2,
            options,
            NullCache::default(),
        )?;
        Ok(Self { inner })
    }

    /// Borrows the lazy composition through rustfst's ordinary FST interface.
    /// See the type-level documentation for its single-touch traversal
    /// contract.
    pub fn as_fst(&self) -> &impl Fst<TropicalWeight> {
        &self.inner
    }
}

/// Convenience constructor for [`FlagOverlayComposeFst`].
pub fn compose_flag_overlay_lazy(
    fst1: FstHandle,
    fst2: FstHandle,
    overlay: FlagOverlay,
) -> Result<FlagOverlayComposeFst> {
    FlagOverlayComposeFst::new(fst1, fst2, overlay)
}

/// Storage-aware variant of [`compose_flag_overlay_lazy`].
///
/// A zero-byte configuration forces rustfst's pair-state interner to scratch;
/// `None` preserves the ordinary unbounded in-memory interner.
pub fn compose_flag_overlay_lazy_with_store(
    fst1: FstHandle,
    fst2: FstHandle,
    overlay: FlagOverlay,
    state_store: Option<ComposeStateStoreConfig>,
) -> Result<FlagOverlayComposeFst> {
    FlagOverlayComposeFst::new_with_state_store(fst1, fst2, overlay, state_store)
}

#[cfg(test)]
mod tests;
