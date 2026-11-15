//! The compilers: what `hfst regexp2fst`, `lexc`, `twolc`, `xfst`,
//! `pmatch2fst`, `strings2fst` and `txt2fst` make of their sources, with the
//! tools' options as the fields of a settings record. A compile that fails
//! gives `compile-failed` with every diagnostic it made; one that succeeds
//! gives its warnings with its result.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use hfst_embed::compile::{
    self as embed, CompileResult, Diagnostic, LexcOptions, LexcVerbosity, LogBase,
    Pmatch2FstOptions, RegexpOptions, Separation, Severity, Source, StringsOptions, TwolcOptions,
    Txt2FstOptions, XfstOptions,
};
use hfst_embed::format::Format;
use hfst_embed::{Code, Error};
use hfst_swift_runtime::GlueError;
use hfst_swift_runtime::error::{self as glue, DiagnosticLabel, Span, invalid};

use crate::files::Files;
use crate::transducer::{Transducer, format_named};

const ALGEBRA: [Format; 2] = [Format::OpenFstTropical, Format::Foma];

/// What the standard input is called when a tool reads its source there.
const STDIN: &str = "<stdin>";

fn span(range: &Range<usize>, at: usize) -> Span {
    Span {
        start: (at + range.start) as u64,
        end: (at + range.end) as u64,
    }
}

/// Which of `sources`, the caller's, the diagnostic's own source text is,
/// or is a piece of, and where that piece starts in it. A compiler that
/// takes its source a piece at a time, as regexp2fst takes an expression a
/// line, gives a diagnostic about the piece, which is placed where the piece
/// first occurs. A diagnostic about a file the compile read is in none.
fn placed(d: &Diagnostic, sources: &[&str]) -> (Option<u32>, usize) {
    let text = d.source();
    if text.is_empty() {
        return (None, 0);
    }
    let whole = sources
        .iter()
        .position(|source| *source == text)
        .map(|at| (at, 0));
    let piece = || {
        sources
            .iter()
            .enumerate()
            .find_map(|(at, source)| source.find(text).map(|offset| (at, offset)))
    };
    match whole.or_else(piece) {
        Some((at, offset)) => (u32::try_from(at).ok(), offset),
        None => (None, 0),
    }
}

// [spec:hfst:req:swift.compile-errors]
/// `d` as the glue gives it: which of `sources`, the caller's, it is about,
/// its spans in UTF-8 bytes of that source, or of the file it is about, and
/// its text rendered without colour.
fn diagnostic(d: &Diagnostic, sources: &[&str]) -> glue::Diagnostic {
    let (source, at) = placed(d, sources);
    glue::Diagnostic {
        severity: match d.severity() {
            Severity::Error => glue::Severity::Error,
            Severity::Warning => glue::Severity::Warning,
            Severity::Info => glue::Severity::Info,
        },
        message: d.message().to_owned(),
        source_name: d.source_name().to_owned(),
        source,
        span: d.primary_span().map(|range| span(&range, at)),
        labels: d
            .labels()
            .iter()
            .map(|label| DiagnosticLabel {
                span: span(&label.span, at),
                message: label.message.clone(),
            })
            .collect(),
        notes: d.notes().to_vec(),
        help: d.help_text().map(str::to_owned),
        rendered: d.render(false),
    }
}

fn diagnostics(made: &[Diagnostic], sources: &[&str]) -> Vec<glue::Diagnostic> {
    made.iter().map(|d| diagnostic(d, sources)).collect()
}

