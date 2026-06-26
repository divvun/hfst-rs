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

// OpenFST's out-of-place algorithms set the output's symbol tables from the
// input; rustfst drops them when it builds a fresh fst. HFST relies on the fst
// carrying its symbol table through every operation (it reads the table back as
// the symbol↔number map when converting to a HfstBasicTransducer), so we
// re-attach the input's tables to the output, mirroring OpenFST.
fn propagate_symbols<W, F1, F2>(ifst: &F1, ofst: &mut F2)
where
    W: Semiring,
    F1: Fst<W>,
    F2: MutableFst<W>,
{
    if let Some(symt) = ifst.input_symbols() {
        ofst.set_input_symbols(std::sync::Arc::clone(symt));
    }
    if let Some(symt) = ifst.output_symbols() {
        ofst.set_output_symbols(std::sync::Arc::clone(symt));
    }
}

// [fst::Determinize] — ofst := det(ifst)
pub fn Determinize<W, F1, F2>(ifst: &F1, ofst: &mut F2)
where
    W: WeaklyDivisibleSemiring + WeightQuantize,
    F1: ExpandedFst<W>,
    F2: MutableFst<W> + AllocableFst<W>,
{
    *ofst = determinize(ifst).expect("rustfst determinize");
    propagate_symbols(ifst, ofst);
}

// [fst::Reverse] — ofst := reverse(ifst)
pub fn Reverse<W, F1, F2>(ifst: &F1, ofst: &mut F2)
where
    W: Semiring<ReverseWeight = W>,
    F1: ExpandedFst<W>,
    F2: MutableFst<W> + ExpandedFst<W> + AllocableFst<W>,
{
    *ofst = reverse(ifst).expect("rustfst reverse");
    // Reverse swaps the two sides, so the tables swap too.
    if let Some(symt) = ifst.output_symbols() {
        ofst.set_input_symbols(std::sync::Arc::clone(symt));
    }
    if let Some(symt) = ifst.input_symbols() {
        ofst.set_output_symbols(std::sync::Arc::clone(symt));
    }
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
    // OpenFST composition: result.input := fst1.input, result.output := fst2.output.
    if let Some(symt) = fst1.input_symbols() {
        ofst.set_input_symbols(std::sync::Arc::clone(symt));
    }
    if let Some(symt) = fst2.output_symbols() {
        ofst.set_output_symbols(std::sync::Arc::clone(symt));
    }
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
    propagate_symbols(ifst, ofst);
}

// [fst::ShortestPath] — ofst := the single best path of ifst.
pub fn ShortestPath<W, FI, FO>(ifst: &FI, ofst: &mut FO)
where
    W: WeaklyDivisibleSemiring + WeightQuantize + Semiring<ReverseWeight = W> + Into<W> + From<W>,
    FI: ExpandedFst<W>,
    FO: MutableFst<W>,
{
    *ofst = shortest_path(ifst).expect("rustfst shortest_path");
    propagate_symbols(ifst, ofst);
}

// ---- rustfst gaps ----
// Not (yet) in rustfst. Per the plan these are implemented in the rustfst fork
// when a ported HFST test proves one is needed; until then they panic loudly so
// a reaching test fails visibly rather than silently misbehaving.

// Collect the non-epsilon input labels of an fst (its acceptor alphabet).
fn labels_of<W: Semiring, F: ExpandedFst<W>>(fst: &F) -> std::collections::BTreeSet<Label> {
    let mut s = std::collections::BTreeSet::new();
    for q in fst.states_iter() {
        if let Ok(trs) = fst.get_trs(q) {
            for tr in trs.trs() {
                if tr.ilabel != 0 {
                    s.insert(tr.ilabel);
                }
            }
        }
    }
    s
}

// complement of an acceptor over the alphabet `sigma`: determinize, complete with
// a sink state (every missing label leads to the always-accepting sink), then flip
// final/non-final. The language becomes Σ* \ L(fst). Mirrors OpenFST's
// ComplementFst as used by Difference.
fn complement_acceptor<W, F>(
    fst: &F,
    sigma: &std::collections::BTreeSet<Label>,
) -> rustfst::fst_impls::VectorFst<W>
where
    W: WeaklyDivisibleSemiring + WeightQuantize,
    F: ExpandedFst<W>,
{
    use rustfst::fst_impls::VectorFst;
    let mut det: VectorFst<W> = determinize(fst).expect("rustfst determinize (for complement)");
    // Empty source language -> complement is all of Σ*: a single accepting state
    // looping on every symbol.
    if det.start().is_none() {
        let mut all: VectorFst<W> = VectorFst::new();
        let s = all.add_state();
        all.set_start(s).unwrap();
        all.set_final(s, W::one()).unwrap();
        for &l in sigma {
            all.add_tr(s, Tr::new(l, l, W::one(), s)).unwrap();
        }
        return all;
    }
    let sink = det.add_state();
    det.set_final(sink, W::one()).unwrap();
    for &l in sigma {
        det.add_tr(sink, Tr::new(l, l, W::one(), sink)).unwrap();
    }
    let states: Vec<StateId> = det.states_iter().collect();
    for &q in &states {
        if q == sink {
            continue;
        }
        let present: std::collections::BTreeSet<Label> = det
            .get_trs(q)
            .unwrap()
            .trs()
            .iter()
            .map(|t| t.ilabel)
            .collect();
        for &l in sigma {
            if !present.contains(&l) {
                det.add_tr(q, Tr::new(l, l, W::one(), sink)).unwrap();
            }
        }
    }
    for &q in &states {
        if q == sink {
            continue;
        }
        if det.final_weight(q).unwrap().is_some() {
            det.delete_final_weight(q).unwrap();
        } else {
            det.set_final(q, W::one()).unwrap();
        }
    }
    det
}

