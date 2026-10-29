//! The compilers, as `@divvun/hfst` exposes them: regular expressions, lexc,
//! twolc and pmatch, each compiling a source string, with the files it names
//! read from a map the caller passes in. Each compiler is driven the way its
//! hfst tool drives it (`regexp2fst`, `lexc`, `twolc`, `pmatch2fst`), so a
//! source and options give the bytes the tool writes for them.

use std::collections::HashMap;
use std::sync::Arc;

use hfst::backend::AlgebraBackend;
use hfst::backend_foma::FomaTransducer;
use hfst::diag::Diagnostic;
use hfst::error::Error;
use hfst::file_resolver::{FileResolver, MemoryFiles};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use hfst::lexc::LexcCompiler;
use hfst::pmatch_compiler::{PmatchCompiler, write_archive};
use hfst::twolc::TwolcCompiler;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;
use hfst_wasm_runtime::{Format, GlueError};
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::diagnostic::{JsDiagnostic, Sources, anchored};
use crate::transducer::{FullTransducer, read_any_transducers};

fn yes() -> bool {
    true
}

/// `compileRegex`'s options; `js/hfst/compile.ts` documents each.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegexOptions {
    source_name: Option<String>,
    #[serde(rename = "type")]
    format: Option<String>,
    #[serde(default)]
    semicolon: bool,
    #[serde(default)]
    disjunct: bool,
    #[serde(default = "yes")]
    harmonize: bool,
    #[serde(default)]
    harmonize_flags: bool,
    #[serde(default)]
    encode_weights: bool,
    #[serde(default = "yes")]
    minimize: bool,
    #[serde(default)]
    xerox_composition: bool,
    #[serde(default)]
    flag_is_epsilon: bool,
    #[serde(default)]
    replace_pass: bool,
    #[serde(default)]
    verbose: bool,
}

/// `compileLexc`'s options; `js/hfst/compile.ts` documents each.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LexcOptions {
    source_name: Option<String>,
    #[serde(rename = "type")]
    format: Option<String>,
    #[serde(default)]
    align_strings: bool,
    #[serde(default)]
    with_flags: bool,
    #[serde(default)]
    minimize_flags: bool,
    #[serde(default)]
    rename_flags: bool,
    #[serde(default)]
    encode_weights: bool,
    #[serde(default = "yes")]
    xerox_composition: bool,
    #[serde(default)]
    flag_is_epsilon: bool,
    #[serde(default)]
    warnings: Vec<String>,
    #[serde(default)]
    split_characters: bool,
    #[serde(default)]
    quiet: bool,
    #[serde(default)]
    verbose: bool,
}

/// `compileTwolc`'s options; `js/hfst/compile.ts` documents each.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TwolcOptions {
    source_name: Option<String>,
    #[serde(rename = "type")]
    format: Option<String>,
    #[serde(default)]
    resolve_left: bool,
    #[serde(default = "yes")]
    resolve_right: bool,
    #[serde(default)]
    quiet: bool,
}

/// `compilePmatch`'s options; `js/hfst/compile.ts` documents each.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PmatchOptions {
    source_name: Option<String>,
    #[serde(default)]
    flatten: bool,
    #[serde(default)]
    include_cosine_distances: bool,
}

/// The files of one compile's map, by name.
struct Files {
    memory: MemoryFiles,
    list: Vec<(String, Vec<u8>)>,
}

impl Files {
    fn new(list: Vec<(String, Vec<u8>)>) -> Self {
        let memory = MemoryFiles::default();
        for (name, bytes) in &list {
            memory.insert(name.clone(), bytes.clone());
        }
        Files { memory, list }
    }

    fn resolver(&self) -> Arc<dyn FileResolver> {
        Arc::new(self.memory.clone())
    }
}

/// What a compile made.
struct Compiled {
    transducers: Vec<FullTransducer>,
    /// The container a pmatch compile wrote, as `hfst pmatch2fst` writes it.
    bytes: Option<Vec<u8>>,
    diagnostics: Vec<JsDiagnostic>,
}

impl Compiled {
    fn into_js(self) -> JsValue {
        let object = js_sys::Object::new();
        let transducers: js_sys::Array = self.transducers.into_iter().map(JsValue::from).collect();
        set(&object, "transducers", &transducers);
        let diagnostics: js_sys::Array = self.diagnostics.iter().map(JsDiagnostic::to_js).collect();
        set(&object, "diagnostics", &diagnostics);
        if let Some(bytes) = self.bytes {
            set(
                &object,
                "bytes",
                &js_sys::Uint8Array::from(bytes.as_slice()),
            );
        }
        object.into()
    }
}

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(object, &JsValue::from_str(key), value)
        .expect("a fresh plain object takes any property");
}

