//! Links the module with a stack of 8 MiB. A JavaScript engine gives a
//! WebAssembly module none to grow into, and rustc's default for
//! wasm32-unknown-unknown, 1 MiB, is less than a compile at the wasm32 depth
//! limits of `hfst::depth` takes: XRE traps at 498 nested operators, under
//! the limit of 500. Setting it here keeps the build free of RUSTFLAGS and
//! cargo config, and leaves native builds and the main module alone.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg-cdylib=-zstack-size=8388608");
    }
}
