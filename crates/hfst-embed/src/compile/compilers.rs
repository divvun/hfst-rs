//! The compilers of the compile surface: what `regexp2fst`, `lexc`, `twolc`,
//! `xfst`, `pmatch2fst`, `strings2fst` and `txt2fst` make of their sources,
//! each with every diagnostic it made.

use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use hfst::diag::DiagnosticSink;
use hfst::file_resolver::FileResolver;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_transducer::{AnyTransducer, FromAnyTransducer, HfstTransducer};
use hfst::tools::regexp2fst::RegexpError;
use hfst::tools::strings2fst::{LineError, negative_weight};
use hfst::tools::txt2fst::{TextEvent, read_text};
use hfst::xfst_compiler::XfstCompiler;
use hfst_openfst::StdVectorFst;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

use super::transducer::{Transducer, implementation_type, read_transducers, stream_bytes};
use crate::error::{Code, Error};
use crate::format::Format;

pub use hfst::diag::{Diagnostic, Label, Severity};
pub use hfst::tools::lexc::{LexcOptions, LexcVerbosity};
pub use hfst::tools::pmatch2fst::Pmatch2FstOptions;
pub use hfst::tools::regexp2fst::{RegexpOptions, Separation};
pub use hfst::tools::strings2fst::{LogBase, StringsOptions};
pub use hfst::tools::twolc::TwolcOptions;
pub use hfst::tools::txt2fst::Txt2FstOptions;
pub use hfst::tools::xfst::XfstOptions;

/// A source to compile: its text, and the file name its diagnostics give it.
#[derive(Clone, Copy, Debug)]
pub struct Source<'a> {
    pub name: &'a str,
    pub text: &'a str,
}

// [spec:hfst:req:embed.compile-diagnostics]
/// What a compile made, and every diagnostic it made on the way.
#[derive(Debug)]
pub struct Compiled<T> {
    pub value: T,
    pub diagnostics: Vec<Diagnostic>,
}

// [spec:hfst:req:embed.compile-diagnostics]
/// Why a compile failed, and every diagnostic it made before it did. A
/// compile that fails gives no transducer.
#[derive(Debug)]
pub struct CompileError {
    pub error: Error,
    pub diagnostics: Vec<Diagnostic>,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for CompileError {}

pub type CompileResult<T> = Result<Compiled<T>, CompileError>;

fn failure(error: Error, diagnostics: Vec<Diagnostic>) -> CompileError {
    CompileError { error, diagnostics }
}

/// The diagnostics of one compile, in the order they were made: the
/// compiler's own, through its sink, and what the library says through
/// `tracing` meanwhile, which the tools print as they go.
#[derive(Clone, Default)]
struct Gathered(Arc<Mutex<Vec<Diagnostic>>>);

impl Gathered {
    fn push(&self, diagnostic: Diagnostic) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(diagnostic);
    }

    fn sink(&self) -> DiagnosticSink {
        let gathered = self.clone();
        DiagnosticSink::new(move |d| gathered.push(d))
    }

    /// Run `compile`, keeping its `tracing` events of level INFO and above.
    fn run<T>(&self, compile: impl FnOnce() -> T) -> T {
        let layer = Traced(self.clone()).with_filter(LevelFilter::INFO);
        tracing::subscriber::with_default(tracing_subscriber::registry().with(layer), compile)
    }

    fn take(&self) -> Vec<Diagnostic> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

struct Traced(Gathered);

impl<S: tracing::Subscriber> Layer<S> for Traced {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let severity = match *event.metadata().level() {
            tracing::Level::ERROR => Severity::Error,
            tracing::Level::WARN => Severity::Warning,
            _ => Severity::Info,
        };
        let mut message = Message(String::new());
        event.record(&mut message);
        self.0.push(Diagnostic::new(severity, message.0.trim_end()));
    }
}

/// The `message` field of an event.
struct Message(String);