/// Why a compile made nothing.
#[derive(Debug)]
enum Failed {
    /// The source did not compile; the diagnostics say why, and at least one
    /// of them is an error.
    Compile {
        message: String,
        diagnostics: Vec<JsDiagnostic>,
    },
    /// The compile could not run, or what it made could not be used.
    Glue(GlueError),
}

impl From<GlueError> for Failed {
    fn from(error: GlueError) -> Self {
        Failed::Glue(error)
    }
}

impl From<Error> for Failed {
    fn from(error: Error) -> Self {
        Failed::Glue(error.into())
    }
}

impl From<Failed> for GlueError {
    fn from(failed: Failed) -> Self {
        match failed {
            Failed::Compile {
                message,
                diagnostics,
            } => GlueError::compile(
                message,
                diagnostics.iter().map(JsDiagnostic::to_js).collect(),
            ),
            Failed::Glue(error) => error,
        }
    }
}

/// The diagnostics of one compile, in the wrapper's shape, oldest first.
struct Report {
    sources: Sources,
    diagnostics: Vec<JsDiagnostic>,
}

impl Report {
    fn new(sources: Sources) -> Self {
        Report {
            sources,
            diagnostics: Vec::new(),
        }
    }

    fn add(&mut self, diagnostics: Vec<Diagnostic>) {
        for d in &diagnostics {
            self.diagnostics.push(self.sources.convert(d));
        }
    }

    /// The failure of this compile. `reason` says why when the compiler has
    /// reported no error of its own, as a compiler that fails with only an
    /// error value, or with nothing at all, does.
    fn fail(mut self, reason: &str) -> Failed {
        if !self.diagnostics.iter().any(JsDiagnostic::is_error) {
            self.diagnostics.push(self.sources.failure(reason));
        }
        let errors: Vec<&JsDiagnostic> = self.diagnostics.iter().filter(|d| d.is_error()).collect();
        let name = self.sources.name();
        let first = errors.first().map_or(reason, |d| d.message());
        let message = match errors.len() {
            0 | 1 => format!("{name} did not compile: {first}"),
            n => format!("{name} did not compile, with {n} errors; the first: {first}"),
        };
        Failed::Compile {
            message,
            diagnostics: self.diagnostics,
        }
    }

    fn done(self, transducers: Vec<FullTransducer>, bytes: Option<Vec<u8>>) -> Compiled {
        Compiled {
            transducers,
            bytes,
            diagnostics: self.diagnostics,
        }
    }
}

/// What an error value says, without the generic prefix of its kind.
fn reason(error: &Error) -> String {
    error
        .message
        .clone()
        .unwrap_or_else(|| error.kind.to_string())
}

fn source_name<'a>(given: &'a Option<String>, default: &'a str) -> Result<&'a str, GlueError> {
    match given.as_deref() {
        Some("") => Err(GlueError::new(
            "invalid-argument",
            "sourceName is empty; a source needs a name for its diagnostics",
        )),
        Some(name) => Ok(name),
        None => Ok(default),
    }
}

fn output_format(name: Option<&str>) -> Result<Format, GlueError> {
    let Some(name) = name else {
        return Ok(Format::OpenFstTropical);
    };
    Format::from_name(name).ok_or_else(|| {
        GlueError::new(
            "invalid-argument",
            format!("there is no transducer type called {name}"),
        )
    })
}

/// The name `hfst-tool-metadata`'s `hfst_set_name` gives the result of
/// operation `op` on `source`.
fn tool_name(op: &str, source: &str) -> String {
    let name = format!("{op}({source})");
    if name.len() <= 1024 {
        return name;
    }
    let mut cut = 1000;
    while !name.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("truncated({}...)", &name[..cut])
}

/// The tropical result of a compile in `format`, made as the tools make it
/// when they write it.
fn finish(t: HfstTransducer<StdVectorFst>, format: Format) -> Result<FullTransducer, GlueError> {
    let any = AnyTransducer::Tropical(t).convert(format.implementation_type(), "")?;
    FullTransducer::new(any)
}

