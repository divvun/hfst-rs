//! OpenFST-shaped algorithm wrappers over rustfst.
//!
//! These mirror the `fst::`-namespace function names and call shapes that HFST's
//! `TropicalWeightTransducer`/`LogWeightTransducer` use (`using namespace fst`),
//! so the wrapper ports can translate `ArcSort(fst, ILabelCompare())`,
//! `Determinize(ifst, &ofst)`, etc. nearly 1:1. Each delegates to rustfst and
//! unwraps its `Result` — OpenFST's algorithms are infallible by contract
//! (errors abort), so a failed rustfst call is a panic here too.
//!
//! Generic over the weight `W` and fst `F` (covering both `VectorFst<Tropical>`
//! and `VectorFst<Log>`); the heavier bounds (`WeaklyDivisibleSemiring`,
//! `WeightQuantize`) are required only by determinize/minimize/reweight, which
//! both HFST semirings satisfy.

use rustfst::prelude::*;
// The module-shaped algorithms need their inner fn (the prelude brings the
// module name, not the function); these explicit `use`s shadow that.
use rustfst::algorithms::compose::compose;
use rustfst::algorithms::concat::concat;
use rustfst::algorithms::determinize::determinize;
use rustfst::algorithms::rm_epsilon::rm_epsilon;
use rustfst::algorithms::tr_compares::{ILabelCompare, OLabelCompare};
use rustfst::algorithms::union::union;
use rustfst::algorithms::{ProjectType, ReweightType};
use rustfst::semirings::{WeaklyDivisibleSemiring, WeightQuantize};

pub use rustfst::algorithms::encode::{EncodeTable, EncodeType, decode, encode};
pub use rustfst::algorithms::{PushType, ReweightType as FstReweightType};

// ---- in-place algorithms ----

// [fst::RmEpsilon]
pub fn RmEpsilon<W: Semiring, F: MutableFst<W>>(fst: &mut F) {
    rm_epsilon(fst).expect("rustfst rm_epsilon");
}

// [fst::ArcSort] with fst::ILabelCompare()
pub fn ArcSortInput<W, F>(fst: &mut F)
where
    W: Semiring,
    F: MutableFst<W>,
{
    tr_sort(fst, ILabelCompare {});
}

// [fst::ArcSort] with fst::OLabelCompare()
pub fn ArcSortOutput<W, F>(fst: &mut F)
where
    W: Semiring,
    F: MutableFst<W>,
{
    tr_sort(fst, OLabelCompare {});
}

// [fst::Invert]
pub fn Invert<W: Semiring, F: MutableFst<W>>(fst: &mut F) {
    invert(fst);
}

// [fst::Connect]
pub fn Connect<W, F>(fst: &mut F)
where
    W: Semiring,
    F: ExpandedFst<W> + MutableFst<W>,
{
    connect(fst).expect("rustfst connect");
}

// [fst::TopSort]
pub fn TopSort<W, F>(fst: &mut F)
where
    W: Semiring,
    F: MutableFst<W>,
{
    top_sort(fst).expect("rustfst top_sort");
}

// [fst::Project] — input side
pub fn ProjectInput<W: Semiring, F: MutableFst<W>>(fst: &mut F) {
    project(fst, ProjectType::ProjectInput);
}

// [fst::Project] — output side
pub fn ProjectOutput<W: Semiring, F: MutableFst<W>>(fst: &mut F) {
    project(fst, ProjectType::ProjectOutput);
}

// [fst::Minimize]
pub fn Minimize<W, F>(fst: &mut F)
where
    W: WeaklyDivisibleSemiring + WeightQuantize + Semiring<ReverseWeight = W>,
    F: MutableFst<W> + ExpandedFst<W> + AllocableFst<W>,
{
    minimize(fst).expect("rustfst minimize");
}

// [fst::Concat] — fst1 := fst1 . fst2
pub fn Concat<W, F1, F2>(fst1: &mut F1, fst2: &F2)
where
    W: Semiring,
    F1: ExpandedFst<W> + MutableFst<W> + AllocableFst<W>,
    F2: ExpandedFst<W>,
{
    concat(fst1, fst2).expect("rustfst concat");
}

// [fst::Union] — fst1 := fst1 | fst2
pub fn Union<W, F1, F2>(fst1: &mut F1, fst2: &F2)
where
    W: Semiring,
    F1: AllocableFst<W> + MutableFst<W>,
    F2: ExpandedFst<W>,
{
    union(fst1, fst2).expect("rustfst union");
}

// [fst::Reweight]
pub fn ReweightToInitial<W, F>(fst: &mut F, potentials: &[W])
where
    W: WeaklyDivisibleSemiring,
    F: MutableFst<W>,
{
    reweight(fst, potentials, ReweightType::ReweightToInitial).expect("rustfst reweight");
}

