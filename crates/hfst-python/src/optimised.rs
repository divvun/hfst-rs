//! The optimised lookup: an optimized-lookup transducer read by divvunspell's
//! `divvun-fst`, and words looked up in it by that crate's lookup.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use hfst_embed::optimised::{Lookup, OptimisedTransducer as Machine};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::PyDict;

use crate::args;
use crate::call::{read_file, without_gil};
use crate::error::format_error;
use crate::transducer::{LookupResult, described, properties};

// [spec:hfst:req:python.lookup]
// [spec:hfst:req:python.threads]
/// An optimized-lookup transducer read for the optimised lookup, which is
/// divvunspell's: its results are what divvunspell gives for the same
/// transducer and word. Transducer.lookup gives hfst's. The two agree for
/// most words; they differ on Unicode normalisation, text outside the
/// alphabet, cycles of epsilon and flag arcs, duplicate paths and what
/// `limit` means, as @divvun/hfst's MIGRATING.md describes. It is immutable,
/// so threads may look up in it at the same time, without the GIL. Make one
/// with read_optimised_transducer or load_optimised_transducer.
#[pyclass(frozen, module = "divvun_hfst")]
pub struct OptimisedTransducer {
    machine: Machine,
    /// divvun-fst's working tables, one set for each lookup in progress, kept
    /// between lookups so they keep their capacity. A lookup takes a set out
    /// and puts it back, so the lock is held for neither the search nor the
    /// GIL.
    tables: Mutex<Vec<Lookup>>,
}

#[pymethods]
impl OptimisedTransducer {
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

    /// The properties of its HFST3 header, as Transducer.properties has them:
    /// without `name`, `type` and `version`. A transducer with no header has
    /// none.
    #[getter]
    fn properties<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        properties(py, self.machine.properties())
    }

    // [spec:hfst:req:python.lookup]
    // [spec:hfst:req:python.threads]
    /// What divvunspell's lookup returns for `input`: every output of a path
    /// that reads it, with the path's weight, final weight included, lightest
    /// first, then by output in code point order. Each output comes once at
    /// each weight it has. `limit` keeps the lightest results; every result is
    /// found first, so there is no time limit.
    #[pyo3(signature = (input, /, *, limit = None))]
    fn lookup(
        &self,
        py: Python<'_>,
        input: &str,
        limit: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<LookupResult>> {
        let limit = args::limit("lookup()", limit)?;
        without_gil(py, || {
            let mut tables = self
                .tables
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop()
                .unwrap_or_default();
            let results = self.machine.lookup(&mut tables, input, limit);
            self.tables
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(tables);
            results
                .into_iter()
                .map(|found| {
                    LookupResult::new(
                        found.output.into(),
                        found.symbols.into_iter().map(String::from).collect(),
                        found.weight.0,
                    )
                })
                .collect()
        })
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        described(
            py,
            "OptimisedTransducer",
            self.machine.format().name(),
            self.machine.name(),
        )
    }
}

// [spec:hfst:req:python.load]
// [spec:hfst:req:python.threads]
/// Reads the bytes `data` gives for the optimised lookup, without the GIL:
/// `divvun-fst` keeps the bytes, so a copy made here is the only one.
fn read(py: Python<'_>, data: impl FnOnce() -> Vec<u8> + Send) -> PyResult<OptimisedTransducer> {
    let read = without_gil(py, || Machine::read(data(), &crate::PACKAGE))?;
    let machine = read.map_err(|error| format_error(py, error))?;
    Ok(OptimisedTransducer {
        machine,
        tables: Mutex::new(Vec::new()),
    })
}

// [spec:hfst:req:python.load]
/// The one optimized-lookup transducer in `data`, weighted or unweighted,
/// with or without an HFST3 header, read for the optimised lookup. Bytes in
/// any other format raise FormatError with code `unsupported-format`, naming
/// it, and bytes that are not a transducer stream, or end partway through
/// one, with code `invalid-format`. A stream of several transducers, such as
/// a pmatch container, raises FormatError with code `transducer-count`.
#[pyfunction]
pub fn read_optimised_transducer(
    py: Python<'_>,
    data: PyBackedBytes,
) -> PyResult<OptimisedTransducer> {
    read(py, || data.to_vec())
}

// [spec:hfst:req:python.load]
/// The optimized-lookup transducer in the file at `path`, as
/// read_optimised_transducer reads its bytes. A file that cannot be read
/// raises the OSError open() raises.
#[pyfunction]
pub fn load_optimised_transducer(py: Python<'_>, path: PathBuf) -> PyResult<OptimisedTransducer> {
    let data = read_file(py, &path)?;
    read(py, || data)
}
