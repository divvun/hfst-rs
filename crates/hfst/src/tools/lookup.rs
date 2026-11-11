//! `hfst lookup` and `hfst flookup`: looking each line of input up in a
//! cascade of transducers, with what each tool reports for it.

use std::io::Write;

use crate::hfst_data_types::{HfstOneLevelPath, HfstOneLevelPaths};
use crate::hfst_lookup_format::{LookupInputFormat, parse_lookup_line};
use crate::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use crate::lookup_driver::{LookupCascade, LookupEngineOptions, LookupReporter};

/// One line looked up.
#[derive(Clone, Debug)]
pub struct LineLookup {
    /// The line as it was looked up: an Apertium line loses its escapes and
    /// its bracketed markup.
    pub line: String,
    pub input: HfstOneLevelPath,
    /// An Apertium line's bracketed markup, or `""`.
    pub markup: String,
    /// The line has a symbol no transducer of the cascade knows, so it was
    /// not looked up.
    pub outside_alphabet: bool,
    /// Every result, in weight order. The tools report those
    /// [`crate::hfst_lookup_format::reported_paths`] keeps.
    pub results: HfstOneLevelPaths,
    /// The lookup was infinitely ambiguous, and so cut short.
    pub infinite: bool,
}

/// Why a line could not be looked up.
#[derive(Debug)]
pub enum LookupError {
    /// The line could not be read as input, such as for bad UTF-8.
    Input(crate::error::Error),
    Lookup(crate::error::Error),
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LookupError::Input(e) | LookupError::Lookup(e) => write!(f, "{}", e),
        }
    }
}

/// A cascade ready to look lines up in, as a lookup tool runs it.
pub struct Lookup<'a> {
    cascade: LookupCascade<'a>,
    tokenizer: HfstStrings2FstTokenizer,
    engine: LookupEngineOptions,
    input_format: LookupInputFormat,
}

impl<'a> Lookup<'a> {
    // [spec:hfst:req:embed.tools-in-library]
    /// Look lines up in `cascade` with `engine`'s options, reading each line
    /// in `input_format`. The input is split into the cascade's
    /// multicharacter symbols, with `engine.epsilon_format` read as the
    /// epsilon.
    pub fn new(
        cascade: LookupCascade<'a>,
        engine: LookupEngineOptions,
        input_format: LookupInputFormat,
    ) -> crate::error::Result<Self> {
        let tokenizer =
            HfstStrings2FstTokenizer::new(cascade.multichar_symbols(), &engine.epsilon_format)?;
        Ok(Lookup {
            cascade,
            tokenizer,
            engine,
            input_format,
        })
    }

    pub fn cascade(&self) -> &LookupCascade<'a> {
        &self.cascade
    }

    pub fn engine(&self) -> &LookupEngineOptions {
        &self.engine
    }

    // [spec:hfst:def:hfst-lookup.process-stream-fn]
    // [spec:hfst:sem:hfst-lookup.process-stream-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Look `line`, without its newline, up. With `--xfst print-pairs` the
    /// engine writes the pairs to `out` itself, and `echo` hears what the
    /// tool echoes on its message stream.
    pub fn look_up(
        &self,
        line: &str,
        reporter: &dyn LookupReporter,
        out: &mut dyn Write,
        echo: &mut dyn Write,
    ) -> Result<LineLookup, LookupError> {
        let mut line = line.to_string();
        let mut markup = String::new();
        let mut outside_alphabet = false;
        let input = parse_lookup_line(
            &mut line,
            &self.tokenizer,
            &mut markup,
            &mut outside_alphabet,
            self.cascade.only_optimized_lookup(),
            self.input_format,
        )
        .map_err(LookupError::Input)?;
        let (results, infinite) = self
            .cascade
            .perform_lookups(&input, outside_alphabet, &self.engine, reporter, out, echo)
            .map_err(LookupError::Lookup)?;
        Ok(LineLookup {
            line,
            input,
            markup,
            outside_alphabet,
            results,
            infinite,
        })
    }
}
