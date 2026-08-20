//! Faithful 1:1 port of the HFST command-line tools (tools/src/*.cc).
//!
//! Scope: the backend-agnostic tools that operate on the ported OpenFST/OL
//! backends. The SFST tool (sfst-main.cc), the readline lexc UI
//! (lexc-readline-ui.cc), and the test scaffold (test.cc) are out of scope;
//! readline-gated interactive paths in otherwise-portable tools are '#if'd out
//! the same way the SFST backend was in the library.
//!
//! The CLI fidelity decision is the same as the library's: an absolute 1:1
//! bug-for-bug translation, getopt and all. The shared command-line
//! infrastructure (hfst-getopt, hfst-commandline, hfst-program-options,
//! hfst-tool-metadata, hfst-file-to-mem) is ported into the modules below; each
//! tool lives as a module under src/tools/ exposing run(args) -> i32, and the
//! single 'hfst' multiplexer binary (src/bin/hfst.rs) dispatches to them by
//! invoked basename or subcommand.

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

pub mod globals;
pub mod hfst_commandline;
pub mod hfst_getopt;
pub mod hfst_program_options;
pub mod hfst_tool_metadata;
pub(crate) mod memory_limit;

// The 'inc/' switch-case and post-parse-validation fragments that every tool
// '#include's into its 'parse_options' (getopt-cases-*.h, check-params-*.h),
// translated once into shared helpers the bin mains call.
pub mod inc;

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
