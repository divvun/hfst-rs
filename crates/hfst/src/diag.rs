//! Source-anchored compiler diagnostics, handed to the caller as values.
//!
//! The front-end parsers (`nfst_lexc`, `nfst_xre`, `nfst_twolc`, ...) carry
//! byte spans on every AST node, so a problem can point at the region of the
//! source it concerns. A [`Diagnostic`] keeps that span with the source text
//! it indexes; [`Diagnostic::render`] turns it into an `ariadne` report with a
//! caret-underlined snippet. The library never prints one: each compiler
//! collects its diagnostics and the caller decides where they go.

use std::ops::Range;
use std::sync::Arc;

use ariadne::{Color, Config, Report, ReportKind, Source};

/// Severity of a diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    /// The word a report opens with.
    fn title(self) -> &'static str {
        match self {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Info => "Info",
        }
    }
}

/// A piece of the source a diagnostic points at, and what it says there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// Byte range in the diagnostic's source.
    pub span: Range<usize>,
    pub message: String,
}

// [spec:hfst:req:javascript.diagnostics-as-data]
/// A diagnostic that points at one or more pieces of the source.
///
/// The first label is the primary one: it gives the report its location and
/// takes the severity's colour. Later labels mark the other pieces involved,
/// such as the definition a bad use refers to. Notes say why; the help line
/// says what to write instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    severity: Severity,
    message: String,
    source_name: String,
    /// The text the label spans index. Shared, because one source can carry
    /// many diagnostics and a lexc source can run to megabytes.
    source: Arc<str>,
    labels: Vec<Label>,
    notes: Vec<String>,
    help: Option<String>,
}

impl Diagnostic {
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            source_name: String::new(),
            source: Arc::from(""),
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

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(Severity::Info, message)
    }

    /// Mark `span` (a byte range in the source) with `text`.
    pub fn label(mut self, span: Range<usize>, text: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: text.into(),
        });
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

    /// Attach the source the labels index, and the name a report shows for
    /// it (a file name, or a sentinel like `"<lexc>"`).
    pub fn in_source(mut self, name: impl Into<String>, source: Arc<str>) -> Self {
        self.source_name = name.into();
        self.source = source;
        self
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// The byte span of the primary label, if there is one.
    pub fn primary_span(&self) -> Option<Range<usize>> {
        self.labels.first().map(|l| l.span.clone())
    }

    /// Every label, the primary one first.
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    pub fn help_text(&self) -> Option<&str> {
        self.help.as_deref()
    }

    /// Whether [`render`](Self::render) draws a source snippet: there is
    /// source text and a label to anchor it to. Otherwise the report is the
    /// message, notes and help as plain lines.
    pub fn has_snippet(&self) -> bool {
        !self.labels.is_empty() && !self.source.is_empty()
    }

    /// The message, each note, then the help, one line each: the report when
    /// there is no snippet to draw.
    pub fn plain_lines(&self) -> Vec<&str> {
        std::iter::once(self.message.as_str())
            .chain(self.notes.iter().map(String::as_str))
            .chain(self.help.as_deref())
            .collect()
    }

    /// The report as text: a header line, the relevant slice of the source
    /// with each label underlined, then the notes and help. `colour` adds
    /// terminal colour codes. Without a snippet to draw it is the plain lines
    /// under the severity's title.
    pub fn render(&self, colour: bool) -> String {
        if !self.has_snippet() {
            return self.render_plain();
        }
        let (kind, color) = match self.severity {
            Severity::Error => (ReportKind::Error, Color::Red),
            Severity::Warning => (ReportKind::Warning, Color::Yellow),
            Severity::Info => (ReportKind::Custom("Info", Color::Blue), Color::Blue),
        };
        let name = self.source_name.as_str();
        let source: &str = &self.source;
        // The parsers report UTF-8 byte offsets, while ariadne's `Range<usize>`
        // spans are character offsets. Convert at this shared boundary so a
        // diagnostic after non-ASCII text still points at the right line and
        // column. The conversion also clamps stale or mid-code-point offsets.
        let primary = byte_span_to_char_span(source, self.labels[0].span.clone());
        let mut builder = Report::build(kind, (name, primary))
            .with_config(Config::default().with_color(colour))
            .with_message(&self.message);
        for (i, label) in self.labels.iter().enumerate() {
            let span = byte_span_to_char_span(source, label.span.clone());
            let label_color = if i == 0 { color } else { Color::Cyan };
            builder = builder.with_label(
                ariadne::Label::new((name, span))
                    .with_message(&label.message)
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
            Ok(()) => String::from_utf8_lossy(&out).into_owned(),
            Err(_) => self.render_plain(),
        }
    }

    fn render_plain(&self) -> String {
        let mut out = format!("{}: {}\n", self.severity.title(), self.message);
        for n in &self.notes {
            out.push_str(&format!("Note: {n}\n"));
        }
        if let Some(h) = &self.help {
            out.push_str(&format!("Help: {h}\n"));
        }
        out
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
    use super::{Diagnostic, Severity, byte_span_to_char_span};
    use std::sync::Arc;

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

    // [spec:hfst:req:javascript.diagnostics-as-data/test]
    #[test]
    fn a_diagnostic_carries_its_pieces_and_renders_them() {
        let source: Arc<str> = Arc::from("a b c\n");
        let d = Diagnostic::warning("odd b")
            .label(2..3, "this b")
            .label(4..5, "and this c")
            .note("why")
            .help("what instead")
            .in_source("x.txt", source);
        assert_eq!(d.severity(), Severity::Warning);
        assert_eq!(d.message(), "odd b");
        assert_eq!(d.source_name(), "x.txt");
        assert_eq!(d.primary_span(), Some(2..3));
        assert_eq!(d.labels()[1].span, 4..5);
        assert_eq!(d.notes(), ["why"]);
        assert_eq!(d.help_text(), Some("what instead"));
        let text = d.render(false);
        assert!(text.starts_with("Warning: odd b"), "{text}");
        assert!(text.contains("x.txt:1:3"), "{text}");
        assert!(
            text.contains("this b") && text.contains("and this c"),
            "{text}"
        );
        assert!(
            !text.contains('\u{1b}'),
            "colour codes without colour: {text}"
        );
        assert!(d.render(true).contains('\u{1b}'));
    }

    // [spec:hfst:req:javascript.diagnostics-as-data/test]
    #[test]
    fn without_source_or_label_the_report_is_plain() {
        let d = Diagnostic::error("broken").note("because").help("fix it");
        assert!(!d.has_snippet());
        assert_eq!(d.plain_lines(), ["broken", "because", "fix it"]);
        assert_eq!(
            d.render(false),
            "Error: broken\nNote: because\nHelp: fix it\n"
        );
        let unanchored = Diagnostic::error("broken").in_source("x", Arc::from("text"));
        assert!(!unanchored.has_snippet());
    }
}
