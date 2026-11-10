//! DHFST files, divvunspell's speller formats, read from bytes or files: an
//! acceptor to look words up in, or an error model to describe.

use std::path::PathBuf;

use hfst_embed::dhfst::{AcceptorReader, Dhfst, ErrorModel};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::PyTuple;

use crate::call::{read_file, without_gil};
use crate::error::format_error;

// [spec:hfst:req:python.dhfst]
/// How a DHFST acceptor stores its arcs, as `dhfst-tools` reports it.
#[pyclass(frozen, get_all, eq, module = "divvun_hfst")]
#[derive(PartialEq, Eq)]
pub struct AcceptorInfo {
    /// State numbers: every state is numbered below this.
    ids: u32,
    /// Slots in the table, at least `ids` plus `label_max`.
    slots: u32,
    /// Slots that hold arcs, free or regular.
    used_slots: u32,
    /// Bytes per slot check: 1 when every regular label is below 256, else 2.
    check_bytes: usize,
    /// The greatest regular label.
    label_max: u32,
    /// Bytes per record, each a target and the index of a weight.
    record_bytes: usize,
    /// Records in the list of arcs that do not fit their slot.
    list_records: u32,
    /// Arcs, free and regular.
    arcs: u64,
    /// Free arcs: epsilon and flag diacritic arcs.
    free_arcs: u64,
    /// Distinct symbol and weight pairs of the free arcs.
    free_pairs: u32,
    /// Distinct regular arc weights.
    weights: u32,
    /// Final states.
    finals: u32,
    /// Whether the file stores each state's distance to a final state.
    distances: bool,
}

#[pymethods]
impl AcceptorInfo {
    fn __repr__(&self) -> String {
        let flag = if self.distances { "True" } else { "False" };
        format!(
            "AcceptorInfo(ids={}, slots={}, used_slots={}, check_bytes={}, label_max={}, \
             record_bytes={}, list_records={}, arcs={}, free_arcs={}, free_pairs={}, \
             weights={}, finals={}, distances={flag})",
            self.ids,
            self.slots,
            self.used_slots,
            self.check_bytes,
            self.label_max,
            self.record_bytes,
            self.list_records,
            self.arcs,
            self.free_arcs,
            self.free_pairs,
            self.weights,
            self.finals,
        )
    }
}

// [spec:hfst:req:python.dhfst]
// [spec:hfst:req:python.threads]
/// A DHFST acceptor (type 2): a speller's lexicon, read and validated as
/// divvunspell reads it. It is immutable, so threads may look words up in it
/// at the same time. Make one with read_dhfst or load_dhfst.
#[pyclass(frozen, module = "divvun_hfst")]
pub struct DhfstAcceptor {
    reader: Box<AcceptorReader<'static>>,
}

#[pymethods]
impl DhfstAcceptor {
    /// Always `"acceptor"`, which tells this from a DhfstErrorModel.
    #[getter(r#type)]
    fn kind(&self) -> &'static str {
        "acceptor"
    }

    /// The format version the file declares.
    #[getter]
    fn version(&self) -> u8 {
        self.reader.version()
    }

    /// The symbol names, `@_EPSILON_SYMBOL_@` first.
    #[getter]
    fn symbols<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.reader.symbol_names())
    }

    /// The JSON the writer left in the file's `meta` section, or None when it
    /// has none.
    #[getter]
    fn meta(&self) -> Option<&str> {
        self.reader.meta()
    }

    /// How the file stores its arcs.
    #[getter]
    fn info(&self) -> AcceptorInfo {
        let info = self.reader.info();
        AcceptorInfo {
            ids: info.ids,
            slots: info.slots,
            used_slots: info.used_slots,
            check_bytes: info.check_bytes,
            label_max: info.label_max,
            record_bytes: info.record_bytes,
            list_records: info.list_records,
            arcs: info.arcs,
            free_arcs: info.free_arcs,
            free_pairs: info.free_pairs,
            weights: info.weights,
            finals: info.finals,
            distances: info.distances,
        }
    }

    // [spec:hfst:req:python.dhfst]
    // [spec:hfst:req:python.threads]
    /// The least weight of a path that accepts `word`, final weight included,
    /// or None when no path does: what divvunspell's speller finds checking
    /// the word as typed against this lexicon alone. The word is read one
    /// extended grapheme cluster at a time, each the symbol of the same text
    /// or else the lexicon's unknown symbol. Epsilon and flag diacritic arcs
    /// are walked, and the flags obeyed, as divvunspell does. There is no
    /// case folding or suggestion search: those are divvunspell's.
    #[pyo3(signature = (word, /))]
    fn lookup(&self, py: Python<'_>, word: &str) -> PyResult<Option<f64>> {
        let reader = &self.reader;
        let weight = without_gil(py, || reader.lookup(word))?;
        Ok(weight.map(f64::from))
    }

    fn __repr__(&self) -> String {
        format!(
            "<divvun_hfst.DhfstAcceptor version {}, {} symbols>",
            self.reader.version(),
            self.reader.symbol_names().len()
        )
    }
}

