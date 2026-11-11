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
