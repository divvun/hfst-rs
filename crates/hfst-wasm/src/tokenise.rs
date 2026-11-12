//! Pmatch tokenisers loaded from bytes.

use hfst::pmatch::PmatchContainer;
use hfst_embed::tokenise;
use hfst_wasm_shared::GlueError;
use hfst_wasm_shared::tokenise::{format_named, format_options, tokenise_options, tokens_to_js};
use wasm_bindgen::prelude::*;

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
        let options = tokenise_options(options)?;
        let tokens = tokenise::tokens(&mut self.container, text, &options);
        Ok(tokens_to_js(text, tokens))
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
        let format = format_named(format)?;
        let options = format_options(options)?;
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