impl tracing::field::Visit for Message {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

/// `ts`, compiled at tropical, as a compiler tool writes them to a stream of
/// `format`. They are converted rather than read back from the stream,
/// which would name a transducer that has no name.
fn as_written(
    ts: Vec<HfstTransducer<StdVectorFst>>,
    format: Format,
) -> Result<Vec<Transducer>, Error> {
    let ty = implementation_type(format);
    ts.into_iter()
        .map(|t| {
            let tropical = AnyTransducer::Tropical(t);
            Ok(Transducer::new(if format == Format::OpenFstTropical {
                tropical
            } else {
                tropical.convert(ty, "")?
            }))
        })
        .collect()
}

/// The byte range of the line numbered `line`, from 1, in `text`, without
/// its newline; empty at the end of the text for a line past it.
fn line_span(text: &str, line: usize) -> Range<usize> {
    let mut start = 0;
    for (number, piece) in text.split_inclusive('\n').enumerate() {
        if number + 1 == line {
            let content = piece.trim_end_matches(['\n', '\r']);
            return start..start + content.len();
        }
        start += piece.len();
    }
    text.len()..text.len()
}

fn at_line(source: &Source<'_>, line: usize, diagnostic: Diagnostic) -> Diagnostic {
    diagnostic
        .label(line_span(source.text, line), "")
        .in_source(source.name, Arc::from(source.text))
}

/// The refusal of optimized lookup, which `compiler` does not build in.
fn not_an_algebra(compiler: &str) -> CompileError {
    failure(
        Error::new(
            Code::InvalidArgument,
            format!("{compiler} compiles to tropical OpenFst or foma, not optimized lookup"),
        ),
        Vec::new(),
    )
}

// -----------------------------------------------------------------------------
// regexp2fst, lexc, strings2fst and txt2fst
// -----------------------------------------------------------------------------

// [spec:hfst:req:embed.compile]
/// The transducers `hfst regexp2fst` writes of `source` in `format`, the
/// files its expressions name read through `files`.
pub fn compile_regexps(
    source: Source<'_>,
    format: Format,
    options: &RegexpOptions,
    files: Arc<dyn FileResolver>,
) -> CompileResult<Vec<Transducer>> {
    let gathered = Gathered::default();
    let mut xre = options.compiler::<StdVectorFst>(source.name);
    xre.set_file_resolver(files);
    xre.set_diagnostic_sink(gathered.sink());
    let mut compiled = Vec::new();
    let result = gathered.run(|| {
        hfst::tools::regexp2fst::compile_regexps(
            &mut xre,
            &mut source.text.as_bytes(),
            options,
            &mut |event| {
                if let hfst::tools::regexp2fst::RegexpEvent::Compiled(t) = event {
                    compiled.push(t);
                }
                Ok(())
            },
        )
    });
    let diagnostics = gathered.take();
    let error = match result {
        Ok(()) => {
            return match as_written(compiled, format) {
                Ok(value) => Ok(Compiled { value, diagnostics }),
                Err(e) => Err(failure(e, diagnostics)),
            };
        }
        Err(RegexpError::Operation(e)) => Error::from(e),
        Err(RegexpError::LineFailed { line }) => Error::new(
            Code::Failed,
            format!("{}:{line}: XRE parsing failed", source.name),
        ),
        Err(e) => Error::new(Code::Failed, e.to_string()),
    };
    Err(failure(error, diagnostics))
}

// [spec:hfst:req:embed.compile]
/// The transducer `hfst lexc` writes in `format` of `sources`, compiled
/// together and named after the first, the files their regexes name read
/// through `files`.
pub fn compile_lexc(
    sources: &[Source<'_>],
    format: Format,
    options: &LexcOptions,
    files: Arc<dyn FileResolver>,
) -> CompileResult<Transducer> {
    let gathered = Gathered::default();
    let mut lexc = options.compiler::<StdVectorFst>();
    lexc.set_file_resolver(files);
    lexc.set_diagnostic_sink(gathered.sink());
    let named: Vec<(String, String)> = sources
        .iter()
        .map(|s| (s.name.to_string(), s.text.to_string()))
        .collect();
    let compiled = gathered.run(|| lexc.compile_named_sources(&named));
    let diagnostics = gathered.take();
    let written = match compiled {
        Ok(t) => as_written(vec![t], format),
        Err(hfst::tools::lexc::LexcSourcesError::Compile(e)) => Err(Error::from(e)),
        Err(e) => Err(Error::new(Code::Failed, e.to_string())),
    };
    match written.map(|mut written| written.pop()) {
        Ok(Some(value)) => Ok(Compiled { value, diagnostics }),
        Ok(None) => Err(failure(
            Error::new(Code::Internal, "the lexicons wrote no transducer"),
            diagnostics,
        )),
        Err(e) => Err(failure(e, diagnostics)),
    }
}

// [spec:hfst:req:embed.compile]
/// The transducers `hfst strings2fst` writes of `source` in `format`: one a
/// line, or their union. Each negative weight it warns of is a warning on
/// its line.
pub fn compile_strings(
    source: Source<'_>,
    format: Format,
    options: &StringsOptions,
) -> CompileResult<Vec<Transducer>> {
    let gathered = Gathered::default();
    let compiled = gathered.run(|| {
        hfst::tools::strings2fst::compile_strings::<StdVectorFst>(source.text, options.clone())
    });
    match compiled {
        Ok(compiled) => {
            for &(line, weight) in &compiled.negative_weights {
                gathered.push(at_line(
                    &source,
                    line,
                    Diagnostic::warning(negative_weight(weight)),
                ));
            }
            let diagnostics = gathered.take();
            match as_written(compiled.transducers, format) {
                Ok(value) => Ok(Compiled { value, diagnostics }),
                Err(e) => Err(failure(e, diagnostics)),
            }
        }
        Err(LineError { line, error }) => {
            let message = error.to_string();
            gathered.push(at_line(&source, line, Diagnostic::error(message.clone())));
            Err(failure(Error::new(Code::Failed, message), gathered.take()))
        }
    }
}

// [spec:hfst:req:embed.compile]
/// The transducers `hfst txt2fst` writes of the AT&T or Prolog text
/// `source` in `format`, each named after the source.
pub fn compile_text(
    source: Source<'_>,
    format: Format,
    options: &Txt2FstOptions,
) -> CompileResult<Vec<Transducer>> {
    let gathered = Gathered::default();
    let mut read = Vec::new();
    let result = gathered.run(|| {
        read_text::<StdVectorFst>(
            &mut source.text.as_bytes(),
            source.name,
            options,
            &mut |event| {
                match event {
                    TextEvent::Progress(_) => {}
                    TextEvent::NegativeWeight { weight, line } => gathered.push(at_line(
                        &source,
                        line as usize,
                        Diagnostic::warning(format!("Negative weight {weight:.6} found :-(")),
                    )),
                    TextEvent::NegativeEpsilonCycles => gathered.push(Diagnostic::warning(
                        "Transducer has epsilon cycles with a negative weight.",
                    )),
                    TextEvent::Read(t) => read.push(t),
                }
                Ok(())
            },
        )
    });
    let diagnostics = gathered.take();
    match result
        .map_err(Error::from)
        .and_then(|()| as_written(read, format))
    {
        Ok(value) => Ok(Compiled { value, diagnostics }),
        Err(e) => Err(failure(e, diagnostics)),
    }
}

// -----------------------------------------------------------------------------
// twolc and pmatch2fst
// -----------------------------------------------------------------------------

// [spec:hfst:req:embed.compile]
/// The rule transducers `hfst twolc` writes of the grammar `source` in
/// `format`, tropical OpenFst or foma.
pub fn compile_twolc(
    source: Source<'_>,
    format: Format,
    options: &TwolcOptions,
) -> CompileResult<Vec<Transducer>> {
    let ty = match format {
        Format::OpenFstTropical | Format::Foma => implementation_type(format),
        Format::OptimizedLookupWeighted | Format::OptimizedLookupUnweighted => {
            return Err(not_an_algebra("twolc"));
        }
    };
    let gathered = Gathered::default();
    let mut compiled = None;
    let stored = gathered.run(|| {
        stream_bytes(ty, true, |out| {
            compiled = Some(hfst::tools::twolc::compile_twolc(
                (source.text, source.name),
                options,
                out,
                Some(gathered.sink()),
            ));
            Ok(())
        })
    });
    let diagnostics = gathered.take();
    match (stored, compiled) {
        (Ok(bytes), Some(Ok(_))) => match read_transducers(&bytes) {
            Ok(value) => Ok(Compiled { value, diagnostics }),
            Err(e) => Err(failure(e, diagnostics)),
        },
        (_, Some(Err(_))) => Err(failure(
            Error::new(Code::Failed, format!("{} did not compile", source.name)),
            diagnostics,
        )),
        (Err(e), _) => Err(failure(e, diagnostics)),
        (Ok(_), None) => Err(failure(
            Error::new(Code::Internal, "the grammar was not compiled"),
            diagnostics,
        )),
    }
}

// [spec:hfst:req:embed.compile]
/// The archive `hfst pmatch2fst` writes of the pmatch script `source`, the
/// files it names read through `files` relative to `include_path`.
pub fn compile_pmatch(
    source: Source<'_>,
    options: &Pmatch2FstOptions,
    files: Arc<dyn FileResolver>,
) -> CompileResult<Vec<u8>> {
    let gathered = Gathered::default();
    let mut compiler = options.compiler();
    compiler.set_file_resolver(files);
    compiler.set_diagnostic_sink(gathered.sink());
    let archive = gathered.run(|| {
        let mut definitions = hfst::tools::pmatch2fst::compile_ruleset(&mut compiler, source.text)
            .map_err(archive_error)?;
        let mut written = Ok(());
        let bytes = stream_bytes(ImplementationType::HFST_OLW_TYPE, true, |out| {
            written = hfst::tools::pmatch2fst::write_ruleset(
                &mut definitions,
                out,
                false,
                &mut std::io::sink(),
            );
            Ok(())
        })?;
        written.map_err(archive_error)?;
        Ok(bytes)
    });
    let diagnostics = gathered.take();
    match archive {
        Ok(value) => Ok(Compiled { value, diagnostics }),
        Err(e) => Err(failure(e, diagnostics)),
    }
}

fn archive_error(error: hfst::tools::pmatch2fst::ArchiveError) -> Error {
    match error {
        hfst::tools::pmatch2fst::ArchiveError::Compile(e)
        | hfst::tools::pmatch2fst::ArchiveError::Write(e) => Error::from(e),
        hfst::tools::pmatch2fst::ArchiveError::Empty => Error::new(Code::Failed, error.to_string()),
    }
}

// -----------------------------------------------------------------------------
// xfst
// -----------------------------------------------------------------------------

/// What an xfst script left behind.
pub struct XfstResult {
    /// What it printed: the tool's standard output.
    pub output: String,
    /// Its stack, the bottom first.
    pub stack: Vec<Transducer>,
    /// The networks it defined, by name.
    pub definitions: Vec<(String, Transducer)>,
    /// The names of the files it wrote through the resolver, in the order
    /// it first wrote each.
    pub written: Vec<String>,
}

/// A resolver that keeps the names of the files written through it.
struct Recording {
    files: Arc<dyn FileResolver>,
    written: Mutex<Vec<String>>,
}

impl Recording {
    fn wrote(&self, name: &str) {
        let mut written = self.written.lock().unwrap_or_else(PoisonError::into_inner);
        if !written.iter().any(|w| w == name) {
            written.push(name.to_string());
        }
    }
}

impl FileResolver for Recording {
    fn read(&self, name: &str) -> std::io::Result<Vec<u8>> {
        self.files.read(name)
    }

    fn create(&self, name: &str) -> std::io::Result<Box<dyn std::io::Write>> {
        let writer = self.files.create(name)?;
        self.wrote(name);
        Ok(writer)
    }

    fn append(&self, name: &str) -> std::io::Result<Box<dyn std::io::Write>> {
        let writer = self.files.append(name)?;
        self.wrote(name);
        Ok(writer)
    }

    fn local_path(&self, name: &str) -> Option<std::path::PathBuf> {
        self.files.local_path(name)
    }
}

fn copy_of<B: hfst::backend::AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> Result<HfstTransducer<B>, Error> {
    Ok(HfstTransducer::new_copy(t)?)
}

fn run_script<B: hfst::backend::AlgebraBackend + FromAnyTransducer>(
    script: Source<'_>,
    (input, options): (&str, &XfstOptions),
    files: &Arc<Recording>,
    held: fn(HfstTransducer<B>) -> AnyTransducer,
) -> CompileResult<XfstResult> {
    let mut session: XfstCompiler<B> = options
        .session()
        .map_err(|e| failure(Error::new(Code::InvalidArgument, e.message), Vec::new()))?;
    let gathered = Gathered::default();
    session.set_diagnostic_sink(gathered.sink());
    session.set_file_resolver(files.clone());
    session.set_input_stream(std::io::Cursor::new(input.as_bytes().to_vec()));
    session.set_source_name(script.name);
    let run = gathered.run(|| session.parse(script.text));
    let diagnostics = gathered.take();
    if let Err(e) = run {
        return Err(failure(
            Error::new(Code::Failed, e.to_string()),
            diagnostics,
        ));
    }
    let result = (|| -> Result<XfstResult, Error> {
        let mut stack = Vec::new();
        for id in &session.stack {
            stack.push(Transducer::new(held(copy_of(session.net(*id))?)));
        }
        let mut definitions = Vec::new();
        for (name, id) in &session.definitions {
            definitions.push((
                name.to_string(),
                Transducer::new(held(copy_of(session.net(*id))?)),
            ));
        }
        let written = files
            .written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Ok(XfstResult {
            output: String::from_utf8_lossy(&session.take_output()).into_owned(),
            stack,
            definitions,
            written,
        })
    })();
    match result {
        Ok(value) => Ok(Compiled { value, diagnostics }),
        Err(e) => Err(failure(e, diagnostics)),
    }
}

// [spec:hfst:req:embed.compile]
// [spec:hfst:req:embed.compile-files]
/// Run the xfst script `script` as `hfst xfst -F` runs it in `format`,
/// tropical OpenFst or foma, with `input` as the standard input its
/// commands read: what it printed, its stack, its definitions and the files
/// it wrote. Every file it reads or writes goes through `files`, and it
/// writes nothing to standard output. A command that fails is an error
/// among the diagnostics; the script fails only when it stops on one.
pub fn compile_xfst(
    script: Source<'_>,
    input: &str,
    format: Format,
    options: &XfstOptions,
    files: Arc<dyn FileResolver>,
) -> CompileResult<XfstResult> {
    let recording = Arc::new(Recording {
        files,
        written: Mutex::new(Vec::new()),
    });
    match format {
        Format::Foma => run_script(script, (input, options), &recording, AnyTransducer::Foma),
        Format::OpenFstTropical => run_script::<StdVectorFst>(
            script,
            (input, options),
            &recording,
            AnyTransducer::Tropical,
        ),
        Format::OptimizedLookupWeighted | Format::OptimizedLookupUnweighted => {
            Err(not_an_algebra("xfst"))
        }
    }
}
