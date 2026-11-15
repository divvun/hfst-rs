//! The UniFFI glue behind the `HFSTCompile` product of `divvun.hfst`.
//!
//! uniffi-bindgen-swift turns it into the Swift module `HFSTCompileGlue`,
//! which `HFSTCompile` imports internally. Its errors, diagnostics, lookup
//! results and tokeniser are the runtime glue's, so `HFSTCompile` shares
//! `HFST`'s Swift types for them. Each call reaches hfst-embed's compile
//! feature, which calls the library function the hfst tool calls.

uniffi::setup_scaffolding!();

// [spec:hfst:req:swift.compile-parity]
// Every compiler, operation, query and writer reaches hfst-embed's compile
// feature, which calls the library function the hfst tool calls, with the
// caller's options turned into the tool's: the bytes and reports are the
// tool's. swift/Tests/HFSTTests/CompileParityTests.swift checks them against
// what the tools write of the inputs the repository's tests give them.

mod compilers;
mod files;
mod operations;
mod queries;
mod transducer;
mod writers;
