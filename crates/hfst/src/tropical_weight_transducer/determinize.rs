//! Bounded weighted determinization and minimization fallbacks.

use super::operations::check_epsilon_cycles;
use super::*;

pub(super) enum AdaptiveDeterminize {
    Determinized(algorithms::EncodeTable<TropicalWeight>),
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
    pub fn minimize(t: StdVectorFst, encode_weights: bool) -> StdVectorFst {
        Self::minimize_with_reverse_fallback(t, encode_weights, true, None)
    }

    // [spec:hfst:req:determinize-envelope.relation-preserved]
    pub(super) fn minimize_with_reverse_fallback(
        mut t: StdVectorFst,
        encode_weights: bool,
        allow_reverse_fallback: bool,
        budget_override: Option<DeterminizeBudget>,
    ) -> StdVectorFst {
        check_epsilon_cycles(&t, "minimize");

        // (USE_FOMA_EPSILON_REMOVAL && HAVE_FOMA) path is not configured here.
        algorithms::RmEpsilon(&mut t);

        let w = TropicalWeightTransducer::get_smallest_weight(&t);
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut t, -w);
        }

        let budget = budget_override.unwrap_or_else(|| Self::determinize_budget(&t));
        let mut det = StdVectorFst::new();
        let outcome =
            Self::determinize_adaptive(&mut t, encode_weights, budget, "minimize", &mut det, true);
        match outcome {
            AdaptiveDeterminize::Determinized(encode_mapper) => {
                algorithms::Minimize(&mut det);
                algorithms::Decode(&mut det, encode_mapper);
            }
            AdaptiveDeterminize::SubsetLimit if allow_reverse_fallback => {
                tracing::warn!(
                    "determinization budget exceeded; minimizing in the reverse orientation"
                );
                let mut reversed = StdVectorFst::new();
                algorithms::Reverse(&t, &mut reversed);
                let reversed =
                    Self::minimize_with_reverse_fallback(reversed, encode_weights, false, None);
                algorithms::Reverse(&reversed, &mut det);
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

        det
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
    // [spec:hfst:req:determinize-envelope.transition-axis]
    fn determinize_budget(input: &StdVectorFst) -> DeterminizeBudget {
        const STATES_PER_INPUT: usize = 256;
        const MAX_STATES: usize = 2_000_000;
        const MAX_SUBSET_ELEMENTS: usize = 4 * 1024 * 1024;
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
                .clamp(1024, MAX_STATES),
            subset_elements: MAX_SUBSET_ELEMENTS,
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
    ) -> AdaptiveDeterminize {
        if encode_weights {
            // Weight encoding is the last determinization strategy available;
            // there is nothing left to fall back to but the input itself, so
            // the budget has to bound it too. Folding the weight into the label
            // separates paths that label-only determinization would have
            // merged, so this strategy can produce MORE states than that one,
            // never fewer.
            let encode_mapper =
                algorithms::Encode(t, algorithms::EncodeType::EncodeWeightsAndLabels);
            return match algorithms::DeterminizeBounded(&*t, det, budget_config(budget)) {
                Ok(()) => AdaptiveDeterminize::Determinized(encode_mapper),
                Err(err) => {
                    tracing::info!(
                        caller,
                        %err,
                        "weight-encoded determinization exceeded its budget"
                    );
                    algorithms::Decode(t, encode_mapper);
                    AdaptiveDeterminize::SubsetLimit
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
        let label_mapper = algorithms::Encode(t, algorithms::EncodeType::EncodeLabels);
        match algorithms::DeterminizeBounded(&*t, det, budget_config(budget)) {
            Ok(()) => AdaptiveDeterminize::Determinized(label_mapper),
            // A subset overrun is about the search, a transition overrun about
            // the result. Weight encoding only ever splits paths further, so it
            // cannot shrink a result that is already too large: retrying it
            // would burn the budget again to reach the same verdict.
            Err(
                algorithms::DeterminizeBoundedError::SubsetElements { .. }
                | algorithms::DeterminizeBoundedError::Transitions { .. },
            ) if preserve_on_subset_limit => {
                algorithms::Decode(t, label_mapper);
                AdaptiveDeterminize::SubsetLimit
            }
            Err(algorithms::DeterminizeBoundedError::Transitions { limit, attempted }) => {
                tracing::info!(
                    caller,
                    limit,
                    attempted,
                    "label-only determinization exceeded its transition budget"
                );
                algorithms::Decode(t, label_mapper);
                AdaptiveDeterminize::SubsetLimit
            }
            Err(_) => {
                tracing::info!(
                    caller,
                    ?budget,
                    "label-only determinization exceeded resource budget; \
                     retrying with weight encoding (hfst/hfst#435)"
                );
                algorithms::Decode(t, label_mapper);
                Self::determinize_adaptive(t, true, budget, caller, det, preserve_on_subset_limit)
            }
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
    // [spec:hfst:req:determinize-envelope.relation-preserved]
    pub fn determinize(mut t: StdVectorFst, encode_weights: bool) -> StdVectorFst {
        check_epsilon_cycles(&t, "determinize");

        algorithms::RmEpsilon(&mut t);

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
        );
        match outcome {
            AdaptiveDeterminize::Determinized(encode_mapper) => {
                algorithms::Decode(&mut det, encode_mapper);
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

        det
    }
}

fn budget_config(budget: DeterminizeBudget) -> algorithms::DeterminizeBudgets {
    algorithms::DeterminizeBudgets {
        max_states: Some(budget.states),
        max_subset_elements: Some(budget.subset_elements),
        max_trs: Some(budget.trs),
    }
}
