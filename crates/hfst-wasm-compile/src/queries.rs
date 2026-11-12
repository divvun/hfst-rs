//! The queries: what the `hfst` tools that read a transducer and report on
//! it report, as plain objects. A query on one transducer is a method of
//! it; one over several, or none, is a function.

use hfst_embed::compile::{self as embed, GuessReport, LookupOutcome, Transducer as Machine};
use hfst_embed::lookup::LookupResult;
use hfst_wasm_shared::GlueError;
use wasm_bindgen::prelude::*;

use crate::list::TransducerList;
use crate::options::Options;
use crate::query_options as parse;
use crate::tokenise::Tokeniser;
use crate::transducer::Transducer;
use crate::values::{Object, array, strings};

fn result(found: &LookupResult) -> JsValue {
    Object::new()
        .with("output", found.output.as_str())
        .with("symbols", strings(&found.symbols))
        .with("weight", f64::from(found.weight))
        .done()
}

/// A lookup tool's report on each line, as `LookupReport`s.
fn lookup_reports(reports: Vec<embed::LookupReport>) -> JsValue {
    array(reports.into_iter().map(|report| {
        let (results, infinite) = match &report.outcome {
            LookupOutcome::OutsideAlphabet => (JsValue::NULL, false),
            LookupOutcome::Found { infinite, results } => {
                (array(results.iter().map(result)), *infinite)
            }
        };
        Object::new()
            .with("input", report.input.as_str())
            .with("markup", report.markup.as_str())
            .with("results", results)
            .with("infinite", infinite)
            .done()
    }))
}

fn reading_all<T>(
    list: &TransducerList,
    f: impl FnOnce(&[&Machine]) -> Result<T, hfst_embed::Error>,
) -> Result<T, GlueError> {
    let read = list.reading()?;
    let machines: Vec<&Machine> = read.iter().map(|t| &**t).collect();
    Ok(f(&machines)?)
}

// [spec:hfst:req:javascript.compile-module]
#[wasm_bindgen]
impl Transducer {
    /// What `hfst summarize` reports on the transducer, and with `verbose`
    /// what `--verbose` measures besides.
    pub fn summarize(&self, options: &JsValue) -> Result<JsValue, GlueError> {
        let verbose = parse::summarize(&Options::new("summarize", options)?)?;
        let s = self.reading()?.summarize(verbose)?;
        let details = match s.details {
            None => JsValue::NULL,
            Some(d) => Object::new()
                .with("infinitelyAmbiguous", d.infinitely_ambiguous)
                .with(
                    "firstInputSymbols",
                    d.first_input_symbols.map_or(JsValue::NULL, strings),
                )
                .done(),
        };
        let ambiguous =
            |(symbol, n): &(String, u32)| array([JsValue::from_str(symbol), JsValue::from(*n)]);
        Ok(Object::new()
            .with("name", s.name.as_str())
            .with("type", s.format.name())
            .with("states", s.states as f64)
            .with("finalStates", s.final_states as f64)
            .with("arcs", s.arcs as f64)
            .with("ioEpsilons", s.io_epsilons as f64)
            .with("inputEpsilons", s.input_epsilons as f64)
            .with("outputEpsilons", s.output_epsilons as f64)
            .with("densestArcs", s.densest_arcs as f64)
            .with("sparsestArcs", s.sparsest_arcs as f64)
            .with("uniqueInputArcs", s.unique_input_arcs as f64)
            .with("uniqueOutputArcs", s.unique_output_arcs as f64)
            .with("mostAmbiguousInput", ambiguous(&s.most_ambiguous_input))
            .with("mostAmbiguousOutput", ambiguous(&s.most_ambiguous_output))
            .with("foundAlphabet", strings(&s.found_alphabet))
            .with(
                "symbolPairs",
                array(s.symbol_pairs.iter().map(|(input, output, n)| {
                    array([
                        JsValue::from_str(input),
                        JsValue::from_str(output),
                        JsValue::from(*n),
                    ])
                })),
            )
            .with("acceptor", s.acceptor)
            .with("inputDeterministic", s.input_deterministic)
            .with("outputDeterministic", s.output_deterministic)
            .with("cyclic", s.cyclic)
            .with("cyclicAtInitialState", s.cyclic_at_initial_state)
            .with("alphabet", strings(&s.alphabet))
            .with("details", details)
            .done())
    }