// [fst::Difference] — ofst := fst1 - fst2 = fst1 ∩ ¬fst2
pub fn Difference<W, F1, F2, F3>(fst1: &F1, fst2: &F2, ofst: &mut F3)
where
    W: WeaklyDivisibleSemiring + WeightQuantize,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
    F3: MutableFst<W> + AllocableFst<W>,
{
    let mut sigma = labels_of(fst1);
    sigma.append(&mut labels_of(fst2));
    let comp = complement_acceptor(fst2, &sigma);
    Intersect(fst1, &comp, ofst);
}

// [fst::Intersect] — ofst := fst1 ∩ fst2 (acceptors); OpenFST implements this as
// the composition of the two acceptors.
pub fn Intersect<W, F1, F2, F3>(fst1: &F1, fst2: &F2, ofst: &mut F3)
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
    F3: MutableFst<W> + AllocableFst<W>,
{
    *ofst =
        compose::<W, F1, F2, F3, &F1, &F2>(fst1, fst2).expect("rustfst intersect (via compose)");
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
//
// OpenFST's `Equivalent` requires both inputs to be DETERMINISTIC and
// EPSILON-FREE acceptors; HFST's `are_equivalent` guarantees this by running
// `RmEpsilon` + `Encode` + `Determinize` before calling here. Under that
// precondition equivalence is decidable by a synchronized product walk: the two
// machines are equivalent iff every reachable paired state agrees on finality
// (and final weight) and exposes exactly the same outgoing labels with equal
// arc weights (determinism makes the per-label successor unique).
pub fn Equivalent<W, F1, F2>(fst1: &F1, fst2: &F2) -> bool
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
{
    use std::collections::{HashMap, HashSet, VecDeque};

    let s1 = fst1.start();
    let s2 = fst2.start();

    // Language-emptiness fallback for the cases where one machine has no start
    // state (the empty language): no final state is reachable from `start`.
    let lang_empty_1 = |start: StateId| -> bool {
        let mut seen: HashSet<StateId> = HashSet::new();
        let mut stack = vec![start];
        seen.insert(start);
        while let Some(q) = stack.pop() {
            if fst1.final_weight(q).unwrap().is_some() {
                return false;
            }
            for tr in fst1.get_trs(q).unwrap().trs() {
                if seen.insert(tr.nextstate) {
                    stack.push(tr.nextstate);
                }
            }
        }
        true
    };
    let lang_empty_2 = |start: StateId| -> bool {
        let mut seen: HashSet<StateId> = HashSet::new();
        let mut stack = vec![start];
        seen.insert(start);
        while let Some(q) = stack.pop() {
            if fst2.final_weight(q).unwrap().is_some() {
                return false;
            }
            for tr in fst2.get_trs(q).unwrap().trs() {
                if seen.insert(tr.nextstate) {
                    stack.push(tr.nextstate);
                }
            }
        }
        true
    };

    match (s1, s2) {
        (None, None) => return true,
        (None, Some(b)) => return lang_empty_2(b),
        (Some(a), None) => return lang_empty_1(a),
        (Some(a), Some(b)) => {
            let mut visited: HashSet<(StateId, StateId)> = HashSet::new();
            let mut queue: VecDeque<(StateId, StateId)> = VecDeque::new();
            visited.insert((a, b));
            queue.push_back((a, b));
            while let Some((q1, q2)) = queue.pop_front() {
                if fst1.final_weight(q1).unwrap() != fst2.final_weight(q2).unwrap() {
                    return false;
                }
                let mut m1: HashMap<Label, (StateId, W)> = HashMap::new();
                for tr in fst1.get_trs(q1).unwrap().trs() {
                    m1.insert(tr.ilabel, (tr.nextstate, tr.weight.clone()));
                }
                let mut m2: HashMap<Label, (StateId, W)> = HashMap::new();
                for tr in fst2.get_trs(q2).unwrap().trs() {
                    m2.insert(tr.ilabel, (tr.nextstate, tr.weight.clone()));
                }
                if m1.len() != m2.len() {
                    return false;
                }
                for (label, (n1, w1)) in &m1 {
                    match m2.get(label) {
                        None => return false,
                        Some((n2, w2)) => {
                            if w1 != w2 {
                                return false;
                            }
                            if visited.insert((*n1, *n2)) {
                                queue.push_back((*n1, *n2));
                            }
                        }
                    }
                }
            }
            true
        }
    }
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
