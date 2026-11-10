//! Transducers read from bytes or files, and lookup in them with the hfst
//! library's engine.

use std::path::PathBuf;

use hfst_embed::lookup::LookupResult as Found;
use hfst_embed::transducer::Transducer as Machine;
use hfst_embed::{Code, Error};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::{PyDict, PyFloat, PyString, PyTuple};

use crate::args;
use crate::call::{read_file, without_gil};
use crate::error::format_error;

// [spec:hfst:req:python.lookup]
/// One path a lookup found: its output symbols, joined and one by one, and
/// its weight. Flag diacritics and epsilons are left out of both, as
/// `hfst lookup` leaves them out of what it prints.
#[pyclass(frozen, eq, module = "divvun_hfst")]
#[derive(PartialEq)]
pub struct LookupResult {
    output: String,
    symbols: Vec<String>,
    weight: f32,
}

impl From<Found> for LookupResult {
    fn from(found: Found) -> Self {
        LookupResult {
            output: found.output,
            symbols: found.symbols,
            weight: found.weight,
        }
    }
}

impl LookupResult {
    pub fn new(output: String, symbols: Vec<String>, weight: f32) -> Self {
        LookupResult {
            output,
            symbols,
            weight,
        }
    }
}

#[pymethods]
impl LookupResult {
    #[getter]
    fn output(&self) -> &str {
        &self.output
    }

    #[getter]
    fn symbols<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, &self.symbols)
    }

    /// The sum of the weights along the path. Lower is better.
    #[getter]
    fn weight(&self) -> f64 {
        f64::from(self.weight)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "LookupResult(output={}, symbols={}, weight={})",
            PyString::new(py, &self.output).repr()?,
            self.symbols(py)?.repr()?,
            PyFloat::new(py, self.weight()).repr()?,
        ))
    }
}

// [spec:hfst:req:python.lookup]
// [spec:hfst:req:python.threads]
/// An optimized-lookup transducer, for lookup with the hfst library's engine,
/// whose results are what `hfst lookup` prints. It is immutable, so threads
/// may look up in it at the same time, each lookup a run of its own without
/// the GIL. Make one with read_transducers, read_transducer or their load_
/// forms.
#[pyclass(frozen, module = "divvun_hfst")]
pub struct Transducer {
    machine: Machine,
}

#[pymethods]
impl Transducer {
    /// Which optimized-lookup format the transducer is in, weighted or
    /// unweighted.
    #[getter(r#type)]
    fn kind(&self) -> &'static str {
        self.machine.format().name()
    }

    /// The `name` property of its HFST3 header, or `""` when there is none.
    #[getter]
    fn name(&self) -> &str {
        self.machine.name()
    }

    /// The properties of its HFST3 header, such as a pmatch container's
    /// settings, as a new dict. `name` and `type` have their own attributes,
    /// and the header's `version` belongs to the stream rather than the
    /// transducer, so none of the three is here. A transducer with no header
    /// has none.
    #[getter]
    fn properties<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        properties(py, self.machine.properties())
    }

    // [spec:hfst:req:python.lookup]
    // [spec:hfst:req:python.threads]
    /// Every result the hfst library's lookup finds for `input`, lightest
    /// first, then by output in code point order, then by symbols. Flag
    /// diacritics are obeyed, so a path they block is not a result. `limit`
    /// stops the search once it has found that many paths, so they are the
    /// first it found, not the lightest. `time_limit` stops it after that many
    /// seconds, and the lookup returns what it found; 0 sets no limit.
    /// OptimisedTransducer.lookup gives divvunspell's results instead.
    #[pyo3(signature = (input, /, *, limit = None, time_limit = None))]
    fn lookup(
        &self,
        py: Python<'_>,
        input: &str,
        limit: Option<&Bound<'_, PyAny>>,
        time_limit: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<LookupResult>> {
        let limit = args::limit("lookup()", limit)?;
        let time_limit = args::time_limit("lookup()", time_limit)?;
        let machine = &self.machine;
        without_gil(py, || {
            machine
                .lookup(input, limit, time_limit)
                .into_iter()
                .map(LookupResult::from)
                .collect()
        })
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        described(
            py,
            "Transducer",
            self.machine.format().name(),
            self.machine.name(),
        )
    }
}

