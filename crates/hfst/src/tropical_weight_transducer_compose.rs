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
    pub fn try_compose_owned(
        t1: StdVectorFst,
        t2: StdVectorFst,
        flag_overlay: Option<&crate::hfst_transducer::FlagDiacriticComposeOverlay>,
        memory_limit_bytes: Option<u64>,
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
        Self::try_compose_owned_with_memory_plan(t1, t2, flag_overlay, memory_plan, scratch_dir)
    }

    pub(crate) fn try_compose_owned_with_memory_plan(
        mut t1: StdVectorFst,
        mut t2: StdVectorFst,
        flag_overlay: Option<&crate::hfst_transducer::FlagDiacriticComposeOverlay>,
        memory_plan: hfst_openfst::compose_storage::ComposeMemoryPlan,
        scratch_dir: std::path::PathBuf,
    ) -> crate::error::Result<StdVectorFst> {
        let input_symbols = t1
            .input_symbols()
            .map(std::sync::Arc::clone)
            .ok_or_else(|| crate::err!(MissingOpenFstInputSymbolTable))?;

        // Match the OpenFst checksum/coding setup of the borrowed path,
        // but mutate the now-owned operands instead of cloning them.
        t1.set_output_symbols(std::sync::Arc::clone(&input_symbols));
        t2.set_input_symbols(std::sync::Arc::clone(&input_symbols));
        algorithms::ArcSortOutput(&mut t1);
        algorithms::ArcSortInput(&mut t2);

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
                hfst_openfst::flag_overlay_compose::FlagOverlay::new(
                    labels(&overlay.left_self_loops)?,
                    labels(&overlay.right_self_loops)?,
                    overlay.enforce_left_before_right,
                )
                .map_err(|error| {
                    crate::err!(Hfst, format!("invalid flag composition overlay: {error}"))
                })?
            }
            None => hfst_openfst::flag_overlay_compose::FlagOverlay::default(),
        };

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
        let lazy = hfst_openfst::flag_overlay_compose::compose_flag_overlay_lazy_with_store(
            std::sync::Arc::new(t1),
            std::sync::Arc::new(t2),
            overlay,
            pair_store,
        )
        .map_err(|error| crate::err!(Hfst, format!("OpenFst compose setup: {error}")))?;
        let mut artifact = hfst_openfst::compose_storage::materialize_fst(lazy.as_fst(), &storage)
            .map_err(|error| {
                crate::err!(Hfst, format!("OpenFst compose materialization: {error}"))
            })?;

        // A spilled artifact is intentionally trimmed and loaded only
        // after the lazy FST releases both potentially huge operands and
        // its pair-state store.
        drop(lazy);
        let externally_trimmed = artifact.prepare_for_reload().map_err(|error| {
            crate::err!(Hfst, format!("OpenFst compose external trim: {error}"))
        })?;
        let mut result = artifact.into_vector_fst().map_err(|error| {
            crate::err!(Hfst, format!("OpenFst compose result reload: {error}"))
        })?;
        if !externally_trimmed {
            hfst_openfst::rustfst::algorithms::connect(&mut result)
                .map_err(|error| crate::err!(Hfst, format!("OpenFst compose connect: {error}")))?;
        }
        result.set_input_symbols(input_symbols);
        Ok(result)
    }
}
