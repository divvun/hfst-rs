use super::*;

impl<B: AlgebraBackend> XreCompiler<B> {
    /// Evaluate a complete expression and apply the root optimization the
    /// grammar's `REGEXP2` reduction stands for.
    ///
    /// Every `REGEXP2` production in `xre_parse.yy` ends its action with
    /// `optimize()`, and a complete expression always reduces through
    /// `REGEXP2`. A handful of those productions ARE operators — composition,
    /// cross product, lenient composition, and the two merges — so their own
    /// action is that reduction's optimization and repeating it here is pure
    /// work, which is what makes the long Giella composition chains expensive.
    /// Every other root still owes the reduction its optimization.
    ///
    /// A bracketed group is NOT one of those productions: `[ ... ]` is
    /// `REGEXP11`, so the optimization inside [`XreExpr::Group`] is the
    /// bracket's own and leaves the `REGEXP2` one still owed. The distinction
    /// is observable rather than bookkeeping, because optimization is not
    /// idempotent under `encode_weights`: `Minimize` sees the weight-encoded
    /// machine as a weighted acceptor and pushes weights toward the initial
    /// state before refining its partition, so it merges the classes induced by
    /// that intermediate weight distribution rather than the machine's own, and
    /// a second pass finds merges the first could not see.
    // [spec:hfst:req:xre-finalization.root-optimize]
    pub(crate) fn eval_finalized(
        &mut self,
        expression: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let already_optimized = matches!(
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
