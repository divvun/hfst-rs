//! The compilers: what `hfst regexp2fst`, `lexc`, `twolc`, `xfst`,
//! `pmatch2fst`, `strings2fst` and `txt2fst` make of their sources, with the
//! options of the dataclasses in `python/divvun_hfst/_options.py`. A compile
//! that fails raises CompileError with every diagnostic it made; one that
//! succeeds issues each of its warnings through `warnings.warn`.

use std::ops::Range;
use std::sync::Arc;

use hfst_embed::compile::{
    self as embed, CompileResult, Diagnostic, FileResolver, FsResolver, LexcOptions as Lexc,
    LexcVerbosity, LogBase, MemoryFiles, Pmatch2FstOptions, RegexpOptions, Separation, Severity,
    Source, StringsOptions as Strings, TwolcOptions as Twolc, Txt2FstOptions, XfstOptions as Xfst,
};
use hfst_embed::format::Format;
use hfst_embed::{Code, Error};
use pyo3::create_exception;
use pyo3::exceptions::{PyTypeError, PyUserWarning, PyValueError};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedStr;
use pyo3::types::{PyBool, PyByteArray, PyBytes, PyDict, PyMapping, PyString, PyTuple};

use crate::args::Value;
use crate::call::without_gil;
use crate::error::HfstError;
use crate::results::result;
use crate::transducer::Transducer;

// [spec:hfst:req:python.compile-errors]
create_exception!(
    divvun_hfst,
    CompileError,
    HfstError,
    "A compile that failed. `diagnostics` holds every diagnostic it made, as \
     Diagnostic objects, and `code` names the failure as HfstError's does."
);
create_exception!(
    divvun_hfst,
    CompileWarning,
    PyUserWarning,
    "A warning of a compile that succeeded, issued through warnings.warn. \
     `diagnostic` is the Diagnostic it stands for."
);

/// What the standard input is called when a tool reads its source there,
/// and so what a source is called unless named.
const STDIN: &str = "<stdin>";

/// The code points of `source` before byte `at`, as `str` indices count. An
/// offset inside a character counts the characters before it.
fn code_points(source: &str, at: usize) -> usize {
    let mut at = at.min(source.len());
    while !source.is_char_boundary(at) {
        at -= 1;
    }
    source[..at].chars().count()
}

/// The span `span`, a byte range of the diagnostic's source, in code points.
/// A diagnostic with no source text keeps its offsets, having no text to
/// count them in.
fn span_of(source: &str, span: &Range<usize>) -> (usize, usize) {
    if source.is_empty() {
        return (span.start, span.end);
    }
    (
        code_points(source, span.start),
        code_points(source, span.end),
    )
}

