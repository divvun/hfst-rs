//! Pmatch tokenisers read from bytes, the tokens they find, and the text
//! `hfst tokenise` prints with them.

use std::sync::Arc;

use hfst::pmatch::PmatchContainer;
use hfst::pmatch_core::PmatchCore;
use hfst_embed::tokenise::{self, FORMATS, FormatOptions, Selection, TokeniseOptions};

use crate::PACKAGE;
use crate::args;
use crate::error::{GlueError, invalid};

// [spec:hfst:req:swift.tokenise]
#[derive(uniffi::Record)]
pub struct Reading {
    pub analysis: String,
    pub weight: f32,
}

// [spec:hfst:req:swift.tokenise]
/// A token, its offsets in UTF-8 bytes of the text, which the Swift layer
/// turns into a range of the caller's string.
#[derive(uniffi::Record)]
pub struct Token {
    pub surface: String,
    pub start: u64,
    pub end: u64,
    pub analysed: bool,
    pub readings: Vec<Reading>,
}

impl From<tokenise::Token> for Token {
    fn from(token: tokenise::Token) -> Self {
        Token {
            surface: token.surface,
            start: token.start as u64,
            end: token.end as u64,
            analysed: token.analysed,
            readings: token
                .readings
                .into_iter()
                .map(|reading| Reading {
                    analysis: reading.analysis,
                    weight: reading.weight,
                })
                .collect(),
        }
    }
}

// [spec:hfst:req:swift.tokenise]
/// The settings `tokenise` and `format` share, each `None` where the caller
/// left the tool's default.
#[derive(uniffi::Record)]
pub struct SelectionSettings {
    pub dedupe: Option<bool>,
    pub max_weight_classes: Option<i64>,
    pub beam: Option<f32>,
    pub time_limit: Option<f64>,
    pub tokenize_multichar: Option<bool>,
    pub print_all: Option<bool>,
}

impl SelectionSettings {
    // [spec:hfst:req:swift.errors]
    /// The library's settings, once each value is one `hfst tokenise` takes.
    fn checked(&self) -> Result<Selection, GlueError> {
        let max_weight_classes = match self.max_weight_classes {
            None => None,
            Some(n) if n < 1 => {
                return Err(invalid(format!(
                    "maxWeightClasses is a whole number of at least 1, not {n}"
                )));
            }
            // A count past what the library holds keeps every class, as
            // its default does.
            Some(n) => Some(i32::try_from(n).unwrap_or(i32::MAX)),
        };
        if let Some(beam) = self.beam
            && (beam.is_nan() || beam < 0.0)
        {
            return Err(invalid(format!(
                "beam is a number of at least 0, not {beam}"
            )));
        }
        Ok(Selection {
            dedupe: self.dedupe,
            max_weight_classes,
            beam: self.beam,
            time_limit: args::seconds("timeLimit", self.time_limit)?,
            tokenize_multichar: self.tokenize_multichar,
            print_all: self.print_all,
        })
    }
}

// [spec:hfst:req:swift.tokenise]
#[derive(uniffi::Record)]
pub struct TokeniseSettings {
    pub selection: SelectionSettings,
    pub weight_cutoff: Option<f32>,
}

// [spec:hfst:req:swift.tokenise]
#[derive(uniffi::Record)]
pub struct FormatSettings {
    pub selection: SelectionSettings,
    pub print_weights: Option<bool>,
    pub hack_uncompose: Option<bool>,
    pub newline: Option<bool>,
    pub keep_newline: Option<bool>,
    pub superblanks: Option<bool>,
}

// [spec:hfst:req:swift.tokenise]
// [spec:hfst:req:swift.threads]
/// A tokeniser read from a pmatch container. It holds the container's
/// tables alone, and each call runs over them with a run state of its own,
/// so threads may tokenise with one at once.
#[derive(uniffi::Object)]
pub struct Tokeniser {
    core: Arc<PmatchCore>,
}

impl Tokeniser {
    pub fn from_core(core: Arc<PmatchCore>) -> Self {
        Tokeniser { core }
    }

    /// A run of its own over the container's tables.
    pub fn run(&self) -> PmatchContainer {
        PmatchContainer::from_core(Arc::clone(&self.core))
    }
}

#[uniffi::export]
impl Tokeniser {
    // [spec:hfst:req:swift.tokenise]
    /// The tokens `@divvun/hfst`'s `tokenise` finds in `text`, with their
    /// offsets in UTF-8 bytes.
    pub fn tokenise(
        &self,
        text: String,
        settings: TokeniseSettings,
    ) -> Result<Vec<Token>, GlueError> {
        if let Some(cutoff) = settings.weight_cutoff
            && cutoff.is_nan()
        {
            return Err(invalid("weightCutoff is a number, not NaN"));
        }
        let options = TokeniseOptions {
            selection: settings.selection.checked()?,
            weight_cutoff: settings.weight_cutoff,
        };
        let found = tokenise::tokens(&mut self.run(), &text, &options);
        Ok(found.into_iter().map(Token::from).collect())
    }

    // [spec:hfst:req:swift.tokenise]
    /// What `hfst tokenise` prints in `format` for `text`.
    pub fn format(
        &self,
        text: String,
        format: String,
        settings: FormatSettings,
    ) -> Result<String, GlueError> {
        let Some(output) = tokenise::output_format(&format) else {
            let names: Vec<&str> = FORMATS.iter().map(|(name, _)| *name).collect();
            return Err(invalid(format!(
                "there is no tokenise format called {format}; the formats are {}",
                names.join(", ")
            )));
        };
        let options = FormatOptions {
            selection: settings.selection.checked()?,
            print_weights: settings.print_weights,
            hack_uncompose: settings.hack_uncompose,
            newline: settings.newline,
            keep_newline: settings.keep_newline,
            superblanks: settings.superblanks,
        };
        Ok(tokenise::format(&mut self.run(), &text, output, &options)?)
    }
}

// [spec:hfst:req:swift.load]
/// The tokeniser in a pmatch container's bytes, as `@divvun/hfst`'s
/// `readTokeniser` reads it.
#[uniffi::export]
pub fn read_tokeniser(bytes: Vec<u8>) -> Result<Arc<Tokeniser>, GlueError> {
    let container = tokenise::read_tokeniser(&bytes, &PACKAGE)?;
    Ok(Arc::new(Tokeniser::from_core(container.core())))
}
