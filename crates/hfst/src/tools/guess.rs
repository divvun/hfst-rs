//! `hfst guess`: guessing the analyses of unknown words with a guesser that
//! `hfst guessify` built, and their model forms with its generator.

use std::borrow::Cow;
use std::io::Write;

use crate::generate_model_forms::StringVectorVector;
use crate::generate_model_forms::{
    compile_generator_from_guesser, get_alphabet_string_tokenizer, get_guesses, get_paradigms,
    is_guesser, model_forms,
};
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::HfstTransducer;
use crate::transducer::Transducer;

/// What `hfst guess`'s options decide.
#[derive(Clone, Copy, Debug)]
pub struct GuessOptions {
    pub max_number_of_guesses: usize,
    /// Model forms generated for each guess.
    pub max_number_of_forms: usize,
    /// Only model forms whose weight is within this of the best.
    pub generate_threshold: f32,
}

impl Default for GuessOptions {
    fn default() -> Self {
        GuessOptions {
            max_number_of_guesses: 5,
            max_number_of_forms: 2,
            generate_threshold: 50.0,
        }
    }
}

/// What the tool prints for a word: its guessed analyses, or with model
/// forms, a paradigm for each guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuessReport {
    /// Each guessed analysis.
    Guesses(Vec<String>),
    /// For each guess, the word, the analysis and the model forms generated
    /// for it, separated by tabs.
    Paradigms(Vec<String>),
}

/// Why a guesser could not be used.
#[derive(Debug)]
pub enum GuessError {
    /// The transducer was not built by `hfst guessify`.
    NotAGuesser,
    /// The model form file did not read.
    ModelForms(crate::error::Error),
    Operation(crate::error::Error),
}

impl std::fmt::Display for GuessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuessError::NotAGuesser => write!(f, "The transducer is not a guesser."),
            GuessError::ModelForms(e) | GuessError::Operation(e) => write!(f, "{}", e),
        }
    }
}

impl From<crate::error::Error> for GuessError {
    fn from(e: crate::error::Error) -> Self {
        GuessError::Operation(e)
    }
}

/// A guesser ready to guess, with the generator and model forms it needs to
/// generate model forms.
pub struct Guesser<'a> {
    guesser: Cow<'a, HfstTransducer<Transducer>>,
    generator: Option<Cow<'a, HfstTransducer<Transducer>>>,
    tokenizer: HfstTokenizer,
    model_forms: Option<StringVectorVector>,
}

/// What [`Guesser::new`] tells its caller as it goes.
pub enum GuessProgress {
    /// No generator came with the guesser, so it is compiled from it.
    CompilingGenerator,
    ReadingModelForms,
}

impl<'a> Guesser<'a> {
    // [spec:hfst:def:hfst-guess.main-fn]
    // [spec:hfst:sem:hfst-guess.main-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Guess with `guesser`. With `model_forms`, the text of a model form
    /// file, model forms are generated with `generator`, or with one
    /// compiled from the guesser when there is none.
    pub fn new(
        guesser: Cow<'a, HfstTransducer<Transducer>>,
        generator: Option<Cow<'a, HfstTransducer<Transducer>>>,
        model_form_text: Option<&str>,
        progress: &mut dyn FnMut(GuessProgress),
    ) -> Result<Self, GuessError> {
        if !is_guesser(&guesser) {
            return Err(GuessError::NotAGuesser);
        }
        let generator = match (model_form_text, generator) {
            (None, _) => None,
            (Some(_), Some(generator)) => Some(generator),
            (Some(_), None) => {
                progress(GuessProgress::CompilingGenerator);
                Some(Cow::Owned(compile_generator_from_guesser(&guesser)?))
            }
        };
        let tokenizer = get_alphabet_string_tokenizer(&guesser)?;
        let model_forms = match model_form_text {
            Some(text) => {
                progress(GuessProgress::ReadingModelForms);
                Some(model_forms(text, &tokenizer).map_err(GuessError::ModelForms)?)
            }
            None => None,
        };
        Ok(Guesser {
            guesser,
            generator,
            tokenizer,
            model_forms,
        })
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// What the tool reports for `word`.
    pub fn guess(&self, word: &str, options: &GuessOptions) -> crate::error::Result<GuessReport> {
        let mut guesses = get_guesses(
            word,
            &self.guesser,
            options.max_number_of_guesses,
            &self.tokenizer,
        )?;
        match (&self.model_forms, &self.generator) {
            (Some(model_forms), Some(generator)) => {
                let paradigms = get_paradigms(
                    word,
                    &guesses,
                    generator,
                    model_forms,
                    options.max_number_of_forms,
                    options.generate_threshold,
                )?;
                Ok(GuessReport::Paradigms(
                    paradigms.iter().map(|p| p.concat()).collect(),
                ))
            }
            (None, _) | (Some(_), None) => Ok(GuessReport::Guesses(
                guesses
                    .iter_mut()
                    .map(|guess| {
                        // The guesser reads its input reversed, so its
                        // analyses come out reversed too.
                        guess.reverse();
                        guess.concat()
                    })
                    .collect(),
            )),
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// The lines `hfst guess` prints for `word` and its `report`, the blank
/// line after them included.
pub fn write_guess(out: &mut dyn Write, word: &str, report: &GuessReport) -> std::io::Result<()> {
    match report {
        GuessReport::Paradigms(paradigms) => {
            for paradigm in paradigms {
                writeln!(out, "{}", paradigm)?;
            }
        }
        GuessReport::Guesses(guesses) => {
            for guess in guesses {
                writeln!(out, "{}\t{}", word, guess)?;
            }
        }
    }
    writeln!(out)
}
