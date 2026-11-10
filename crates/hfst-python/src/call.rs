//! Running the library without the GIL, and reading the files it loads.

use std::path::Path;

use pyo3::prelude::*;

use crate::error::os_error;
use crate::log;

// [spec:hfst:req:python.threads]
// [spec:hfst:req:python.logging]
/// Runs `f` without the GIL, so other threads run Python meanwhile, and
/// hands the events it logged to `logging` once the GIL is back.
pub fn without_gil<T: Send>(py: Python<'_>, f: impl FnOnce() -> T + Send) -> PyResult<T> {
    let thresholds = log::thresholds(py)?;
    let (value, kept) = py.detach(|| log::collecting(thresholds, f));
    kept.hand_over(py)?;
    Ok(value)
}

// [spec:hfst:req:python.load]
// [spec:hfst:req:python.errors]
/// The bytes of the file at `path`, read without the GIL. A file that cannot
/// be read raises the `OSError` `open()` raises for it.
pub fn read_file(py: Python<'_>, path: &Path) -> PyResult<Vec<u8>> {
    py.detach(|| std::fs::read(path))
        .map_err(|error| os_error(py, error, path))
}
