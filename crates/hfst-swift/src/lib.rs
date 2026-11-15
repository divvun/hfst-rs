//! The static library the `divvun.hfst` Swift package links: the UniFFI glue
//! of its products, built together so the app's linker sees one Rust build.

// [spec:hfst:req:swift.linking+1]
// Built with the swift-release profile, fat LTO in one codegen unit, so the
// archive is one object of what the C interface reaches, which
// swift/build.sh prelinks so that interface is its only global symbols.
pub use hfst_swift_compile;
pub use hfst_swift_runtime;
