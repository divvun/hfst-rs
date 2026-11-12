//! Transducers of every type the hfst library reads, read from bytes, files
//! or THFST directories, and lookup in them. Each is behind a lock, which an
//! operation takes to change it and every other call to read it.

use std::path::PathBuf;
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use hfst_embed::compile::{self as embed, Operand, Transducer as Machine};
use hfst_embed::format::Format;
use hfst_embed::lookup::LookupResult as Found;
use hfst_embed::{Code, Error};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::{PyDict, PyFloat, PyString, PyTuple};

use crate::args;
use crate::call::{read_file, without_gil};
use crate::error::{format_error, hfst_error};

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

// [spec:hfst:req:python.transducer]
// [spec:hfst:req:python.threads+1]
/// A transducer of any type the hfst library reads: tropical OpenFst, foma,
/// and optimized lookup, weighted and unweighted. Its operations change it in
/// place and return None; copy() gives one of its own to change. Threads may
/// share one: an operation holds it for its length, and a call on a
/// transducer another thread is changing waits until that change is done.
/// Every call releases the GIL while the library works. Make one with
/// read_transducers, read_transducer or their load_ forms.
#[pyclass(frozen, module = "divvun_hfst")]
pub struct Transducer {
    machine: RwLock<Machine>,
}

impl Transducer {
    pub fn new(machine: Machine) -> Self {
        Transducer {
            machine: RwLock::new(machine),
        }
    }

    /// The transducer to read, once no operation is changing it. An
    /// operation that failed partway leaves it as the library left it, which
    /// is still a transducer, so a poisoned lock is read through.
    pub fn reading(&self) -> RwLockReadGuard<'_, Machine> {
        self.machine.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// The transducer to change, once no other call holds it.
    pub fn changing(&self) -> RwLockWriteGuard<'_, Machine> {
        self.machine.write().unwrap_or_else(PoisonError::into_inner)
    }

    // [spec:hfst:req:python.threads+1]
    /// `f` on the transducer, read without the GIL. A failure raises
    /// HfstError.
    pub fn read<T: Send>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&Machine) -> Result<T, Error> + Send,
    ) -> PyResult<T> {
        let done = without_gil(py, || f(&self.reading()))?;
        done.map_err(|error| hfst_error(py, error))
    }

    // [spec:hfst:req:python.threads+1]
    /// `f` changing the transducer, without the GIL. A failure raises
    /// HfstError.
    pub fn change<T: Send>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&mut Machine) -> Result<T, Error> + Send,
    ) -> PyResult<T> {
        let done = without_gil(py, || f(&mut self.changing()))?;
        done.map_err(|error| hfst_error(py, error))
    }

    // [spec:hfst:req:python.threads+1]
    /// `f` changing the transducer with `other` as its operand, without the
    /// GIL. When `other` is this transducer, the operation reads it as it was
    /// before the change.
    pub fn change_with<T: Send>(
        &self,
        py: Python<'_>,
        other: &Transducer,
        f: impl FnOnce(&mut Machine, Operand<'_>) -> Result<T, Error> + Send,
    ) -> PyResult<T> {
        let done = without_gil(py, || {
            let (changed, read) = locked(Some(self), &[other]);
            let mut changed = changed.ok_or_else(unlocked)?;
            match read.get(other) {
                Some(other) => f(&mut changed, Operand::Other(other)),
                None => f(&mut changed, Operand::Itself),
            }
        })?;
        done.map_err(|error| hfst_error(py, error))
    }
}

/// The error for a lock [`locked`] was asked for and did not take, which
/// cannot happen.
pub fn unlocked() -> Error {
    Error::new(Code::Internal, "the transducer to change was not locked")
}

fn address(t: &Transducer) -> usize {
    std::ptr::from_ref(t).addr()
}

/// The transducers a call reads, each locked once.
pub struct Reading<'a> {
    guards: Vec<(usize, RwLockReadGuard<'a, Machine>)>,
}

impl Reading<'_> {
    /// What `t` holds, unless `t` is the transducer the call changes.
    pub fn get(&self, t: &Transducer) -> Option<&Machine> {
        let at = address(t);
        self.guards
            .iter()
            .find(|(held, _)| *held == at)
            .map(|(_, guard)| &**guard)
    }
}

// [spec:hfst:req:python.threads+1]
/// Locks `changed` to change and every transducer of `read` to read. Each is
/// locked once, however often it is named, and all of them in the order of
/// their addresses, so two calls that lock the same transducers take them in
/// the same order, and neither can hold one the other waits for while
/// waiting for one the other holds: `a.compose(b)` and `b.compose(a)` on two
/// threads take turns.
pub fn locked<'a>(
    changed: Option<&'a Transducer>,
    read: &[&'a Transducer],
) -> (Option<RwLockWriteGuard<'a, Machine>>, Reading<'a>) {
    let mut all: Vec<&'a Transducer> = read.iter().copied().chain(changed).collect();
    all.sort_by_key(|t| address(t));
    all.dedup_by_key(|t| address(t));
    let changed_at = changed.map(address);
    let mut guard = None;
    let mut reading = Reading { guards: Vec::new() };
    for t in all {
        if Some(address(t)) == changed_at {
            guard = Some(t.changing());
        } else {
            reading.guards.push((address(t), t.reading()));
        }
    }
    (guard, reading)
}

/// The type named `name`, or the ValueError a parameter of `call` taking a
/// `TransducerType` raises.
pub fn format_named(call: &str, name: &str) -> PyResult<Format> {
    if let Some(format) = Format::ALL.iter().find(|f| f.name() == name) {
        return Ok(*format);
    }
    let names: Vec<String> = Format::ALL
        .iter()
        .map(|f| format!("'{}'", f.name()))
        .collect();
    Err(PyValueError::new_err(format!(
        "{call} argument 'type' must be one of {}, not '{name}'",
        names.join(", ")
    )))
}

