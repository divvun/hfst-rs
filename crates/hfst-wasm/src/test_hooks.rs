//! Exports for the JavaScript test suite: a small object that owns module
//! memory, a panic, log events at every level, and a library error. Only the
//! `test-hooks` feature compiles them, and only `js/build.mjs --test` turns it
//! on, writing outside the package directory, so the published package never
//! carries them.

use hfst::error::{Error, ErrorKind};
use wasm_bindgen::prelude::*;

use crate::error::GlueError;

const TARGET: &str = "hfst_wasm::test_hooks";

/// A small owned object for testing handle lifetime. Dropping it logs a
/// debug event, so a test can see when its memory is released.
#[wasm_bindgen]
pub struct TestObject {
    label: String,
}

#[wasm_bindgen]
impl TestObject {
    #[wasm_bindgen(constructor)]
    pub fn new(label: String) -> TestObject {
        TestObject { label }
    }

    pub fn label(&self) -> String {
        self.label.clone()
    }

    /// Traps while wasm-bindgen holds a borrow of the object, a borrow the
    /// trap never gives back, so freeing the object afterwards would fail.
    #[wasm_bindgen(js_name = panicWhileBorrowed)]
    pub fn panic_while_borrowed(&self, message: &str) {
        panic!("{}: {message}", self.label)
    }
}

impl Drop for TestObject {
    fn drop(&mut self) {
        tracing::debug!(target: TARGET, "dropped {}", self.label);
    }
}

#[wasm_bindgen(js_name = testPanic)]
pub fn test_panic(message: &str) {
    panic!("{message}")
}

/// Logs `message` at `level`, in the wrapper's numbering (1 is error, 5 is trace).
#[wasm_bindgen(js_name = testLog)]
pub fn test_log(level: u8, message: &str) {
    match level {
        0..=1 => tracing::error!(target: TARGET, "{message}"),
        2 => tracing::warn!(target: TARGET, "{message}"),
        3 => tracing::info!(target: TARGET, "{message}"),
        4 => tracing::debug!(target: TARGET, "{message}"),
        5.. => tracing::trace!(target: TARGET, "{message}"),
    }
}

#[wasm_bindgen(js_name = testLibraryError)]
pub fn test_library_error(message: &str) -> Result<(), GlueError> {
    Err(Error::with_message(ErrorKind::HfstTransducerTypeMismatch, message).into())
}