/// `properties` as a new dict, so a caller who changes it changes nothing
/// the transducer holds.
pub fn properties<'a, 'py>(
    py: Python<'py>,
    properties: impl Iterator<Item = (&'a str, &'a str)>,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    for (key, value) in properties {
        dict.set_item(key, value)?;
    }
    Ok(dict)
}

/// The repr of a transducer: its class, its type, and its name when it has
/// one.
pub fn described(py: Python<'_>, class: &str, kind: &str, name: &str) -> PyResult<String> {
    if name.is_empty() {
        return Ok(format!("<divvun_hfst.{class} {kind}>"));
    }
    let name = PyString::new(py, name).repr()?;
    Ok(format!("<divvun_hfst.{class} {kind} {name}>"))
}

// [spec:hfst:req:python.load]
// [spec:hfst:req:python.threads]
/// Every transducer in the bytes of an HFST transducer stream, in stream
/// order, without the GIL. A pmatch container reads as TOP followed by its
/// definitions, each under its name.
fn read(py: Python<'_>, data: &[u8]) -> PyResult<Vec<Transducer>> {
    let read = without_gil(py, || {
        hfst_embed::transducer::read_transducers(data, &crate::PACKAGE)
    })?;
    let machines = read.map_err(|error| format_error(py, error))?;
    Ok(machines
        .into_iter()
        .map(|machine| Transducer { machine })
        .collect())
}

/// The one transducer of `all`, or the `transducer-count` FormatError
/// naming `call` when there are more.
fn single(py: Python<'_>, call: &str, mut all: Vec<Transducer>) -> PyResult<Transducer> {
    if all.len() == 1
        && let Some(transducer) = all.pop()
    {
        return Ok(transducer);
    }
    let error = Error::new(
        Code::TransducerCount,
        format!(
            "{call} takes exactly one transducer, and the stream holds {}",
            all.len()
        ),
    );
    Err(format_error(py, error))
}

// [spec:hfst:req:python.load]
/// Every transducer in the bytes of an HFST transducer stream, in stream
/// order: weighted and unweighted optimized-lookup transducers, with or
/// without an HFST3 header. A pmatch container reads as TOP followed by its
/// definitions, each under its name. Bytes in any other format raise
/// FormatError with code `unsupported-format`, naming the format; bytes that
/// are not a transducer stream, or end partway through one, raise FormatError
/// with code `invalid-format`.
#[pyfunction]
pub fn read_transducers(py: Python<'_>, data: PyBackedBytes) -> PyResult<Vec<Transducer>> {
    read(py, &data)
}

// [spec:hfst:req:python.load]
/// The one transducer in the bytes of a stream, as read_transducers reads
/// it. A stream holding more raises FormatError with code
/// `transducer-count`.
#[pyfunction]
pub fn read_transducer(py: Python<'_>, data: PyBackedBytes) -> PyResult<Transducer> {
    let all = read(py, &data)?;
    single(py, "read_transducer", all)
}

// [spec:hfst:req:python.load]
/// Every transducer in the file at `path`, as read_transducers reads its
/// bytes. A file that cannot be read raises the OSError open() raises.
#[pyfunction]
pub fn load_transducers(py: Python<'_>, path: PathBuf) -> PyResult<Vec<Transducer>> {
    let data = read_file(py, &path)?;
    read(py, &data)
}

// [spec:hfst:req:python.load]
/// The one transducer in the file at `path`, as read_transducer reads its
/// bytes. A file that cannot be read raises the OSError open() raises.
#[pyfunction]
pub fn load_transducer(py: Python<'_>, path: PathBuf) -> PyResult<Transducer> {
    let data = read_file(py, &path)?;
    let all = read(py, &data)?;
    single(py, "load_transducer", all)
}
