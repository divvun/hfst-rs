//! What tokenising takes from JavaScript and gives back to it: the options
//! the wrapper passes as objects, and the tokens as plain objects.
//!
//! The public helpers are `#[inline]`, so each module compiles them into the
//! method that calls them, and sharing them costs the main module no size.

use hfst::pmatch_tokenize::OutputFormat;
use hfst_embed::Code;
use hfst_embed::tokenise::{
    self, FormatOptions, Reading, Selection, Token, TokeniseOptions, output_format,
};
use wasm_bindgen::prelude::*;

use crate::error::{GlueError, set};
use crate::options::Options;

// [spec:hfst:req:javascript.tokenise]
/// The output format `hfst tokenise` calls `name`.
#[inline]
pub fn format_named(name: &str) -> Result<OutputFormat, GlueError> {
    output_format(name).ok_or_else(|| {
        GlueError::new(
            Code::InvalidArgument,
            format!("there is no tokenise format called {name}"),
        )
    })
}

// [spec:hfst:req:javascript.tokenise]
/// `tokens`, found in `text`, as plain objects with the fields of the
/// wrapper's `Token`, their offsets in UTF-16 units as JavaScript string
/// indices count.
#[inline]
pub fn tokens_to_js(text: &str, mut tokens: Vec<Token>) -> Vec<JsValue> {
    tokenise::recount(text, &mut tokens, |stretch| stretch.encode_utf16().count());
    tokens.iter().map(token_to_js).collect()
}

fn token_to_js(token: &Token) -> JsValue {
    let object = js_sys::Object::new();
    set(&object, "surface", &JsValue::from_str(&token.surface));
    set(&object, "start", &JsValue::from_f64(token.start as f64));
    set(&object, "end", &JsValue::from_f64(token.end as f64));
    set(&object, "analysed", &JsValue::from_bool(token.analysed));
    let readings: js_sys::Array = token.readings.iter().map(reading_to_js).collect();
    set(&object, "readings", &readings);
    object.into()
}

fn reading_to_js(reading: &Reading) -> JsValue {
    let object = js_sys::Object::new();
    set(&object, "analysis", &JsValue::from_str(&reading.analysis));
    set(
        &object,
        "weight",
        &JsValue::from_f64(f64::from(reading.weight)),
    );
    object.into()
}

/// The options `tokenise` and `format` share, as JavaScript numbers and
/// booleans, each `None` where the caller gave none.
#[derive(Clone, Copy, Debug, Default)]
struct Given {
    dedupe: Option<bool>,
    max_weight_classes: Option<f64>,
    beam: Option<f64>,
    time_limit_ms: Option<f64>,
    tokenize_multichar: Option<bool>,
    print_all: Option<bool>,
}

impl Given {
    fn read(options: &Options<'_>) -> Result<Self, GlueError> {
        Ok(Given {
            dedupe: options.flag("dedupe")?,
            max_weight_classes: options.number("maxWeightClasses")?,
            beam: options.number("beam")?,
            time_limit_ms: options.number("timeLimit")?,
            tokenize_multichar: options.flag("tokenizeMultichar")?,
            print_all: options.flag("printAll")?,
        })
    }

    /// The selection these stand for, checked as `hfst tokenise` checks the
    /// option values it parses.
    fn selection(&self) -> Result<Selection, GlueError> {
        Ok(Selection {
            dedupe: self.dedupe,
            max_weight_classes: self.max_weight_classes.map(weight_classes).transpose()?,
            beam: self
                .beam
                .map(|beam| non_negative("beam", beam).map(|beam| beam as f32))
                .transpose()?,
            time_limit: self
                .time_limit_ms
                .map(|ms| non_negative("timeLimit", ms).map(|ms| ms / 1000.0))
                .transpose()?,
            tokenize_multichar: self.tokenize_multichar,
            print_all: self.print_all,
        })
    }
}

// [spec:hfst:req:javascript.tokenise]
/// The options of `tokenise`, from the wrapper's object.
#[inline]
pub fn tokenise_options(options: &JsValue) -> Result<TokeniseOptions, GlueError> {
    let options = Options::new(options)?;
    let selection = Given::read(&options)?.selection()?;
    let weight_cutoff = match options.number("weightCutoff")? {
        Some(cutoff) if cutoff.is_nan() => {
            return Err(GlueError::new(
                Code::InvalidArgument,
                "weightCutoff is a number, and NaN is not one",
            ));
        }
        cutoff => cutoff.map(|cutoff| cutoff as f32),
    };
    Ok(TokeniseOptions {
        selection,
        weight_cutoff,
    })
}

// [spec:hfst:req:javascript.tokenise]
/// The options of `format`, from the wrapper's object.
#[inline]
pub fn format_options(options: &JsValue) -> Result<FormatOptions, GlueError> {
    let options = Options::new(options)?;
    Ok(FormatOptions {
        selection: Given::read(&options)?.selection()?,
        print_weights: options.flag("printWeights")?,
        hack_uncompose: options.flag("hackUncompose")?,
        newline: options.flag("newline")?,
        keep_newline: options.flag("keepNewline")?,
        superblanks: options.flag("superblanks")?,
    })
}

/// `--weight-classes` takes a whole number of at least 1. Infinity and any
/// count past `i32::MAX` keep every class, as the setting's default does.
fn weight_classes(classes: f64) -> Result<i32, GlueError> {
    if classes >= 1.0 && (classes.fract() == 0.0 || classes.is_infinite()) {
        return Ok(classes.min(f64::from(i32::MAX)) as i32);
    }
    Err(GlueError::new(
        Code::InvalidArgument,
        format!("maxWeightClasses is a whole number of at least 1, not {classes}"),
    ))
}

fn non_negative(name: &str, value: f64) -> Result<f64, GlueError> {
    if value >= 0.0 {
        return Ok(value);
    }
    Err(GlueError::new(
        Code::InvalidArgument,
        format!("{name} is a number of at least 0, not {value}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_values_are_checked_like_the_command_line() {
        let code = |given: Given| given.selection().map(|_| ()).map_err(|error| error.code());
        for classes in [0.0, 1.5, -3.0, f64::NAN] {
            let given = Given {
                max_weight_classes: Some(classes),
                ..Given::default()
            };
            assert_eq!(code(given), Err("invalid-argument"), "{classes}");
        }
        for (beam, time_limit_ms) in [
            (Some(-0.5), None),
            (None, Some(-1.0)),
            (Some(f64::NAN), None),
        ] {
            let given = Given {
                beam,
                time_limit_ms,
                ..Given::default()
            };
            assert_eq!(
                code(given),
                Err("invalid-argument"),
                "{beam:?} {time_limit_ms:?}"
            );
        }
        assert_eq!(weight_classes(f64::INFINITY).ok(), Some(i32::MAX));
        assert_eq!(weight_classes(1e12).ok(), Some(i32::MAX));
        let milliseconds = Given {
            time_limit_ms: Some(1500.0),
            ..Given::default()
        };
        assert_eq!(
            milliseconds.selection().ok().and_then(|s| s.time_limit),
            Some(1.5)
        );
    }

    #[test]
    fn the_wrapper_lists_the_command_line_format_names() {
        let wrapper = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/tokenise.ts"
        ))
        .expect("the wrapper is in the repository");
        for (name, _) in tokenise::FORMATS {
            assert!(
                wrapper.contains(&format!("\"{name}\"")),
                "the wrapper lists {name}"
            );
        }
    }
}
