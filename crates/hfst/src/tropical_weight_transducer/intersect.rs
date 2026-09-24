//! Owned tropical intersection with virtual flag-diacritic loops.

use std::collections::BTreeSet;
use std::sync::Arc;

use hfst_openfst::rustfst::algorithms::compose::compose;

use super::*;
use crate::hfst_transducer::FlagDiacriticOverlay;

impl TropicalWeightTransducer {
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.intersect-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.intersect-fn]
    pub fn intersect(t1: &StdVectorFst, t2: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
        let owned_t1 = t1.clone();
        let owned_t2 = t2.clone();
        Self::intersect_owned(owned_t1, owned_t2)
    }

    /// Intersects owned operands while exposing missing flag loops lazily.
    // [spec:hfst:req:virtual-flag-algebra.intersection]
    pub fn try_intersect_owned(
        mut t1: StdVectorFst,
        mut t2: StdVectorFst,
        flag_overlay: Option<&FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<StdVectorFst> {
        if flag_overlay.is_none() && memory_limit_bytes.is_none() {
            return Self::intersect_owned(t1, t2);
        }
        super::operations::check_epsilon_cycles(&t1, "intersect");
        super::operations::check_epsilon_cycles(&t2, "intersect");

        let input_symbols = t1
            .input_symbols()
            .map(Arc::clone)
            .ok_or_else(|| crate::err!(MissingOpenFstInputSymbolTable))?;
        let output_symbols = t1.output_symbols().map(Arc::clone);

        rm_epsilon(&mut t1).map_err(openfst_error("rm_epsilon"))?;
        rm_epsilon(&mut t2).map_err(openfst_error("rm_epsilon"))?;

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

        // Intersection is OpenFST composition after each input/output pair is
        // encoded as one acceptor label. Weights deliberately remain outside
        // the encoding so matching paths retain normal tropical multiplication.
        let encoder = encode(&mut t1, EncodeType::EncodeLabels).map_err(openfst_error("encode"))?;
        let encoder = encode_into(&mut t2, encoder).map_err(openfst_error("encode"))?;
        let (encoder, overlay) = encode_overlay(
            encoder,
            flag_overlay,
            &input_symbols,
            &ordering_epsilon_inputs,
        )?;

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
                        format!("cannot resolve intersection scratch directory: {error}")
                    )
                })?
            }
        };
        let mut result = super::compose::try_flag_overlay_product_owned(
            t1,
            t2,
            overlay,
            memory_plan,
            scratch_dir,
            "intersect",
            super::compose::ProductPruning::Sequence,
        )?;
        decode(&mut result, encoder).map_err(openfst_error("decode"))?;
        result.set_input_symbols(input_symbols);
        if let Some(symbols) = output_symbols {
            result.set_output_symbols(symbols);
        } else {
            result.take_output_symbols();
        }
        Ok(result)
    }

    fn intersect_owned(
        mut t1: StdVectorFst,
        mut t2: StdVectorFst,
    ) -> crate::error::Result<StdVectorFst> {
        super::operations::check_epsilon_cycles(&t1, "intersect");
        super::operations::check_epsilon_cycles(&t2, "intersect");
        rm_epsilon(&mut t1).map_err(openfst_error("rm_epsilon"))?;
        rm_epsilon(&mut t2).map_err(openfst_error("rm_epsilon"))?;
        tr_sort(&mut t1, OLabelCompare {});
        tr_sort(&mut t2, ILabelCompare {});

        // Weights deliberately remain outside the shared label encoder: paths
        // with the same pair labels but different weights must still match.
        let encoder = encode(&mut t1, EncodeType::EncodeLabels).map_err(openfst_error("encode"))?;
        let encoder = encode_into(&mut t2, encoder).map_err(openfst_error("encode"))?;
        tr_sort(&mut t1, OLabelCompare {});
        tr_sort(&mut t2, ILabelCompare {});

        // Intersection of the encoded acceptors is their composition.
        let mut result: StdVectorFst = compose(&t1, &t2).map_err(openfst_error("intersect"))?;
        carry_symbol_tables(&t1, &mut result);
        decode(&mut result, encoder).map_err(openfst_error("decode"))?;
        Ok(result)
    }
}

pub(super) fn encode_overlay(
    mut encoder: EncodeTable<TropicalWeight>,
    overlay: Option<&FlagDiacriticOverlay>,
    symbols: &SymbolTable,
    ordering_epsilon_inputs: &BTreeSet<Label>,
) -> crate::error::Result<(
    EncodeTable<TropicalWeight>,
    hfst_openfst::flag_overlay_compose::FlagOverlay,
)> {
    let Some(overlay) = overlay else {
        return Ok((
            encoder,
            hfst_openfst::flag_overlay_compose::FlagOverlay::default(),
        ));
    };

    let mut labels = StdVectorFst::new();
    let state = labels.add_state();
    labels.set_start(state).expect("fresh state is valid");
    let left_count = overlay.left_self_loops.len();
    let right_count = overlay.right_self_loops.len();
    for symbol in overlay
        .left_self_loops
        .iter()
        .chain(overlay.right_self_loops.iter())
    {
        let label = symbols.get_label(symbol.as_str()).ok_or_else(|| {
            crate::err!(
                SymbolNotFound,
                format!("flag overlay symbol '{symbol}' is absent from the canonical symbol table")
            )
        })?;
        labels
            .add_tr(
                state,
                StdTransition::new(label, label, TropicalWeight::one(), state),
            )
            .expect("fresh state is valid");
    }
    for input in ordering_epsilon_inputs {
        labels
            .add_tr(
                state,
                StdTransition::new(*input, EPS_LABEL, TropicalWeight::one(), state),
            )
            .expect("fresh state is valid");
    }
    encoder = encode_into(&mut labels, encoder).map_err(openfst_error("encode"))?;

    let encoded_transitions = labels.get_trs(state).expect("fresh state is valid");
    let encoded = encoded_transitions.trs();
    let left = encoded[..left_count]
        .iter()
        .map(|transition| transition.ilabel)
        .collect();
    let right = encoded[left_count..left_count + right_count]
        .iter()
        .map(|transition| transition.ilabel)
        .collect();
    let ordering_epsilon = encoded[left_count + right_count..]
        .iter()
        .map(|transition| transition.ilabel)
        .collect();
    let overlay = hfst_openfst::flag_overlay_compose::FlagOverlay::new(
        left,
        right,
        overlay.enforce_left_before_right,
    )
    .and_then(|overlay| overlay.with_ordering_epsilon_labels(ordering_epsilon))
    .map_err(|error| crate::err!(Hfst, format!("invalid flag intersection overlay: {error}")))?;
    Ok((encoder, overlay))
}
