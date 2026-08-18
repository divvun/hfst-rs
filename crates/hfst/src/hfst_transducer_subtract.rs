//! Facade subtraction paths that consume a virtual flag overlay.

use super::*;

impl<B: AlgebraBackend> HfstTransducer<B> {
    /// Subtract with missing flag-diacritic loops supplied virtually.
    // [spec:hfst:req:virtual-flag-algebra.subtraction]
    pub fn subtract_with_flag_overlay(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
        flag_overlay: Option<&FlagDiacriticOverlay>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if flag_overlay.is_some() && !B::SUPPORTS_VIRTUAL_FLAG_SUBTRACTION {
            crate::bail!(
                Hfst,
                "this backend does not support virtual flag subtraction"
            );
        }
        self.is_trie = false;
        let another = self.harmonize_for_binary_op(another, harmonize)?;
        let left = std::mem::replace(&mut self.fst, B::empty());
        self.fst = left.try_flag_operation_owned(
            another.fst,
            FlagDiacriticOperation::Subtract,
            flag_overlay,
            None,
        )?;
        Ok(self)
    }
}
