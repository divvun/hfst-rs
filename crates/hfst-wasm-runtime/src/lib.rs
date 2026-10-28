//! The wasm-bindgen glue behind `@divvun/hfst-runtime`.
//!
//! These exports are not a public API. The TypeScript wrapper under
//! `js/shared/` is, and every call reaches the glue through it. The names
//! here are chosen for that wrapper, not for end users.
//!
//! `crates/hfst-wasm` links this crate in, so both npm packages share one
//! implementation of everything the runtime exports.

// [spec:hfst:req:javascript.portable-build]
// This crate and hfst-wasm build for wasm32-unknown-unknown from their
// manifests alone. They need no RUSTFLAGS and no cargo config: hfst comes in
// with only the features each crate names, and every dependency they pull in
// (rustfst without os-rng, web-time, wasm-bindgen, js-sys) has no OS entropy,
// threads or filesystem behind the features chosen.
use wasm_bindgen::prelude::wasm_bindgen;

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