// [spec:hfst:req:python.dhfst]
/// A DHFST error model (type 1), read and validated as divvunspell reads
/// one. The package offers no suggestion search, so an error model only
/// describes itself, and keeps none of its file's bytes.
#[pyclass(frozen, module = "divvun_hfst")]
pub struct DhfstErrorModel {
    model: ErrorModel,
}

#[pymethods]
impl DhfstErrorModel {
    /// Always `"error-model"`, which tells this from a DhfstAcceptor.
    #[getter(r#type)]
    fn kind(&self) -> &'static str {
        "error-model"
    }

    /// The format version the file declares.
    #[getter]
    fn version(&self) -> u8 {
        self.model.version
    }

    /// The number of states, one for each state of the source transducer
    /// that its start state reaches.
    #[getter]
    fn state_count(&self) -> u32 {
        self.model.state_count
    }

    /// The symbol names, `@_EPSILON_SYMBOL_@` first.
    #[getter]
    fn symbols<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, &self.model.symbols)
    }

    /// The JSON the writer left in the file's `meta` section, or None when it
    /// has none.
    #[getter]
    fn meta(&self) -> Option<&str> {
        self.model.meta.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "<divvun_hfst.DhfstErrorModel version {}, {} states>",
            self.model.version, self.model.state_count
        )
    }
}

// [spec:hfst:req:python.load]
// [spec:hfst:req:python.threads]
/// Reads a DHFST file from `data` without the GIL.
fn read(py: Python<'_>, data: &[u8]) -> PyResult<Py<PyAny>> {
    let read = without_gil(py, || hfst_embed::dhfst::read_dhfst(data, &crate::PACKAGE))?;
    match read.map_err(|error| format_error(py, error))? {
        Dhfst::Acceptor(reader) => Ok(Py::new(py, DhfstAcceptor { reader })?.into_any()),
        Dhfst::ErrorModel(model) => Ok(Py::new(py, DhfstErrorModel { model })?.into_any()),
    }
}

// [spec:hfst:req:python.load]
// [spec:hfst:req:python.dhfst]
/// The DHFST file in `data`, a DhfstAcceptor or a DhfstErrorModel, as its
/// `type` says. A file that fails the validation divvunspell runs on load, or
/// is truncated, raises FormatError with code `invalid-format` and the
/// reason. Bytes that are not DHFST raise FormatError with code
/// `invalid-format`, or `unsupported-format` naming a format the package does
/// not read.
#[pyfunction]
pub fn read_dhfst(py: Python<'_>, data: PyBackedBytes) -> PyResult<Py<PyAny>> {
    read(py, &data)
}

// [spec:hfst:req:python.load]
/// The DHFST file at `path`, as read_dhfst reads its bytes. A file that
/// cannot be read raises the OSError open() raises.
#[pyfunction]
pub fn load_dhfst(py: Python<'_>, path: PathBuf) -> PyResult<Py<PyAny>> {
    let data = read_file(py, &path)?;
    read(py, &data)
}
