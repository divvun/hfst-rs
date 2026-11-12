//! Bounded weighted determinization and minimization fallbacks.

use super::operations::{check_epsilon_cycles, reverse_swapping_tables};
use super::*;

pub(super) enum AdaptiveDeterminize {
    Determinized(EncodeTable<TropicalWeight>),
    SubsetLimit,
}

/// The three axes that bound a determinization, each measuring a resource the
/// other two cannot see.
#[derive(Clone, Copy, Debug)]
pub(super) struct DeterminizeBudget {
    /// DFA states produced.
    pub states: usize,
    /// Logical weighted-subset elements held across the expansion.
    pub subset_elements: usize,
    /// Transitions written to the determinized machine.
    pub trs: usize,
}

impl TropicalWeightTransducer {
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn]
    // [spec:hfst:req:determinize-envelope.bounded-strategies]
    pub fn minimize(t: StdVectorFst, encode_weights: bool) -> crate::error::Result<StdVectorFst> {
        Self::minimize_with_reverse_fallback(t, encode_weights, true, None)
    }

    // [spec:hfst:req:determinize-envelope.relation-preserved]
    pub(super) fn minimize_with_reverse_fallback(
        mut t: StdVectorFst,
        encode_weights: bool,
        allow_reverse_fallback: bool,
        budget_override: Option<DeterminizeBudget>,
    ) -> crate::error::Result<StdVectorFst> {
        check_epsilon_cycles(&t, "minimize");

        // (USE_FOMA_EPSILON_REMOVAL && HAVE_FOMA) path is not configured here.
        rm_epsilon(&mut t).map_err(openfst_error("rm_epsilon"))?;

        let w = TropicalWeightTransducer::get_smallest_weight(&t);
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut t, -w);
        }

        let budget = budget_override.unwrap_or_else(|| Self::determinize_budget(&t));
        let mut det = StdVectorFst::new();
        let outcome =
            Self::determinize_adaptive(&mut t, encode_weights, budget, "minimize", &mut det, true)?;
        match outcome {
            AdaptiveDeterminize::Determinized(encode_mapper) => {
                refuse_long_paths(&det)?;
                minimize(&mut det).map_err(openfst_error("minimize"))?;
                decode(&mut det, encode_mapper).map_err(openfst_error("decode"))?;
            }
            AdaptiveDeterminize::SubsetLimit if allow_reverse_fallback => {
                tracing::warn!(
                    "determinization budget exceeded; minimizing in the reverse orientation"
                );
                let reversed = reverse_swapping_tables(&t)?;
                let reversed =
                    Self::minimize_with_reverse_fallback(reversed, encode_weights, false, None)?;
                det = reverse_swapping_tables(&reversed)?;
            }
            AdaptiveDeterminize::SubsetLimit => {
                tracing::warn!(
                    "determinization budget exceeded in both orientations; preserving the exact input language without further minimization"
                );
                det = std::mem::replace(&mut t, StdVectorFst::new());
            }
        }

        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut det, w);
        }

        Ok(det)
    }

    // Bounds all three axes of weighted determinization. A state cap catches
    // non-twins machines that split forever (hfst/hfst#435); the element cap
    // catches a smaller number of enormous weighted subsets before their
    // vectors, normalization maps, and state-table keys consume gigabytes; the
    // transition cap catches the case neither of those sees, where a modest
    // number of states each carry an enormous out-degree.
    //
    // Four million working elements correspond to roughly 256 MiB at the
    // conservative 64-byte transient accounting used to select this fixed
    // safety envelope. The transition floor is the same kind of fixed envelope
    // for the machine actually written out — 128 Mi transitions is about 2 GiB
    // of arc records. It is a floor rather than a ceiling because an input that
    // already holds more transitions than that can afford an output of
    // comparable size: a determinization whose result stays within a small
    // multiple of its input has not blown up, whatever its absolute size, and
    // must keep behaving exactly as before.
    //
    // The state and subset-element axes are floors of the same kind. An input
    // that already holds n states affords a determinized machine within a
    // small multiple of n, and the subset-element counter accumulates across
    // every stored subset without ever shrinking — it tracks the output's
    // size even when every subset is a singleton — so a fixed cap on either
    // axis rejects every determinization whose output exceeds it, however
    // tame the expansion. The measured case is the lang-smj speller
    // generator, a 3.8M-state flag-elimination input whose legitimate
    // determinized form holds 5.9M states (1.56x input, then minimized to
    // 1.2M): both fixed caps sat below it in both orientations, so minimize
    // shipped the 3x-larger unminimized machine. The scaled terms engage only
    // above MAX_STATES / STATES_FLOOR_PER_INPUT_STATE (500,000) and
    // MAX_SUBSET_ELEMENTS / SUBSET_ELEMENTS_FLOOR_PER_INPUT_STATE (524,288)
    // input states; below that the budgets are bit-identical to the fixed
    // envelope, so everything it already bounded — the error-model union
    // among them — keeps behaving exactly as before.
    // [spec:hfst:req:determinize-envelope.transition-axis]
    // [spec:hfst:req:determinize-envelope.input-scaled-floors]
    pub(super) fn determinize_budget(input: &StdVectorFst) -> DeterminizeBudget {
        const STATES_PER_INPUT: usize = 256;
        const MAX_STATES: usize = 2_000_000;
        const STATES_FLOOR_PER_INPUT_STATE: usize = 4;
        const MAX_SUBSET_ELEMENTS: usize = 4 * 1024 * 1024;
        const SUBSET_ELEMENTS_FLOOR_PER_INPUT_STATE: usize = 8;
        const MIN_TRS: usize = 128 * 1024 * 1024;
        const TRS_PER_INPUT_TR: usize = 4;

        let input_states = input.num_states();
        let input_trs: usize = input
            .states_iter()
            .map(|s| input.num_trs(s).expect("s is a valid state of this fst"))
            .sum();

        DeterminizeBudget {
            states: STATES_PER_INPUT
                .saturating_mul(input_states)
                .clamp(1024, MAX_STATES)
                .max(STATES_FLOOR_PER_INPUT_STATE.saturating_mul(input_states)),
            subset_elements: MAX_SUBSET_ELEMENTS
                .max(SUBSET_ELEMENTS_FLOOR_PER_INPUT_STATE.saturating_mul(input_states)),
            trs: TRS_PER_INPUT_TR.saturating_mul(input_trs).max(MIN_TRS),
        }
    }

    // Runs Encode + Determinize with the adaptive budget/fallback and writes
    // the determinized machine into `det`, returning the EncodeTable the
    // caller must Decode with. `t` is encoded in place; on the encoded-weight
    // fallback `t` is re-encoded with weights, so the returned table matches
    // whatever `det` was produced from.
    // [spec:hfst:req:determinize-envelope.bounded-strategies]
    pub(super) fn determinize_adaptive(
        t: &mut StdVectorFst,
        encode_weights: bool,
        budget: DeterminizeBudget,
        caller: &str,
        det: &mut StdVectorFst,
        preserve_on_subset_limit: bool,
    ) -> crate::error::Result<AdaptiveDeterminize> {
        if encode_weights {
            // Weight encoding is the last determinization strategy available;
            // there is nothing left to fall back to but the input itself, so
            // the budget has to bound it too. Folding the weight into the label
            // separates paths that label-only determinization would have
            // merged, so this strategy can produce MORE states than that one,
            // never fewer.
            let encode_mapper =
                encode(t, EncodeType::EncodeWeightsAndLabels).map_err(openfst_error("encode"))?;
            return match algorithms::determinize_bounded(&*t, budget_config(budget)) {
                Ok(determinized) => {
                    *det = determinized;
                    Ok(AdaptiveDeterminize::Determinized(encode_mapper))
                }
                Err(err) => {
                    tracing::info!(
                        caller,
                        %err,
                        "weight-encoded determinization exceeded its budget"
                    );
                    decode(t, encode_mapper).map_err(openfst_error("decode"))?;
                    Ok(AdaptiveDeterminize::SubsetLimit)
                }
            };
        }

        // Label-only path: bound the determinization and fall back to weight
        // encoding if the budget is exceeded. Encode `t` IN PLACE and
        // determinize from it directly rather than from a clone: on the common
        // within-budget path that keeps only one copy of the ~input-sized
        // machine live alongside the growing output (the profiled lang-sma
        // compile determinizes ~1M-state machines here, and the extra clone
        // was a full second copy at peak). On the rare budget overrun the
        // machine is Decoded back to its original labels/weights before the
        // weighted retry, so that path's result is unchanged.
        let label_mapper = encode(t, EncodeType::EncodeLabels).map_err(openfst_error("encode"))?;
        match algorithms::determinize_bounded(&*t, budget_config(budget)) {
            Ok(determinized) => {
                *det = determinized;
                Ok(AdaptiveDeterminize::Determinized(label_mapper))
            }
            // A subset overrun is about the search, a transition overrun about
            // the result. Weight encoding only ever splits paths further, so it
            // cannot shrink a result that is already too large: retrying it
            // would burn the budget again to reach the same verdict.
            Err(
                err @ (algorithms::DeterminizeBoundedError::SubsetElements { .. }
                | algorithms::DeterminizeBoundedError::Transitions { .. }),
            ) if preserve_on_subset_limit => {
                // The relation survives, but the caller falls back to a weaker
                // representation; report which axis tripped and by how much,
                // or the fallback is undiagnosable from a build log.
                tracing::info!(
                    caller,
                    %err,
                    "label-only determinization exceeded its budget; skipping the weight-encoded retry"
                );
                decode(t, label_mapper).map_err(openfst_error("decode"))?;
                Ok(AdaptiveDeterminize::SubsetLimit)
            }
            Err(algorithms::DeterminizeBoundedError::Transitions { limit, attempted }) => {
                tracing::info!(
                    caller,
                    limit,
                    attempted,
                    "label-only determinization exceeded its transition budget"
                );
                decode(t, label_mapper).map_err(openfst_error("decode"))?;
                Ok(AdaptiveDeterminize::SubsetLimit)
            }
            Err(_) => {
                tracing::info!(
                    caller,
                    ?budget,
                    "label-only determinization exceeded resource budget; \
                     retrying with weight encoding (hfst/hfst#435)"
                );
                decode(t, label_mapper).map_err(openfst_error("decode"))?;
                Self::determinize_adaptive(t, true, budget, caller, det, preserve_on_subset_limit)
            }
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
    // [spec:hfst:req:determinize-envelope.relation-preserved]
    pub fn determinize(
        mut t: StdVectorFst,
        encode_weights: bool,
    ) -> crate::error::Result<StdVectorFst> {
        check_epsilon_cycles(&t, "determinize");

        rm_epsilon(&mut t).map_err(openfst_error("rm_epsilon"))?;

        let w = TropicalWeightTransducer::get_smallest_weight(&t);
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut t, -w);
        }

        let budget = Self::determinize_budget(&t);
        let mut det = StdVectorFst::new();
        let outcome = Self::determinize_adaptive(
            &mut t,
            encode_weights,
            budget,
            "determinize",
            &mut det,
            false,
        )?;
        match outcome {
            AdaptiveDeterminize::Determinized(encode_mapper) => {
                decode(&mut det, encode_mapper).map_err(openfst_error("decode"))?;
            }
            // Every strategy is budget-bounded, so an input whose subset
            // construction runs away has no determinized form to return within
            // the envelope. Callers consume the relation, and the input already
            // denotes it exactly; handing it back undeterminized is a weaker
            // result than promised but a correct one, where the alternative is
            // exhausting memory.
            AdaptiveDeterminize::SubsetLimit => {
                tracing::warn!(
                    "determinization budget exceeded in every strategy; \
                     preserving the exact input relation undeterminized"
                );
                det = std::mem::replace(&mut t, StdVectorFst::new());
            }
        }

        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut det, w);
        }

        Ok(det)
    }
}

