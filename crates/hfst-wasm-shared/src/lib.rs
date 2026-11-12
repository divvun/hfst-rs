//! What the two WebAssembly modules behind `@divvun/hfst` share: the errors
//! they hand JavaScript, the sink a panic and a log event reach the wrapper
//! through, the options objects the wrapper passes, lookup results, and
//! tokenising with a pmatch container.
//!
//! Each module links its own copy, so each has its own sink, logger and
//! level, and a trap in one leaves the other alone. Its exports, `install`
//! and `setLogLevel`, and the test hooks in a test build, are exports of both
//! modules. `crates/hfst-wasm` cannot be linked by `crates/hfst-wasm-compile`
//! instead: its exports, and the readers behind them, would come with it.

pub mod error;
pub mod lookup;
pub mod options;
pub mod sink;
#[cfg(feature = "test-hooks")]
pub mod test_hooks;
pub mod tokenise;

pub use error::{GlueError, set};
