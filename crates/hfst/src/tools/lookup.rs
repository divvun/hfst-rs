//! `hfst lookup` and `hfst flookup`: looking each line of input up in a
//! cascade of transducers, with what each tool reports for it.

use std::io::Write;

use crate::hfst_data_types::{HfstOneLevelPath, HfstOneLevelPaths, ImplementationType};
use crate::hfst_lookup_format::{LookupInputFormat, parse_lookup_line};
use crate::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use crate::hfst_transducer::AnyTransducer;
use crate::lookup_driver::{
    FlagPolicy, LookupCascade, LookupEngineOptions, LookupReporter, is_optimized_lookup_type,
};

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

// [spec:hfst:req:embed.tools-in-library]
/// Why a lookup tool refuses its transducers. Each `Display`s as the tool's
/// message.
#[derive(Debug)]
pub enum CascadeError {
    /// `hfst flookup` looks up in each transducer inverted, which it does to
    /// optimized lookup only when told to with `--force-ol`.
    InvertsOptimizedLookup,
    PairsOnOptimizedLookup,
    /// `hfst lookup --xfst obey-flags` off, on optimized lookup.
    IgnoresFlagsOnOptimizedLookup,
    Library(crate::error::Error),
}

impl std::fmt::Display for CascadeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CascadeError::InvertsOptimizedLookup => f.write_str(
                "lookup not supported for optimized lookup transducers: convert to openfst format,\n\
                 invert, and convert back to optimized lookup format or specify --force-ol\n",
            ),
            CascadeError::PairsOnOptimizedLookup => {
                f.write_str("pair printing not supported on optimized lookup transducers")
            }
            CascadeError::IgnoresFlagsOnOptimizedLookup => {
                f.write_str("not obeying flags not supported on optimized lookup transducers")
            }
            CascadeError::Library(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CascadeError {}

impl From<crate::error::Error> for CascadeError {
    fn from(e: crate::error::Error) -> Self {
        CascadeError::Library(e)
    }
}

/// `t` inverted. Optimized lookup has no inversion of its own, so it goes to
/// tropical OpenFst, is inverted there and comes back.
fn inverted(t: AnyTransducer) -> crate::error::Result<AnyTransducer> {
    match t {
        AnyTransducer::Tropical(mut t) => {
            t.invert()?;
            Ok(AnyTransducer::Tropical(t))
        }
        #[cfg(feature = "foma")]
        AnyTransducer::Foma(mut t) => {
            t.invert()?;
            Ok(AnyTransducer::Foma(t))
        }
        ol @ (AnyTransducer::OlW(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_)) => {
            let ty = ol.get_type();
            let mut tropical = ol.convert(ImplementationType::TROPICAL_OPENFST_TYPE, "")?;
            if let AnyTransducer::Tropical(t) = &mut tropical {
                t.invert()?;
            }
            tropical.convert(ty, "")
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// The cascade `hfst flookup` looks lines up in, built a transducer at a
/// time: each inverted unless the tool's `--invert` (`-R`) asks for it as it
/// is, so a line is looked up on the transducer's output side.
pub struct FlookupCascade<'a> {
    cascade: LookupCascade<'a>,
    as_is: bool,
    force_ol: bool,
}

impl<'a> FlookupCascade<'a> {
    /// A cascade that looks up in each transducer `as_is`, or inverted; with
    /// `force_ol`, as `--force-ol`, an optimized-lookup transducer is
    /// inverted too, through tropical OpenFst, where it is otherwise refused.
    pub fn new(as_is: bool, force_ol: bool) -> Self {
        FlookupCascade {
            cascade: LookupCascade::new(),
            as_is,
            force_ol,
        }
    }

    /// Add the next transducer of the stream, inverted or as it is, naming
    /// it `fallback_name` in its progress line when it has no name.
    pub fn push(
        &mut self,
        transducer: AnyTransducer,
        fallback_name: &str,
        reporter: &dyn LookupReporter,
    ) -> Result<(), CascadeError> {
        self.refuse(&transducer)?;
        self.cascade
            .begin_transducer(&transducer, fallback_name, reporter);
        let transducer = if self.as_is {
            transducer
        } else {
            inverted(transducer)?
        };
        Ok(self.cascade.push_transducer(transducer, reporter)?)
    }

    /// [`Self::push`] for a transducer the caller keeps, which is copied
    /// only to be inverted.
    pub fn push_borrowed(
        &mut self,
        transducer: &'a AnyTransducer,
        fallback_name: &str,
        reporter: &dyn LookupReporter,
    ) -> Result<(), CascadeError> {
        self.refuse(transducer)?;
        self.cascade
            .begin_transducer(transducer, fallback_name, reporter);
        if self.as_is {
            Ok(self.cascade.push_borrowed(transducer, reporter)?)
        } else {
            let transducer = inverted(transducer.copy()?)?;
            Ok(self.cascade.push_transducer(transducer, reporter)?)
        }
    }

    fn refuse(&self, transducer: &AnyTransducer) -> Result<(), CascadeError> {
        if is_optimized_lookup_type(transducer.get_type()) && !self.as_is && !self.force_ol {
            Err(CascadeError::InvertsOptimizedLookup)
        } else {
            Ok(())
        }
    }

    /// The cascade, ready for [`Lookup::new`], once every transducer of the
    /// stream is in. `print_pairs` is the tool's `--xfst print-pairs`.
    pub fn finish(self, print_pairs: bool) -> Result<LookupCascade<'a>, CascadeError> {
        if print_pairs && is_optimized_lookup_type(self.cascade.first_type()) {
            return Err(CascadeError::PairsOnOptimizedLookup);
        }
        Ok(self.cascade)
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
    /// epsilon. Optimized lookup obeys flags in the lookup or not at all, so
    /// a lookup that obeys them there is refused it when it does not.
    pub fn new(
        cascade: LookupCascade<'a>,
        engine: LookupEngineOptions,
        input_format: LookupInputFormat,
    ) -> Result<Self, CascadeError> {
        if !engine.obey_flags
            && engine.flags == FlagPolicy::InLookup
            && is_optimized_lookup_type(cascade.first_type())
        {
            return Err(CascadeError::IgnoresFlagsOnOptimizedLookup);
        }
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
