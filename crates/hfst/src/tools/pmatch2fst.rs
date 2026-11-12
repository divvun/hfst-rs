//! `hfst pmatch2fst`: a pmatch script compiled into the archive of its
//! definitions that `hfst pmatch` runs.

use std::collections::HashMap;

use hfst_openfst::StdVectorFst;

use crate::hfst_output_stream::HfstOutputStream;
use crate::hfst_transducer::HfstTransducer;
use crate::pmatch_compiler::{PmatchCompiler, build_archive_harmonizer, write_archive};

/// The definitions of a pmatch script, each under its name. A script
/// compiles at tropical, as the tool's did.
pub type Definitions = HashMap<String, HfstTransducer<StdVectorFst>>;

/// `hfst pmatch2fst`'s options.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pmatch2FstOptions {
    /// Compile every RTN into the network that uses it: `--flatten`.
    pub flatten: bool,
    /// `--cosine-distances`, which only `Like()` reads.
    pub include_cosine_distances: bool,
    /// The directory a relative `@bin"FILE"` or `@txt"FILE"` is found in,
    /// ending with `/`, or `""` for the working directory.
    pub include_path: String,
    /// Report each step on the progress writer: `--verbose`.
    pub verbose: bool,
}

impl Pmatch2FstOptions {
    /// A compiler set as these options say.
    pub fn compiler(&self) -> PmatchCompiler<StdVectorFst> {
        let mut compiler = PmatchCompiler::new();
        compiler.set_verbose(self.verbose);
        compiler.set_flatten(self.flatten);
        compiler.set_include_cosine_distances(self.include_cosine_distances);
        compiler.set_include_path(self.include_path.clone());
        compiler
    }
}

/// Why no archive was made. Each `Display`s as the tool's message, which
/// for all but `Write` follows the program name.
#[derive(Clone, Debug)]
pub enum ArchiveError {
    /// The script did not compile; the compiler's diagnostics say where.
    Compile(crate::error::Error),
    /// The script defines no `TOP`, or nothing with a symbol in it.
    Empty,
    /// The archive did not write.
    Write(crate::error::Error),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Compile(e) | ArchiveError::Write(e) => write!(f, "{e}"),
            ArchiveError::Empty => write!(f, "Empty ruleset, nothing to write"),
        }
    }
}

/// Whether `definitions` make an archive: they have a `TOP`, and carry at
/// least one symbol.
pub fn has_archive(definitions: &Definitions) -> bool {
    definitions.contains_key("TOP") && matches!(build_archive_harmonizer(definitions), Ok(Some(_)))
}

// [spec:hfst:sem:hfst-pmatch2fst.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
/// The definitions `source` compiles to with `compiler`, refused when they
/// make no archive. A source of one byte or none compiles to nothing.
pub fn compile_ruleset(
    compiler: &mut PmatchCompiler<StdVectorFst>,
    source: &str,
) -> Result<Definitions, ArchiveError> {
    let definitions = if source.len() > 1 {
        compiler.compile(source).map_err(ArchiveError::Compile)?
    } else {
        Definitions::new()
    };
    if has_archive(&definitions) {
        Ok(definitions)
    } else {
        Err(ArchiveError::Empty)
    }
}

// [spec:hfst:sem:hfst-pmatch2fst.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
/// Write the archive of `definitions` to `out`, a weighted optimized-lookup
/// stream, with `verbose` steps written to `progress`.
pub fn write_ruleset(
    definitions: &mut Definitions,
    out: &mut HfstOutputStream,
    verbose: bool,
    progress: &mut dyn std::io::Write,
) -> Result<(), ArchiveError> {
    match write_archive(definitions, out, verbose, progress) {
        Ok(true) => Ok(()),
        Ok(false) => Err(ArchiveError::Empty),
        Err(e) => Err(ArchiveError::Write(e)),
    }
}
