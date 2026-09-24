//! Rich source-anchored compiler diagnostics via `ariadne`.
//!
//! The lexc/xre compilers historically emitted a bare one-line message
//! (`error!`/`warn!`) for problems like `-Wmissing-alphabets`, with no hint as
//! to *where* in the source the problem was. Since the front-end parsers
//! (`nfst_lexc`/`nfst_xre`) already carry byte spans on every AST node, we can
//! render the offending region with a caret-underlined snippet instead.

use std::io::IsTerminal;
use std::ops::Range;

use ariadne::{Color, Config, Label, Report, ReportKind, Source};

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
    let mut d = Diagnostic::new(severity, message).label(span, message);
    for n in notes {
        d = d.note(n.clone());
    }
    d.emit(name, source);
}

/// A diagnostic that points at several pieces of the source at once.
///
/// The first label is the primary one: it gives the report its location and
/// takes the severity's colour. Later labels mark the other pieces involved,
/// such as the definition a bad use refers to. Notes say why; the help line
/// says what to write instead.
pub struct Diagnostic {
    severity: Severity,
    message: String,
    labels: Vec<(Range<usize>, String)>,
    notes: Vec<String>,
    help: Option<String>,
}

impl Diagnostic {
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message)
    }

    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }

    pub fn label(mut self, span: Range<usize>, text: impl Into<String>) -> Self {
        self.labels.push((span, text.into()));
        self
    }

    pub fn note(mut self, text: impl Into<String>) -> Self {
        self.notes.push(text.into());
        self
    }

    pub fn help(mut self, text: impl Into<String>) -> Self {
        self.help = Some(text.into());
        self
    }

    /// Render to stderr against `source`, labelled `name`. Falls back to
    /// plain lines when there is no source text or no label to anchor to.
    pub fn emit(&self, name: &str, source: &str) {
        let Some((primary, _)) = self.labels.first() else {
            self.emit_plain();
            return;
        };
        if source.is_empty() {
            self.emit_plain();
            return;
        }
        // The parsers report UTF-8 byte offsets, while ariadne's `Range<usize>`
        // spans are character offsets. Convert at this shared boundary so a
        // diagnostic after non-ASCII text still points at the right line and
        // column. The conversion also clamps stale or mid-code-point offsets.
        let (kind, color) = match self.severity {
            Severity::Error => (ReportKind::Error, Color::Red),
            Severity::Warning => (ReportKind::Warning, Color::Yellow),
            Severity::Info => (ReportKind::Custom("Info", Color::Blue), Color::Blue),
        };
        let primary = byte_span_to_char_span(source, primary.clone());
        // Colour only a terminal: a build log redirected to a file should not
        // fill with escape codes. NO_COLOR opts out everywhere.
        let colour = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        let mut builder = Report::build(kind, (name, primary))
            .with_config(Config::default().with_color(colour))
            .with_message(&self.message);
        for (i, (span, text)) in self.labels.iter().enumerate() {
            let span = byte_span_to_char_span(source, span.clone());
            let label_color = if i == 0 { color } else { Color::Cyan };
            builder = builder.with_label(
                Label::new((name, span))
                    .with_message(text)
                    .with_color(label_color)
                    .with_order(i as i32),
            );
        }
        for n in &self.notes {
            builder = builder.with_note(n);
        }
        if let Some(h) = &self.help {
            builder = builder.with_help(h);
        }

        let mut out: Vec<u8> = Vec::new();
        match builder
            .finish()
            .write((name, Source::from(source)), &mut out)
        {
            Ok(()) => eprint!("{}", String::from_utf8_lossy(&out)),
            Err(_) => self.emit_plain(),
        }
    }

    fn emit_plain(&self) {
        emit_plain(self.severity, &self.message);
        for n in &self.notes {
            emit_plain(self.severity, n);
        }
        if let Some(h) = &self.help {
            emit_plain(self.severity, h);
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