/// What `write` writes to an HFST3 stream of type `ty`, with what it returned.
fn stream_bytes<R>(
    ty: ImplementationType,
    write: impl FnOnce(&mut HfstOutputStream) -> R,
) -> Result<(R, Vec<u8>), GlueError> {
    const FILE: &str = "stream";
    let files = MemoryFiles::default();
    let sink = files
        .create(FILE)
        .map_err(|error| GlueError::new("internal", error.to_string()))?;
    let mut stream = HfstOutputStream::from_writer(sink, ty, true)?;
    let written = write(&mut stream);
    stream.close();
    let bytes = files.get(FILE).unwrap_or_default();
    Ok((written, bytes))
}

fn read_options<'de, T: Deserialize<'de>>(options: &'de str) -> Result<T, GlueError> {
    serde_json::from_str(options).map_err(|error| {
        GlueError::new(
            "invalid-argument",
            format!("the compile options are not valid: {error}"),
        )
    })
}

/// The wrapper's map of file names to bytes.
fn read_files(object: &js_sys::Object) -> Result<Files, GlueError> {
    let internal = |what: String| GlueError::new("internal", what);
    let mut list = Vec::new();
    for entry in js_sys::Object::entries(object).iter() {
        let entry: js_sys::Array = entry
            .dyn_into()
            .map_err(|_| internal("the file map has an entry that is not a pair".to_owned()))?;
        let name = entry
            .get(0)
            .as_string()
            .ok_or_else(|| internal("the file map has a name that is not a string".to_owned()))?;
        let bytes: js_sys::Uint8Array = entry
            .get(1)
            .dyn_into()
            .map_err(|_| internal(format!("the file {name} is not a Uint8Array")))?;
        list.push((name, bytes.to_vec()));
    }
    Ok(Files::new(list))
}

// [spec:hfst:req:javascript.compile]
/// Compiles `source` as `hfst regexp2fst` compiles a file: one expression a
/// line, or with `semicolon`, expressions ending in `;`. `options` is JSON.
#[wasm_bindgen(js_name = compileRegex)]
pub fn compile_regex(
    source: &str,
    options: &str,
    files: &js_sys::Object,
) -> Result<JsValue, GlueError> {
    Ok(regex(source, &read_options(options)?, &read_files(files)?)?.into_js())
}

// [spec:hfst:req:javascript.compile]
/// Compiles the lexc `source` as `hfst lexc` compiles a file. `options` is
/// JSON.
#[wasm_bindgen(js_name = compileLexc)]
pub fn compile_lexc(
    source: &str,
    options: &str,
    files: &js_sys::Object,
) -> Result<JsValue, GlueError> {
    Ok(lexc(source, &read_options(options)?, &read_files(files)?)?.into_js())
}

// [spec:hfst:req:javascript.compile]
/// Compiles the twolc grammar `source` as `hfst twolc` compiles a file, to one
/// transducer a rule. `options` is JSON. A grammar names no files, so the map
/// is not read.
#[wasm_bindgen(js_name = compileTwolc)]
pub fn compile_twolc(source: &str, options: &str) -> Result<JsValue, GlueError> {
    let options: TwolcOptions = read_options(options)?;
    let rules: RuleWriter = match output_format(options.format.as_deref())? {
        Format::OpenFstTropical => twolc_rules::<StdVectorFst>,
        Format::Foma => twolc_rules::<FomaTransducer>,
        format @ (Format::OptimizedLookupWeighted | Format::OptimizedLookupUnweighted) => {
            return Err(GlueError::new(
                "invalid-argument",
                format!(
                    "twolc compiles to openfst-tropical or foma, not {}",
                    format.name()
                ),
            ));
        }
    };
    Ok(twolc(source, &options, rules)?.into_js())
}

// [spec:hfst:req:javascript.compile]
/// Compiles the pmatch script `source` as `hfst pmatch2fst` compiles a file,
/// to the container it writes. `options` is JSON.
#[wasm_bindgen(js_name = compilePmatch)]
pub fn compile_pmatch(
    source: &str,
    options: &str,
    files: &js_sys::Object,
) -> Result<JsValue, GlueError> {
    Ok(pmatch(source, &read_options(options)?, &read_files(files)?)?.into_js())
}

