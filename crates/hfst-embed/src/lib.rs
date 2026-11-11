//! What `@divvun/hfst` and `divvun-hfst` read and run, with no host in it.
//!
//! Both packages are glue over this crate: crates/hfst-wasm for JavaScript and
//! crates/hfst-python for Python. Each turns its host's values into the types
//! here and back, and checks its callers' arguments in its own terms.
//! Everything they read and every result they give comes from here, so the two
//! give the same answers for the same bytes.
//!
//! It reads optimized-lookup transducers, pmatch containers and DHFST files
//! from bytes, and predicts words a unit at a time from a language's
//! generator, unit grammar and prediction model. Without the `compile`
//! feature nothing in it reaches a compiler, the foma backend, the tropical
//! OpenFst backend or the transducer algebra, so a module built over it links
//! none of them. With it, [`compile`] does what the `hfst` tools do to a
//! transducer.

#[cfg(feature = "compile")]
pub mod compile;
pub mod dhfst;
pub mod error;
pub mod format;
pub mod lookup;
pub mod optimised;
pub mod predict;
mod stream;
pub mod tokenise;
pub mod transducer;

pub use error::{Code, Error};

/// A package, by the names it gives its readers. An error that sends its
/// caller from one reader to another names the reader the caller has.
#[derive(Clone, Copy, Debug)]
pub struct Package {
    /// The name the package is installed under.
    pub name: &'static str,
    pub read_transducers: &'static str,
    pub read_optimised_transducer: &'static str,
    pub read_dhfst: &'static str,
}

#[cfg(test)]
pub(crate) const TEST_PACKAGE: Package = Package {
    name: "the test package",
    read_transducers: "readTransducers",
    read_optimised_transducer: "readOptimisedTransducer",
    read_dhfst: "readDhfst",
};
