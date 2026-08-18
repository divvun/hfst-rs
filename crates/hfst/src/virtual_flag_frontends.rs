//! Shared virtual flag preparation for composition frontends.

use crate::backend::AlgebraBackend;
use crate::hfst_transducer::{EngineConfig, FlagDiacriticOverlay, HfstTransducer};

// [spec:hfst:req:virtual-flag-algebra.frontend-compose]
pub(crate) fn prepare_compose_flag_overlay<B: AlgebraBackend>(
    left: &mut HfstTransducer<B>,
    right: &mut HfstTransducer<B>,
    harmonize_flags: bool,
    _config: &EngineConfig,
) -> crate::error::Result<Option<FlagDiacriticOverlay>> {
    if !harmonize_flags || !(left.has_flag_diacritics() || right.has_flag_diacritics()) {
        return Ok(None);
    }
    if B::SUPPORTS_VIRTUAL_FLAG_COMPOSE {
        return left.prepare_flag_diacritics_for_operation(right).map(Some);
    }
    left.harmonize_flag_diacritics(right, true)?;
    Ok(None)
}
