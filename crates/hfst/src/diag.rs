//! Rich source-anchored compiler diagnostics via `ariadne`.
//!
//! The lexc/xre compilers historically emitted a bare one-line message
//! (`error!`/`warn!`) for problems like `-Wmissing-alphabets`, with no hint as
//! to *where* in the source the problem was. Since the front-end parsers
//! (`nfst_lexc`/`nfst_xre`) already carry byte spans on every AST node, we can
//! render the offending region with a caret-underlined snippet instead.

use std::ops::Range;

use ariadne::{Color, Label, Report, ReportKind, Source};

/// Severity of a rendered diagnostic.
#[derive(Clone, Copy)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Render one diagnostic to stderr: a header line, the relevant slice of
/// `source` with `span` underlined, and `message` as the label.
///
/// `name` is the source label shown in the report (a file name, or a sentinel
/// like `"<lexc>"`). When `source` is empty or the span is unusable, this falls
/// back to a plain one-line message so no diagnostic is ever lost.
pub fn emit(name: &str, source: &str, span: Range<usize>, severity: Severity, message: &str) {
    emit_with_notes(name, source, span, severity, message, &[]);
}

/// As [`emit`], with follow-up advice rendered under the snippet.
///
/// A diagnostic that only states what went wrong leaves the reader to work out
/// what to type instead; `notes` carries that second half (the escape to use,
/// the command that was probably meant). Notes survive the plain-message
/// fallback, where they are printed as further lines.
pub fn emit_with_notes(
    name: &str,
    source: &str,
    span: Range<usize>,
    severity: Severity,
    message: &str,
    notes: &[String],
) {
    if source.is_empty() {
        emit_plain(severity, message);
        for n in notes {
            emit_plain(severity, n);
        }
        return;
    }
    // The parsers report UTF-8 byte offsets, while ariadne's `Range<usize>`
    // spans are character offsets. Convert at this shared boundary so a
    // diagnostic after non-ASCII text still points at the right line and
    // column. The conversion also clamps stale or mid-code-point offsets.
    let span = byte_span_to_char_span(source, span);

    let (kind, color) = match severity {
        Severity::Error => (ReportKind::Error, Color::Red),
        Severity::Warning => (ReportKind::Warning, Color::Yellow),
        Severity::Info => (ReportKind::Custom("Info", Color::Blue), Color::Blue),
    };

    let mut builder = Report::build(kind, (name, span.clone()))
        .with_message(message)
        .with_label(
            Label::new((name, span))
                .with_message(message)
                .with_color(color),
        );
    for n in notes {
        builder = builder.with_note(n);
    }

    let mut out: Vec<u8> = Vec::new();
    let rendered = builder
        .finish()
        .write((name, Source::from(source)), &mut out);

    match rendered {
        Ok(()) => eprint!("{}", String::from_utf8_lossy(&out)),
        Err(_) => {
            emit_plain(severity, message);
            for n in notes {
                emit_plain(severity, n);
            }
        }
    }
}

fn emit_plain(severity: Severity, message: &str) {
    match severity {
        Severity::Error => tracing::error!("{}", message),
        Severity::Warning => tracing::warn!("{}", message),
        Severity::Info => tracing::info!("{}", message),
    }
}

fn byte_span_to_char_span(source: &str, span: Range<usize>) -> Range<usize> {
    let end = span.end.min(source.len());
    let start = span.start.min(end);

    let mut start_byte = start;
    while start_byte > 0 && !source.is_char_boundary(start_byte) {
        start_byte -= 1;
    }

    let mut end_byte = end;
    while end_byte < source.len() && !source.is_char_boundary(end_byte) {
        end_byte += 1;
    }

    source[..start_byte].chars().count()..source[..end_byte].chars().count()
}

#[cfg(test)]
mod tests {
    use super::byte_span_to_char_span;

    #[test]
    fn byte_span_after_unicode_becomes_character_span() {
        let source = "LEXICON Root\n组织机构 enddomain ;\nभा enddomain ;\n";
        let byte_start = source.find("भा").expect("fixture contains grapheme");
        let byte_span = byte_start..byte_start + "भा".len();
        let char_start = source[..byte_start].chars().count();

        assert_eq!(
            byte_span_to_char_span(source, byte_span),
            char_start..char_start + 2
        );
    }

    #[test]
    fn mid_codepoint_offsets_expand_to_character_boundaries() {
        let source = "aभाb";

        assert_eq!(byte_span_to_char_span(source, 2..6), 1..3);
    }
}
