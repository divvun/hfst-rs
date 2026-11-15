//! The static library the `divvun.hfst` Swift package links: the UniFFI glue
//! of its products, built together so the app's linker sees one Rust build.

// [spec:hfst:req:swift.linking]
// The library is an ordinary archive of rustc's objects. Linked beside
// another Rust static library, a member defining what the other already
// defines is never loaded, so the two share one copy of the standard
// library; only force-loading every member makes the copies clash.
pub use hfst_swift_runtime;