    /// What `hfst check-alpha` reports on the transducer and `other`.
    #[wasm_bindgen(js_name = checkAlpha)]
    pub fn check_alpha(&self, other: &Transducer) -> Result<JsValue, GlueError> {
        let first = self.reading()?;
        let c = if self.is(&other.machine) {
            first.check_alpha(&first)?
        } else {
            first.check_alpha(&*other.reading()?)?
        };
        Ok(Object::new()
            .with("firstUsed", strings(&c.first_used))
            .with("secondUsed", strings(&c.second_used))
            .with("firstDeclared", strings(&c.first_declared))
            .with("secondDeclared", strings(&c.second_declared))
            .with("mismatch", c.mismatch)
            .done())
    }

    /// Whether the transducer and `other` are equivalent, as `hfst compare`
    /// decides.
    pub fn compare(&self, other: &Transducer, options: &JsValue) -> Result<bool, GlueError> {
        let options = parse::compare(&Options::new("compare", options)?)?;
        let first = self.reading()?;
        if self.is(&other.machine) {
            return Ok(first.compare(&first, &options)?);
        }
        Ok(first.compare(&*other.reading()?, &options)?)
    }

    /// What `hfst dump-alphabets` reports on the transducer.
    #[wasm_bindgen(js_name = dumpAlphabets)]
    pub fn dump_alphabets(&self) -> Result<JsValue, GlueError> {
        let a = self.reading()?.dump_alphabets()?;
        Ok(Object::new()
            .with("declared", strings(&a.declared))
            .with("seen", strings(&a.seen))
            .done())
    }

    /// The paths `hfst fst2strings` prints, in its order.
    #[wasm_bindgen(js_name = extractPaths)]
    pub fn extract_paths(&self, options: &JsValue) -> Result<JsValue, GlueError> {
        let options = parse::extract_paths(&Options::new("extractPaths", options)?)?;
        let paths = self.reading()?.extract_paths(&options)?;
        Ok(array(paths.iter().map(|path| {
            Object::new()
                .with("input", path.input.as_str())
                .with("output", path.output.as_str())
                .with(
                    "pairs",
                    array(path.pairs.iter().map(|(i, o)| strings([i, o]))),
                )
                .with("weight", f64::from(path.weight))
                .done()
        })))
    }

    /// What `hfst guess` prints for each of `words` with the transducer, a
    /// guesser `guessify` made.
    pub fn guess(&self, words: Vec<String>, options: &JsValue) -> Result<JsValue, GlueError> {
        self.guess_with(words, None, options)
    }

    /// What `hfst guess` prints for each of `words` with the transducer and
    /// the model form generator `generator`.
    #[wasm_bindgen(js_name = guessWith)]
    pub fn guess_with_generator(
        &self,
        words: Vec<String>,
        generator: &Transducer,
        options: &JsValue,
    ) -> Result<JsValue, GlueError> {
        self.guess_with(words, Some(generator), options)
    }

    /// The tokeniser `hfst tokenize` makes of a transducer that is not a
    /// pmatch container.
    #[wasm_bindgen(js_name = naiveTokeniser)]
    pub fn naive_tokeniser(&self) -> Result<Tokeniser, GlueError> {
        let container = embed::naive_tokeniser(&*self.reading()?, false)?;
        Ok(Tokeniser::new(container))
    }
}

impl Transducer {
    fn guess_with(
        &self,
        words: Vec<String>,
        generator: Option<&Transducer>,
        options: &JsValue,
    ) -> Result<JsValue, GlueError> {
        let (options, model_forms) = parse::guess(&Options::new("guess", options)?)?;
        let guesser = self.reading()?;
        let generated = match generator {
            Some(g) if !self.is(&g.machine) => Some(g.reading()?),
            Some(_) | None => None,
        };
        let generator: Option<&Machine> = match generator {
            Some(g) if self.is(&g.machine) => Some(&*guesser),
            Some(_) | None => generated.as_deref(),
        };
        let read: Vec<&str> = words.iter().map(String::as_str).collect();
        let reports = embed::guess(&guesser, generator, model_forms.as_deref(), &read, &options)?;
        Ok(array(words.iter().zip(reports).map(|(word, report)| {
            let (paradigms, guesses) = match report {
                GuessReport::Guesses(guesses) => (false, guesses),
                GuessReport::Paradigms(paradigms) => (true, paradigms),
            };
            Object::new()
                .with("input", word.as_str())
                .with("paradigms", paradigms)
                .with("guesses", strings(&guesses))
                .done()
        })))
    }
}