// [spec:hfst:req:swift.compile-errors]
/// The value of a compile with its warnings, or the error that carries every
/// diagnostic it made: `compile-failed` for a compile that failed, or the
/// error `check` refuses the value with. What a compile says at the info
/// level goes to the log rather than to its warnings.
fn finished<T, U>(
    compiled: CompileResult<T>,
    sources: &[&str],
    check: impl FnOnce(T) -> Result<U, Error>,
) -> Result<(U, Vec<glue::Diagnostic>), GlueError> {
    let compiled = compiled.map_err(|failed| GlueError::Failed {
        code: "compile-failed".to_owned(),
        message: failed.error.into_message(),
        diagnostics: diagnostics(&failed.diagnostics, sources),
    })?;
    let (warnings, said): (Vec<Diagnostic>, Vec<Diagnostic>) = compiled
        .diagnostics
        .into_iter()
        .partition(|d| !matches!(d.severity(), Severity::Info));
    for d in &said {
        tracing::info!(target: "hfst", "{}", d.render(false).trim_end());
    }
    match check(compiled.value) {
        Ok(value) => Ok((value, diagnostics(&warnings, sources))),
        Err(error) => Err(GlueError::Failed {
            code: error.code().name().to_owned(),
            message: error.into_message(),
            diagnostics: diagnostics(&warnings, sources),
        }),
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

/// The type `name` names, which must be one of `allowed`.
fn format_of(name: &str, allowed: &[Format]) -> Result<Format, GlueError> {
    let format = format_named(name)?;
    if allowed.contains(&format) {
        Ok(format)
    } else {
        Err(invalid(format!(
            "this compiler compiles to tropical OpenFst or foma, not {name}"
        )))
    }
}

fn not_both(first: (&str, bool), second: (&str, bool)) -> Result<(), GlueError> {
    if first.1 && second.1 {
        return Err(invalid(format!(
            "{} and {} do not go together",
            first.0, second.0
        )));
    }
    Ok(())
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct CompiledTransducer {
    pub transducer: Arc<Transducer>,
    pub warnings: Vec<glue::Diagnostic>,
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct CompiledRules {
    pub transducers: Vec<Arc<Transducer>>,
    pub warnings: Vec<glue::Diagnostic>,
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct CompiledArchive {
    pub archive: Vec<u8>,
    pub warnings: Vec<glue::Diagnostic>,
}

/// A file an xfst script wrote, with what it left in it.
#[derive(uniffi::Record)]
pub struct WrittenFile {
    pub name: String,
    pub contents: Vec<u8>,
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct XfstOutcome {
    pub output: String,
    pub stack: Vec<Arc<Transducer>>,
    pub definitions: HashMap<String, Arc<Transducer>>,
    pub written: Vec<WrittenFile>,
    pub warnings: Vec<glue::Diagnostic>,
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct RegexSettings {
    pub format: String,
    pub semicolon: bool,
    pub disjunct: bool,
    pub do_not_harmonize: bool,
    pub harmonize_flags: bool,
    pub do_not_minimize: bool,
    pub encode_weights: bool,
    pub xerox_composition: bool,
    pub flag_is_epsilon: bool,
    pub replace_pass: bool,
    pub verbose: bool,
}

// [spec:hfst:req:swift.compile]
/// The transducer `hfst regexp2fst` writes of `source`.
#[uniffi::export]
pub fn regex(
    source: String,
    name: String,
    settings: RegexSettings,
    files: Arc<Files>,
) -> Result<CompiledTransducer, GlueError> {
    let format = format_named(&settings.format)?;
    let options = RegexpOptions {
        separation: if settings.semicolon {
            Separation::Semicolons
        } else {
            Separation::Lines
        },
        disjunct: settings.disjunct,
        harmonize: !settings.do_not_harmonize,
        harmonize_flags: settings.harmonize_flags,
        minimize: !settings.do_not_minimize,
        flag_is_epsilon: settings.flag_is_epsilon,
        xerox_composition: settings.xerox_composition,
        encode_weights: settings.encode_weights,
        replace_pass: settings.replace_pass,
        verbose: settings.verbose,
    };
    let compiled = embed::compile_regexps(
        Source {
            name: &name,
            text: &source,
        },
        format,
        &options,
        files.resolver_for(),
    );
    let one = one(
        "regex",
        "RegexOptions(disjunct: true) compiles a source of several expressions to their union",
    );
    let (t, warnings) = finished(compiled, &[&source], one)?;
    Ok(CompiledTransducer {
        transducer: Transducer::new(t),
        warnings,
    })
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct LexcSettings {
    pub format: String,
    pub align_strings: bool,
    pub with_flags: bool,
    pub minimize_flags: bool,
    pub rename_flags: bool,
    pub xerox_composition: bool,
    pub flag_is_epsilon: bool,
    pub encode_weights: bool,
    pub split_characters: bool,
    pub werror: bool,
    pub warn_one_sided_flags: bool,
    pub warn_unused_lexicons: bool,
    pub warn_repeated_lexicons: bool,
    pub warn_missing_lexicons: bool,
    pub warn_missing_alphabets: bool,
    pub warn_unnecessary_escapes: bool,
    pub quiet: bool,
    pub verbose: bool,
}

// [spec:hfst:req:swift.compile]
/// The transducer `hfst lexc` writes of `sources`, compiled together; each
/// named after the name of `names` at the same place.
#[uniffi::export]
pub fn compile_lexc(
    sources: Vec<String>,
    names: Vec<String>,
    settings: LexcSettings,
    files: Arc<Files>,
) -> Result<CompiledTransducer, GlueError> {
    if names.len() != sources.len() {
        return Err(invalid(format!(
            "compileLexc names each of the {} sources, not {}",
            sources.len(),
            names.len()
        )));
    }
    not_both(("quiet", settings.quiet), ("verbose", settings.verbose))?;
    let format = format_named(&settings.format)?;
    let options = LexcOptions {
        align_strings: settings.align_strings,
        with_flags: settings.with_flags,
        minimize_flags: settings.minimize_flags,
        rename_flags: settings.rename_flags,
        warnings_as_errors: settings.werror,
        warn_one_sided_flags: settings.warn_one_sided_flags,
        warn_unused_lexicons: settings.warn_unused_lexicons,
        warn_repeated_lexicons: settings.warn_repeated_lexicons,
        warn_missing_lexicons: settings.warn_missing_lexicons,
        warn_missing_alphabets: settings.warn_missing_alphabets,
        warn_unnecessary_escapes: settings.warn_unnecessary_escapes,
        xerox_composition: settings.xerox_composition,
        encode_weights: settings.encode_weights,
        flag_is_epsilon: settings.flag_is_epsilon,
        split_characters: settings.split_characters,
        verbosity: if settings.quiet {
            LexcVerbosity::Quiet
        } else if settings.verbose {
            LexcVerbosity::Verbose
        } else {
            LexcVerbosity::Normal
        },
    };
    let given: Vec<Source<'_>> = names
        .iter()
        .zip(&sources)
        .map(|(name, text)| Source { name, text })
        .collect();
    let compiled = embed::compile_lexc(&given, format, &options, files.resolver_for());
    let texts: Vec<&str> = sources.iter().map(String::as_str).collect();
    let (t, warnings) = finished(compiled, &texts, Ok)?;
    Ok(CompiledTransducer {
        transducer: Transducer::new(t),
        warnings,
    })
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct TwolcSettings {
    pub format: String,
    pub resolve: bool,
    pub dont_resolve_right: bool,
    pub quiet: bool,
    pub verbose: bool,
}

// [spec:hfst:req:swift.compile]
/// The rule transducers `hfst twolc` writes of `source`.
#[uniffi::export]
pub fn compile_twolc(
    source: String,
    name: String,
    settings: TwolcSettings,
) -> Result<CompiledRules, GlueError> {
    not_both(("quiet", settings.quiet), ("verbose", settings.verbose))?;
    let format = format_of(&settings.format, &ALGEBRA)?;
    let options = TwolcOptions {
        quiet: settings.quiet,
        verbose: settings.verbose,
        resolve_left_conflicts: settings.resolve,
        resolve_right_conflicts: !settings.dont_resolve_right,
    };
    let compiled = embed::compile_twolc(
        Source {
            name: &name,
            text: &source,
        },
        format,
        &options,
    );
    let (rules, warnings) = finished(compiled, &[&source], Ok)?;
    Ok(CompiledRules {
        transducers: rules.into_iter().map(Transducer::new).collect(),
        warnings,
    })
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct XfstSettings {
    pub format: String,
    pub print_weight: bool,
    pub restricted_mode: bool,
    pub silent: bool,
}

// [spec:hfst:req:swift.compile]
// [spec:hfst:req:swift.compile-files]
/// Runs the xfst script `source` as `hfst xfst -F` runs it, with `input` as
/// its standard input: what it printed, its stack, its definitions and each
/// file it wrote, read back from where it wrote it.
#[uniffi::export]
pub fn compile_xfst(
    source: String,
    input: String,
    name: String,
    settings: XfstSettings,
    files: Arc<Files>,
) -> Result<XfstOutcome, GlueError> {
    let format = format_of(&settings.format, &ALGEBRA)?;
    let options = XfstOptions {
        print_weight: settings.print_weight,
        restricted_mode: settings.restricted_mode,
        silent: settings.silent,
    };
    let compiled = embed::compile_xfst(
        Source {
            name: &name,
            text: &source,
        },
        &input,
        format,
        &options,
        files.resolver_for(),
    );
    let (ran, warnings) = finished(compiled, &[&source], Ok)?;
    let written = ran
        .written
        .into_iter()
        .map(|file| {
            let contents = files
                .written(&file)
                .map_err(|error| Error::new(Code::Io, format!("{file}: {error}")))?;
            Ok(WrittenFile {
                name: file,
                contents,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(XfstOutcome {
        output: ran.output,
        stack: ran.stack.into_iter().map(Transducer::new).collect(),
        definitions: ran
            .definitions
            .into_iter()
            .map(|(defined, t)| (defined, Transducer::new(t)))
            .collect(),
        written,
        warnings,
    })
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct PmatchSettings {
    pub flatten: bool,
    pub cosine_distances: bool,
    pub verbose: bool,
}

// [spec:hfst:req:swift.compile]
/// The archive `hfst pmatch2fst` writes of `source`. A relative
/// `@bin"FILE"` or `@txt"FILE"` is found beside `name`, as the tool finds
/// one beside its input.
#[uniffi::export]
pub fn compile_pmatch(
    source: String,
    name: String,
    settings: PmatchSettings,
    files: Arc<Files>,
) -> Result<CompiledArchive, GlueError> {
    let mut options = Pmatch2FstOptions {
        flatten: settings.flatten,
        include_cosine_distances: settings.cosine_distances,
        include_path: String::new(),
        verbose: settings.verbose,
    };
    if name != STDIN
        && let Some(slash) = name.rfind('/')
    {
        options.include_path = name[..=slash].to_owned();
    }
    let compiled = embed::compile_pmatch(
        Source {
            name: &name,
            text: &source,
        },
        &options,
        files.resolver_for(),
    );
    let (archive, warnings) = finished(compiled, &[&source], Ok)?;
    Ok(CompiledArchive { archive, warnings })
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct StringsSettings {
    pub format: String,
    pub disjunct_strings: bool,
    pub norm: bool,
    pub log: bool,
    pub log10: bool,
    pub pairstrings: bool,
    pub has_spaces: bool,
    pub epsilon: String,
    pub multichar_symbols: Vec<String>,
    pub warn_negative_weights: bool,
    pub werror: bool,
}

// [spec:hfst:req:swift.compile]
/// The transducer `hfst strings2fst` writes of the lines of `source`.
#[uniffi::export]
pub fn compile_strings(
    source: String,
    name: String,
    settings: StringsSettings,
) -> Result<CompiledTransducer, GlueError> {
    not_both(("log", settings.log), ("log10", settings.log10))?;
    let format = format_named(&settings.format)?;
    let options = StringsOptions {
        epsilon: settings.epsilon,
        multichar_symbols: settings.multichar_symbols,
        pairstrings: settings.pairstrings,
        has_spaces: settings.has_spaces,
        disjunct: settings.disjunct_strings,
        normalize: settings.norm,
        log: if settings.log {
            Some(LogBase::E)
        } else if settings.log10 {
            Some(LogBase::Ten)
        } else {
            None
        },
        warn_negative_weights: settings.warn_negative_weights,
        warnings_are_errors: settings.werror,
    };
    let compiled = embed::compile_strings(
        Source {
            name: &name,
            text: &source,
        },
        format,
        &options,
    );
    let one = one(
        "compileStrings",
        "StringsOptions(disjunctStrings: true) compiles a source of several lines to their union",
    );
    let (t, warnings) = finished(compiled, &[&source], one)?;
    Ok(CompiledTransducer {
        transducer: Transducer::new(t),
        warnings,
    })
}

// [spec:hfst:req:swift.compile]
#[derive(uniffi::Record)]
pub struct TextSettings {
    pub format: String,
    pub prolog: bool,
    pub epsilon: String,
    pub disjunct: bool,
    pub check_negative_epsilon_cycles: bool,
    pub warn_negative_weights: bool,
}

// [spec:hfst:req:swift.compile]
/// The transducer `hfst txt2fst` writes of the AT&T text `source`, or with
/// `prolog` of the Prolog text.
#[uniffi::export]
pub fn parse_text(
    source: String,
    name: String,
    settings: TextSettings,
) -> Result<CompiledTransducer, GlueError> {
    let format = format_named(&settings.format)?;
    let options = if settings.prolog {
        Txt2FstOptions {
            prolog: true,
            check_negative_epsilon_cycles: settings.check_negative_epsilon_cycles,
            ..Txt2FstOptions::default()
        }
    } else {
        Txt2FstOptions {
            prolog: false,
            epsilon: settings.epsilon,
            check_negative_epsilon_cycles: settings.check_negative_epsilon_cycles,
            warn_negative_weights: settings.warn_negative_weights,
            disjunct: settings.disjunct,
        }
    };
    let compiled = embed::compile_text(
        Source {
            name: &name,
            text: &source,
        },
        format,
        &options,
    );
    let one = if settings.prolog {
        one("parseProlog", "give the text of one network")
    } else {
        one(
            "parseAtt",
            "AttOptions(disjunct: true) reads a text of several transducers as their union",
        )
    };
    let (t, warnings) = finished(compiled, &[&source], one)?;
    Ok(CompiledTransducer {
        transducer: Transducer::new(t),
        warnings,
    })
}
