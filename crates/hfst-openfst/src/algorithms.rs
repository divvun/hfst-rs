//! FST algorithms rustfst does not provide: budgeted determinization,
//! complementation, and an equivalence test.
//!
//! Generic over the weight 'W' and fst 'F' (instantiated with
//! 'VectorFst<Tropical>'); the heavier bounds ('WeaklyDivisibleSemiring',
//! 'WeightQuantize') are required only where determinization is involved,
//! which the tropical semiring satisfies.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use anyhow::Result;
use rustfst::algorithms::determinize::{
    DeterminizeConfig, DeterminizeSubsetLimitExceeded, determinize, determinize_with_config,
};
use rustfst::algorithms::lazy::ComputeTrLimitExceeded;
use rustfst::fst_impls::VectorFst;
use rustfst::prelude::*;
use rustfst::semirings::{WeaklyDivisibleSemiring, WeightQuantize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeterminizeBoundedError {
    SubsetElements { limit: usize, attempted: usize },
    Transitions { limit: usize, attempted: usize },
    Other(String),
}

impl std::fmt::Display for DeterminizeBoundedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SubsetElements { limit, attempted } => write!(
                formatter,
                "weighted-subset element budget of {limit} exceeded (attempted {attempted})"
            ),
            Self::Transitions { limit, attempted } => write!(
                formatter,
                "transition budget of {limit} exceeded (attempted {attempted})"
            ),
            Self::Other(message) => formatter.write_str(message),
        }
    }
}

/// The three independent dimensions a bounded determinization may abort on.
/// `None` leaves the corresponding dimension unbounded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeterminizeBudgets {
    pub max_states: Option<usize>,
    pub max_subset_elements: Option<usize>,
    pub max_trs: Option<usize>,
}

// det(ifst) with state, weighted-subset and transition budgets: abort before
// any input-dependent structure can run away. No one dimension implies the
// others: one determinized state can hold a huge weighted NFA subset, and a
// state count well inside its bound can still carry a machine with orders of
// magnitude more transitions than the input.
// [spec:hfst:req:determinize-envelope.transition-axis]
pub fn determinize_bounded<W, F1, F2>(
    ifst: &F1,
    budgets: DeterminizeBudgets,
) -> std::result::Result<F2, DeterminizeBoundedError>
where
    W: WeaklyDivisibleSemiring + WeightQuantize,
    F1: ExpandedFst<W>,
    F2: MutableFst<W> + AllocableFst<W>,
{
    let config = DeterminizeConfig::default()
        .with_max_states(budgets.max_states)
        .with_max_subset_elements(budgets.max_subset_elements)
        .with_max_trs(budgets.max_trs);
    determinize_with_config(ifst, config).map_err(|error| {
        if let Some(limit) = error.downcast_ref::<DeterminizeSubsetLimitExceeded>() {
            return DeterminizeBoundedError::SubsetElements {
                limit: limit.limit,
                attempted: limit.attempted,
            };
        }
        if let Some(limit) = error.downcast_ref::<ComputeTrLimitExceeded>() {
            return DeterminizeBoundedError::Transitions {
                limit: limit.limit,
                attempted: limit.attempted,
            };
        }
        DeterminizeBoundedError::Other(error.to_string())
    })
}