fn regex(source: &str, options: &RegexOptions, files: &Files) -> Result<Compiled, Failed> {
    let format = output_format(options.format.as_deref())?;
    let name = source_name(&options.source_name, "<regex>")?;
    let mut report = Report::new(Sources::new(name, source, &files.list));
    let mut comp = XreCompiler::<StdVectorFst>::new();
    comp.set_source_name(name);
    comp.set_file_resolver(files.resolver());
    comp.set_verbosity(options.verbose);
    comp.set_harmonization(options.harmonize);
    comp.set_flag_harmonization(options.harmonize_flags);
    comp.set_minimize_result(options.minimize);
    comp.set_flag_is_epsilon(options.flag_is_epsilon);
    comp.set_xerox_composition(options.xerox_composition);
    comp.set_encode_weights(options.encode_weights);
    comp.set_replace_pass(options.replace_pass);
    let compiled = if options.semicolon {
        semicolon_separated(&mut comp, source, &mut report)
    } else {
        line_separated(&mut comp, source, &mut report)
    };
    let mut compiled = match compiled {
        Ok(compiled) => compiled,
        Err(reason) => return Err(report.fail(&reason)),
    };
    if options.disjunct {
        let mut union = HfstTransducer::new();
        for t in &compiled {
            union.disjunct(t, options.harmonize)?;
        }
        compiled = vec![union];
    }
    let mut t = match compiled.len() {
        1 => compiled.remove(0),
        n => {
            return Err(GlueError::new(
                "transducer-count",
                format!(
                    "compileRegex makes one transducer, and the source holds {n} expressions; pass disjunct: true for their union"
                ),
            )
            .into());
        }
    };
    t.set_name(&tool_name("xre", "?"));
    Ok(report.done(vec![finish(t, format)?], None))
}

/// The expressions of `source`, one a line, as `hfst regexp2fst -l` reads
/// them, or why they did not compile. A line compiles on its own and its
/// diagnostics count from the line, so where the line starts in `source` is
/// added to each.
fn line_separated(
    comp: &mut XreCompiler<StdVectorFst>,
    source: &str,
    report: &mut Report,
) -> Result<Vec<HfstTransducer<StdVectorFst>>, String> {
    let mut compiled = Vec::new();
    let mut line_start = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        let start = line_start;
        line_start += line.len();
        let expression = line.trim_start_matches(['\n', '\r', ' ']);
        if expression.is_empty() {
            continue;
        }
        let shift = start + line.len() - expression.len();
        let t = comp.compile(expression);
        let diagnostics = comp
            .take_diagnostics()
            .iter()
            .map(|d| {
                if d.source() == expression {
                    let text = Arc::clone(report.sources.text());
                    anchored(d, report.sources.name(), text, shift)
                } else {
                    d.clone()
                }
            })
            .collect();
        report.add(diagnostics);
        match t {
            Some(t) => compiled.push(t),
            None if comp.contained_only_comments() => {}
            None => {
                return Err(format!(
                    "the regular expression on line {} did not compile",
                    index + 1
                ));
            }
        }
    }
    if compiled.is_empty() {
        return Err(only_comments());
    }
    Ok(compiled)
}

/// The expressions of `source`, each ending in `;`, as `hfst regexp2fst -S`
/// reads them, or why they did not compile.
fn semicolon_separated(
    comp: &mut XreCompiler<StdVectorFst>,
    source: &str,
    report: &mut Report,
) -> Result<Vec<HfstTransducer<StdVectorFst>>, String> {
    let mut compiled = Vec::new();
    let mut offset = 0;
    loop {
        let mut read = 0;
        let t = comp.compile_first_at(source, offset, &mut read);
        report.add(comp.take_diagnostics());
        let Some(t) = t else {
            if !comp.contained_only_comments() {
                return Err(format!(
                    "expression {} of the source did not compile",
                    compiled.len() + 1
                ));
            }
            if compiled.is_empty() {
                return Err(only_comments());
            }
            break;
        };
        compiled.push(t);
        offset += read as usize;
        if read == 0 || offset >= source.len() {
            break;
        }
    }
    Ok(compiled)
}

fn only_comments() -> String {
    "the source holds no regular expression, only whitespace and comments".to_owned()
}

/// The `-W` switches of `hfst lexc` other than `error`, applied in order to
/// the six warnings in the order the lexc compiler names them.
const LEXC_WARNINGS: [&str; 6] = [
    "one-sided-flags",
    "missing-lexicons",
    "unused-lexicons",
    "repeated-lexicons",
    "missing-alphabets",
    "unnecessary-escapes",
];

