//! Compiler diagnostics in the shape the wrapper's `Diagnostic` type declares
//! in `js/shared/errors.ts`. The library counts in UTF-8 bytes; JavaScript
//! strings index UTF-16 code units, so every offset is converted against the
//! text the diagnostic points into.

use std::ops::Range;
use std::sync::Arc;

use hfst::diag::{Diagnostic, Severity};
use serde::Serialize;
use wasm_bindgen::JsValue;

/// A diagnostic as the wrapper hands it out.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsDiagnostic {
    severity: &'static str,
    message: String,
    source_name: String,
    start: usize,
    end: usize,
    labels: Vec<JsLabel>,
    notes: Vec<String>,
    help: Option<String>,
    rendered: String,
}

#[derive(Debug, Serialize)]
struct JsLabel {
    start: usize,
    end: usize,
    message: String,
}

impl JsDiagnostic {
    pub fn is_error(&self) -> bool {
        self.severity == "error"
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// The diagnostic as a plain JavaScript object.
    pub fn to_js(&self) -> JsValue {
        let json = serde_json::to_string(self)
            .expect("a diagnostic holds only strings and numbers, which always serialise");
        js_sys::JSON::parse(&json).expect("serde_json writes valid JSON")
    }
}

/// The texts one compile was given: its source under the name diagnostics
/// show for it, and the files of its map that are UTF-8 text.
pub struct Sources {
    name: String,
    text: Arc<str>,
    files: Vec<(String, Arc<str>)>,
}

impl Sources {
    pub fn new(name: &str, text: &str, files: &[(String, Vec<u8>)]) -> Self {
        let files = files
            .iter()
            .filter_map(|(name, bytes)| {
                let text = std::str::from_utf8(bytes).ok()?;
                Some((name.clone(), Arc::from(text)))
            })
            .collect();
        Sources {
            name: name.to_owned(),
            text: Arc::from(text),
            files,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &Arc<str> {
        &self.text
    }

    /// `d` in the wrapper's shape. A diagnostic in the source is named as the
    /// source is, whatever name its compiler gave it, and one in a file of the
    /// map takes that file's name. Any other is in text the compiler made
    /// itself, such as a lexc entry's regular expression, which lexc prints
    /// again before compiling it; it keeps the name and offsets the compiler
    /// gave it, which the rendered report quotes.
    pub fn convert(&self, d: &Diagnostic) -> JsDiagnostic {
        let file = || {
            let mut same = self.files.iter().filter(|(_, text)| **text == *d.source());
            let named = same.clone().find(|(name, _)| name == d.source_name());
            named.or_else(|| same.next())
        };
        let place = if *self.text == *d.source() {
            Some((&self.name, &self.text))
        } else {
            file().map(|(name, text)| (name, text))
        };
        match place {
            Some((name, text)) if name != d.source_name() => {
                js_diagnostic(&anchored(d, name, Arc::clone(text), 0))
            }
            Some(_) | None => js_diagnostic(d),
        }
    }

    /// An error saying `message`, for a compile that failed without a
    /// diagnostic of its own to say why. It points at no place in the source.
    pub fn failure(&self, message: &str) -> JsDiagnostic {
        js_diagnostic(&Diagnostic::error(message).in_source(&self.name, Arc::clone(&self.text)))
    }
}

/// `d` with `shift` added to every span, in `text` under `name`: for a
/// diagnostic about a slice of `text` that starts `shift` bytes in.
pub fn anchored(d: &Diagnostic, name: &str, text: Arc<str>, shift: usize) -> Diagnostic {
    let mut moved = Diagnostic::new(d.severity(), d.message());
    for label in d.labels() {
        let span = label.span.start + shift..label.span.end + shift;
        moved = moved.label(span, label.message.clone());
    }
    for note in d.notes() {
        moved = moved.note(note.clone());
    }
    if let Some(help) = d.help_text() {
        moved = moved.help(help);
    }
    moved.in_source(name, text)
}

fn js_diagnostic(d: &Diagnostic) -> JsDiagnostic {
    let text = d.source();
    let labels: Vec<JsLabel> = d
        .labels()
        .iter()
        .map(|label| {
            let (start, end) = utf16_span(text, label.span.clone());
            JsLabel {
                start,
                end,
                message: label.message.clone(),
            }
        })
        .collect();
    let (start, end) = labels.first().map_or((0, 0), |l| (l.start, l.end));
    JsDiagnostic {
        severity: match d.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        },
        message: d.message().to_owned(),
        source_name: d.source_name().to_owned(),
        start,
        end,
        labels,
        notes: d.notes().to_vec(),
        help: d.help_text().map(str::to_owned),
        rendered: d.render(false),
    }
}

/// The UTF-16 offsets of the byte range `span` of `text`. An offset past the
/// end is taken as the end, and one inside a character moves to that
/// character's edge, the start back and the end on, so the range still covers
/// everything it marked.
pub fn utf16_span(text: &str, span: Range<usize>) -> (usize, usize) {
    let mut end = span.end.min(text.len());
    let mut start = span.start.min(end);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    while !text.is_char_boundary(end) {
        end += 1;
    }
    let start16 = utf16_len(&text[..start]);
    (start16, start16 + utf16_len(&text[start..end]))
}

fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_offsets_are_byte_offsets() {
        assert_eq!(utf16_span("abc def", 4..7), (4, 7));
    }