// [spec:hfst:req:python.compile-errors]
/// `d` as a `Diagnostic`: its spans in code points, and its text rendered
/// without colour.
fn diagnostic<'py>(py: Python<'py>, d: &Diagnostic) -> PyResult<Bound<'py, PyAny>> {
    let source = d.source();
    let labels = d
        .labels()
        .iter()
        .map(|label| {
            result!(py, "DiagnosticLabel" {
                "span": span_of(source, &label.span),
                "message": label.message.as_str(),
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    let severity = match d.severity() {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    };
    result!(py, "Diagnostic" {
        "severity": severity,
        "message": d.message(),
        "source_name": d.source_name(),
        "span": d.primary_span().map(|span| span_of(source, &span)),
        "labels": PyTuple::new(py, labels)?,
        "notes": PyTuple::new(py, d.notes())?,
        "help": d.help_text(),
        "rendered": d.render(false),
    })
}

// [spec:hfst:req:python.compile-errors]
fn compile_error(py: Python<'_>, error: &Error, diagnostics: &[Diagnostic]) -> PyErr {
    let raised = CompileError::new_err(error.message().to_owned());
    let carried = (|| -> PyResult<()> {
        let value = raised.value(py);
        value.setattr("code", error.code().name())?;
        let diagnostics = diagnostics
            .iter()
            .map(|d| diagnostic(py, d))
            .collect::<PyResult<Vec<_>>>()?;
        value.setattr("diagnostics", PyTuple::new(py, diagnostics)?)
    })();
    match carried {
        Ok(()) => raised,
        Err(failed) => failed,
    }
}

// [spec:hfst:req:python.compile-errors]
/// Issues each warning of a compile that succeeded through `warnings.warn`,
/// as a CompileWarning carrying its Diagnostic. An error a compile made
/// without failing, as a command of an xfst script does, is issued the same
/// way. What the compile says at the info level, as with `verbose`, goes to
/// the `hfst` logger at the info level.
fn issue(py: Python<'_>, diagnostics: &[Diagnostic]) -> PyResult<()> {
    if diagnostics.is_empty() {
        return Ok(());
    }
    let warn = py.import("warnings")?.getattr("warn")?;
    let category = py.get_type::<CompileWarning>();
    for d in diagnostics {
        match d.severity() {
            Severity::Error | Severity::Warning => {
                let warning = category.call1((d.message(),))?;
                warning.setattr("diagnostic", diagnostic(py, d)?)?;
                warn.call1((warning,))?;
            }
            Severity::Info => {
                py.import("logging")?
                    .call_method1("getLogger", ("hfst",))?
                    .call_method1("info", (d.render(false).trim_end(),))?;
            }
        }
    }
    Ok(())
}

/// The value of a compile, raising CompileError when it failed and issuing
/// its warnings when it did not. `check` may still refuse the value, raising
/// a CompileError of its own that carries the compile's diagnostics.
fn finished<T, U>(
    py: Python<'_>,
    compiled: CompileResult<T>,
    check: impl FnOnce(T) -> Result<U, Error>,
) -> PyResult<U> {
    match compiled {
        Ok(compiled) => match check(compiled.value) {
            Ok(value) => {
                issue(py, &compiled.diagnostics)?;
                Ok(value)
            }
            Err(error) => Err(compile_error(py, &error, &compiled.diagnostics)),
        },
        Err(failed) => Err(compile_error(py, &failed.error, &failed.diagnostics)),
    }
}

/// The one transducer a compile made, or the `transducer-count` error that
/// names `call` and what makes one of several, `one_of`.
fn one(
    call: &str,
    one_of: &str,
) -> impl FnOnce(Vec<embed::Transducer>) -> Result<embed::Transducer, Error> {
    move |mut all| {
        if all.len() == 1
            && let Some(t) = all.pop()
        {
            return Ok(t);
        }
        Err(Error::new(
            Code::TransducerCount,
            format!(
                "{call} returns one transducer, and the source compiled to {}; {one_of}",
                all.len()
            ),
        ))
    }
}

// [spec:hfst:req:python.compile]
/// The files a compile reads and writes: those of `given`, a mapping from
/// names to `str` or `bytes`, held in memory, or else the filesystem's.
fn files(call: &str, given: Option<&Bound<'_, PyAny>>) -> PyResult<Option<MemoryFiles>> {
    let Some(given) = given.filter(|g| !g.is_none()) else {
        return Ok(None);
    };
    let wrong = || {
        Value {
            call,
            name: "files",
            value: given,
        }
        .wrong_type("a mapping of str to str or bytes")
    };
    let mapping = given.cast::<PyMapping>().map_err(|_| wrong())?;
    let memory = MemoryFiles::default();
    for item in mapping.items()?.iter() {
        let (name, contents): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item.extract()?;
        let name: PyBackedStr = name.extract().map_err(|_| wrong())?;
        let contents = if let Ok(text) = contents.cast::<PyString>() {
            text.to_str()?.as_bytes().to_vec()
        } else if let Ok(bytes) = contents.cast::<PyBytes>() {
            bytes.as_bytes().to_vec()
        } else if let Ok(bytes) = contents.cast::<PyByteArray>() {
            bytes.to_vec()
        } else {
            return Err(wrong());
        };
        memory.insert(&*name, contents);
    }
    Ok(Some(memory))
}

fn resolver(memory: &Option<MemoryFiles>) -> Arc<dyn FileResolver> {
    match memory {
        Some(memory) => Arc::new(memory.clone()),
        None => Arc::new(FsResolver),
    }
}

/// The fields of one of the option dataclasses, read by name. Each field of
/// the wrong type raises the TypeError a typed parameter would, and each
/// value the tool would turn down a ValueError.
struct Fields<'a, 'py> {
    options: &'a Bound<'py, PyAny>,
    call: String,
}

impl<'py> Fields<'_, 'py> {
    fn value(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.options.getattr(name)
    }

    fn flag(&self, name: &str) -> PyResult<bool> {
        let value = self.value(name)?;
        let read = Value {
            call: &self.call,
            name,
            value: &value,
        };
        value
            .cast::<PyBool>()
            .map(|b| b.is_true())
            .map_err(|_| read.wrong_type("bool"))
    }

    fn text(&self, name: &str) -> PyResult<String> {
        let value = self.value(name)?;
        value.extract::<String>().map_err(|_| {
            Value {
                call: &self.call,
                name,
                value: &value,
            }
            .wrong_type("str")
        })
    }

    fn texts(&self, name: &str) -> PyResult<Vec<String>> {
        let value = self.value(name)?;
        let wrong = || {
            Value {
                call: &self.call,
                name,
                value: &value,
            }
            .wrong_type("a sequence of str")
        };
        if value.is_instance_of::<PyString>() {
            return Err(wrong());
        }
        value.extract::<Vec<String>>().map_err(|_| wrong())
    }

    fn format(&self, allowed: &[Format]) -> PyResult<Format> {
        let name = self.text("format")?;
        if let Some(format) = allowed.iter().find(|f| f.name() == name) {
            return Ok(*format);
        }
        let names: Vec<String> = allowed.iter().map(|f| format!("'{}'", f.name())).collect();
        Err(PyValueError::new_err(format!(
            "{} argument 'format' must be one of {}, not '{name}'",
            self.call,
            names.join(", ")
        )))
    }

    /// Refuses two flags that ask for opposite things.
    fn not_both(&self, first: &str, second: &str) -> PyResult<()> {
        if self.flag(first)? && self.flag(second)? {
            return Err(PyValueError::new_err(format!(
                "{} takes '{first}' or '{second}', not both",
                self.call
            )));
        }
        Ok(())
    }
}

const ALGEBRA: [Format; 2] = [Format::OpenFstTropical, Format::Foma];

/// The fields of `options`, an instance of `class` from
/// `divvun_hfst._options`, or of a fresh one when `options` is None, as a
/// parameter of `call` taking that class reads them.
fn fields<'a, 'py>(
    py: Python<'py>,
    options: Option<&'a Bound<'py, PyAny>>,
    class: &str,
    call: &str,
    made: &'a mut Option<Bound<'py, PyAny>>,
) -> PyResult<Fields<'a, 'py>> {
    let expected = py.import("divvun_hfst._options")?.getattr(class)?;
    let options = match options.filter(|o| !o.is_none()) {
        Some(options) => {
            if !options.is_instance(&expected)? {
                let given = options.get_type().name()?;
                return Err(PyTypeError::new_err(format!(
                    "{call} argument 'options' must be {class} or None, not {given}"
                )));
            }
            options
        }
        None => made.insert(expected.call0()?),
    };
    Ok(Fields {
        options,
        call: format!("{class}()"),
    })
}

fn regex_options(f: &Fields<'_, '_>) -> PyResult<(Format, RegexpOptions)> {
    Ok((
        f.format(&Format::ALL)?,
        RegexpOptions {
            separation: if f.flag("semicolon")? {
                Separation::Semicolons
            } else {
                Separation::Lines
            },
            disjunct: f.flag("disjunct")?,
            harmonize: !f.flag("do_not_harmonize")?,
            harmonize_flags: f.flag("harmonize_flags")?,
            minimize: !f.flag("do_not_minimize")?,
            flag_is_epsilon: f.flag("flag_is_epsilon")?,
            xerox_composition: f.flag("xerox_composition")?,
            encode_weights: f.flag("encode_weights")?,
            replace_pass: f.flag("replace_pass")?,
            verbose: f.flag("verbose")?,
        },
    ))
}

fn lexc_options(f: &Fields<'_, '_>) -> PyResult<(Format, Lexc)> {
    f.not_both("quiet", "verbose")?;
    let verbosity = if f.flag("quiet")? {
        LexcVerbosity::Quiet
    } else if f.flag("verbose")? {
        LexcVerbosity::Verbose
    } else {
        LexcVerbosity::Normal
    };
    Ok((
        f.format(&Format::ALL)?,
        Lexc {
            align_strings: f.flag("align_strings")?,
            with_flags: f.flag("with_flags")?,
            minimize_flags: f.flag("minimize_flags")?,
            rename_flags: f.flag("rename_flags")?,
            warnings_as_errors: f.flag("werror")?,
            warn_one_sided_flags: f.flag("warn_one_sided_flags")?,
            warn_unused_lexicons: f.flag("warn_unused_lexicons")?,
            warn_repeated_lexicons: f.flag("warn_repeated_lexicons")?,
            warn_missing_lexicons: f.flag("warn_missing_lexicons")?,
            warn_missing_alphabets: f.flag("warn_missing_alphabets")?,
            warn_unnecessary_escapes: f.flag("warn_unnecessary_escapes")?,
            xerox_composition: f.flag("xerox_composition")?,
            encode_weights: f.flag("encode_weights")?,
            flag_is_epsilon: f.flag("flag_is_epsilon")?,
            split_characters: f.flag("split_characters")?,
            verbosity,
        },
    ))
}

fn twolc_options(f: &Fields<'_, '_>) -> PyResult<(Format, Twolc)> {
    f.not_both("quiet", "verbose")?;
    Ok((
        f.format(&ALGEBRA)?,
        Twolc {
            quiet: f.flag("quiet")?,
            verbose: f.flag("verbose")?,
            resolve_left_conflicts: f.flag("resolve")?,
            resolve_right_conflicts: !f.flag("dont_resolve_right")?,
        },
    ))
}

fn xfst_options(f: &Fields<'_, '_>) -> PyResult<(Format, Xfst)> {
    Ok((
        f.format(&ALGEBRA)?,
        Xfst {
            print_weight: f.flag("print_weight")?,
            restricted_mode: f.flag("restricted_mode")?,
            silent: f.flag("silent")?,
        },
    ))
}

fn pmatch_options(f: &Fields<'_, '_>) -> PyResult<Pmatch2FstOptions> {
    Ok(Pmatch2FstOptions {
        flatten: f.flag("flatten")?,
        include_cosine_distances: f.flag("cosine_distances")?,
        include_path: String::new(),
        verbose: f.flag("verbose")?,
    })
}

fn strings_options(f: &Fields<'_, '_>) -> PyResult<(Format, Strings)> {
    f.not_both("log", "log10")?;
    let log = if f.flag("log")? {
        Some(LogBase::E)
    } else if f.flag("log10")? {
        Some(LogBase::Ten)
    } else {
        None
    };
    Ok((
        f.format(&Format::ALL)?,
        Strings {
            epsilon: f.text("epsilon")?,
            multichar_symbols: f.texts("multichar_symbols")?,
            pairstrings: f.flag("pairstrings")?,
            has_spaces: f.flag("has_spaces")?,
            disjunct: f.flag("disjunct_strings")?,
            normalize: f.flag("norm")?,
            log,
            warn_negative_weights: f.flag("warn_negative_weights")?,
            warnings_are_errors: f.flag("werror")?,
        },
    ))
}

fn att_options(f: &Fields<'_, '_>) -> PyResult<(Format, Txt2FstOptions)> {
    Ok((
        f.format(&Format::ALL)?,
        Txt2FstOptions {
            prolog: false,
            epsilon: f.text("epsilon")?,
            check_negative_epsilon_cycles: f.flag("check_negative_epsilon_cycles")?,
            warn_negative_weights: f.flag("warn_negative_weights")?,
            disjunct: f.flag("disjunct")?,
        },
    ))
}

fn prolog_options(f: &Fields<'_, '_>) -> PyResult<(Format, Txt2FstOptions)> {
    Ok((
        f.format(&Format::ALL)?,
        Txt2FstOptions {
            prolog: true,
            check_negative_epsilon_cycles: f.flag("check_negative_epsilon_cycles")?,
            ..Txt2FstOptions::default()
        },
    ))
}

// [spec:hfst:req:python.compile]
/// Checks one of the compile option dataclasses as it is made, raising what
/// a compiler reading its fields would raise.
#[pyfunction]
#[pyo3(name = "_check_compile_options")]
pub fn check_compile_options(options: &Bound<'_, PyAny>) -> PyResult<()> {
    let class = options.get_type().name()?.to_string();
    let f = Fields {
        options,
        call: format!("{class}()"),
    };
    match class.as_str() {
        "RegexOptions" => regex_options(&f).map(drop),
        "LexcOptions" => lexc_options(&f).map(drop),
        "TwolcOptions" => twolc_options(&f).map(drop),
        "XfstOptions" => xfst_options(&f).map(drop),
        "PmatchOptions" => pmatch_options(&f).map(drop),
        "StringsOptions" => strings_options(&f).map(drop),
        "AttOptions" => att_options(&f).map(drop),
        "PrologOptions" => prolog_options(&f).map(drop),
        _ => Err(PyTypeError::new_err(format!(
            "{class} is not one of the compile options"
        ))),
    }
}

// [spec:hfst:req:python.compile]
/// The transducer `hfst regexp2fst` writes of the regular expression
/// `source`. With RegexOptions(disjunct=True) a source of several
/// expressions, one a line or with `semicolon` each ending in `;`, compiles
/// to their union; without it, a source of more than one raises CompileError
/// with code `transducer-count`. `name` is the file name the tool would read
/// the source from, which diagnostics give it. Each file the expression
/// reads comes from `files` when it is given, and from the filesystem
/// otherwise. A source that does not compile raises CompileError, and each
/// warning of one that does is issued as a CompileWarning.
#[pyfunction]
#[pyo3(signature = (source, /, options = None, *, name = "<stdin>", files = None))]
pub fn regex(
    py: Python<'_>,
    source: &str,
    options: Option<&Bound<'_, PyAny>>,
    name: &str,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<Transducer> {
    let mut made = None;
    let f = fields(py, options, "RegexOptions", "regex()", &mut made)?;
    let (format, options) = regex_options(&f)?;
    let memory = self::files("regex()", files)?;
    let files = resolver(&memory);
    let compiled = without_gil(py, || {
        embed::compile_regexps(Source { name, text: source }, format, &options, files)
    })?;
    let one = one(
        "regex()",
        "RegexOptions(disjunct=True) compiles a source of several expressions to their union",
    );
    finished(py, compiled, one).map(Transducer::new)
}

// [spec:hfst:req:python.compile]
/// The transducer `hfst lexc` writes of the lexc `sources`, compiled
/// together and named after the first. `names` are the file names the tool
/// would read them from, which diagnostics and the result's name give them;
/// each is `<stdin>` unless given. Each file their regular expressions read
/// comes from `files` when it is given, and from the filesystem otherwise.
#[pyfunction]
#[pyo3(signature = (sources, /, options = None, *, names = None, files = None))]
pub fn compile_lexc(
    py: Python<'_>,
    sources: Vec<String>,
    options: Option<&Bound<'_, PyAny>>,
    names: Option<Vec<String>>,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<Transducer> {
    let mut made = None;
    let f = fields(py, options, "LexcOptions", "compile_lexc()", &mut made)?;
    let (format, options) = lexc_options(&f)?;
    let names = names.unwrap_or_else(|| vec![STDIN.to_owned(); sources.len()]);
    if names.len() != sources.len() {
        return Err(PyValueError::new_err(format!(
            "compile_lexc() argument 'names' must name each of the {} sources, not {}",
            sources.len(),
            names.len()
        )));
    }
    let memory = self::files("compile_lexc()", files)?;
    let files = resolver(&memory);
    let compiled = without_gil(py, || {
        let sources: Vec<Source<'_>> = names
            .iter()
            .zip(&sources)
            .map(|(name, text)| Source { name, text })
            .collect();
        embed::compile_lexc(&sources, format, &options, files)
    })?;
    finished(py, compiled, Ok).map(Transducer::new)
}

// [spec:hfst:req:python.compile]
/// The rule transducers `hfst twolc` writes of the twolc grammar `source`, in
/// tropical OpenFst or foma. `name` is the file name the tool would read it
/// from, `<twolc>` for its standard input. A twolc grammar names no file, so
/// `files` has none to supply.
#[pyfunction]
#[pyo3(signature = (source, /, options = None, *, name = "<twolc>", files = None))]
pub fn compile_twolc(
    py: Python<'_>,
    source: &str,
    options: Option<&Bound<'_, PyAny>>,
    name: &str,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Transducer>> {
    let mut made = None;
    let f = fields(py, options, "TwolcOptions", "compile_twolc()", &mut made)?;
    let (format, options) = twolc_options(&f)?;
    self::files("compile_twolc()", files)?;
    let compiled = without_gil(py, || {
        embed::compile_twolc(Source { name, text: source }, format, &options)
    })?;
    let rules = finished(py, compiled, Ok)?;
    Ok(rules.into_iter().map(Transducer::new).collect())
}

// [spec:hfst:req:python.compile]
/// Runs the xfst script `source` as `hfst xfst -F` runs it, in tropical
/// OpenFst or foma, with `input` as the standard input its commands read:
/// what it printed, its stack, its defined networks and the files it wrote.
/// Each file it reads comes from `files` when it is given, and each it
/// writes lands there; without `files`, it reads and writes the filesystem.
/// Either way `XfstResult.written` holds each file it wrote. A command that
/// fails is a warning, and the script raises CompileError only when it stops
/// on one.
#[pyfunction]
#[pyo3(signature = (source, /, options = None, *, input = "", name = "<stdin>", files = None))]
pub fn compile_xfst<'py>(
    py: Python<'py>,
    source: &str,
    options: Option<&Bound<'_, PyAny>>,
    input: &str,
    name: &str,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut made = None;
    let f = fields(py, options, "XfstOptions", "compile_xfst()", &mut made)?;
    let (format, options) = xfst_options(&f)?;
    let memory = self::files("compile_xfst()", files)?;
    let files = resolver(&memory);
    let compiled = without_gil(py, || {
        embed::compile_xfst(
            Source { name, text: source },
            input,
            format,
            &options,
            files,
        )
    })?;
    let ran = finished(py, compiled, Ok)?;
    let written: Vec<(String, std::io::Result<Vec<u8>>)> = without_gil(py, || {
        ran.written
            .into_iter()
            .map(|file| {
                let bytes = match &memory {
                    Some(memory) => memory.get(&file).ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::NotFound, "no such file")
                    }),
                    None => std::fs::read(&file),
                };
                (file, bytes)
            })
            .collect()
    })?;
    let files = PyDict::new(py);
    for (file, bytes) in written {
        let bytes = bytes
            .map_err(|error| crate::error::os_error(py, error, std::path::Path::new(&file)))?;
        files.set_item(file, PyBytes::new(py, &bytes))?;
    }
    let definitions = PyDict::new(py);
    for (defined, t) in ran.definitions {
        definitions.set_item(defined, Transducer::new(t))?;
    }
    let stack: Vec<Transducer> = ran.stack.into_iter().map(Transducer::new).collect();
    result!(py, "XfstResult" {
        "output": ran.output,
        "stack": PyTuple::new(py, stack)?,
        "definitions": definitions,
        "written": files,
    })
}

// [spec:hfst:req:python.compile]
/// The archive `hfst pmatch2fst` writes of the pmatch script `source`, which
/// read_tokeniser reads. `name` is the file name the tool would read it
/// from: a relative `@bin"FILE"` or `@txt"FILE"` is found beside it, as the
/// tool finds one beside its input. Each file the script reads comes from
/// `files` when it is given, and from the filesystem otherwise.
#[pyfunction]
#[pyo3(signature = (source, /, options = None, *, name = "<stdin>", files = None))]
pub fn compile_pmatch<'py>(
    py: Python<'py>,
    source: &str,
    options: Option<&Bound<'_, PyAny>>,
    name: &str,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<Bound<'py, PyBytes>> {
    let mut made = None;
    let f = fields(py, options, "PmatchOptions", "compile_pmatch()", &mut made)?;
    let mut options = pmatch_options(&f)?;
    if name != STDIN
        && let Some(slash) = name.rfind('/')
    {
        options.include_path = name[..=slash].to_owned();
    }
    let memory = self::files("compile_pmatch()", files)?;
    let files = resolver(&memory);
    let compiled = without_gil(py, || {
        embed::compile_pmatch(Source { name, text: source }, &options, files)
    })?;
    let archive = finished(py, compiled, Ok)?;
    Ok(PyBytes::new(py, &archive))
}

