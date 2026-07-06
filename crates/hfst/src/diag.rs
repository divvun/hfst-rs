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
}

/// Render one diagnostic to stderr: a header line, the relevant slice of
/// `source` with `span` underlined, and `message` as the label.
///
/// `name` is the source label shown in the report (a file name, or a sentinel
/// like `"<lexc>"`). When `source` is empty or the span is unusable, this falls
/// back to a plain one-line message so no diagnostic is ever lost.
pub fn emit(name: &str, source: &str, span: Range<usize>, severity: Severity, message: &str) {
    if source.is_empty() {
        emit_plain(severity, message);
        return;
    }
    // Clamp to the source bounds — a stale or degenerate span must never make
    // ariadne panic on an out-of-range slice.
    let end = span.end.min(source.len());
    let start = span.start.min(end);
    let span = start..end;

    let (kind, color) = match severity {
        Severity::Error => (ReportKind::Error, Color::Red),
        Severity::Warning => (ReportKind::Warning, Color::Yellow),
    };

    let mut out: Vec<u8> = Vec::new();
    let rendered = Report::build(kind, (name, span.clone()))
        .with_message(message)
        .with_label(
            Label::new((name, span))
                .with_message(message)
                .with_color(color),
        )
        .finish()
        .write((name, Source::from(source)), &mut out);

    match rendered {
        Ok(()) => eprint!("{}", String::from_utf8_lossy(&out)),
        Err(_) => emit_plain(severity, message),
    }
}

fn emit_plain(severity: Severity, message: &str) {
    match severity {
        Severity::Error => tracing::error!("{}", message),
        Severity::Warning => tracing::warn!("{}", message),
    }
}