#[pymethods]
impl Transducer {
    // [spec:hfst:req:python.transducer]
    /// The transducer's type.
    #[getter(r#type)]
    fn kind(&self, py: Python<'_>) -> &'static str {
        py.detach(|| self.reading().format().name())
    }

    // [spec:hfst:req:python.transducer]
    /// The `name` property of its HFST3 header, or `""` when there is none.
    #[getter]
    fn name(&self, py: Python<'_>) -> String {
        py.detach(|| self.reading().name())
    }

    // [spec:hfst:req:python.transducer]
    /// The properties of its HFST3 header, such as a pmatch container's
    /// settings, as a new dict. `name` and `type` have their own attributes,
    /// and the header's `version` belongs to the stream rather than the
    /// transducer, so none of the three is here.
    #[getter]
    fn properties<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let pairs = py.detach(|| self.reading().properties());
        properties(
            py,
            pairs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
    }

    // [spec:hfst:req:python.lookup]
    // [spec:hfst:req:python.transducer]
    // [spec:hfst:req:python.threads+1]
    /// Every result the hfst library's lookup finds for `input`, lightest
    /// first, then by output in code point order, then by symbols. In
    /// optimized lookup, flag diacritics are obeyed, so a path they block is
    /// not a result; `limit` stops the search once it has found that many
    /// paths, so they are the first it found, not the lightest; `time_limit`
    /// stops it after that many seconds, and the lookup returns what it
    /// found, 0 setting no limit. In the other types the results are what
    /// `hfst lookup` finds, in its order, at most `limit` of them.
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
        let found = self.read(py, |t| t.lookup(input, limit, time_limit))?;
        Ok(found.into_iter().map(LookupResult::from).collect())
    }

    // [spec:hfst:req:python.transducer]
    /// A transducer of its own with the same contents, which changes
    /// independently of this one.
    fn copy(&self, py: Python<'_>) -> PyResult<Transducer> {
        self.read(py, Machine::copy).map(Transducer::new)
    }

    // [spec:hfst:req:python.operations]
    /// Converts the transducer to `type`, in place, as `hfst fst2fst`
    /// converts it. `quick` packs optimized-lookup tables less tightly, as
    /// `--quick`.
    #[pyo3(signature = (r#type, /, *, quick = false))]
    fn convert(&self, py: Python<'_>, r#type: &str, quick: bool) -> PyResult<()> {
        let format = format_named("convert()", r#type)?;
        self.change(py, |t| t.convert(format, quick))
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let (kind, name) = py.detach(|| {
            let t = self.reading();
            (t.format().name(), t.name())
        });
        described(py, "Transducer", kind, &name)
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

// [spec:hfst:req:python.load+1]
// [spec:hfst:req:python.threads+1]
/// Every transducer in the bytes of an HFST transducer stream, in stream
/// order, without the GIL.
fn read(py: Python<'_>, data: &[u8]) -> PyResult<Vec<Transducer>> {
    let read = without_gil(py, || embed::read_transducers(data))?;
    let machines = read.map_err(|error| format_error(py, error))?;
    Ok(machines.into_iter().map(Transducer::new).collect())
}

// [spec:hfst:req:python.load+1]
/// Every transducer at `path`: a THFST directory's one, or a stream file's,
/// each read without the GIL. A file that cannot be read raises the OSError
/// open() raises.
fn load(py: Python<'_>, path: &std::path::Path) -> PyResult<Vec<Transducer>> {
    if py.detach(|| path.is_dir()) {
        let read = without_gil(py, || embed::read_thfst(path))?;
        let machine = read.map_err(|error| format_error(py, error))?;
        return Ok(vec![Transducer::new(machine)]);
    }
    let data = read_file(py, path)?;
    read(py, &data)
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

// [spec:hfst:req:python.load+1]
/// Every transducer in the bytes of an HFST transducer stream, in stream
/// order, of any type the hfst library reads: tropical OpenFst, foma, and
/// optimized lookup, weighted and unweighted, with an HFST3 header or
/// without one. A pmatch container reads as TOP followed by its definitions,
/// each under its name. Bytes that are not a transducer stream raise
/// FormatError, its code saying why.
#[pyfunction]
pub fn read_transducers(py: Python<'_>, data: PyBackedBytes) -> PyResult<Vec<Transducer>> {
    read(py, &data)
}

// [spec:hfst:req:python.load+1]
/// The one transducer in the bytes of a stream, as read_transducers reads
/// it. A stream holding more raises FormatError with code
/// `transducer-count`.
#[pyfunction]
pub fn read_transducer(py: Python<'_>, data: PyBackedBytes) -> PyResult<Transducer> {
    let all = read(py, &data)?;
    single(py, "read_transducer", all)
}

// [spec:hfst:req:python.load+1]
/// Every transducer at `path`, as read_transducers reads a file's bytes; a
/// THFST directory, as `hfst fst2fst -f thfst` writes one, reads as its one
/// transducer, in weighted optimized lookup. A file that cannot be read
/// raises the OSError open() raises.
#[pyfunction]
pub fn load_transducers(py: Python<'_>, path: PathBuf) -> PyResult<Vec<Transducer>> {
    load(py, &path)
}

// [spec:hfst:req:python.load+1]
/// The one transducer at `path`, as load_transducers reads it. A stream
/// holding more raises FormatError with code `transducer-count`.
#[pyfunction]
pub fn load_transducer(py: Python<'_>, path: PathBuf) -> PyResult<Transducer> {
    let all = load(py, &path)?;
    single(py, "load_transducer", all)
}
