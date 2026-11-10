//! The wasm-bindgen glue behind `@divvun/hfst`.
//!
//! These exports are not a public API. The TypeScript wrapper under
//! `js/hfst/` is, and every call reaches the glue through it. The names here
//! are chosen for that wrapper, not for end users.

// [spec:hfst:req:javascript.portable-build+1]
// This crate builds for wasm32-unknown-unknown from its manifest alone. It
// needs no RUSTFLAGS and no cargo config: hfst comes in with no features, and
// every dependency it pulls in (hfst-embed, rustfst without os-rng, web-time,
// divvun-fst without fs, wasm-bindgen, js-sys) has no OS entropy, threads or
// filesystem behind the features chosen.
use wasm_bindgen::prelude::wasm_bindgen;

mod dhfst;
mod error;
mod lookup;
mod optimised;
mod sink;
#[cfg(feature = "test-hooks")]
mod test_hooks;
mod tokenise;
mod transducer;

/// What the package calls itself and its readers, for the errors hfst-embed
/// raises that send a caller to another reader.
const PACKAGE: hfst_embed::Package = hfst_embed::Package {
    name: "@divvun/hfst",
    read_transducers: "readTransducers",
    read_optimised_transducer: "readOptimisedTransducer",
    read_dhfst: "readDhfst",
};

// [spec:hfst:req:javascript.read-only-module]
// These and version() are the module's exports, with the test hooks only in a
// test build. Apart from the log sink and version(), each reads an
// optimized-lookup transducer, a pmatch container or a DHFST file, or runs
// lookup or tokenise on one, through hfst-embed; the optimised lookup reads
// and looks up with divvun-fst. None reaches a compiler, the foma backend, the
// tropical OpenFst backend or the transducer algebra, so the linker leaves
// their code out, and js/build.mjs fails the build when a function from any
// of them is in the module.
pub use dhfst::{DhfstAcceptor, DhfstErrorModel, read_dhfst};
pub use optimised::{OptimisedTransducer, read_optimised_transducer};
pub use sink::{install, set_log_level};
#[cfg(feature = "test-hooks")]
pub use test_hooks::{TestObject, test_library_error, test_log, test_panic};
pub use tokenise::{Tokeniser, read_tokeniser};
pub use transducer::{Transducer, read_transducers};

// [spec:hfst:req:javascript.version]
/// The version of the npm package, which this crate carries as its own.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[cfg(test)]
mod tests {
    // [spec:hfst:req:javascript.version/test]
    #[test]
    fn the_package_is_at_the_crates_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
        let manifest = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/package.json"
        ))
        .expect("js/hfst/package.json is in the repository");
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest).expect("package.json is JSON");
        assert_eq!(manifest["name"], "@divvun/hfst");
        assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
    }
}