// [spec:hfst:req:python.compile]
/// The transducer `hfst strings2fst` writes of the strings `source`, one a
/// line, each with an optional tab and weight. With
/// StringsOptions(disjunct_strings=True) the lines compile to their union;
/// without it, a source of more than one line raises CompileError with code
/// `transducer-count`. Strings name no file, so `files` has none to supply.
#[pyfunction]
#[pyo3(signature = (source, /, options = None, *, name = "<stdin>", files = None))]
pub fn compile_strings(
    py: Python<'_>,
    source: &str,
    options: Option<&Bound<'_, PyAny>>,
    name: &str,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<Transducer> {
    let mut made = None;
    let f = fields(
        py,
        options,
        "StringsOptions",
        "compile_strings()",
        &mut made,
    )?;
    let (format, options) = strings_options(&f)?;
    self::files("compile_strings()", files)?;
    let compiled = without_gil(py, || {
        embed::compile_strings(Source { name, text: source }, format, &options)
    })?;
    let one = one(
        "compile_strings()",
        "StringsOptions(disjunct_strings=True) compiles a source of several lines to their union",
    );
    finished(py, compiled, one).map(Transducer::new)
}

// [spec:hfst:req:python.compile]
/// The transducer `hfst txt2fst` writes of the AT&T text `source`, named
/// after `name`. With AttOptions(disjunct=True) a text of several
/// transducers, separated by `--` lines, reads as their union; without it, a
/// text of more than one raises CompileError with code `transducer-count`.
/// AT&T text names no file, so `files` has none to supply.
#[pyfunction]
#[pyo3(signature = (source, /, options = None, *, name = "<stdin>", files = None))]
pub fn parse_att(
    py: Python<'_>,
    source: &str,
    options: Option<&Bound<'_, PyAny>>,
    name: &str,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<Transducer> {
    let mut made = None;
    let f = fields(py, options, "AttOptions", "parse_att()", &mut made)?;
    let (format, options) = att_options(&f)?;
    self::files("parse_att()", files)?;
    let compiled = without_gil(py, || {
        embed::compile_text(Source { name, text: source }, format, &options)
    })?;
    let one = one(
        "parse_att()",
        "AttOptions(disjunct=True) reads a text of several transducers as their union",
    );
    finished(py, compiled, one).map(Transducer::new)
}

// [spec:hfst:req:python.compile]
/// The transducer `hfst txt2fst --prolog` writes of the Prolog text
/// `source`, named after `name`. A text of more than one network raises
/// CompileError with code `transducer-count`. Prolog text names no file, so
/// `files` has none to supply.
#[pyfunction]
#[pyo3(signature = (source, /, options = None, *, name = "<stdin>", files = None))]
pub fn parse_prolog(
    py: Python<'_>,
    source: &str,
    options: Option<&Bound<'_, PyAny>>,
    name: &str,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<Transducer> {
    let mut made = None;
    let f = fields(py, options, "PrologOptions", "parse_prolog()", &mut made)?;
    let (format, options) = prolog_options(&f)?;
    self::files("parse_prolog()", files)?;
    let compiled = without_gil(py, || {
        embed::compile_text(Source { name, text: source }, format, &options)
    })?;
    let one = one("parse_prolog()", "give the text of one network");
    finished(py, compiled, one).map(Transducer::new)
}
