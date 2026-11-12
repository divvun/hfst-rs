//! The values the queries return, made as the frozen dataclasses of
//! `python/divvun_hfst/_results.py`.

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule};

/// `divvun_hfst._results`. A module is imported once for the life of the
/// process, so asking once is enough.
static RESULTS: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// An instance of the class `class` of `divvun_hfst._results`, its fields
/// given by name.
pub fn make<'py>(
    py: Python<'py>,
    class: &str,
    fields: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let module = RESULTS.get_or_try_init(py, || -> PyResult<_> {
        Ok(py.import("divvun_hfst._results")?.unbind())
    })?;
    module.bind(py).getattr(class)?.call((), Some(fields))
}

/// An instance of a class of `divvun_hfst._results`, each field named by a
/// string and given any value Python can hold.
macro_rules! result {
    ($py:expr, $class:literal { $($field:literal : $value:expr),* $(,)? }) => {{
        let fields = pyo3::types::PyDict::new($py);
        $( fields.set_item($field, $value)?; )*
        $crate::results::make($py, $class, &fields)
    }};
}

pub(crate) use result;
