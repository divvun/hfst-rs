//! Faithful 1:1 port of the HFST command-line tools (tools/src/*.cc).
//!
//! Scope: the backend-agnostic tools that operate on the ported OpenFST/OL
//! backends. The SFST tool (sfst-main.cc), the readline lexc UI
//! (lexc-readline-ui.cc), and the test scaffold (test.cc) are out of scope;
//! readline-gated interactive paths in otherwise-portable tools are '#if'd out
//! the same way the SFST backend was in the library.
//!
//! The CLI fidelity bar is behavioural, not textual: every tool declares its
//! command line as a clap-4 derive struct (the shared groups and the argv
//! pre-pass live in [`cli`]), and what is preserved bug-for-bug is the
//! MEASURED surface the Giella builds and the C++ tools' users depend on —
//! option spellings (clusters, attached values, single-dash longs, '--opt=val',
//! permutation, '-'/'--'), operand resolution, data output, exit codes, and
//! HFST's own argument-error shape. Help text is clap's, rendered from the
//! derive structs' doc comments, not the C's hand-written print_usage blocks.
//! The rest of the shared infrastructure (hfst-commandline) remains a close
//! port; each tool lives as a module under src/tools/ with an
//! execute(args) -> ToolResult entry point, and the single 'hfst' multiplexer
//! binary (src/bin/hfst.rs) dispatches to them by invoked basename or
//! subcommand. What a tool decides about what it writes is the library's
//! (`hfst::tools`, and the name and formula `hfst::tools::metadata` gives a
//! result); a tool keeps its arguments, files, streams and exit status.

// -----------------------------------------------------------------------------
// The one-per-stream-read dispatch of [dec:hfst:monomorphic-backends] step 5:
// each tool reads an 'AnyTransducer' from 'HfstInputStream' and matches ONCE
// per transducer into a generic body.
// -----------------------------------------------------------------------------

/// Dispatch an expression over all 'AnyTransducer' variants (each arm
/// monomorphizes separately). The 'Foma' arm is present only under the `foma`
/// feature, matching the cfg-gated enum variant.
macro_rules! for_any {
    ($any:expr, $t:ident => $body:expr) => {
        match $any {
            hfst::hfst_transducer::AnyTransducer::Tropical($t) => $body,
            hfst::hfst_transducer::AnyTransducer::OlW($t) => $body,
            hfst::hfst_transducer::AnyTransducer::OlU($t) => $body,
            #[cfg(feature = "foma")]
            hfst::hfst_transducer::AnyTransducer::Foma($t) => $body,
            hfst::hfst_transducer::AnyTransducer::Thfst($t) => $body,
        }
    };
}
pub(crate) use for_any;

/// Dispatch an expression over the algebra variants; the OL variants run the
/// 'else' arm (the tools' C++-era optimized-lookup rejection). Foma is a full
/// FST (algebra) backend, so its cfg-gated arm runs the body, not the 'else'.
macro_rules! for_algebra {
    ($any:expr, $t:ident => $body:expr, else => $ol:expr) => {
        match $any {
            hfst::hfst_transducer::AnyTransducer::Tropical($t) => $body,
            #[cfg(feature = "foma")]
            hfst::hfst_transducer::AnyTransducer::Foma($t) => $body,
            _ => $ol,
        }
    };
}
pub(crate) use for_algebra;

