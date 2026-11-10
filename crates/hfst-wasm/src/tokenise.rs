//! Pmatch tokenisers loaded from bytes, and the options the wrapper passes
//! them as JavaScript objects.

use hfst::pmatch::PmatchContainer;
use hfst_embed::Code;
use hfst_embed::tokenise::{
    self, FormatOptions, Reading, Selection, Token, TokeniseOptions, output_format,
};
use wasm_bindgen::prelude::*;

use crate::error::GlueError;

/// A pmatch container, with the run state its matches write to. The
/// wrapper's `Tokeniser` class in `js/hfst/tokenise.ts` owns one.
#[wasm_bindgen]
pub struct Tokeniser {
    container: PmatchContainer,
}

#[wasm_bindgen]
impl Tokeniser {
    // [spec:hfst:req:javascript.tokenise]
    /// The tokens of `text`, as plain objects with the fields of the
    /// wrapper's `Token`, their offsets in UTF-16 units as JavaScript string
    /// indices count.
    pub fn tokenise(&mut self, text: &str, options: &JsValue) -> Result<Vec<JsValue>, GlueError> {
        let options = tokenise_options(&Options::new(options)?)?;
        let mut tokens = tokenise::tokens(&mut self.container, text, &options);
        tokenise::recount(text, &mut tokens, |stretch| stretch.encode_utf16().count());
        Ok(tokens.iter().map(token_to_js).collect())
    }

    // [spec:hfst:req:javascript.tokenise]
    /// What `hfst tokenise` prints for `text` on its standard input, in the
    /// format named `format`.
    pub fn format(
        &mut self,
        text: &str,
        format: &str,
        options: &JsValue,
    ) -> Result<String, GlueError> {
        let format = output_format(format).ok_or_else(|| {
            GlueError::new(
                Code::InvalidArgument,
                format!("there is no tokenise format called {format}"),
            )
        })?;
        let options = format_options(&Options::new(options)?)?;
        Ok(tokenise::format(
            &mut self.container,
            text,
            format,
            &options,
        )?)
    }
}

// [spec:hfst:req:javascript.tokenise]
/// The tokeniser in pmatch container bytes, as `hfst pmatch2fst` writes them.
#[wasm_bindgen(js_name = readTokeniser)]
pub fn read_tokeniser(bytes: &[u8]) -> Result<Tokeniser, GlueError> {
    Ok(Tokeniser {
        container: tokenise::read_tokeniser(bytes, &crate::PACKAGE)?,
    })
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

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(object, &JsValue::from_str(key), value)
        .expect("a fresh plain object takes any property");
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

fn tokenise_options(options: &Options<'_>) -> Result<TokeniseOptions, GlueError> {
    let selection = Given::read(options)?.selection()?;
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

fn format_options(options: &Options<'_>) -> Result<FormatOptions, GlueError> {
    Ok(FormatOptions {
        selection: Given::read(options)?.selection()?,
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

/// An options object from the wrapper. `undefined` and `null`, for the
/// object or any field, mean the option was not given.
struct Options<'a>(Option<&'a JsValue>);

impl<'a> Options<'a> {
    fn new(options: &'a JsValue) -> Result<Self, GlueError> {
        if options.is_undefined() || options.is_null() {
            return Ok(Options(None));
        }
        if !options.is_object() {
            return Err(GlueError::new(
                Code::InvalidArgument,
                "the options are an object",
            ));
        }
        Ok(Options(Some(options)))
    }

    fn get(&self, name: &str) -> Result<Option<JsValue>, GlueError> {
        let Some(options) = self.0 else {
            return Ok(None);
        };
        let value = js_sys::Reflect::get(options, &JsValue::from_str(name)).map_err(|_| {
            GlueError::new(
                Code::InvalidArgument,
                format!("the option {name} cannot be read"),
            )
        })?;
        Ok((!value.is_undefined() && !value.is_null()).then_some(value))
    }

    fn flag(&self, name: &str) -> Result<Option<bool>, GlueError> {
        self.get(name)?
            .map(|value| {
                value.as_bool().ok_or_else(|| {
                    GlueError::new(
                        Code::InvalidArgument,
                        format!("the option {name} is a boolean"),
                    )
                })
            })
            .transpose()
    }

    fn number(&self, name: &str) -> Result<Option<f64>, GlueError> {
        self.get(name)?
            .map(|value| {
                value.as_f64().ok_or_else(|| {
                    GlueError::new(
                        Code::InvalidArgument,
                        format!("the option {name} is a number"),
                    )
                })
            })
            .transpose()
    }
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
