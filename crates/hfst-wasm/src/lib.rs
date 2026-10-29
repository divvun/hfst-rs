//! The wasm-bindgen glue behind `@divvun/hfst`.
//!
//! The full package carries everything the runtime package does, so this
//! module re-exports the runtime glue, and its one `.wasm` file holds the
//! whole runtime API. The TypeScript wrapper under `js/shared/` is the public
//! API; these names serve that wrapper.

// [spec:hfst:req:javascript.portable-build]
// Builds for wasm32-unknown-unknown from the manifest alone, with no RUSTFLAGS
// and no cargo config. The foma backend this crate turns on in hfst is plain
// Rust with no host facilities behind it.
pub use hfst_wasm_runtime::*;

mod lookup;
mod transducer;

pub use transducer::{FullTransducer, read_any_transducers, read_att};

#[cfg(test)]
mod tests {
    #[test]
    fn the_runtime_glue_is_part_of_this_module() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