    #[test]
    fn offsets_after_non_ascii_count_utf16_units() {
        // ö takes two bytes and one unit; 𝒜 takes four bytes and two units.
        let text = "ö 𝒜 x";
        let x = text.find('x').expect("the text has an x");
        assert_eq!(x, 8);
        assert_eq!(utf16_span(text, x..x + 1), (5, 6));
        let script = text.find('𝒜').expect("the text has the letter");
        assert_eq!(utf16_span(text, script..script + 4), (2, 4));
    }

    #[test]
    fn offsets_inside_a_character_widen_to_its_edges() {
        assert_eq!(utf16_span("a𝒜b", 2..4), (1, 3));
        assert_eq!(utf16_span("aöb", 2..2), (1, 2));
    }

    #[test]
    fn offsets_past_the_end_stop_at_it() {
        assert_eq!(utf16_span("ab", 1..9), (1, 2));
        assert_eq!(utf16_span("ab", 7..9), (2, 2));
        assert_eq!(utf16_span("", 0..3), (0, 0));
    }

    fn json(d: &JsDiagnostic) -> serde_json::Value {
        serde_json::to_value(d).expect("serialises")
    }

    #[test]
    fn source_diagnostic_takes_its_name_and_utf16_offsets() {
        let sources = Sources::new("words.re", "ö [a", &[]);
        let d = Diagnostic::error("expected ]")
            .label(5..5, "here")
            .note("why")
            .help("add ]")
            .in_source("<regex>", Arc::from("ö [a"));
        let out = json(&sources.convert(&d));
        assert_eq!(out["severity"], "error");
        assert_eq!(out["sourceName"], "words.re");
        assert_eq!(
            (out["start"].clone(), out["end"].clone()),
            (4.into(), 4.into())
        );
        assert_eq!(out["labels"][0]["message"], "here");
        assert_eq!(out["notes"][0], "why");
        assert_eq!(out["help"], "add ]");
        let rendered = out["rendered"].as_str().expect("rendered is text");
        assert!(rendered.contains("words.re:1:5"), "{rendered}");
    }

    #[test]
    fn map_file_diagnostic_takes_the_file_name() {
        let files = [("inc.txt".to_owned(), "é x".as_bytes().to_vec())];
        let sources = Sources::new("<pmatch>", "other", &files);
        let d = Diagnostic::warning("odd x")
            .label(3..4, "x")
            .in_source("<pmatch>", Arc::from("é x"));
        let out = json(&sources.convert(&d));
        assert_eq!(out["sourceName"], "inc.txt");
        assert_eq!(out["start"], 2);
    }

    #[test]
    fn a_diagnostic_in_derived_text_keeps_its_place() {
        let sources = Sources::new("a.lexc", "LEXICON Root\n< a > # ;\n", &[]);
        let d = Diagnostic::error("bad")
            .label(0..1, "bad")
            .in_source("<regex>", Arc::from("a"));
        let out = json(&sources.convert(&d));
        assert_eq!(out["sourceName"], "<regex>");
        assert_eq!(out["end"], 1);
        let failure = json(&sources.failure("broken"));
        assert_eq!(failure["sourceName"], "a.lexc");
        assert_eq!(failure["labels"].as_array().map(Vec::len), Some(0));
        assert_eq!(failure["rendered"], "Error: broken\n");
    }
}