// [spec:hfst:req:javascript.compile-module]
/// What `hfst lookup` reports for each of `inputs` with the stream
/// `transducers`.
#[wasm_bindgen(js_name = lookupReport)]
pub fn lookup_report(
    transducers: &TransducerList,
    inputs: Vec<String>,
    options: &JsValue,
) -> Result<JsValue, GlueError> {
    let options = parse::lookup_report(&Options::new("lookupReport", options)?)?;
    let lines: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let reports = reading_all(transducers, |ts| embed::lookup_report(ts, &lines, &options))?;
    Ok(lookup_reports(reports))
}

// [spec:hfst:req:javascript.compile-module]
/// What `hfst flookup` reports for each of `inputs` with the stream
/// `transducers`.
#[wasm_bindgen(js_name = flookupReport)]
pub fn flookup_report(
    transducers: &TransducerList,
    inputs: Vec<String>,
    options: &JsValue,
) -> Result<JsValue, GlueError> {
    let options = parse::flookup_report(&Options::new("flookupReport", options)?)?;
    let lines: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let reports = reading_all(transducers, |ts| {
        embed::flookup_report(ts, &lines, &options)
    })?;
    Ok(lookup_reports(reports))
}

// [spec:hfst:req:javascript.compile-module]
/// What `hfst optimized-lookup` reports for each of `words` with the union
/// of the stream `transducers`.
#[wasm_bindgen(js_name = optimizedLookupReport)]
pub fn optimized_lookup_report(
    transducers: &TransducerList,
    words: Vec<String>,
    options: &JsValue,
) -> Result<JsValue, GlueError> {
    let options = parse::optimized_lookup_report(&Options::new("optimizedLookupReport", options)?)?;
    let read: Vec<&str> = words.iter().map(String::as_str).collect();
    let reports = reading_all(transducers, |ts| {
        embed::optimized_lookup_report(ts, &read, &options)
    })?;
    Ok(array(reports.into_iter().map(|report| {
        let analyses = report.analyses.map_or(JsValue::NULL, |analyses| {
            array(analyses.iter().map(|(output, weight)| {
                array([JsValue::from_str(output), JsValue::from(f64::from(*weight))])
            }))
        });
        Object::new()
            .with("input", report.input.as_str())
            .with("analyses", analyses)
            .done()
    })))
}

// [spec:hfst:req:javascript.compile-module]
/// What `hfst pair-test` reports with the twolc rule transducers `rules` on
/// `tests`.
#[wasm_bindgen(js_name = pairTest)]
pub fn pair_test(
    rules: &TransducerList,
    tests: &str,
    options: &JsValue,
) -> Result<JsValue, GlueError> {
    let options = parse::pair_test(&Options::new("pairTest", options)?)?;
    let found = reading_all(rules, |ts| embed::pair_test(ts, tests, &options))?;
    Ok(array(found.iter().map(|test| {
        let failures = test.failures.iter().map(|f| {
            Object::new()
                .with("rule", f.rule.as_str())
                .with("recognizedPrefix", f.recognized_prefix as f64)
                .done()
        });
        Object::new()
            .with("test", test.test.as_str())
            .with("positive", test.positive)
            .with(
                "pairs",
                array(test.pairs.iter().map(|(i, o)| strings([i, o]))),
            )
            .with("failed", test.failed)
            .with("failures", array(failures))
            .done()
    })))
}

// [spec:hfst:req:javascript.compile-module]
/// What `hfst grep` reports for `pattern` on `text`.
#[wasm_bindgen]
pub fn grep(pattern: &str, text: &str, options: &JsValue) -> Result<JsValue, GlueError> {
    let options = parse::grep(&Options::new("grep", options)?)?;
    let report = embed::grep(pattern, text, &options)?;
    let lines = report.lines.iter().map(|line| {
        Object::new()
            .with("number", line.number as f64)
            .with("text", line.text.as_str())
            .done()
    });
    Ok(Object::new()
        .with("lines", array(lines))
        .with("matched", report.matched)
        .done())
}
