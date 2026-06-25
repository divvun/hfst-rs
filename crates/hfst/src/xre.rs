//! Deferred stub for the XRE (Xerox regular expression) compiler.
//!
//! The real implementation comes from the sibling `nfst` parser crates (XRE
//! tokenizing/parsing) plus a ported AST→transducer evaluator — out of scope for
//! the facade layer. Until then, construction is a no-op and any actual compile
//! panics loudly so a reaching test fails visibly.

use crate::hfst_data_types::ImplementationType;
use crate::hfst_transducer::HfstTransducer;

/// Placeholder for `hfst::xre::XreConstructorArguments`.
pub struct XreConstructorArguments {
    pub list_definitions: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

/// Placeholder for `hfst::xre::XreCompiler`.
pub struct XreCompiler;

impl XreCompiler {
    /// Accepts either an `ImplementationType` or `&XreConstructorArguments`
    /// (the two C++ constructor overloads).
    pub fn new<T>(_arg: T) -> Self {
        XreCompiler
    }

    pub fn set_verbosity(&mut self, _verbose: bool) {}

    pub fn compile(&mut self, _expression: &str) -> *mut HfstTransducer {
        unimplemented!("deferred: XRE compiler (served by the nfst parser integration)")
    }

    /// Marker so the unused-import lint stays quiet on the type alias above.
    pub fn _phantom(_t: ImplementationType, _a: &XreConstructorArguments) {}
}
