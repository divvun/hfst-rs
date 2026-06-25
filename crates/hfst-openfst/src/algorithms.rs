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
