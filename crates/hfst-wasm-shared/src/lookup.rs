//! Lookup: the limits the wrapper passes in, and the results it gets back.

use std::fmt::Write as _;

use hfst_embed::Code;

use crate::error::GlueError;

/// The limits of one lookup, in the terms `hfst_embed` takes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookupLimits {
    pub results: Option<usize>,
    pub seconds: Option<f64>,
}

impl LookupLimits {
    /// The limits for a result count and a time in milliseconds, either left
    /// out for no limit. The wrapper caps the count at `u32::MAX`; a cap the
    /// target cannot hold is no cap, since nothing finds that many results.
    pub fn new(limit: Option<u32>, time_limit_ms: Option<f64>) -> Result<LookupLimits, GlueError> {
        let results = limit.map(|limit| usize::try_from(limit).unwrap_or(usize::MAX));
        let seconds = match time_limit_ms {
            None => None,
            Some(ms) if ms.is_finite() && ms >= 0.0 => Some(ms / 1000.0),
            Some(ms) => {
                return Err(GlueError::new(
                    Code::InvalidArgument,
                    format!("a time limit is a finite number of milliseconds, 0 or more, not {ms}"),
                ));
            }
        };
        Ok(LookupLimits { results, seconds })
    }
}

// [spec:hfst:req:javascript.lookup]
// [spec:hfst:req:javascript.optimised-lookup+2]
/// Lookup results, each as its output, its symbols and its weight, as the
/// JSON array of `{ output, symbols, weight }` objects the wrapper parses.
/// Both lookups hand JavaScript this one string rather than objects built a
/// property at a time across the boundary. Each weight is the f64 that holds
/// the f32 exactly, written in the fewest digits that read back as that f64,
/// so `JSON.parse` gives the same number. JSON has no number for a weight
/// that is not finite, so such a weight is the string `Infinity`, `-Infinity`
/// or `NaN`, which the wrapper turns back.
pub fn encode<'a, S: AsRef<str> + 'a>(
    results: impl IntoIterator<Item = (&'a str, &'a [S], f32)>,
) -> String {
    let mut text = String::from("[");
    for (i, (output, symbols, weight)) in results.into_iter().enumerate() {
        if i > 0 {
            text.push(',');
        }
        text.push_str("{\"output\":");
        push_json_string(&mut text, output);
        text.push_str(",\"symbols\":[");
        for (j, symbol) in symbols.iter().enumerate() {
            if j > 0 {
                text.push(',');
            }
            push_json_string(&mut text, symbol.as_ref());
        }
        text.push_str("],\"weight\":");
        let weight = f64::from(weight);
        if weight.is_finite() {
            write!(text, "{weight}").expect("writing to a String does not fail");
        } else if weight.is_nan() {
            text.push_str("\"NaN\"");
        } else if weight > 0.0 {
            text.push_str("\"Infinity\"");
        } else {
            text.push_str("\"-Infinity\"");
        }
        text.push('}');
    }
    text.push(']');
    text
}

/// `value` as a JSON string. Only the quote, the backslash and the control
/// characters below U+0020 need escaping.
fn push_json_string(text: &mut String, value: &str) {
    text.push('"');
    let mut plain = 0;
    for (at, c) in value.char_indices() {
        if !matches!(c, '"' | '\\' | '\0'..='\u{1f}') {
            continue;
        }
        text.push_str(&value[plain..at]);
        match c {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            control => write!(text, "\\u{:04x}", u32::from(control))
                .expect("writing to a String does not fail"),
        }
        plain = at + c.len_utf8();
    }
    text.push_str(&value[plain..]);
    text.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_take_the_library_terms() {
        assert_eq!(
            LookupLimits::new(None, None).expect("no limits"),
            LookupLimits {
                results: None,
                seconds: None
            }
        );
        assert_eq!(
            LookupLimits::new(Some(3), Some(250.0)).expect("both limits"),
            LookupLimits {
                results: Some(3),
                seconds: Some(0.25)
            }
        );
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            let error = LookupLimits::new(None, Some(bad)).expect_err("a bad time limit");
            assert_eq!(error.code(), "invalid-argument", "{bad}");
        }
    }

    fn encoded(results: &[(&[&str], f32)]) -> String {
        let outputs: Vec<String> = results
            .iter()
            .map(|(symbols, _)| symbols.concat())
            .collect();
        encode(
            results
                .iter()
                .zip(&outputs)
                .map(|((symbols, weight), output)| (output.as_str(), *symbols, *weight)),
        )
    }

    // [spec:hfst:req:javascript.lookup/test]
    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn results_encode_as_json_objects() {
        let text = encoded(&[(&["c", "a", "t", "+N"], 0.1), (&[], 2.0)]);
        assert_eq!(
            text,
            "[{\"output\":\"cat+N\",\"symbols\":[\"c\",\"a\",\"t\",\"+N\"],\"weight\":0.10000000149011612},\
             {\"output\":\"\",\"symbols\":[],\"weight\":2}]"
        );
        assert_eq!(encoded(&[]), "[]");
    }

    #[test]
    fn strings_escape_quotes_backslashes_and_control_characters() {
        let mut text = String::new();
        push_json_string(&mut text, "a\"b\\c\td\u{1}é😀");
        assert_eq!(text, "\"a\\\"b\\\\c\\u0009d\\u0001é😀\"");
    }

    #[test]
    fn weights_that_are_not_finite_are_named() {
        let text = encoded(&[
            (&["a"], f32::INFINITY),
            (&["b"], f32::NEG_INFINITY),
            (&["c"], f32::NAN),
        ]);
        assert!(text.contains("\"weight\":\"Infinity\""), "{text}");
        assert!(text.contains("\"weight\":\"-Infinity\""), "{text}");
        assert!(text.contains("\"weight\":\"NaN\""), "{text}");
    }
}
