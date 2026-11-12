//! What a compile said, as the plain `Diagnostic` objects the wrapper
//! declares, with their spans in UTF-16 units.

use std::ops::Range;

use hfst_embed::compile::{Diagnostic, Severity};
use wasm_bindgen::JsValue;

use crate::values::{Object, array, strings};

/// The UTF-16 units of `source` before byte `at`, as JavaScript string
/// indices count. An offset inside a character counts the characters before
/// it.
fn utf16(source: &str, at: usize) -> usize {
    let mut at = at.min(source.len());
    while !source.is_char_boundary(at) {
        at -= 1;
    }
    source[..at].encode_utf16().count()
}

/// The byte range `span` of the diagnostic's source in UTF-16 units. A
/// diagnostic with no source text keeps its offsets, having no text to count
/// them in.
pub fn utf16_span(source: &str, span: &Range<usize>) -> Range<usize> {
    if source.is_empty() {
        return span.clone();
    }
    utf16(source, span.start)..utf16(source, span.end)
}

fn span_object(source: &str, span: &Range<usize>) -> JsValue {
    let span = utf16_span(source, span);
    Object::new()
        .with("start", span.start as f64)
        .with("end", span.end as f64)
        .done()
}

fn severity(d: &Diagnostic) -> &'static str {
    match d.severity() {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

// [spec:hfst:req:javascript.compile-errors]
/// `d` as a `Diagnostic`: its severity, message and source name, the span of
/// its primary label and its other labels in UTF-16 units, its notes and
/// help, and its text rendered without colour.
pub fn diagnostic(d: &Diagnostic) -> JsValue {
    let source = d.source();
    let labels = d.labels().iter().map(|label| {
        Object::new()
            .with("span", span_object(source, &label.span))
            .with("message", label.message.as_str())
            .done()
    });
    Object::new()
        .with("severity", severity(d))
        .with("message", d.message())
        .with("sourceName", d.source_name())
        .with(
            "span",
            d.primary_span()
                .map_or(JsValue::NULL, |span| span_object(source, &span)),
        )
        .with("labels", array(labels))
        .with("notes", strings(d.notes()))
        .with(
            "help",
            d.help_text().map_or(JsValue::NULL, JsValue::from_str),
        )
        .with("rendered", d.render(false))
        .done()
}

pub fn diagnostics(all: &[Diagnostic]) -> JsValue {
    array(all.iter().map(diagnostic))
}

// [spec:hfst:req:javascript.compile-errors]
/// The warnings of a compile that succeeded, to return with its result: each
/// warning, and each error a compile made without failing, as a command of
/// an xfst script does. What it says at the info level, as with `verbose`,
/// goes to the logger at the info level, under the target `hfst`, as
/// divvun-hfst sends it to the `hfst` logger.
pub fn warnings(all: &[Diagnostic]) -> JsValue {
    let mut warnings = Vec::new();
    for d in all {
        match d.severity() {
            Severity::Error | Severity::Warning => warnings.push(diagnostic(d)),
            Severity::Info => {
                tracing::info!(target: "hfst", "{}", d.render(false).trim_end());
            }
        }
    }
    array(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:hfst:req:javascript.compile-errors/test]
    #[test]
    fn spans_count_utf16_units() {
        let text = "a\u{1F600}b\u{e5}c";
        assert_eq!(utf16_span(text, &(0..1)), 0..1);
        assert_eq!(
            utf16_span(text, &(1..5)),
            1..3,
            "one character outside the BMP is two units"
        );
        assert_eq!(utf16_span(text, &(5..8)), 3..5, "å is one unit, two bytes");
        assert_eq!(
            utf16_span(text, &(2..6)),
            1..4,
            "an offset inside a character counts the characters before it"
        );
        assert_eq!(utf16_span("", &(3..9)), 3..9, "no text keeps the offsets");
    }
}
