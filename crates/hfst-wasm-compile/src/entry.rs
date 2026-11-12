//! The compilers as the module exports them: each takes its source, the
//! source's name, the files and the options as the wrapper passes them, runs
//! the compiler of `crate::compile`, and hands back its result with its
//! warnings as `{ transducer, warnings }` or the like.

use hfst_embed::Code;
use hfst_embed::compile::{Compiled, Source, Transducer as Machine};
use hfst_wasm_shared::GlueError;
use wasm_bindgen::prelude::*;

use crate::compile;
use crate::diagnostics::warnings;
use crate::failure::Failure;
use crate::files::Files;
use crate::options::Options;
use crate::transducer::Transducer;
use crate::values::{Object, array, record};

/// Runs `compile` over the files `files` the wrapper passed, asking the
/// caller's function for a file it does not hold.
fn run<T>(
    files: &JsValue,
    compile: impl FnOnce(&Files) -> Result<Compiled<T>, Failure>,
) -> Result<Compiled<T>, Failure> {
    let given = Files::new(files)?;
    given.asking(files, || compile(&given))
}

fn machines(all: Vec<Machine>) -> JsValue {
    array(all.into_iter().map(|t| Transducer::new(t).into()))
}

fn with_transducer(compiled: Compiled<Machine>) -> JsValue {
    Object::new()
        .with("transducer", Transducer::new(compiled.value))
        .with("warnings", warnings(&compiled.diagnostics))
        .done()
}

// [spec:hfst:req:javascript.compile-module]
#[wasm_bindgen]
pub fn regex(
    source: &str,
    name: &str,
    files: JsValue,
    options: &JsValue,
) -> Result<JsValue, Failure> {
    let o = Options::new("regex", options)?;
    let compiled = run(&files, |files| {
        compile::regex(Source { name, text: source }, files, &o)
    })?;
    Ok(with_transducer(compiled))
}

// [spec:hfst:req:javascript.compile-module]
/// The lexc `sources`, each named by the name at its place in `names`.
#[wasm_bindgen(js_name = compileLexc)]
pub fn compile_lexc(
    sources: Vec<String>,
    names: Vec<String>,
    files: JsValue,
    options: &JsValue,
) -> Result<JsValue, Failure> {
    if names.len() != sources.len() {
        let message = format!(
            "compileLexc takes a name for each of the {} sources, not {}",
            sources.len(),
            names.len()
        );
        return Err(GlueError::new(Code::InvalidArgument, message).into());
    }
    let o = Options::new("compileLexc", options)?;
    let named: Vec<Source<'_>> = names
        .iter()
        .zip(&sources)
        .map(|(name, text)| Source { name, text })
        .collect();
    let compiled = run(&files, |files| compile::lexc(&named, files, &o))?;
    Ok(with_transducer(compiled))
}

// [spec:hfst:req:javascript.compile-module]
/// `{ transducers, warnings }`.
#[wasm_bindgen(js_name = compileTwolc)]
pub fn compile_twolc(
    source: &str,
    name: &str,
    files: JsValue,
    options: &JsValue,
) -> Result<JsValue, Failure> {
    Files::new(&files)?;
    let compiled = compile::twolc(
        Source { name, text: source },
        &Options::new("compileTwolc", options)?,
    )?;
    Ok(Object::new()
        .with("transducers", machines(compiled.value))
        .with("warnings", warnings(&compiled.diagnostics))
        .done())
}

// [spec:hfst:req:javascript.compile-module]
// [spec:hfst:req:javascript.compile-files]
/// `{ output, stack, definitions, written, warnings }`, the files it wrote
/// as bytes by name.
#[wasm_bindgen(js_name = compileXfst)]
pub fn compile_xfst(
    source: &str,
    input: &str,
    name: &str,
    files: JsValue,
    options: &JsValue,
) -> Result<JsValue, Failure> {
    let o = Options::new("compileXfst", options)?;
    let compiled = run(&files, |files| {
        compile::xfst(Source { name, text: source }, input, files, &o)
    })?;
    let ran = compiled.value;
    let written = ran.written.into_iter().map(|(file, bytes)| {
        let bytes = js_sys::Uint8Array::from(bytes.as_slice());
        (file, JsValue::from(bytes))
    });
    let definitions = ran
        .definitions
        .into_iter()
        .map(|(defined, t)| (defined, Transducer::new(t).into()));
    Ok(Object::new()
        .with("output", ran.output)
        .with("stack", machines(ran.stack))
        .with("definitions", record(definitions))
        .with("written", record(written))
        .with("warnings", warnings(&compiled.diagnostics))
        .done())
}

// [spec:hfst:req:javascript.compile-module]
// [spec:hfst:req:javascript.compile-files]
/// `{ archive, warnings }`.
#[wasm_bindgen(js_name = compilePmatch)]
pub fn compile_pmatch(
    source: &str,
    name: &str,
    files: JsValue,
    options: &JsValue,
) -> Result<JsValue, Failure> {
    let o = Options::new("compilePmatch", options)?;
    let compiled = run(&files, |files| {
        compile::pmatch(Source { name, text: source }, files, &o)
    })?;
    Ok(Object::new()
        .with(
            "archive",
            js_sys::Uint8Array::from(compiled.value.as_slice()),
        )
        .with("warnings", warnings(&compiled.diagnostics))
        .done())
}

// [spec:hfst:req:javascript.compile-module]
/// The transducer of the text `source` of `kind`: `strings` as
/// `compileStrings` takes them, or `att` or `prolog` text as `parseAtt` and
/// `parseProlog` read it.
#[wasm_bindgen(js_name = compileText)]
pub fn compile_text(
    kind: &str,
    source: &str,
    name: &str,
    files: JsValue,
    options: &JsValue,
) -> Result<JsValue, Failure> {
    let Some((kind, call)) = compile::Text::named(kind) else {
        let message = format!("there is no kind of text called {kind}");
        return Err(GlueError::new(Code::InvalidArgument, message).into());
    };
    Files::new(&files)?;
    let o = Options::new(call, options)?;
    Ok(with_transducer(compile::text(
        kind,
        Source { name, text: source },
        &o,
    )?))
}
