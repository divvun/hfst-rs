//! The compile driver: guard the input, parse it, and evaluate the root
//! expression.

use nfst_xre::{parse, parse_all};

use super::*;

impl<B: AlgebraBackend> XreCompiler<B> {
    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-fn]
    // Returns the compiled transducer, or None on parse failure / comments-only
    // (the C++ 'HfstTransducer*' null contract expressed as an Option).
    pub fn compile(&mut self, expression: &str) -> Option<HfstTransducer<B>> {
        self.compile_impl(expression)
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
        // Retain the source so diagnostics can render the offending snippet.
        self.source = expression.to_string();
        self.contains_only_comments = false;
        // Whitespace/comment-only input: the C++ lexer consumed it to EOF and
        // set contains_only_comments; nfst's parse_all errors on it instead.
        if is_only_whitespace_or_comments(expression) {
            self.contains_only_comments = true;
            *chars_read = 0;
            return None;
        }
        // Reject pathologically deep nesting before it can overflow the parser's
        // recursion; report it as an ordinary parse failure, never an abort.
        if exceeds_max_nesting_depth(expression) {
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
                self.eval_finalized(first).ok()
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

    // Internal compile driver: parse → eval root → optimize. None on parse error
    // or comments-only (the latter also flips the contains_only_comments flag,
    // matching the 'XRE: (empty) { contains_only_comments = true; }' action).
    fn compile_impl(&mut self, src: &str) -> Option<HfstTransducer<B>> {
        // Retain the source so diagnostics can render the offending snippet.
        self.source = src.to_string();
        self.contains_only_comments = false;
        if is_only_whitespace_or_comments(src) {
            self.contains_only_comments = true;
            return None;
        }
        // Reject pathologically deep nesting before it can overflow the parser's
        // recursion; report it as an ordinary parse failure, never an abort.
        if exceeds_max_nesting_depth(src) {
            return None;
        }
        match parse(src) {
            Ok(expr) => self.eval_finalized(&expr).ok(),
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
}

/// Whether `src` contains only whitespace and `!`-to-end-of-line XRE comments
/// (what the C++ lexer silently consumed before reporting comments-only).
fn is_only_whitespace_or_comments(src: &str) -> bool {
    src.lines()
        .all(|line| line.trim_start().is_empty() || line.trim_start().starts_with('!'))
}

/// Ceiling on grouping-delimiter nesting depth accepted before parsing.
///
/// The `nfst-xre` parser is recursive-descent and the AST evaluator recurses
/// per node, so pathologically deep bracket/paren nesting (`[[[[…`) exhausts
/// even the worker thread's large stack and aborts the process (SIGABRT), which
/// no `catch_unwind` can recover. Rejecting over-deep input up front turns that
/// abort into an ordinary parse failure. The bound sits far below the empirical
/// overflow point (~12k) yet far above any hand-written grammar's real nesting.
const MAX_NESTING_DEPTH: usize = 4000;

/// Whether `src`'s grouping-delimiter nesting (`[`, `(`, `[.`) ever runs deeper
/// than [`MAX_NESTING_DEPTH`]. `%`-escaped characters and `"…"`-quoted literals
/// are skipped so their brackets never count toward the depth.
fn exceeds_max_nesting_depth(src: &str) -> bool {
    let mut depth: usize = 0;
    let mut in_quote = false;
    let mut chars = src.chars();
    while let Some(c) = chars.next() {
        match c {
            // A '%' escapes the next character (an ordinary literal, never a
            // grouping delimiter); consume it so its bracket does not count.
            '%' => {
                let _ = chars.next();
            }
            '"' => in_quote = !in_quote,
            _ if in_quote => {}
            '[' | '(' => {
                depth += 1;
                if depth > MAX_NESTING_DEPTH {
                    return true;
                }
            }
            ']' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}
