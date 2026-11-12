//! `hfst lexc`: compiling the lexicons of several lexc sources into one
//! transducer, named after the first source.

use crate::backend::AlgebraBackend;
use crate::hfst_transducer::HfstTransducer;
use crate::lexc::LexcCompiler;
use crate::tools::metadata::{hfst_set_formula, hfst_set_name};

/// Why the sources did not compile. Each `Display`s as the message the tool
/// prints for it.
#[derive(Clone, Debug)]
pub enum LexcSourcesError {
    /// The lexicons did not compile; the diagnostics say why. Carries the
    /// first source's name and how many sources there were.
    NotCompiled {
        first: String,
        sources: usize,
    },
    Compile(crate::error::Error),
}

impl std::fmt::Display for LexcSourcesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexcSourcesError::NotCompiled { first, sources: 1 } => {
                write!(f, "The file {} did not compile cleanly.", first)
            }
            LexcSourcesError::NotCompiled { first, .. } => {
                write!(f, "The files {}... did not compile cleanly.", first)
            }
            LexcSourcesError::Compile(e) => write!(f, "{}", e),
        }
    }
}

impl From<crate::error::Error> for LexcSourcesError {
    fn from(e: crate::error::Error) -> Self {
        LexcSourcesError::Compile(e)
    }
}

impl<B: AlgebraBackend> LexcCompiler<B> {
    // [spec:hfst:req:embed.tools-in-library]
    /// Parse `source` as the lexc file `name`, adding what it defines to the
    /// sources parsed before it.
    pub fn parse_source(&mut self, name: &str, source: &str) -> crate::error::Result<()> {
        self.set_source_name(name);
        self.parse(source)?;
        Ok(())
    }

    // [spec:hfst:def:hfst-lexc-compiler.lexc-streams-fn]
    // [spec:hfst:sem:hfst-lexc-compiler.lexc-streams-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Compile the lexicons of the sources parsed, `names` naming them in
    /// order, into the transducer `hfst lexc` writes: named and given a
    /// formula after the first source.
    pub fn compile_sources(
        &mut self,
        names: &[String],
    ) -> Result<HfstTransducer<B>, LexcSourcesError> {
        let first = names.first().cloned().unwrap_or_default();
        let Some(mut compiled) = self.compile_lexical()? else {
            return Err(LexcSourcesError::NotCompiled {
                first,
                sources: names.len(),
            });
        };
        hfst_set_name(&mut compiled, &first, "lexc");
        hfst_set_formula(&mut compiled, &first, "L");
        Ok(compiled)
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// Parse each of `sources`, a name and its text, in order, and compile
    /// them as [`Self::compile_sources`] does.
    pub fn compile_named_sources(
        &mut self,
        sources: &[(String, String)],
    ) -> Result<HfstTransducer<B>, LexcSourcesError> {
        for (name, source) in sources {
            self.parse_source(name, source)?;
        }
        let names: Vec<String> = sources.iter().map(|(name, _)| name.clone()).collect();
        self.compile_sources(&names)
    }
}

/// How much a lexc compile says: what `--quiet` and `--verbose` choose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LexcVerbosity {
    /// No warnings, info or progress.
    Quiet,
    #[default]
    Normal,
    /// Progress as well.
    Verbose,
}

/// `hfst lexc`'s options that change what it compiles or warns of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexcOptions {
    /// `--alignStrings`: align the characters of a string pair one to one.
    pub align_strings: bool,
    /// `--withFlags`: join the lexicons with flag diacritics rather than
    /// composing them.
    pub with_flags: bool,
    /// `--minimizeFlags`, with `with_flags`.
    pub minimize_flags: bool,
    /// `--renameFlags`, with `with_flags`.
    pub rename_flags: bool,
    /// `--Werror`: a warning fails the compile.
    pub warnings_as_errors: bool,
    pub warn_one_sided_flags: bool,
    pub warn_unused_lexicons: bool,
    pub warn_repeated_lexicons: bool,
    pub warn_missing_lexicons: bool,
    pub warn_missing_alphabets: bool,
    pub warn_unnecessary_escapes: bool,
    /// Compose as Xerox tools do, which the tool does unless told
    /// `--xerox-composition=no`.
    pub xerox_composition: bool,
    /// `--encode-weights` when minimizing.
    pub encode_weights: bool,
    /// `--xfst flag-is-epsilon`.
    pub flag_is_epsilon: bool,
    /// `--split-characters`: no tokenising into Unicode characters.
    pub split_characters: bool,
    pub verbosity: LexcVerbosity,
}

impl Default for LexcOptions {
    fn default() -> Self {
        LexcOptions {
            align_strings: false,
            with_flags: false,
            minimize_flags: false,
            rename_flags: false,
            warnings_as_errors: false,
            warn_one_sided_flags: false,
            warn_unused_lexicons: false,
            warn_repeated_lexicons: false,
            warn_missing_lexicons: false,
            warn_missing_alphabets: false,
            warn_unnecessary_escapes: false,
            xerox_composition: true,
            encode_weights: false,
            flag_is_epsilon: false,
            split_characters: false,
            verbosity: LexcVerbosity::Normal,
        }
    }
}

impl LexcOptions {
    // [spec:hfst:req:embed.tools-in-library]
    /// A compiler set as `hfst lexc` sets one with these options.
    pub fn compiler<B: AlgebraBackend>(&self) -> LexcCompiler<B> {
        let mut lexc = LexcCompiler::<B>::new_with_flags(self.with_flags, self.align_strings);
        lexc.set_minimize_flags(self.minimize_flags);
        lexc.set_rename_flags(self.rename_flags);
        lexc.set_flag_is_epsilon(self.flag_is_epsilon);
        lexc.set_xerox_composition(self.xerox_composition);
        lexc.set_encode_weights(self.encode_weights);
        lexc.set_verbosity(match self.verbosity {
            LexcVerbosity::Quiet => 0,
            LexcVerbosity::Normal => 1,
            LexcVerbosity::Verbose => 2,
        });
        if self.warnings_as_errors {
            lexc.set_treat_warnings_as_errors(true);
        }
        for (warning, on) in [
            ("-Wone-sided-flags", self.warn_one_sided_flags),
            ("-Wunused-lexicons", self.warn_unused_lexicons),
            ("-Wrepeated-lexicons", self.warn_repeated_lexicons),
            ("-Wmissing-lexicons", self.warn_missing_lexicons),
            ("-Wmissing-alphabets", self.warn_missing_alphabets),
            ("-Wunnecessary-escapes", self.warn_unnecessary_escapes),
        ] {
            lexc.set_warning(warning, on);
        }
        if self.split_characters {
            lexc.set_split_characters(true);
        }
        lexc
    }
}
