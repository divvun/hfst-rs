//! Pmatch tokenisers read from bytes or files, the tokens they find, and the
//! options of `tokenise` and `format`, as the fields of the dataclasses in
//! `python/divvun_hfst/_options.py`.

use std::path::PathBuf;
use std::sync::Arc;

use hfst::pmatch::PmatchContainer;
use hfst::pmatch_core::PmatchCore;
use hfst_embed::tokenise::{
    self, FORMATS, FormatOptions, Selection, TokeniseOptions, output_format,
};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::pybacked::{PyBackedBytes, PyBackedStr};
use pyo3::types::{PyFloat, PyString, PyTuple};

use crate::args::Value;
use crate::call::{read_file, without_gil};
use crate::error::{format_error, hfst_error};

// [spec:hfst:req:python.tokenise]
/// One reading of a token.
#[pyclass(frozen, eq, module = "divvun_hfst")]
#[derive(Clone, PartialEq)]
pub struct Reading {
    analysis: String,
    weight: f32,
}

#[pymethods]
impl Reading {
    /// The tokeniser's output for the token, such as `cat+N`.
    #[getter]
    fn analysis(&self) -> &str {
        &self.analysis
    }

    /// The reading's weight. Lower is better.
    #[getter]
    fn weight(&self) -> f64 {
        f64::from(self.weight)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Reading(analysis={}, weight={})",
            PyString::new(py, &self.analysis).repr()?,
            PyFloat::new(py, self.weight()).repr()?,
        ))
    }
}

// [spec:hfst:req:python.tokenise]
/// A token the tokeniser found or, under `print_all`, a stretch of text
/// between tokens where no pattern matched.
#[pyclass(frozen, eq, module = "divvun_hfst")]
#[derive(PartialEq)]
pub struct Token {
    surface: String,
    start: usize,
    end: usize,
    analysed: bool,
    readings: Vec<Reading>,
}

impl From<tokenise::Token> for Token {
    fn from(token: tokenise::Token) -> Self {
        Token {
            surface: token.surface,
            start: token.start,
            end: token.end,
            analysed: token.analysed,
            readings: token
                .readings
                .into_iter()
                .map(|reading| Reading {
                    analysis: reading.analysis,
                    weight: reading.weight,
                })
                .collect(),
        }
    }
}

#[pymethods]
impl Token {
    /// The token's text, as the tokeniser read it.
    #[getter]
    fn surface(&self) -> &str {
        &self.surface
    }

    /// Where the token starts in the text, in code points, as `str` indices
    /// count. `text[start:end]` is the surface, unless the text holds
    /// something the tokeniser reads without printing, such as the spelling
    /// of a flag diacritic.
    #[getter]
    fn start(&self) -> usize {
        self.start
    }

    /// Where the token ends in the text, in code points, exclusive.
    #[getter]
    fn end(&self) -> usize {
        self.end
    }

    /// Whether any reading is an analysis. A reading the tokeniser marks as
    /// unknown, by an empty output or one containing ` ??`, is not one.
    #[getter]
    fn analysed(&self) -> bool {
        self.analysed
    }

    /// The readings `hfst tokenise --cg --print-weights` prints for the
    /// token, in that order. A stretch where no pattern matched has none.
    #[getter]
    fn readings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.readings.iter().cloned())
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let flag = if self.analysed { "True" } else { "False" };
        Ok(format!(
            "Token(surface={}, start={}, end={}, analysed={flag}, readings={})",
            PyString::new(py, &self.surface).repr()?,
            self.start,
            self.end,
            self.readings(py)?.repr()?,
        ))
    }
}

// [spec:hfst:req:python.tokenise]
// [spec:hfst:req:python.threads]
/// A tokeniser read from a pmatch container, as `hfst tokenise` reads one.
/// It holds the container's tables alone and is immutable, so threads may
/// tokenise with it at the same time: each call is a run of its own, without
/// the GIL. Make one with read_tokeniser or load_tokeniser.
#[pyclass(frozen, module = "divvun_hfst")]
pub struct Tokeniser {
    core: Arc<PmatchCore>,
}