// Collect the non-epsilon input labels of an fst (its acceptor alphabet).
pub fn input_labels<W: Semiring, F: ExpandedFst<W>>(fst: &F) -> BTreeSet<Label> {
    let mut s = BTreeSet::new();
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

// complement of an acceptor over the alphabet 'sigma': determinize, complete with
// a sink state (every missing label leads to the always-accepting sink), then flip
// final/non-final. The language becomes Σ* \ L(fst). Mirrors OpenFST's
// ComplementFst as used by Difference.
pub fn complement_acceptor<W, F>(fst: &F, sigma: &BTreeSet<Label>) -> Result<VectorFst<W>>
where
    W: WeaklyDivisibleSemiring + WeightQuantize,
    F: ExpandedFst<W>,
{
    let mut det: VectorFst<W> = determinize(fst)?;
    // Empty source language -> complement is all of Σ*: a single accepting state
    // looping on every symbol.
    if det.start().is_none() {
        let mut all: VectorFst<W> = VectorFst::new();
        let s = all.add_state();
        all.set_start(s)?;
        all.set_final(s, W::one())?;
        for &l in sigma {
            all.add_tr(s, Tr::new(l, l, W::one(), s))?;
        }
        return Ok(all);
    }
    let sink = det.add_state();
    det.set_final(sink, W::one())?;
    for &l in sigma {
        det.add_tr(sink, Tr::new(l, l, W::one(), sink))?;
    }
    let states: Vec<StateId> = det.states_iter().collect();
    for &q in &states {
        if q == sink {
            continue;
        }
        let present: BTreeSet<Label> = det.get_trs(q)?.trs().iter().map(|t| t.ilabel).collect();
        for &l in sigma {
            if !present.contains(&l) {
                det.add_tr(q, Tr::new(l, l, W::one(), sink))?;
            }
        }
    }
    for &q in &states {
        if q == sink {
            continue;
        }
        if det.final_weight(q)?.is_some() {
            det.delete_final_weight(q)?;
        } else {
            det.set_final(q, W::one())?;
        }
    }
    Ok(det)
}

// Are fst1 and fst2 equivalent?
//
// Like OpenFST's 'Equivalent', this requires both inputs to be DETERMINISTIC
// and EPSILON-FREE acceptors; HFST's 'are_equivalent' guarantees this by
// removing epsilons, encoding and determinizing before calling here. Under that
// precondition equivalence is decidable by a synchronized product walk: the two
// machines are equivalent iff every reachable paired state agrees on finality
// (and final weight) and exposes exactly the same outgoing labels with equal
// arc weights (determinism makes the per-label successor unique).
pub fn equivalent<W, F1, F2>(fst1: &F1, fst2: &F2) -> Result<bool>
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
{
    // A machine with no start state denotes the empty language; the other is
    // then equivalent iff no final state is reachable from its start.
    match (fst1.start(), fst2.start()) {
        (None, None) => Ok(true),
        (None, Some(b)) => language_is_empty(fst2, b),
        (Some(a), None) => language_is_empty(fst1, a),
        (Some(a), Some(b)) => paired_walk_agrees(fst1, fst2, a, b),
    }
}

fn language_is_empty<W: Semiring, F: ExpandedFst<W>>(fst: &F, start: StateId) -> Result<bool> {
    let mut seen: HashSet<StateId> = HashSet::new();
    let mut stack = vec![start];
    seen.insert(start);
    while let Some(q) = stack.pop() {
        if fst.final_weight(q)?.is_some() {
            return Ok(false);
        }
        for tr in fst.get_trs(q)?.trs() {
            if seen.insert(tr.nextstate) {
                stack.push(tr.nextstate);
            }
        }
    }
    Ok(true)
}

fn paired_walk_agrees<W, F1, F2>(fst1: &F1, fst2: &F2, a: StateId, b: StateId) -> Result<bool>
where
    W: Semiring,
    F1: ExpandedFst<W>,
    F2: ExpandedFst<W>,
{
    let mut visited: HashSet<(StateId, StateId)> = HashSet::new();
    let mut queue: VecDeque<(StateId, StateId)> = VecDeque::new();
    visited.insert((a, b));
    queue.push_back((a, b));
    while let Some((q1, q2)) = queue.pop_front() {
        if fst1.final_weight(q1)? != fst2.final_weight(q2)? {
            return Ok(false);
        }
        let mut m1: HashMap<Label, (StateId, W)> = HashMap::new();
        for tr in fst1.get_trs(q1)?.trs() {
            m1.insert(tr.ilabel, (tr.nextstate, tr.weight.clone()));
        }
        let mut m2: HashMap<Label, (StateId, W)> = HashMap::new();
        for tr in fst2.get_trs(q2)?.trs() {
            m2.insert(tr.ilabel, (tr.nextstate, tr.weight.clone()));
        }
        if m1.len() != m2.len() {
            return Ok(false);
        }
        for (label, (n1, w1)) in &m1 {
            match m2.get(label) {
                None => return Ok(false),
                Some((n2, w2)) => {
                    if w1 != w2 {
                        return Ok(false);
                    }
                    if visited.insert((*n1, *n2)) {
                        queue.push_back((*n1, *n2));
                    }
                }
            }
        }
    }
    Ok(true)
}
