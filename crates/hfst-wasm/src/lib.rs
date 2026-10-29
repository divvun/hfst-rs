//! The wasm-bindgen glue behind `@divvun/hfst`.
//!
//! These exports are not a public API. The TypeScript wrapper under
//! `js/hfst/` is, and every call reaches the glue through it. The names here
//! are chosen for that wrapper, not for end users.

// [spec:hfst:req:javascript.portable-build+1]
// This crate builds for wasm32-unknown-unknown from its manifest alone. It
// needs no RUSTFLAGS and no cargo config: hfst comes in with no features, and
// every dependency it pulls in (rustfst without os-rng, web-time, wasm-bindgen,
// js-sys) has no OS entropy, threads or filesystem behind the features chosen.
use wasm_bindgen::prelude::wasm_bindgen;

mod error;
mod format;
mod lookup;
mod sink;
mod stream;
#[cfg(feature = "test-hooks")]
mod test_hooks;
mod tokenise;
mod transducer;

pub use sink::{install, set_log_level};
#[cfg(feature = "test-hooks")]
pub use test_hooks::{TestObject, test_library_error, test_log, test_panic};
pub use tokenise::{Tokeniser, read_tokeniser};
pub use transducer::{Transducer, read_transducers};

/// The Divvun HFST version this module was built from.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_the_workspace_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
