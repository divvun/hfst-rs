//! uniffi-bindgen-swift, which swift/build.sh runs on the built library to
//! write the Swift, the headers and the module map of its UniFFI crates.

fn main() {
    uniffi::uniffi_bindgen_swift()
}
