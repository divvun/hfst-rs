use super::*;

impl<B: AlgebraBackend> XreCompiler<B> {
    /// Evaluate a complete expression and ensure its returned graph is
    /// optimized exactly once. Several grammar actions already optimize their
    /// result as their final step; immediately repeating that operation at the
    /// compiler boundary is pure work and is particularly expensive for the
    /// large composition chains emitted by Giella build rules.
    pub(crate) fn eval_finalized(
        &mut self,
        expression: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let already_optimized = matches!(&expression.value, XreExpr::Group(_))
            || matches!(
                &expression.value,
                XreExpr::Weighted { expr, .. } if matches!(expr.value, XreExpr::Group(_))
            )
            || matches!(
                &expression.value,
                XreExpr::Binary(
                    BinaryOp::Compose
                        | BinaryOp::CrossProduct
                        | BinaryOp::LenientCompose
                        | BinaryOp::MergeRight
                        | BinaryOp::MergeLeft,
                    _,
                    _
                )
            );
        let mut transducer = self.eval(expression)?;
        if !already_optimized {
            transducer.optimize_with_config(&self.opt_cfg())?;
        }
        Ok(transducer)
    }
}