#[pymethods]
impl Tokeniser {
    // [spec:hfst:req:python.tokenise]
    // [spec:hfst:req:python.threads]
    /// The tokens the tokeniser finds in `text`, in text order, matching the
    /// whole text as one input. `hfst tokenise` takes each paragraph as one
    /// input, so for text with no blank line these are the cohorts
    /// `hfst tokenise --cg --print-weights` prints, given the same options.
    #[pyo3(signature = (text, /, options = None))]
    fn tokenise(
        &self,
        py: Python<'_>,
        text: &str,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<Token>> {
        let options = match options.filter(|options| !options.is_none()) {
            Some(options) => {
                expect_class(options, "TokeniseOptions", "tokenise()")?;
                tokenise_options(options)?
            }
            None => TokeniseOptions::default(),
        };
        let core = &self.core;
        without_gil(py, || {
            let mut run = PmatchContainer::from_core(Arc::clone(core));
            let mut tokens = tokenise::tokens(&mut run, text, &options);
            tokenise::recount(text, &mut tokens, |stretch| stretch.chars().count());
            tokens.into_iter().map(Token::from).collect()
        })
    }

    // [spec:hfst:req:python.tokenise]
    // [spec:hfst:req:python.threads]
    /// Exactly what `hfst tokenise` prints in `format` when `text` is its
    /// standard input, given the flags `options` stand for.
    #[pyo3(signature = (text, format, /, options = None))]
    fn format(
        &self,
        py: Python<'_>,
        text: &str,
        format: &str,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let Some(output) = output_format(format) else {
            let names: Vec<String> = FORMATS.iter().map(|(n, _)| format!("'{n}'")).collect();
            return Err(PyValueError::new_err(format!(
                "format() argument 'format' must be one of {}, not {}",
                names.join(", "),
                PyString::new(py, format).repr()?
            )));
        };
        let options = match options.filter(|options| !options.is_none()) {
            Some(options) => {
                expect_class(options, "FormatOptions", "format()")?;
                format_options(options)?
            }
            None => FormatOptions::default(),
        };
        let core = &self.core;
        let printed = without_gil(py, || {
            let mut run = PmatchContainer::from_core(Arc::clone(core));
            tokenise::format(&mut run, text, output, &options)
        })?;
        printed.map_err(|error| hfst_error(py, error))
    }

    fn __repr__(&self) -> &'static str {
        "<divvun_hfst.Tokeniser>"
    }
}

/// Refuses `options` unless it is an instance of `class` from
/// `divvun_hfst._options`, as a typed parameter of `call` would.
fn expect_class(options: &Bound<'_, PyAny>, class: &str, call: &str) -> PyResult<()> {
    let expected = options
        .py()
        .import("divvun_hfst._options")?
        .getattr(class)?;
    if options.is_instance(&expected)? {
        return Ok(());
    }
    let given = options.get_type().name()?;
    Err(PyTypeError::new_err(format!(
        "{call} argument 'options' must be {class} or None, not {given}"
    )))
}

/// The fields of an options dataclass, read as the settings they stand for.
/// Each one left out is `None`.
#[derive(Default)]
struct Fields {
    selection: Selection,
    weight_cutoff: Option<f32>,
    print_weights: Option<bool>,
    hack_uncompose: Option<bool>,
    newline: Option<bool>,
    keep_newline: Option<bool>,
    superblanks: Option<bool>,
}

// [spec:hfst:req:python.tokenise]
/// Reads each field of `options`, one of the dataclasses `class` names, by
/// its name. A field of the wrong type raises the `TypeError` a typed
/// parameter would, and a value `hfst tokenise` would turn down raises
/// `ValueError`. Each dataclass's `__post_init__` reads its fields here, so
/// options that were made are options a call can read.
fn fields(options: &Bound<'_, PyAny>, accepted: &[&str]) -> PyResult<Fields> {
    let py = options.py();
    let class = options.get_type().name()?.to_string();
    let call = format!("{class}()");
    let mut read = Fields::default();
    let fields = py
        .import("dataclasses")?
        .call_method1("fields", (options,))?;
    for field in fields.try_iter()? {
        let name: PyBackedStr = field?.getattr("name")?.extract()?;
        let name: &str = &name;
        if !accepted.contains(&name) {
            return Err(PyTypeError::new_err(format!(
                "{class} has a field '{name}' no call reads"
            )));
        }
        let value = options.getattr(name)?;
        let v = Value {
            call: &call,
            name,
            value: &value,
        };
        let s = &mut read.selection;
        match name {
            "dedupe" => s.dedupe = v.flag()?,
            "max_weight_classes" => {
                // A count past i32::MAX keeps every class, as the setting's
                // default does.
                s.max_weight_classes = v.count(1)?.map(|n| i32::try_from(n).unwrap_or(i32::MAX));
            }
            "beam" => {
                s.beam = v
                    .number("a number of at least 0", |b| b >= 0.0)?
                    .map(|b| b as f32);
            }
            "time_limit" => {
                s.time_limit = v.number("a number of seconds, 0 or more", |t| t >= 0.0)?;
            }
            "tokenize_multichar" => s.tokenize_multichar = v.flag()?,
            "print_all" => s.print_all = v.flag()?,
            "weight_cutoff" => {
                read.weight_cutoff = v
                    .number("a number, not NaN", |w| !w.is_nan())?
                    .map(|w| w as f32);
            }
            "print_weights" => read.print_weights = v.flag()?,
            "hack_uncompose" => read.hack_uncompose = v.flag()?,
            "newline" => read.newline = v.flag()?,
            "keep_newline" => read.keep_newline = v.flag()?,
            "superblanks" => read.superblanks = v.flag()?,
            _ => {
                return Err(PyTypeError::new_err(format!(
                    "{class} has a field '{name}' no call reads"
                )));
            }
        }
    }
    Ok(read)
}