/// The six warnings' settings after `switches`, and whether `error` was
/// among them.
fn lexc_warnings(switches: &[String]) -> Result<([bool; 6], bool), GlueError> {
    let mut on = [false; 6];
    let mut errors = false;
    for switch in switches {
        if switch == "error" {
            errors = true;
        } else if switch == "all" {
            on = [true; 6];
        } else {
            let (value, name) = match switch.strip_prefix("no-") {
                Some(name) => (false, name),
                None => (true, switch.as_str()),
            };
            let index = LEXC_WARNINGS
                .iter()
                .position(|&w| w == name)
                .ok_or_else(|| {
                    GlueError::new(
                        "invalid-argument",
                        format!(
                            "there is no lexc warning switch called {switch}; the switches are all, error, and {}, each also with no- before it",
                            LEXC_WARNINGS.join(", ")
                        ),
                    )
                })?;
            on[index] = value;
        }
    }
    Ok((on, errors))
}

fn lexc(source: &str, options: &LexcOptions, files: &Files) -> Result<Compiled, Failed> {
    let format = output_format(options.format.as_deref())?;
    let name = source_name(&options.source_name, "<lexc>")?;
    let (warnings, warnings_are_errors) = lexc_warnings(&options.warnings)?;
    let mut report = Report::new(Sources::new(name, source, &files.list));
    let mut lexc =
        LexcCompiler::<StdVectorFst>::new_with_flags(options.with_flags, options.align_strings);
    lexc.set_minimize_flags(options.minimize_flags);
    lexc.set_rename_flags(options.rename_flags);
    lexc.set_flag_is_epsilon(options.flag_is_epsilon);
    lexc.set_xerox_composition(options.xerox_composition);
    lexc.set_encode_weights(options.encode_weights);
    lexc.set_verbosity(if options.quiet {
        0
    } else if options.verbose {
        2
    } else {
        1
    });
    lexc.set_treat_warnings_as_errors(warnings_are_errors);
    for (warning, on) in LEXC_WARNINGS.iter().zip(warnings) {
        lexc.set_warning(&format!("-W{warning}"), on);
    }
    lexc.set_split_characters(options.split_characters);
    lexc.set_file_resolver(files.resolver());
    lexc.set_source_name(name);
    let parsed = lexc.parse(source).map(|_| ());
    report.add(lexc.take_diagnostics());
    if let Err(error) = parsed {
        return Err(report.fail(&reason(&error)));
    }
    let compiled = lexc.compile_lexical();
    report.add(lexc.take_diagnostics());
    let mut t = match compiled {
        Ok(Some(t)) => t,
        Ok(None) => return Err(report.fail("the source did not compile cleanly")),
        Err(error) => return Err(report.fail(&reason(&error))),
    };
    t.set_name(&tool_name("lexc", name));
    // hfst_set_formula: the operation and the source name's first character
    // when it is ASCII.
    let formula = match name.as_bytes().first() {
        Some(&c) if c.is_ascii() && c != 0 => format!("L {}", char::from(c)),
        Some(_) | None => "L U8".to_owned(),
    };
    t.set_property("formulaic-definition", &formula);
    Ok(report.done(vec![finish(t, format)?], None))
}

/// [`twolc_rules`] at the backend of one output type.
type RuleWriter = fn(&str, &str, &TwolcOptions, &mut Report) -> Result<Option<Vec<u8>>, GlueError>;

fn twolc(source: &str, options: &TwolcOptions, rules: RuleWriter) -> Result<Compiled, Failed> {
    let name = source_name(&options.source_name, "<twolc>")?;
    let mut report = Report::new(Sources::new(name, source, &[]));
    match rules(source, name, options, &mut report)? {
        // A grammar with no rules stores none, and hfst twolc writes an empty
        // file for it.
        Some(bytes) if bytes.is_empty() => Ok(report.done(Vec::new(), None)),
        Some(bytes) => {
            let transducers = read_any_transducers(&bytes)?;
            Ok(report.done(transducers, None))
        }
        None => Err(report.fail("the grammar did not compile")),
    }
}

/// The stream of rule transducers `hfst twolc` writes for `source`, compiled
/// at backend `B`, or `None` when the grammar did not compile.
fn twolc_rules<B: AlgebraBackend>(
    source: &str,
    name: &str,
    options: &TwolcOptions,
    report: &mut Report,
) -> Result<Option<Vec<u8>>, GlueError> {
    let mut comp = TwolcCompiler::<B>::new_with_options(
        options.quiet,
        false,
        options.resolve_left,
        options.resolve_right,
    );
    comp.set_source_name(name);
    let (stored, bytes) = stream_bytes(B::TYPE, |out| comp.compile_and_store(source, out))?;
    report.add(comp.take_diagnostics());
    Ok(stored.map(|()| bytes))
}

