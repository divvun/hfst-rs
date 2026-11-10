//! The exceptions the library's failures reach Python as.

use hfst_embed::Error;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyOSError};
use pyo3::prelude::*;

// [spec:hfst:req:python.errors]
create_exception!(
    divvun_hfst,
    HfstError,
    PyException,
    "A failure the hfst library reports. `code` names it as @divvun/hfst does, \
     in a form code can test."
);
create_exception!(
    divvun_hfst,
    FormatError,
    HfstError,
    "Bytes a reader does not load: a format the package does not read, bytes \
     that are not well formed in the format they claim, a stream mixing types, \
     or more transducers than the reader takes. `code` says which."
);

// [spec:hfst:req:python.errors]
/// The `FormatError` for bytes a reader did not load, whatever the library
/// called the failure.
pub fn format_error(py: Python<'_>, error: Error) -> PyErr {
    with_code(py, FormatError::new_err(error.message().to_owned()), &error)
}

// [spec:hfst:req:python.errors]
/// The `HfstError` for a call on a loaded object that failed.
pub fn hfst_error(py: Python<'_>, error: Error) -> PyErr {
    with_code(py, HfstError::new_err(error.message().to_owned()), &error)
}

/// `raised` with the error's code as its `code`. Setting an attribute on a
/// fresh exception does not fail, short of running out of memory, and then
/// that failure is the one to raise.
fn with_code(py: Python<'_>, raised: PyErr, error: &Error) -> PyErr {
    match raised.value(py).setattr("code", error.code().name()) {
        Ok(()) => raised,
        Err(failed) => failed,
    }
}

// [spec:hfst:req:python.errors]
/// The `OSError` `open()` raises for `path`: `OSError` picks the subclass,
/// such as `FileNotFoundError`, from the errno, and the file name is the
/// path as `os.fspath` gives it.
pub fn os_error(py: Python<'_>, error: std::io::Error, path: &std::path::Path) -> PyErr {
    let Some(errno) = error.raw_os_error() else {
        return error.into();
    };
    let strerror = py
        .import("os")
        .and_then(|os| os.call_method1("strerror", (errno,)))
        .and_then(|s| s.extract::<String>())
        .unwrap_or_else(|_| error.to_string());
    PyOSError::new_err((errno, strerror, path.as_os_str().to_owned()))
}
