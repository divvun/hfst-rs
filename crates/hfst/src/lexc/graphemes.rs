//! Reporting undeclared multi-code-point graphemes in entry data.

use icu::normalizer::ComposingNormalizerBorrowed;
use icu::segmenter::GraphemeClusterSegmenter;

use super::*;

impl<B: AlgebraBackend> LexcCompiler<B> {
    /// Port of 'LexcCompiler::unicodeCheck_': report every grapheme in 'data'
    /// that takes more than one code point and that no 'Multichar_Symbols'
    /// declaration covers, then register it as an alphabet symbol.
    ///
    /// Registering is what makes the report fire once per distinct grapheme
    /// rather than once per occurrence; it also spares '-Wmissing-alphabets'
    /// from naming the same symbol a second time, since 'add_string_entry's own
    /// loop would otherwise register it moments later. Under 'split_characters'
    /// the reading is per code point by request, so there is nothing to report.
    pub(super) fn unicode_check(&mut self, data: &str) -> &mut Self {
        if self.split_characters {
            return self;
        }
        for grapheme in self.undeclared_graphemes(data) {
            let d = self.grapheme_diagnostic(&grapheme);
            let severity = if self.treat_warnings_as_errors {
                self.parseErrors_ = true;
                crate::diag::Severity::Error
            } else {
                crate::diag::Severity::Warning
            };
            crate::diag::emit_with_notes(
                &self.source_name,
                &self.source,
                d.span.clone(),
                severity,
                &d.message,
                &d.notes,
            );
            self.grapheme_diags.push(d);
            self.add_alphabet(&grapheme);
        }
        self
    }

    /// Every grapheme reported by [`unicode_check`](Self::unicode_check) since
    /// the last `reset`, in source order.
    ///
    /// The rendering itself goes to stderr, which a caller cannot read back;
    /// this is the same data, kept so the shaping can be inspected.
    pub fn grapheme_diagnostics(&self) -> &[GraphemeDiagnostic] {
        &self.grapheme_diags
    }

    /// The distinct multi-code-point graphemes in `data` that `alphabets` does
    /// not already cover, in order of first appearance.
    fn undeclared_graphemes(&self, data: &str) -> Vec<String> {
        let segmenter = GraphemeClusterSegmenter::new();
        let mut out: Vec<String> = Vec::new();
        let mut prev = 0usize;
        for next in segmenter.segment_str(data).skip(1) {
            let cluster = &data[prev..next];
            prev = next;
            if cluster.chars().nth(1).is_none() {
                continue;
            }
            // A CRLF line ending is a two-code-point cluster; nothing about the
            // shape of a line break belongs in a symbol table.
            if cluster.chars().any(char::is_control) {
                continue;
            }
            if self.alphabets.contains(cluster) || out.iter().any(|g| g == cluster) {
                continue;
            }
            out.push(cluster.to_string());
        }
        out
    }

    /// Shape the report for `grapheme`: a caret under its first occurrence in
    /// the entry being walked, its code points spelled out in full, and the
    /// declaration that settles how it is read.
    fn grapheme_diagnostic(&self, grapheme: &str) -> GraphemeDiagnostic {
        // The entry text is the source the user wrote, so it may carry escapes
        // the AST already stripped; searching it for the grapheme's own bytes
        // survives that, and falls back to the whole entry when it cannot.
        let span = self
            .source
            .get(self.current_span.clone())
            .and_then(|entry| entry.find(grapheme))
            .map(|off| {
                let start = self.current_span.start + off;
                start..start + grapheme.len()
            })
            .unwrap_or_else(|| self.current_span.clone());

        let points: Vec<String> = grapheme
            .chars()
            .map(|c| format!("U+{:04X}", c as u32))
            .collect();
        let mut notes = vec![
            format!(
                "its code points are {} — lexc reads them as one symbol only because they form a single grapheme cluster",
                points.join(" ")
            ),
            format!(
                "add '{grapheme}' to the Multichar_Symbols section to declare that reading explicitly"
            ),
        ];
        let composed = ComposingNormalizerBorrowed::new_nfc().normalize(grapheme);
        if let Some(single) = composed.chars().next()
            && composed.chars().nth(1).is_none()
        {
            notes.push(format!(
                "'{grapheme}' also has a one-code-point spelling (U+{:04X}); normalising this file to NFC would avoid the question",
                single as u32
            ));
        }

        GraphemeDiagnostic {
            span,
            message: format!(
                "undeclared multi-code-point grapheme '{grapheme}' ({} code points)",
                points.len()
            ),
            notes,
        }
    }
}
