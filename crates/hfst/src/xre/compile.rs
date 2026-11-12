//! The compile driver: guard the input, parse it, and evaluate the root
//! expression.

use nfst_xre::{parse, parse_all};

use super::*;

impl<B: AlgebraBackend> XreCompiler<B> {
    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-fn]
    // Returns the compiled transducer, or None on parse failure / comments-only
    // (the C++ 'HfstTransducer*' null contract expressed as an Option).
    // Comments-only also flips the contains_only_comments flag, matching the
    // 'XRE: (empty) { contains_only_comments = true; }' action.
    pub fn compile(&mut self, src: &str) -> Option<HfstTransducer<B>> {
        // Retain the source so diagnostics can point into it.
        self.source = Arc::from(src);
        self.span_base = 0;
        self.contains_only_comments = false;
        if is_only_whitespace_or_comments(src) {
            self.contains_only_comments = true;
            return None;
        }
        if !self.shallow_enough(src) {
            return None;
        }
        match parse(src) {
            Ok(expr) => self.eval_reported(&expr),
            Err(e) => {
                // Distinguish comments-only (parse_all yields []) from a real
                // parse error.
                if let Ok(exprs) = parse_all(src)
                    && exprs.is_empty()
                {
                    self.contains_only_comments = true;
                } else {
                    self.diag_parse_error(&e);
                }
                None
            }
        }
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
    // 'allow_extra_text_at_end' semantics: parse the whole string, keep only the
    // first expression, and report chars_read as that expression's span end.
    //
    // The merged free driver 'hfst::xre::compile_first' (xre_utils.cc) is folded
    // in here: it set 'allow_extra_text_at_end', ran the parser, then returned
    // 'last_compiled' and 'chars_read = cr'. The AST walk reproduces the same
    // behaviour without the flex/bison position counters.
    // [spec:hfst:def:xre-utils.hfst.xre.compile-first-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.compile-first-fn]
    pub fn compile_first(
        &mut self,
        expression: &str,
        chars_read: &mut u32,
    ) -> Option<HfstTransducer<B>> {
        self.compile_first_at(expression, 0, chars_read)
    }

    /// [`Self::compile_first`] on `text[offset..]`, for a caller walking a
    /// file one expression at a time. Diagnostics are placed in the whole
    /// `text`, so they name the file's own line and column rather than ones
    /// counted from the slice. `chars_read` counts from `offset`.
    pub fn compile_first_at(
        &mut self,
        text: &str,
        offset: usize,
        chars_read: &mut u32,
    ) -> Option<HfstTransducer<B>> {
        let expression = &text[offset..];
        // Retain the source so diagnostics can point into it.
        self.source = Arc::from(text);
        self.span_base = offset;
        self.contains_only_comments = false;
        // Whitespace/comment-only input: the C++ lexer consumed it to EOF and
        // set contains_only_comments; nfst's parse_all errors on it instead.
        if is_only_whitespace_or_comments(expression) {
            self.contains_only_comments = true;
            *chars_read = 0;
            return None;
        }
        if !self.shallow_enough(expression) {
            *chars_read = 0;
            return None;
        }
        match parse_all(expression) {
            Ok(exprs) if !exprs.is_empty() => {
                let first = &exprs[0];
                // nfst spans exclude the ';' terminator; the C++ lexer position
                // this mirrors sat just past it. Consume through the ';' so the
                // caller's next slice starts on the following expression.
                let bytes = expression.as_bytes();
                let mut end = first.span.end();
                while end < bytes.len() && bytes[end] != b';' {
                    end += 1;
                }
                if end < bytes.len() {
                    end += 1;
                }
                *chars_read = end as u32;
                self.eval_reported(first)
            }
            Ok(_) => {
                self.contains_only_comments = true;
                *chars_read = 0;
                None
            }
            Err(e) => {
                self.diag_parse_error(&e);
                *chars_read = 0;
                None
            }
        }
    }

    // [spec:hfst:req:embed.no-panic]
    /// Whether `src` parses to no more than the parser and the evaluator can
    /// descend into on the stack a compile has (see [`crate::depth`]). Too
    /// deep, it fails as an ordinary parse failure would, never an abort.
    /// Brackets nested past [`crate::depth::MAX_NESTING_DEPTH`] fail with
    /// nothing said, which is what regexp2fst prints for them; any other
    /// limit with an error naming it.
    fn shallow_enough(&mut self, src: &str) -> bool {
        match crate::depth::scan_xre(src) {
            Ok(_) => true,
            Err(e) if e.limit == crate::depth::Limit::Nesting => false,
            Err(e) => {
                self.diag_at(e.span.clone(), Severity::Error, &e.to_string());
                false
            }
        }
    }

    /// Evaluate a parsed root, reporting why it failed when it does.
    fn eval_reported(&mut self, expression: &SpannedXre) -> Option<HfstTransducer<B>> {
        match self.eval_finalized(expression) {
            Ok(t) => Some(t),
            Err(e) => {
                self.diag_eval_error(&e);
                None
            }
        }
    }
}

/// Whether `src` contains only whitespace and `!`-to-end-of-line XRE comments
/// (what the C++ lexer silently consumed before reporting comments-only).
fn is_only_whitespace_or_comments(src: &str) -> bool {
    src.lines()
        .all(|line| line.trim_start().is_empty() || line.trim_start().starts_with('!'))
}