pub fn ReweightToFinal<W, F>(fst: &mut F, potentials: &[W])
where
    W: WeaklyDivisibleSemiring,
    F: MutableFst<W>,
{
    reweight(fst, potentials, ReweightType::ReweightToFinal).expect("rustfst reweight");
}

// [fst::Encode] / [fst::Decode] — OpenFST passes an EncodeMapper; rustfst returns
// an EncodeTable that Decode consumes.
pub fn Encode<W: Semiring, F: MutableFst<W>>(
    fst: &mut F,
    encode_type: EncodeType,
) -> EncodeTable<W> {
    encode(fst, encode_type).expect("rustfst encode")
}

pub fn Decode<W: Semiring, F: MutableFst<W>>(fst: &mut F, table: EncodeTable<W>) {
    decode(fst, table).expect("rustfst decode");
}

// ---- out-of-place algorithms (OpenFST out-param shape: result written to ofst) ----

// [fst::Determinize] — ofst := det(ifst)
pub fn Determinize<W, F1, F2>(ifst: &F1, ofst: &mut F2)
where
    W: WeaklyDivisibleSemiring + WeightQuantize,
    F1: ExpandedFst<W>,
    F2: MutableFst<W> + AllocableFst<W>,
{
    *ofst = determinize(ifst).expect("rustfst determinize");
}

// [fst::Reverse] — ofst := reverse(ifst)
pub fn Reverse<W, F1, F2>(ifst: &F1, ofst: &mut F2)
where
    W: Semiring<ReverseWeight = W>,
    F1: ExpandedFst<W>,
    F2: MutableFst<W> + ExpandedFst<W> + AllocableFst<W>,
{
    *ofst = reverse(ifst).expect("rustfst reverse");
}

// [fst::Compose] — ofst := fst1 ∘ fst2
pub fn Compose<W, F1, F2, F3>(fst1: &F1, fst2: &F2, ofst: &mut F3)
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
    F3: MutableFst<W> + AllocableFst<W>,
{
    *ofst = compose::<W, F1, F2, F3, &F1, &F2>(fst1, fst2).expect("rustfst compose");
}

// [fst::Push<Arc, reweight_type>] — ofst := push(ifst); push_type selects
// weights/labels (fst::kPushWeights / fst::kPushLabels).
pub fn Push<W, F1, F2>(ifst: &F1, ofst: &mut F2, reweight_type: ReweightType, push_type: PushType)
where
    W: WeaklyDivisibleSemiring + WeightQuantize + Semiring<ReverseWeight = W>,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W> + MutableFst<W> + AllocableFst<W>,
{
    *ofst = push(ifst, reweight_type, push_type).expect("rustfst push");
}

// [fst::ShortestPath] — ofst := the single best path of ifst.
pub fn ShortestPath<W, FI, FO>(ifst: &FI, ofst: &mut FO)
where
    W: WeaklyDivisibleSemiring + WeightQuantize + Semiring<ReverseWeight = W> + Into<W> + From<W>,
    FI: ExpandedFst<W>,
    FO: MutableFst<W>,
{
    *ofst = shortest_path(ifst).expect("rustfst shortest_path");
}

// ---- rustfst gaps ----
// Not (yet) in rustfst. Per the plan these are implemented in the rustfst fork
// when a ported HFST test proves one is needed; until then they panic loudly so
// a reaching test fails visibly rather than silently misbehaving.

// [fst::Difference] — ofst := fst1 - fst2
pub fn Difference<W, F1, F2, F3>(_fst1: &F1, _fst2: &F2, _ofst: &mut F3)
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
    F3: MutableFst<W>,
{
    unimplemented!(
        "rustfst gap: Difference — implement in the rustfst fork when a ported test needs it"
    );
}

// [fst::Intersect] — ofst := fst1 ∩ fst2 (acceptors)
pub fn Intersect<W, F1, F2, F3>(_fst1: &F1, _fst2: &F2, _ofst: &mut F3)
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
    F3: MutableFst<W>,
{
    unimplemented!(
        "rustfst gap: Intersect — implement in the rustfst fork when a ported test needs it"
    );
}

// [fst::Prune] — in-place prune of paths worse than `threshold`
pub fn Prune<W, F>(_fst: &mut F, _threshold: W)
where
    W: Semiring,
    F: MutableFst<W>,
{
    unimplemented!(
        "rustfst gap: Prune — implement in the rustfst fork when a ported test needs it"
    );
}

// [fst::Equivalent] — are fst1 and fst2 equivalent?
pub fn Equivalent<W, F1, F2>(_fst1: &F1, _fst2: &F2) -> bool
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
{
    unimplemented!(
        "rustfst gap: Equivalent — implement in the rustfst fork when a ported test needs it"
    );
}

// [fst::EpsNormalize] — ofst := eps-normalized ifst
pub fn EpsNormalize<W, F1, F2>(_ifst: &F1, _ofst: &mut F2)
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: MutableFst<W>,
{
    unimplemented!(
        "rustfst gap: EpsNormalize — implement in the rustfst fork when a ported test needs it"
    );
}
