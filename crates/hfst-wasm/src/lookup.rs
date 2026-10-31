//! Lookup: the limits the wrapper passes in, the results it gets back, and
//! lookup on the optimized-lookup transducers the package holds.

use std::fmt::Write as _;

use hfst::hfst_data_types::HfstOneLevelPaths;
use hfst::hfst_flag_diacritics::FdOperation;
use hfst::hfst_symbol_defs::is_epsilon;
use hfst::lookup_state::LookupState;
use wasm_bindgen::prelude::*;

use crate::error::GlueError;
use crate::transducer::{Machine, Transducer};

/// The limits of one lookup, in the terms the library's lookups take them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LookupLimits {
    /// The most results, or -1 for no limit.
    results: isize,
    /// Seconds of search, or 0.0 for no limit.
    seconds: f64,
}

impl LookupLimits {
    /// The limits for a result count and a time in milliseconds, either left
    /// out for no limit. The wrapper caps the count at `u32::MAX`; a cap the
    /// target cannot hold is no cap, since nothing finds that many results.
    fn new(limit: Option<u32>, time_limit_ms: Option<f64>) -> Result<LookupLimits, GlueError> {
        let results = limit.map_or(-1, |limit| isize::try_from(limit).unwrap_or(isize::MAX));
        let seconds = match time_limit_ms {
            None => 0.0,
            Some(ms) if ms.is_finite() && ms >= 0.0 => ms / 1000.0,
            Some(ms) => {
                return Err(GlueError::new(
                    "invalid-argument",
                    format!("a time limit is a finite number of milliseconds, 0 or more, not {ms}"),
                ));
            }
        };
        Ok(LookupLimits { results, seconds })
    }
}

// [spec:hfst:req:javascript.lookup]
/// One result of a lookup, as the wrapper hands it to JavaScript.
#[derive(Clone, Debug, PartialEq)]
struct LookupResult {
    output: String,
    /// The output symbols of the path, without its flag diacritics and
    /// epsilons, which `hfst lookup` does not print either.
    symbols: Vec<String>,
    weight: f32,
}

// [spec:hfst:req:javascript.lookup]
/// The results for the paths a lookup found: one for each path, ordered by
/// weight, then by output, then by symbols. Two paths that differ only in
/// their flags or epsilons stay two results, as `hfst lookup` prints both.
fn lookup_results(paths: HfstOneLevelPaths) -> Vec<LookupResult> {
    let mut results: Vec<LookupResult> = paths
        .into_iter()
        .map(|path| {
            let symbols: Vec<String> = path
                .second
                .iter()
                .filter(|symbol| {
                    !symbol.is_empty() && !is_epsilon(symbol) && !FdOperation::is_diacritic(symbol)
                })
                .map(|symbol| symbol.to_string())
                .collect();
            LookupResult {
                output: symbols.concat(),
                symbols,
                weight: path.first,
            }
        })
        .collect();
    results.sort_by(|a, b| {
        a.weight
            .total_cmp(&b.weight)
            .then_with(|| a.output.cmp(&b.output))
            .then_with(|| a.symbols.cmp(&b.symbols))
    });
    results
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
pub(crate) fn encode<'a, S: AsRef<str> + 'a>(
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

#[wasm_bindgen]
impl Transducer {
    // [spec:hfst:req:javascript.lookup]
    /// Every result the library's flag-aware lookup finds for `input` within
    /// the limits, as `lookup_results` orders them, in the JSON text `encode`
    /// writes.
    pub fn lookup(
        &self,
        input: &str,
        limit: Option<u32>,
        time_limit_ms: Option<f64>,
    ) -> Result<String, GlueError> {
        let limits = LookupLimits::new(limit, time_limit_ms)?;
        let results = self.lookup_results(input, limits);
        Ok(encode(results.iter().map(|result| {
            (result.output.as_str(), &result.symbols[..], result.weight)
        })))
    }
}

impl Transducer {
    /// The results of looking `input` up within `limits`. Each lookup makes
    /// its own run state: the library's run state borrows the machine it runs
    /// on, so one kept beside that machine would make this object refer to
    /// itself. Making one costs a reference count and empty tapes; the tables
    /// are borrowed, never copied.
    fn lookup_results(&self, input: &str, limits: LookupLimits) -> Vec<LookupResult> {
        let paths = match &self.machine {
            Machine::Weighted(machine) => {
                LookupState::new(machine).lookup_fd(input, limits.results, limits.seconds)
            }
            Machine::Unweighted(machine) => {
                LookupState::new(machine).lookup_fd(input, limits.results, limits.seconds)
            }
        };
        lookup_results(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::hfst_data_types::{HfstOneLevelPath, Symbol};

    fn path(weight: f32, symbols: &[&str]) -> HfstOneLevelPath {
        HfstOneLevelPath {
            first: weight,
            second: symbols.iter().map(|s| Symbol::from(*s)).collect(),
        }
    }

    fn summary(results: &[LookupResult]) -> Vec<String> {
        results
            .iter()
            .map(|r| format!("{} {} {}", r.weight, r.output, r.symbols.join(" ")))
            .collect()
    }

    #[test]
    fn flags_and_epsilons_leave_the_symbols() {
        let paths = [path(
            0.5,
            &[
                "@P.CASE.NOM@",
                "t",
                "",
                "@_EPSILON_SYMBOL_@",
                "o",
                "@R.CASE.NOM@",
                "+N",
            ],
        )]
        .into_iter()
        .collect();
        let results = lookup_results(paths);
        assert_eq!(summary(&results), ["0.5 to+N t o +N"]);
    }

    #[test]
    fn results_order_by_weight_output_then_symbols() {
        let paths = [
            path(1.0, &["xy"]),
            path(1.0, &["x", "y"]),
            path(1.0, &["b"]),
            path(0.25, &["c"]),
            path(0.25, &["a", "b"]),
            path(2.0, &["a"]),
        ]
        .into_iter()
        .collect();
        let results = lookup_results(paths);
        assert_eq!(
            summary(&results),
            [
                "0.25 ab a b",
                "0.25 c c",
                "1 b b",
                "1 xy x y",
                "1 xy xy",
                "2 a a"
            ]
        );
    }

    #[test]
    fn flag_only_differences_stay_two_results() {
        let paths = [path(0.0, &["@U.X.A@", "a"]), path(0.0, &["@U.X.B@", "a"])]
            .into_iter()
            .collect();
        assert_eq!(summary(&lookup_results(paths)), ["0 a a", "0 a a"]);
    }

    #[test]
    fn limits_take_the_library_terms() {
        assert_eq!(
            LookupLimits::new(None, None).expect("no limits"),
            LookupLimits {
                results: -1,
                seconds: 0.0
            }
        );
        assert_eq!(
            LookupLimits::new(Some(3), Some(250.0)).expect("both limits"),
            LookupLimits {
                results: 3,
                seconds: 0.25
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
