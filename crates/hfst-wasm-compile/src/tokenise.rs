//! Pmatch tokenisers: read from a container's bytes, or made of a
//! transducer as `hfst tokenize` makes one, and what `hfst pmatch` reports
//! with them.

use hfst::pmatch::PmatchContainer;
use hfst_embed::compile::{self as embed, PmatchResult};
use hfst_embed::tokenise;
use hfst_wasm_shared::GlueError;
use hfst_wasm_shared::tokenise::{format_named, format_options, tokenise_options, tokens_to_js};
use wasm_bindgen::prelude::*;

use crate::options::Options;
use crate::query_options as parse;
use crate::values::{Object, array};

/// A pmatch container, with the run state its matches write to. The
/// wrapper's `Tokeniser` class in `js/hfst/compile-tokeniser.ts` owns one.
#[wasm_bindgen]
pub struct Tokeniser {
    container: PmatchContainer,
}

impl Tokeniser {
    pub(crate) fn new(container: PmatchContainer) -> Tokeniser {
        Tokeniser { container }
    }
}

#[wasm_bindgen]
impl Tokeniser {
    // [spec:hfst:req:javascript.compile-module]
    /// The tokens of `text`, as the main module's tokeniser gives them.
    pub fn tokenise(&mut self, text: &str, options: &JsValue) -> Result<Vec<JsValue>, GlueError> {
        let options = tokenise_options(options)?;
        let tokens = tokenise::tokens(&mut self.container, text, &options);
        Ok(tokens_to_js(text, tokens))
    }

    // [spec:hfst:req:javascript.compile-module]
    /// What `hfst tokenise` prints for `text` in the format named `format`.
    pub fn format(
        &mut self,
        text: &str,
        format: &str,
        options: &JsValue,
    ) -> Result<String, GlueError> {
        let format = format_named(format)?;
        let options = format_options(options)?;
        Ok(tokenise::format(
            &mut self.container,
            text,
            format,
            &options,
        )?)
    }

    // [spec:hfst:req:javascript.compile-module]
    /// What `hfst pmatch` reports for `text` with the tokeniser's container,
    /// each piece of it and its matches. Each call runs on a container of
    /// its own over the same tables, so the settings it overrides do not
    /// outlast it.
    pub fn pmatch(&self, text: &str, options: &JsValue) -> Result<JsValue, GlueError> {
        let options = parse::pmatch(&Options::new("pmatch", options)?)?;
        let mut run = PmatchContainer::from_core(self.container.core());
        let pieces = embed::pmatch(&mut run, text, &options);
        Ok(array(pieces.into_iter().map(|(piece, found)| {
            let (marked, located) = match found {
                PmatchResult::Marked(marked) => (JsValue::from(marked), JsValue::NULL),
                PmatchResult::Located(locations) => (
                    JsValue::NULL,
                    array(locations.iter().map(|l| {
                        Object::new()
                            .with("start", l.start)
                            .with("length", l.length)
                            .with("output", l.output.as_str())
                            .with("tag", l.tag.as_str())
                            .with("weight", f64::from(l.weight))
                            .done()
                    })),
                ),
            };
            Object::new()
                .with("input", piece)
                .with("marked", marked)
                .with("located", located)
                .done()
        })))
    }
}

// [spec:hfst:req:javascript.compile-module]
/// The tokeniser in pmatch container bytes, as `hfst pmatch2fst` writes
/// them and `compilePmatch` returns them.
#[wasm_bindgen(js_name = readTokeniser)]
pub fn read_tokeniser(bytes: &[u8]) -> Result<Tokeniser, GlueError> {
    Ok(Tokeniser::new(tokenise::read_tokeniser(
        bytes,
        &crate::PACKAGE,
    )?))
}
