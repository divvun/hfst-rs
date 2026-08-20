//! Facade intersection paths that consume a virtual flag overlay.

use super::*;

impl<B: AlgebraBackend> HfstTransducer<B> {
    /// Intersect with missing flag-diacritic loops supplied virtually.
    // [spec:hfst:req:virtual-flag-algebra.intersection]
    pub fn intersect_with_flag_overlay(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
        flag_overlay: Option<&FlagDiacriticOverlay>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if flag_overlay.is_some() && !B::SUPPORTS_VIRTUAL_FLAG_INTERSECTION {
            crate::bail!(
                Hfst,
                "this backend does not support virtual flag intersection"
            );
        }
        self.is_trie = false;
        let another = self.harmonize_for_binary_op(another, harmonize)?;
        let left = std::mem::replace(&mut self.fst, B::empty());
        self.fst = left.try_flag_operation_owned(
            another.fst,
            FlagDiacriticOperation::Intersect,
            flag_overlay,
            None,
        )?;
        Ok(self)
    }
}
