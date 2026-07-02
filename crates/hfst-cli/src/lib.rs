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
//! tool's main lives in src/bin/ and drives them.

pub mod globals;
pub mod hfst_commandline;
pub mod hfst_getopt;
pub mod hfst_program_options;
pub mod hfst_tool_metadata;

// The 'inc/' switch-case and post-parse-validation fragments that every tool
// '#include's into its 'parse_options' (getopt-cases-*.h, check-params-*.h),
// translated once into shared helpers the bin mains call.
pub mod inc;

// The shared main/<op>_streams scaffolding of the two-input-stream (BINARY)
// tools, lifted out of the per-tool verbatim copies and parameterized by an
// op descriptor.
pub mod binary_ops;
