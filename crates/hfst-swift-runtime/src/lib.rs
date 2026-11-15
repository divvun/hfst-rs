//! The UniFFI glue behind the `HFST` product of `divvun.hfst`, Divvun HFST
//! for Swift.
//!
//! uniffi-bindgen-swift turns it into the Swift module `HFSTGlue`, which the
//! `HFST` module in `swift/Sources/HFST` imports internally and wraps in the
//! public API. Each call reaches hfst-embed, the code `@divvun/hfst` and
//! `divvun-hfst` run too, with nothing between but the conversion of its
//! arguments and results.

uniffi::setup_scaffolding!();

pub mod args;
mod dhfst;
pub mod error;
pub mod log;
mod optimised;
pub mod tokenise;
pub mod transducer;

pub use error::GlueError;

/// What the package calls itself and its readers, for the errors hfst-embed
/// raises that send a caller from one reader to another.
pub const PACKAGE: hfst_embed::Package = hfst_embed::Package {
    name: "divvun.hfst",
    read_transducers: "Transducer.all(in:)",
    read_optimised_transducer: "OptimisedTransducer(data:)",
    read_dhfst: "Dhfst(data:)",
};

// [spec:hfst:req:swift.version]
/// The version of the Swift package, which this crate carries.
#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