const SELECTION: [&str; 6] = [
    "dedupe",
    "max_weight_classes",
    "beam",
    "time_limit",
    "tokenize_multichar",
    "print_all",
];

fn tokenise_options(options: &Bound<'_, PyAny>) -> PyResult<TokeniseOptions> {
    let accepted = [&SELECTION[..], &["weight_cutoff"]].concat();
    let read = fields(options, &accepted)?;
    Ok(TokeniseOptions {
        selection: read.selection,
        weight_cutoff: read.weight_cutoff,
    })
}

fn format_options(options: &Bound<'_, PyAny>) -> PyResult<FormatOptions> {
    let accepted = [
        &SELECTION[..],
        &[
            "print_weights",
            "hack_uncompose",
            "newline",
            "keep_newline",
            "superblanks",
        ],
    ]
    .concat();
    let read = fields(options, &accepted)?;
    Ok(FormatOptions {
        selection: read.selection,
        print_weights: read.print_weights,
        hack_uncompose: read.hack_uncompose,
        newline: read.newline,
        keep_newline: read.keep_newline,
        superblanks: read.superblanks,
    })
}

/// Checks a `divvun_hfst.TokeniseOptions` as it is made, raising what a
/// call reading its fields would raise.
#[pyfunction]
#[pyo3(name = "_check_tokenise_options")]
pub fn check_tokenise_options(options: &Bound<'_, PyAny>) -> PyResult<()> {
    tokenise_options(options).map(drop)
}

/// Checks a `divvun_hfst.FormatOptions` as it is made, raising what a call
/// reading its fields would raise.
#[pyfunction]
#[pyo3(name = "_check_format_options")]
pub fn check_format_options(options: &Bound<'_, PyAny>) -> PyResult<()> {
    format_options(options).map(drop)
}

// [spec:hfst:req:python.load]
// [spec:hfst:req:python.threads]
/// Reads a tokeniser from `data` without the GIL.
fn read(py: Python<'_>, data: &[u8]) -> PyResult<Tokeniser> {
    let read = without_gil(py, || {
        tokenise::read_tokeniser(data, &crate::PACKAGE).map(|run| run.core())
    })?;
    let core = read.map_err(|error| format_error(py, error))?;
    Ok(Tokeniser { core })
}

// [spec:hfst:req:python.load]
/// The tokeniser in a pmatch container's bytes, as `hfst pmatch2fst` writes
/// them. Bytes that are not a pmatch container raise FormatError with code
/// `invalid-format`, among them a transducer that is not one: `hfst
/// tokenise` wraps such a transducer in a tokeniser of its own, which this
/// package does not make.
#[pyfunction]
pub fn read_tokeniser(py: Python<'_>, data: PyBackedBytes) -> PyResult<Tokeniser> {
    read(py, &data)
}

// [spec:hfst:req:python.load]
/// The tokeniser in the pmatch container at `path`, as read_tokeniser reads
/// its bytes. A file that cannot be read raises the OSError open() raises.
#[pyfunction]
pub fn load_tokeniser(py: Python<'_>, path: PathBuf) -> PyResult<Tokeniser> {
    let data = read_file(py, &path)?;
    read(py, &data)
}
