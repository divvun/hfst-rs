//! Owned tropical subtraction with virtual flag-diacritic loops.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::hfst_transducer::FlagDiacriticOverlay;

impl TropicalWeightTransducer {
    /// Subtracts owned operands while exposing missing flag loops lazily.
    // [spec:hfst:req:virtual-flag-algebra.subtraction]
    pub fn try_subtract_owned(
        mut t1: StdVectorFst,
        mut t2: StdVectorFst,
        flag_overlay: Option<&FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<StdVectorFst> {
        if flag_overlay.is_none() && memory_limit_bytes.is_none() {
            return Ok(Self::subtract(&t1, &t2));
        }

        if t1.output_symbols().is_none()
            && let Some(symbols) = t1.input_symbols().map(Arc::clone)
        {
            t1.set_output_symbols(symbols);
        }
        if t2.output_symbols().is_none()
            && let Some(symbols) = t2.input_symbols().map(Arc::clone)
        {
            t2.set_output_symbols(symbols);
        }

        super::operations::check_epsilon_cycles(&t1, "subtract");
        super::operations::check_epsilon_cycles(&t2, "subtract");

        let input_symbols = t1
            .input_symbols()
            .map(Arc::clone)
            .ok_or_else(|| crate::err!(MissingOpenFstInputSymbolTable))?;
        let output_symbols = t1.output_symbols().map(Arc::clone);

        algorithms::RmEpsilon(&mut t1);
        algorithms::RmEpsilon(&mut t2);

        for state in 0..t2.num_states() as StateId {
            let transition_count = t2.get_trs(state).expect("state comes from this FST").len();
            let mut transitions = t2.tr_iter_mut(state).expect("state comes from this FST");
            for position in 0..transition_count {
                transitions
                    .set_weight(position, TropicalWeight::one())
                    .expect("position comes from this transition vector");
            }
            if t2.is_final(state).expect("state comes from this FST") {
                t2.set_final(state, TropicalWeight::one())
                    .expect("state comes from this FST");
            }
        }

        let ordering_epsilon_inputs =
            if flag_overlay.is_some_and(|overlay| overlay.enforce_left_before_right) {
                t1.states_iter()
                    .flat_map(|state| {
                        t1.get_trs(state)
                            .expect("state comes from this FST")
                            .trs()
                            .iter()
                            .filter(|transition| transition.olabel == EPS_LABEL)
                            .map(|transition| transition.ilabel)
                            .collect::<Vec<_>>()
                    })
                    .collect()
            } else {
                BTreeSet::new()
            };

        let encoder = algorithms::Encode(&mut t1, algorithms::EncodeType::EncodeLabels);
        let encoder = algorithms::EncodeInto(&mut t2, encoder);
        let (encoder, overlay) = super::intersect::encode_overlay(
            encoder,
            flag_overlay,
            &input_symbols,
            &ordering_epsilon_inputs,
        )?;

        // Right-side virtual loops commute with determinization. Excluding
        // those labels from complement completion prevents them from being
        // sent to the accepting sink; the lazy product supplies a self-loop
        // at every complement state instead, which is complement(B + loops).
        let mut sigma = algorithms::Labels(&t1);
        sigma.append(&mut algorithms::Labels(&t2));
        // A left-side virtual loop is an actual transition in the eager
        // minuend, so Difference includes its label in the complement
        // alphabet even when the corresponding right-side flag exists only
        // in the subtrahend's alphabet. Without this, the complement has no
        // sink transition for that virtual left event and subtraction drops
        // paths that the eager construction retains.
        sigma.extend(overlay.left_self_loops().iter().copied());
        for label in overlay.right_self_loops() {
            sigma.remove(label);
        }
        let complement = algorithms::ComplementAcceptor(&t2, &sigma);

        let memory_plan =
            hfst_openfst::compose_storage::ComposeMemoryPlan::from_allowance(memory_limit_bytes);
        let scratch_dir = match memory_plan {
            hfst_openfst::compose_storage::ComposeMemoryPlan::Unbounded => {
                std::path::PathBuf::new()
            }
            hfst_openfst::compose_storage::ComposeMemoryPlan::Bounded { .. } => {
                std::env::current_dir().map_err(|error| {
                    crate::err!(
                        Hfst,
                        format!("cannot resolve subtraction scratch directory: {error}")
                    )
                })?
            }
        };
        let mut result = super::compose::try_flag_overlay_product_owned(
            t1,
            complement,
            overlay,
            memory_plan,
            scratch_dir,
            "subtract",
            super::compose::ProductPruning::Sequence,
        )?;
        algorithms::Decode(&mut result, encoder);
        result.set_input_symbols(input_symbols);
        if let Some(symbols) = output_symbols {
            result.set_output_symbols(symbols);
        } else {
            result.take_output_symbols();
        }
        Ok(result)
    }
}