fn pmatch(source: &str, options: &PmatchOptions, files: &Files) -> Result<Compiled, Failed> {
    let name = source_name(&options.source_name, "<pmatch>")?;
    let mut report = Report::new(Sources::new(name, source, &files.list));
    let mut comp = PmatchCompiler::<StdVectorFst>::new();
    comp.set_flatten(options.flatten);
    comp.set_include_cosine_distances(options.include_cosine_distances);
    // With no include path, the names a script gives are the map's names.
    comp.set_file_resolver(files.resolver());
    // hfst pmatch2fst compiles only a script longer than one byte, and has
    // an empty ruleset for any other.
    let compiled = if source.len() > 1 {
        comp.compile(source)
    } else {
        Ok(HashMap::new())
    };
    report.add(comp.take_diagnostics());
    let mut definitions = match compiled {
        Ok(definitions) => definitions,
        Err(error) => return Err(report.fail(&reason(&error))),
    };
    let (written, bytes) = stream_bytes(ImplementationType::HFST_OLW_TYPE, |out| {
        write_archive(&mut definitions, out, false, &mut std::io::sink())
    })?;
    if !written? {
        return Err(
            report.fail("the script compiles to an empty ruleset, so there is nothing to write")
        );
    }
    let transducers = read_any_transducers(&bytes)?;
    Ok(report.done(transducers, Some(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options<'de, T: Deserialize<'de>>(json: &'de str) -> T {
        read_options(json).expect("valid options")
    }

    fn failure(result: Result<Compiled, Failed>) -> (String, Vec<serde_json::Value>) {
        match result {
            Ok(_) => panic!("the compile succeeded"),
            Err(Failed::Glue(error)) => panic!("not a compile failure: {error:?}"),
            Err(Failed::Compile {
                message,
                diagnostics,
            }) => (
                message,
                diagnostics
                    .iter()
                    .map(|d| serde_json::to_value(d).expect("serialises"))
                    .collect(),
            ),
        }
    }

    fn no_files() -> Files {
        Files::new(Vec::new())
    }

    #[test]
    fn options_take_their_defaults_and_refuse_unknown_names() {
        let regex: RegexOptions = options("{}");
        assert!(regex.harmonize && regex.minimize && !regex.semicolon);
        let lexc: LexcOptions = options(r#"{"sourceName":"a.lexc"}"#);
        assert!(lexc.xerox_composition);
        assert_eq!(lexc.source_name.as_deref(), Some("a.lexc"));
        let twolc: TwolcOptions = options("{}");
        assert!(twolc.resolve_right && !twolc.resolve_left);
        let unknown = read_options::<TwolcOptions>(r#"{"resolve":true}"#);
        assert!(matches!(unknown, Err(e) if e.code() == "invalid-argument"));
    }

    #[test]
    fn lexc_warning_switches_apply_in_order() {
        let switches = |s: &[&str]| s.iter().map(|&s| s.to_owned()).collect::<Vec<_>>();
        let (on, errors) = lexc_warnings(&switches(&["all", "no-unused-lexicons", "error"]))
            .expect("known switches");
        assert_eq!(on, [true, true, false, true, true, true]);
        assert!(errors);
        assert!(lexc_warnings(&switches(&["loud"])).is_err());
    }

    // A failing line's diagnostics are placed in the whole source, so the
    // second line's error counts from the first line's start.
    #[test]
    fn a_failing_line_is_placed_in_the_source() {
        let (message, diagnostics) = failure(regex("a\nö [b\n", &options("{}"), &no_files()));
        assert!(message.starts_with("<regex> did not compile"), "{message}");
        let first = &diagnostics[0];
        assert_eq!(first["severity"], "error");
        assert_eq!(first["start"], 6);
        let rendered = first["rendered"].as_str().expect("text");
        assert!(rendered.contains("<regex>:2:"), "{rendered}");
    }

    #[test]
    fn a_source_of_comments_fails_with_a_reason() {
        let (_, diagnostics) = failure(regex("! nothing\n", &options("{}"), &no_files()));
        assert_eq!(diagnostics[0]["message"], only_comments());
    }

    #[test]
    fn a_missing_include_names_the_file() {
        let source = "@include\"nope.pmatch\"\ndefine TOP {a} ;\n";
        let (message, _) = failure(pmatch(source, &options("{}"), &no_files()));
        assert!(message.contains("nope.pmatch"), "{message}");
    }
}
