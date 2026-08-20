//! Owned tropical composition with optional bounded spill storage.

use super::*;

impl TropicalWeightTransducer {
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-fn]
    pub fn compose(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
        Self::try_compose_owned(t1.clone(), t2.clone(), None, None)
            .expect("OpenFst composition of valid tropical transducers")
    }

    /// Consuming, fallible composition used by the bounded/spilling
    /// backend path. Both operand graphs are sorted in place and then held
    /// by the lazy FST, so there is no full operand clone at peak memory.
    // [spec:hfst:req:virtual-flag-algebra.backend-core]
    pub fn try_compose_owned(
        t1: StdVectorFst,
        t2: StdVectorFst,
        flag_overlay: Option<&crate::hfst_transducer::FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<StdVectorFst> {
        Self::try_compose_mode(t1, t2, flag_overlay, memory_limit_bytes, false)
    }

    /// Composition for product-heavy callers such as one-rule
    /// compose-intersect. Label reachability rejects pairs that cannot lead to
    /// another match before they are interned or materialized.
    pub fn try_compose_lookahead_owned(
        t1: StdVectorFst,
        t2: StdVectorFst,
        flag_overlay: Option<&crate::hfst_transducer::FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<StdVectorFst> {
        Self::try_compose_mode_with_pruning(
            t1,
            t2,
            flag_overlay,
            memory_limit_bytes,
            false,
            ProductPruning::LabelLookAhead,
        )
    }

    pub(crate) fn try_compose_mode(
        t1: StdVectorFst,
        t2: StdVectorFst,
        flag_overlay: Option<&crate::hfst_transducer::FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
        flags_as_epsilon: bool,
    ) -> crate::error::Result<StdVectorFst> {
        Self::try_compose_mode_with_pruning(
            t1,
            t2,
            flag_overlay,
            memory_limit_bytes,
            flags_as_epsilon,
            ProductPruning::Sequence,
        )
    }

    fn try_compose_mode_with_pruning(
        t1: StdVectorFst,
        t2: StdVectorFst,
        flag_overlay: Option<&crate::hfst_transducer::FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
        flags_as_epsilon: bool,
        pruning: ProductPruning,
    ) -> crate::error::Result<StdVectorFst> {
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
                        format!("cannot resolve compose scratch directory: {error}")
                    )
                })?
            }
        };
        Self::try_compose_owned_with_memory_plan(
            t1,
            t2,
            flag_overlay,
            memory_plan,
            scratch_dir,
            flags_as_epsilon,
            pruning,
        )
    }

    pub(crate) fn try_compose_owned_with_memory_plan(
        mut t1: StdVectorFst,
        mut t2: StdVectorFst,
        flag_overlay: Option<&crate::hfst_transducer::FlagDiacriticOverlay>,
        memory_plan: hfst_openfst::compose_storage::ComposeMemoryPlan,
        scratch_dir: std::path::PathBuf,
        flags_as_epsilon: bool,
        pruning: ProductPruning,
    ) -> crate::error::Result<StdVectorFst> {
        let input_symbols = t1
            .input_symbols()
            .map(std::sync::Arc::clone)
            .ok_or_else(|| crate::err!(MissingOpenFstInputSymbolTable))?;

        // Match the OpenFst checksum/coding setup of the borrowed path,
        // but mutate the now-owned operands instead of cloning them.
        t1.set_output_symbols(std::sync::Arc::clone(&input_symbols));
        t2.set_input_symbols(std::sync::Arc::clone(&input_symbols));

        let overlay = match flag_overlay {
            Some(overlay) => {
                let labels = |symbols: &StringSet| -> crate::error::Result<Vec<u32>> {
                    symbols
                        .iter()
                        .map(|symbol| {
                            input_symbols.get_label(symbol.as_str()).ok_or_else(|| {
                                crate::err!(
                                    SymbolNotFound,
                                    format!(
                                        "flag overlay symbol '{}' is absent from the canonical symbol table",
                                        symbol
                                    )
                                )
                            })
                        })
                        .collect()
                };
                let overlay = hfst_openfst::flag_overlay_compose::FlagOverlay::new(
                    labels(&overlay.left_self_loops)?,
                    labels(&overlay.right_self_loops)?,
                    overlay.enforce_left_before_right,
                )
                .map_err(|error| {
                    crate::err!(Hfst, format!("invalid flag composition overlay: {error}"))
                })?;
                if flags_as_epsilon {
                    overlay.with_flags_as_epsilon()
                } else {
                    overlay
                }
            }
            None => hfst_openfst::flag_overlay_compose::FlagOverlay::default(),
        };

        let mut result = try_flag_overlay_product_owned(
            t1,
            t2,
            overlay,
            memory_plan,
            scratch_dir,
            "compose",
            pruning,
        )?;
        result.set_input_symbols(input_symbols);
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductPruning {
    Sequence,
    LabelLookAhead,
}

pub(super) fn try_flag_overlay_product_owned(
    mut t1: StdVectorFst,
    mut t2: StdVectorFst,
    overlay: hfst_openfst::flag_overlay_compose::FlagOverlay,
    memory_plan: hfst_openfst::compose_storage::ComposeMemoryPlan,
    scratch_dir: std::path::PathBuf,
    operation: &str,
    pruning: ProductPruning,
) -> crate::error::Result<StdVectorFst> {
    algorithms::ArcSortOutput(&mut t1);
    algorithms::ArcSortInput(&mut t2);

    let (pair_store, storage) = match memory_plan {
        hfst_openfst::compose_storage::ComposeMemoryPlan::Unbounded => (
            None,
            hfst_openfst::compose_storage::ComposeStorageConfig::unbounded(scratch_dir),
        ),
        hfst_openfst::compose_storage::ComposeMemoryPlan::Bounded {
            pair_interner_cap_bytes,
            materializer_cap_bytes,
            ..
        } => (
            Some(
                hfst_openfst::rustfst::algorithms::compose::ComposeStateStoreConfig::new(
                    pair_interner_cap_bytes,
                    scratch_dir.clone(),
                ),
            ),
            hfst_openfst::compose_storage::ComposeStorageConfig::bounded(
                materializer_cap_bytes,
                scratch_dir,
            ),
        ),
    };
    let t1 = std::sync::Arc::new(t1);
    let t2 = std::sync::Arc::new(t2);
    let mut artifact = if pruning == ProductPruning::LabelLookAhead {
        let lazy = hfst_openfst::flag_overlay_compose::compose_lookahead_with_store(
            t1, t2, overlay, pair_store,
        )
        .map_err(|error| {
            crate::err!(
                Hfst,
                format!("OpenFst {operation} lookahead setup: {error}")
            )
        })?;
        let artifact = hfst_openfst::compose_storage::materialize_fst(lazy.as_fst(), &storage)
            .map_err(|error| {
                crate::err!(
                    Hfst,
                    format!("OpenFst {operation} materialization: {error}")
                )
            })?;
        drop(lazy);
        artifact
    } else {
        let lazy = hfst_openfst::flag_overlay_compose::compose_flag_overlay_lazy_with_store(
            t1, t2, overlay, pair_store,
        )
        .map_err(|error| crate::err!(Hfst, format!("OpenFst {operation} setup: {error}")))?;
        let artifact = hfst_openfst::compose_storage::materialize_fst(lazy.as_fst(), &storage)
            .map_err(|error| {
                crate::err!(
                    Hfst,
                    format!("OpenFst {operation} materialization: {error}")
                )
            })?;
        drop(lazy);
        artifact
    };

    // A spilled artifact is intentionally trimmed and loaded only after the
    // lazy FST releases both operands and its pair-state store.
    let externally_trimmed = artifact.prepare_for_reload().map_err(|error| {
        crate::err!(Hfst, format!("OpenFst {operation} external trim: {error}"))
    })?;
    let mut result = artifact.into_vector_fst().map_err(|error| {
        crate::err!(Hfst, format!("OpenFst {operation} result reload: {error}"))
    })?;
    if !externally_trimmed {
        hfst_openfst::rustfst::algorithms::connect(&mut result)
            .map_err(|error| crate::err!(Hfst, format!("OpenFst {operation} connect: {error}")))?;
    }
    Ok(result)
}
