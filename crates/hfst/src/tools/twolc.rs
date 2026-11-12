//! `hfst twolc`: the rule transducers of a twolc grammar.

use crate::backend::AlgebraBackend;
use crate::diag::{Diagnostic, DiagnosticSink};
use crate::hfst_data_types::ImplementationType;
use crate::hfst_output_stream::HfstOutputStream;
use crate::twolc::TwolcCompiler;

/// `hfst twolc`'s options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwolcOptions {
    /// `--quiet`: warnings are dropped, errors never.
    pub quiet: bool,
    /// `--verbose`.
    pub verbose: bool,
    /// `--resolve`; off by default, unlike right-arrow conflicts.
    pub resolve_left_conflicts: bool,
    /// Off only with `--dont-resolve-right`.
    pub resolve_right_conflicts: bool,
}

impl Default for TwolcOptions {
    fn default() -> Self {
        TwolcOptions {
            quiet: false,
            verbose: false,
            resolve_left_conflicts: false,
            resolve_right_conflicts: true,
        }
    }
}

fn store<B: AlgebraBackend>(
    (source, source_name): (&str, &str),
    options: &TwolcOptions,
    out: &mut HfstOutputStream,
    sink: Option<DiagnosticSink>,
) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let mut compiler = TwolcCompiler::<B>::new_with_options(
        options.quiet,
        options.verbose,
        options.resolve_left_conflicts,
        options.resolve_right_conflicts,
    );
    if let Some(sink) = sink {
        compiler.set_diagnostic_sink(sink);
    }
    compiler.set_source_name(source_name);
    let stored = compiler.compile_and_store(source, out);
    let diagnostics = compiler.take_diagnostics();
    match stored {
        Some(()) => Ok(diagnostics),
        None => Err(diagnostics),
    }
}

// [spec:hfst:sem:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
// [spec:hfst:req:embed.tools-in-library]
/// Compile the twolc grammar `source`, from the file `source_name`, and
/// write its rule transducers to `out`, as `hfst twolc` does: foma rules
/// for a foma stream, tropical OpenFst for any other. The diagnostics go to
/// `sink` when there is one, and are otherwise handed back, with `Err` when
/// the grammar did not compile.
pub fn compile_twolc(
    source: (&str, &str),
    options: &TwolcOptions,
    out: &mut HfstOutputStream,
    sink: Option<DiagnosticSink>,
) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    match out.get_type() {
        #[cfg(feature = "foma")]
        ImplementationType::FOMA_TYPE => {
            store::<crate::backend_foma::FomaTransducer>(source, options, out, sink)
        }
        _ => store::<hfst_openfst::StdVectorFst>(source, options, out, sink),
    }
}