/// Re-wrap a typed facade transducer into the matching 'AnyTransducer'
/// variant (the write-back side of a dispatch that must keep the value).
pub(crate) trait IntoAny {
    fn into_any(self) -> hfst::hfst_transducer::AnyTransducer;
}
impl IntoAny for hfst::hfst_transducer::HfstTransducer<hfst_openfst::StdVectorFst> {
    fn into_any(self) -> hfst::hfst_transducer::AnyTransducer {
        hfst::hfst_transducer::AnyTransducer::Tropical(self)
    }
}
impl IntoAny
    for hfst::hfst_transducer::HfstTransducer<
        hfst::transducer::Transducer<hfst::transducer::WeightedTables>,
    >
{
    fn into_any(self) -> hfst::hfst_transducer::AnyTransducer {
        hfst::hfst_transducer::AnyTransducer::OlW(self)
    }
}
impl IntoAny
    for hfst::hfst_transducer::HfstTransducer<
        hfst::transducer::Transducer<hfst::transducer::UnweightedTables>,
    >
{
    fn into_any(self) -> hfst::hfst_transducer::AnyTransducer {
        hfst::hfst_transducer::AnyTransducer::OlU(self)
    }
}
#[cfg(feature = "foma")]
impl IntoAny for hfst::hfst_transducer::HfstTransducer<hfst::backend_foma::FomaTransducer> {
    fn into_any(self) -> hfst::hfst_transducer::AnyTransducer {
        hfst::hfst_transducer::AnyTransducer::Foma(self)
    }
}
impl IntoAny for hfst::hfst_transducer::HfstTransducer<hfst::backend_thfst::ThfstTransducer> {
    fn into_any(self) -> hfst::hfst_transducer::AnyTransducer {
        hfst::hfst_transducer::AnyTransducer::Thfst(self)
    }
}

// -----------------------------------------------------------------------------
// The bridge from the tools' message printers to the library lookup engine.
// -----------------------------------------------------------------------------

/// Routes the lookup engine's messages through the tool's own printers, so a
/// warning raised inside the library is indistinguishable from one the tool
/// raised itself: verbose lines go to the tool's message writer, and warnings
/// to `hfst_warning` (and nowhere at all when the tool is silent).
pub struct CliLookupReporter<'a> {
    common: &'a globals::CommonOptions,
}

impl<'a> CliLookupReporter<'a> {
    pub fn new(common: &'a globals::CommonOptions) -> CliLookupReporter<'a> {
        CliLookupReporter { common }
    }
}

impl hfst::lookup_driver::LookupReporter for CliLookupReporter<'_> {
    fn verbose(&self, msg: &str) {
        hfst_commandline::verbose_print(self.common, msg);
    }

    fn warning(&self, msg: &str) {
        if !self.common.silent {
            hfst_commandline::hfst_warning(self.common, 0, 0, msg);
        }
    }
}

