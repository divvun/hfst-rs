//! The wasm-bindgen glue behind `@divvun/hfst/compile`.
//!
//! These exports are not a public API. The TypeScript wrapper under
//! `js/hfst/` is, and every call reaches the glue through it. This module
//! holds what hfst-embed's `compile` feature does: the compilers, the
//! transducer algebra on every type the library reads, the queries and the
//! writers, which the main module, `crates/hfst-wasm`, must not link.
//!
//! Every call runs where it is called: on wasm32 hfst-embed runs a compile
//! or an operation in place, with no thread of its own, under the depth
//! limits `hfst::depth` sets for wasm32. build.rs links the module with a
//! stack those limits fit in.

// [spec:hfst:req:javascript.compile-parity]
// Every compiler, operation, query and writer reaches hfst-embed's compile
// feature, which divvun-hfst's binding reaches too, with the caller's options
// turned into the tool's: the bytes and reports are the tool's, and
// divvun-hfst's. js/test/compile-parity.ts checks them against what the tools
// write of the inputs the repository's tests give them, the cases
// python/tests/test_compile_parity.py checks divvun-hfst on.

// [spec:hfst:req:javascript.portable-build+1]
// This crate builds for wasm32-unknown-unknown from its manifest alone:
// build.rs sets its stack size, and every dependency it pulls in, box-format
// for BHFST and foma among them, has no OS entropy, threads or filesystem
// behind the features chosen.
use wasm_bindgen::prelude::wasm_bindgen;

mod compile;
mod diagnostics;
mod entry;
mod failure;
mod files;
mod list;
mod operation_options;
mod operations;
mod options;
mod queries;
mod query_options;
mod tokenise;
mod transducer;
mod values;
mod writers;

/// What this entry point calls itself and the readers of each format, for
/// the errors hfst-embed raises that send a caller to another reader. The
/// optimised lookup and DHFST are read by the main entry point.
const PACKAGE: hfst_embed::Package = hfst_embed::Package {
    name: "@divvun/hfst/compile",
    read_transducers: "readTransducers",
    read_optimised_transducer: "readOptimisedTransducer of @divvun/hfst",
    read_dhfst: "readDhfst of @divvun/hfst",
};

pub use entry::{compile_lexc, compile_pmatch, compile_text, compile_twolc, compile_xfst, regex};
pub use list::TransducerList;
pub use queries::{flookup_report, grep, lookup_report, optimized_lookup_report, pair_test};
pub use tokenise::{Tokeniser, read_tokeniser};
pub use transducer::{Transducer, read_transducers};
pub use writers::pack_bhfst;

// [spec:hfst:req:javascript.compile-module]
/// The version of the npm package, which `crates/hfst-wasm` carries and this
/// crate carries with it.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[cfg(test)]
pub(crate) mod tests {
    use hfst_embed::compile::{Source, Transducer as Machine};

    use crate::files::Files;
    use crate::options::given;
    use crate::transducer::Transducer;

    /// The tropical transducer of the regular expression `regex`.
    pub fn machine(regex: &str) -> Machine {
        let source = Source {
            name: "test.xre",
            text: regex,
        };
        crate::compile::regex(source, &Files::of(Vec::new()), &given("regex", &[]))
            .unwrap_or_else(|_| panic!("{regex} compiles"))
            .value
    }

    /// The paths of `t`, as `input:output` or `input` where the two are
    /// one, in the order `hfst fst2strings` prints them.
    pub fn paths(t: &Transducer) -> Vec<String> {
        let options = hfst_embed::compile::ExtractPathsOptions::default();
        let paths = t
            .reading()
            .expect("free")
            .extract_paths(&options)
            .expect("paths");
        paths
            .into_iter()
            .map(|p| {
                if p.input == p.output {
                    p.input
                } else {
                    format!("{}:{}", p.input, p.output)
                }
            })
            .collect()
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn the_module_is_at_the_packages_version() {
        let manifest = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/package.json"
        ))
        .expect("js/hfst/package.json is in the repository");
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest).expect("package.json is JSON");
        assert_eq!(super::version(), manifest["version"]);
        assert_eq!(
            manifest["exports"]["./compile"]["default"],
            "./dist/compile.js"
        );
    }
}
