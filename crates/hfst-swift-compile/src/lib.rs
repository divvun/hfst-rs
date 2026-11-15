//! The UniFFI glue behind the `HFSTCompile` product of `divvun.hfst`.
//!
//! uniffi-bindgen-swift turns it into the Swift module `HFSTCompileGlue`,
//! which `HFSTCompile` imports internally. Its errors, diagnostics, lookup
//! results and tokeniser are the runtime glue's, so `HFSTCompile` shares
//! `HFST`'s Swift types for them. Each call reaches hfst-embed's compile
//! feature, which calls the library function the hfst tool calls.

uniffi::setup_scaffolding!();

mod compilers;
mod files;
mod operations;
mod queries;
mod transducer;
mod writers;