/// Look `line`, the `linen`th of the input, up as `hfst lookup` and
/// `hfst flookup` do: a line that does not read or look up is reported as
/// they report it, and `--verbose` shows the symbols it was read as. `echo`
/// is the stream a print-pairs lookup echoes its input on.
pub(crate) fn look_up_line(
    common: &globals::CommonOptions,
    lookup: &hfst::tools::lookup::Lookup<'_>,
    (line, linen): (&str, usize),
    out: &mut dyn std::io::Write,
    echo: &mut dyn std::io::Write,
) -> Result<hfst::tools::lookup::LineLookup, i32> {
    use hfst::tools::lookup::LookupError;
    let reporter = CliLookupReporter::new(common);
    let looked_up = match lookup.look_up(line, &reporter, out, echo) {
        Ok(looked_up) => looked_up,
        Err(LookupError::Input(e)) => {
            if e.kind == hfst::error::ErrorKind::IncorrectUtf8Coding {
                hfst_commandline::hfst_error_at_line(
                    common,
                    1,
                    0,
                    &common.input_filename,
                    linen as u32,
                    e.message.as_deref().unwrap_or(""),
                );
            }
            hfst_commandline::hfst_error(common, 1, 0, &format!("{e}"));
            return Err(1);
        }
        Err(LookupError::Lookup(e)) => {
            hfst_commandline::hfst_error(common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    if common.verbose {
        hfst_commandline::verbose_print(common, "Tokenized to: ");
        for s in looked_up.input.second.iter() {
            hfst_commandline::verbose_print(common, &format!("{} ", s));
        }
        hfst_commandline::verbose_print(common, "\n");
    }
    Ok(looked_up)
}

/// Routes the progress lines of the library engines that only report progress
/// — the substitution engine, the pair-test engine — through the tool's own
/// `--verbose` writer.
pub struct CliVerboseReporter<'a> {
    common: &'a globals::CommonOptions,
}

impl<'a> CliVerboseReporter<'a> {
    pub fn new(common: &'a globals::CommonOptions) -> CliVerboseReporter<'a> {
        CliVerboseReporter { common }
    }
}

impl hfst::substitute_driver::SubstituteReporter for CliVerboseReporter<'_> {
    fn verbose(&self, msg: &str) {
        hfst_commandline::verbose_print(self.common, msg);
    }
}

// The clap-4 argument layer: the shared derive groups, the argv pre-pass and
// the error/exit-code mapping every tool parses through. The hand-ported
// getopt fallback (hfst_getopt) and the inc/ getopt-cases fragments it
// replaced are gone.
pub mod cli;

pub mod globals;
pub mod hfst_commandline;
pub(crate) mod memory_limit;

// The shared main/<op>_streams scaffolding of the two-input-stream (BINARY)
// tools, lifted out of the per-tool verbatim copies and parameterized by an
// op descriptor.
pub mod binary_ops;

// The unary analogue: the run()/process_stream scaffolding of the
// single-input-stream transform tools.
pub mod unary_ops;

// The tools themselves, one module per former standalone binary, plus the
// TOOLS dispatch table the 'hfst' multiplexer binary drives.
pub mod tools;

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::hfst_lookup_format::{CascadeVariant, LookupInputFormat};
    use hfst::hfst_transducer::AnyTransducer;
    use hfst::lookup_driver::{
        AmbiguityLimit, FlagPolicy, LookupCascade, LookupEngineOptions, PairPrintStyle,
    };
    use hfst::tools::lookup::Lookup;
    use hfst::xre::XreCompiler;
    use hfst_openfst::StdVectorFst;

    fn engine() -> LookupEngineOptions {
        LookupEngineOptions {
            obey_flags: true,
            show_flags: false,
            print_pairs: false,
            print_space: false,
            quote_special: false,
            epsilon_format: String::new(),
            beam: -1.0,
            time_cutoff: 0.0,
            infinite_cutoff: 5,
            cascade: CascadeVariant::Union,
            flags: FlagPolicy::InLookup,
            ambiguity: AmbiguityLimit::MaxResults {
                max_number: -1,
                default_max: 5,
            },
            pair_style: PairPrintStyle::Lookup,
        }
    }

    // [spec:hfst:req:embed.tools-in-library/test]
    // A line looks up through the library's lookup, which the lookup tools
    // share.
    #[test]
    fn a_line_looks_up_through_the_library() {
        let common = globals::CommonOptions::default();
        let reporter = CliLookupReporter::new(&common);
        let t = AnyTransducer::Tropical(
            XreCompiler::<StdVectorFst>::new()
                .compile("{cat}:{dog}::0.5")
                .expect("compiles"),
        );
        let mut cascade = LookupCascade::new();
        cascade.begin_transducer(&t, "", &reporter);
        cascade.push_borrowed(&t, &reporter).expect("pushes");
        let lookup =
            Lookup::new(cascade, engine(), LookupInputFormat::Utf8TokenInput).expect("ready");
        let found = look_up_line(
            &common,
            &lookup,
            ("cat", 1),
            &mut std::io::sink(),
            &mut std::io::sink(),
        )
        .expect("looks up");
        let result = found.results.iter().next().expect("a result");
        assert_eq!(result.first, 0.5);
        let missed = look_up_line(
            &common,
            &lookup,
            ("cow", 2),
            &mut std::io::sink(),
            &mut std::io::sink(),
        )
        .expect("looks up");
        assert!(missed.results.is_empty());
        assert_eq!(missed.line, "cow");
    }
}
