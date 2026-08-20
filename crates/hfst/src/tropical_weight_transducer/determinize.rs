//! Bounded weighted determinization and minimization fallbacks.

use super::operations::check_epsilon_cycles;
use super::*;

pub(super) enum AdaptiveDeterminize {
    Determinized(algorithms::EncodeTable<TropicalWeight>),
    SubsetLimit,
}

impl TropicalWeightTransducer {
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn]
    pub fn minimize(t: StdVectorFst, encode_weights: bool) -> StdVectorFst {
        Self::minimize_with_reverse_fallback(t, encode_weights, true, None)
    }

    pub(super) fn minimize_with_reverse_fallback(
        mut t: StdVectorFst,
        encode_weights: bool,
        allow_reverse_fallback: bool,
        budget_override: Option<(usize, usize)>,
    ) -> StdVectorFst {
        check_epsilon_cycles(&t, "minimize");

        // (USE_FOMA_EPSILON_REMOVAL && HAVE_FOMA) path is not configured here.
        algorithms::RmEpsilon(&mut t);

        let w = TropicalWeightTransducer::get_smallest_weight(&t);
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut t, -w);
        }

        let input_states = t.num_states();
        let budgets = budget_override.unwrap_or_else(|| Self::determinize_budget(input_states));
        let mut det = StdVectorFst::new();
        let outcome =
            Self::determinize_adaptive(&mut t, encode_weights, budgets, "minimize", &mut det, true);
        match outcome {
            AdaptiveDeterminize::Determinized(encode_mapper) => {
                algorithms::Minimize(&mut det);
                algorithms::Decode(&mut det, encode_mapper);
            }
            AdaptiveDeterminize::SubsetLimit if allow_reverse_fallback => {
                tracing::warn!(
                    "weighted-subset memory budget exceeded; minimizing in the reverse orientation"
                );
                let mut reversed = StdVectorFst::new();
                algorithms::Reverse(&t, &mut reversed);
                let reversed =
                    Self::minimize_with_reverse_fallback(reversed, encode_weights, false, None);
                algorithms::Reverse(&reversed, &mut det);
            }
            AdaptiveDeterminize::SubsetLimit => {
                tracing::warn!(
                    "weighted-subset memory budget exceeded in both orientations; preserving the exact input language without further minimization"
                );
                det = std::mem::replace(&mut t, StdVectorFst::new());
            }
        }

        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut det, w);
        }

        det
    }

    // Bounds both axes of label-only weighted determinization. A state cap
    // catches non-twins machines that split forever (hfst/hfst#435), while
    // the element cap catches a smaller number of enormous weighted subsets
    // before their vectors, normalization maps, and state-table keys consume
    // gigabytes. Four million working elements correspond to roughly 256 MiB
    // at the conservative 64-byte transient accounting used to select this
    // fixed safety envelope; the input and final FST are not included.
    fn determinize_budget(input_states: usize) -> (usize, usize) {
        const STATES_PER_INPUT: usize = 256;
        const MAX_STATES: usize = 2_000_000;
        const MAX_SUBSET_ELEMENTS: usize = 4 * 1024 * 1024;
        let states = STATES_PER_INPUT
            .saturating_mul(input_states)
            .clamp(1024, MAX_STATES);
        (states, MAX_SUBSET_ELEMENTS)
    }

    // Runs Encode + Determinize with the adaptive budget/fallback and writes
    // the determinized machine into `det`, returning the EncodeTable the
    // caller must Decode with. `t` is encoded in place; on the encoded-weight
    // fallback `t` is re-encoded with weights, so the returned table matches
    // whatever `det` was produced from.
    pub(super) fn determinize_adaptive(
        t: &mut StdVectorFst,
        encode_weights: bool,
        budgets: (usize, usize),
        caller: &str,
        det: &mut StdVectorFst,
        preserve_on_subset_limit: bool,
    ) -> AdaptiveDeterminize {
        if encode_weights {
            let encode_mapper =
                algorithms::Encode(t, algorithms::EncodeType::EncodeWeightsAndLabels);
            algorithms::Determinize(t, det);
            return AdaptiveDeterminize::Determinized(encode_mapper);
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
        let (state_budget, subset_element_budget) = budgets;
        let label_mapper = algorithms::Encode(t, algorithms::EncodeType::EncodeLabels);
        match algorithms::DeterminizeBounded(
            &*t,
            det,
            Some(state_budget),
            Some(subset_element_budget),
        ) {
            Ok(()) => AdaptiveDeterminize::Determinized(label_mapper),
            Err(algorithms::DeterminizeBoundedError::SubsetElements { limit, attempted })
                if preserve_on_subset_limit =>
            {
                tracing::info!(
                    caller,
                    limit,
                    attempted,
                    "weighted-subset memory budget exceeded"
                );
                algorithms::Decode(t, label_mapper);
                AdaptiveDeterminize::SubsetLimit
            }
            Err(_) => {
                tracing::info!(
                    caller,
                    state_budget,
                    subset_element_budget,
                    "label-only determinization exceeded resource budget; \
                     retrying with weight encoding (hfst/hfst#435)"
                );
                algorithms::Decode(t, label_mapper);
                let encode_mapper =
                    algorithms::Encode(t, algorithms::EncodeType::EncodeWeightsAndLabels);
                algorithms::Determinize(t, det);
                AdaptiveDeterminize::Determinized(encode_mapper)
            }
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
    pub fn determinize(mut t: StdVectorFst, encode_weights: bool) -> StdVectorFst {
        check_epsilon_cycles(&t, "determinize");

        algorithms::RmEpsilon(&mut t);

        let w = TropicalWeightTransducer::get_smallest_weight(&t);
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut t, -w);
        }

        let input_states = t.num_states();
        let budgets = Self::determinize_budget(input_states);
        let mut det = StdVectorFst::new();
        let outcome = Self::determinize_adaptive(
            &mut t,
            encode_weights,
            budgets,
            "determinize",
            &mut det,
            false,
        );
        let AdaptiveDeterminize::Determinized(encode_mapper) = outcome else {
            unreachable!("determinize never preserves a nondeterministic input")
        };
        algorithms::Decode(&mut det, encode_mapper);

        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut det, w);
        }

        det
    }
}