fn budget_config(budget: DeterminizeBudget) -> algorithms::DeterminizeBudgets {
    algorithms::DeterminizeBudgets {
        max_states: Some(budget.states),
        max_subset_elements: Some(budget.subset_elements),
        max_trs: Some(budget.trs),
    }
}

// [spec:hfst:req:embed.no-panic]
/// Fail when `t` is acyclic with a path longer than
/// [`crate::depth::MAX_PATH_LENGTH`]: rustfst minimizes an acyclic machine by
/// recursing along its paths from the start, which would overflow the stack.
/// A path visits a state once, so a machine of no more states passes
/// unread.
fn refuse_long_paths(t: &StdVectorFst) -> crate::error::Result<()> {
    if t.num_states() <= crate::depth::MAX_PATH_LENGTH {
        return Ok(());
    }
    match longest_path(t) {
        Some(arcs) if arcs > crate::depth::MAX_PATH_LENGTH => crate::bail!(
            Hfst,
            format!(
                "cannot minimize a transducer with a path more than {} arcs long",
                crate::depth::MAX_PATH_LENGTH
            )
        ),
        Some(_) | None => Ok(()),
    }
}

/// The most arcs of a path from the start of `t`, or `None` when a cycle is
/// reachable from it, read without recursing.
fn longest_path(t: &StdVectorFst) -> Option<usize> {
    let start = t.start()?;
    let targets = |s: StateId| -> Vec<StateId> {
        t.get_trs(s)
            .map(|trs| trs.trs().iter().map(|tr| tr.nextstate).collect())
            .unwrap_or_default()
    };
    let n = t.num_states();
    let mut reached = vec![false; n];
    let mut incoming = vec![0usize; n];
    let mut stack = vec![start];
    reached[start as usize] = true;
    while let Some(s) = stack.pop() {
        for next in targets(s) {
            incoming[next as usize] += 1;
            if !reached[next as usize] {
                reached[next as usize] = true;
                stack.push(next);
            }
        }
    }
    // Whatever reaches the start lies on a cycle through it.
    if incoming[start as usize] > 0 {
        return None;
    }
    let mut arcs = vec![0usize; n];
    let mut ready = vec![start];
    let mut done = 0;
    while let Some(s) = ready.pop() {
        done += 1;
        for next in targets(s) {
            arcs[next as usize] = arcs[next as usize].max(arcs[s as usize] + 1);
            incoming[next as usize] -= 1;
            if incoming[next as usize] == 0 {
                ready.push(next);
            }
        }
    }
    let reachable = reached.iter().filter(|&&r| r).count();
    (done == reachable).then(|| arcs.into_iter().max().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine whose start leads along `arcs` arcs in a line, with a
    /// state to the side of each, so it has more states than arcs on its
    /// longest path; `cycle` closes the line back to the start.
    fn line(arcs: usize, cycle: bool) -> StdVectorFst {
        let mut fst = StdVectorFst::new();
        let start = fst.add_state();
        fst.set_start(start).expect("a state");
        let mut last = start;
        for _ in 0..arcs {
            let next = fst.add_state();
            let side = fst.add_state();
            for target in [next, side] {
                fst.add_tr(
                    last,
                    StdTransition::new(1, 1, TropicalWeight::one(), target),
                )
                .expect("a state");
            }
            last = next;
        }
        if cycle {
            fst.add_tr(last, StdTransition::new(1, 1, TropicalWeight::one(), start))
                .expect("a state");
        }
        fst.set_final(last, TropicalWeight::one()).expect("a state");
        fst
    }

    // [spec:hfst:req:embed.no-panic/test]
    #[test]
    fn a_path_past_the_limit_is_refused() {
        let limit = crate::depth::MAX_PATH_LENGTH;
        assert_eq!(longest_path(&line(limit, false)), Some(limit));
        assert!(refuse_long_paths(&line(limit, false)).is_ok());
        let refused = refuse_long_paths(&line(limit + 1, false)).expect_err("too long");
        assert!(refused.to_string().contains("path more than"), "{refused}");
    }

    // [spec:hfst:req:embed.no-panic/test]
    #[test]
    fn cyclic_machines_go_to_the_cyclic_minimizer() {
        let limit = crate::depth::MAX_PATH_LENGTH;
        assert_eq!(longest_path(&line(limit + 1, true)), None);
        assert!(refuse_long_paths(&line(limit + 1, true)).is_ok());
    }
}
