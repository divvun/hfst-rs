//! The queries: what the `hfst` tools that read a transducer and report on
//! it report, as the dataclasses of `divvun_hfst._results`. A query on one
//! transducer is a method of it; one over several, or none, is a function.

use std::sync::Arc;

use hfst::pmatch::PmatchContainer;
use hfst_embed::compile::{
    self as embed, Cascade, CompareOptions, ExtractPathsOptions, FlookupOptions, GrepOptions,
    GuessOptions, GuessReport, LookupInput, LookupOptions, LookupOutcome, OptimizedLookupOptions,
    PairTestOptions, PmatchOptions, PmatchResult, Transducer as Machine,
};
use hfst_embed::lookup::LookupResult as Found;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::call::without_gil;
use crate::error::hfst_error;
use crate::operations::choice;
use crate::results::result;
use crate::tokenise::Tokeniser;
use crate::transducer::{LookupResult, Transducer, locked};

/// `f` reading `transducers`, each locked once, without the GIL. A failure
/// raises HfstError.
fn reading_all<T: Send>(
    py: Python<'_>,
    transducers: &[&Transducer],
    f: impl FnOnce(&[&Machine]) -> Result<T, hfst_embed::Error> + Send,
) -> PyResult<T> {
    let done = without_gil(py, || {
        let (_, read) = locked(None, transducers);
        let machines: Vec<&Machine> = transducers.iter().filter_map(|t| read.get(t)).collect();
        f(&machines)
    })?;
    done.map_err(|error| hfst_error(py, error))
}

fn results<'py>(py: Python<'py>, found: Vec<Found>) -> PyResult<Bound<'py, PyTuple>> {
    let results: Vec<LookupResult> = found.into_iter().map(LookupResult::from).collect();
    PyTuple::new(py, results)
}

/// A lookup tool's report on each line, as `LookupReport`s.
fn lookup_reports(
    py: Python<'_>,
    reports: Vec<embed::LookupReport>,
) -> PyResult<Vec<Bound<'_, PyAny>>> {
    reports
        .into_iter()
        .map(|report| {
            let (found, infinite) = match report.outcome {
                LookupOutcome::OutsideAlphabet => (None, false),
                LookupOutcome::Found { infinite, results } => (Some(results), infinite),
            };
            let found = found.map(|found| results(py, found)).transpose()?;
            result!(py, "LookupReport" {
                "input": report.input,
                "markup": report.markup,
                "results": found,
                "infinite": infinite,
            })
        })
        .collect()
}

pub const INPUT_FORMATS: [(&str, LookupInput); 3] = [
    ("text", LookupInput::Text),
    ("spaced", LookupInput::Spaced),
    ("apertium", LookupInput::Apertium),
];

// [spec:hfst:req:python.operations]
// [spec:hfst:req:python.threads+1]
#[pymethods]
impl Transducer {
    /// What `hfst summarize` reports on the transducer, and with `verbose`
    /// what `--verbose` measures besides.
    #[pyo3(signature = (*, verbose = false))]
    fn summarize<'py>(&self, py: Python<'py>, verbose: bool) -> PyResult<Bound<'py, PyAny>> {
        let s = self.read(py, |t| t.summarize(verbose))?;
        let details = match s.details {
            Some(d) => Some(result!(py, "SummaryDetails" {
                "infinitely_ambiguous": d.infinitely_ambiguous,
                "first_input_symbols": d
                    .first_input_symbols
                    .map(|symbols| PyTuple::new(py, symbols))
                    .transpose()?,
            })?),
            None => None,
        };
        result!(py, "Summary" {
            "name": s.name,
            "type": s.format.name(),
            "states": s.states,
            "final_states": s.final_states,
            "arcs": s.arcs,
            "io_epsilons": s.io_epsilons,
            "input_epsilons": s.input_epsilons,
            "output_epsilons": s.output_epsilons,
            "densest_arcs": s.densest_arcs,
            "sparsest_arcs": s.sparsest_arcs,
            "unique_input_arcs": s.unique_input_arcs,
            "unique_output_arcs": s.unique_output_arcs,
            "most_ambiguous_input": s.most_ambiguous_input,
            "most_ambiguous_output": s.most_ambiguous_output,
            "found_alphabet": PyTuple::new(py, s.found_alphabet)?,
            "symbol_pairs": PyTuple::new(py, s.symbol_pairs)?,
            "acceptor": s.acceptor,
            "input_deterministic": s.input_deterministic,
            "output_deterministic": s.output_deterministic,
            "cyclic": s.cyclic,
            "cyclic_at_initial_state": s.cyclic_at_initial_state,
            "alphabet": PyTuple::new(py, s.alphabet)?,
            "details": details,
        })
    }

    /// What `hfst check-alpha` reports on the transducer and `other`.
    #[pyo3(signature = (other, /))]
    fn check_alpha<'py>(
        &self,
        py: Python<'py>,
        other: &Bound<'_, Transducer>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let c = reading_all(py, &[self, other.get()], |ts| match ts {
            [first, second] => first.check_alpha(second),
            [one] => one.check_alpha(one),
            _ => Err(crate::transducer::unlocked()),
        })?;
        result!(py, "AlphabetComparison" {
            "first_used": PyTuple::new(py, c.first_used)?,
            "second_used": PyTuple::new(py, c.second_used)?,
            "first_declared": PyTuple::new(py, c.first_declared)?,
            "second_declared": PyTuple::new(py, c.second_declared)?,
            "mismatch": c.mismatch,
        })
    }

    /// Whether the transducer and `other` are equivalent, as `hfst compare`
    /// decides. Like the tool, it compares two transducers of one algebra
    /// type; flag diacritics are harmonized unless `do_not_harmonize`, and
    /// eliminated first with `eliminate_flags`.
    #[pyo3(signature = (other, /, *, do_not_harmonize = false, eliminate_flags = false))]
    fn compare(
        &self,
        py: Python<'_>,
        other: &Bound<'_, Transducer>,
        do_not_harmonize: bool,
        eliminate_flags: bool,
    ) -> PyResult<bool> {
        let options = CompareOptions {
            do_not_harmonize,
            eliminate_flags,
        };
        reading_all(py, &[self, other.get()], |ts| match ts {
            [first, second] => first.compare(second, &options),
            [one] => one.compare(one, &options),
            _ => Err(crate::transducer::unlocked()),
        })
    }

    /// What `hfst dump-alphabets` reports on the transducer.
    fn dump_alphabets<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let a = self.read(py, Machine::dump_alphabets)?;
        result!(py, "Alphabets" {
            "declared": PyTuple::new(py, a.declared)?,
            "seen": PyTuple::new(py, a.seen)?,
        })
    }

    /// The paths `hfst fst2strings` prints, in its order. Each limit left as
    /// None sets none: at most `max_strings` paths, the `nbest` lightest,
    /// `random` paths chosen at random, cycles followed at most `cycles`
    /// times, and only paths within `beam` of the lightest, whose input is
    /// at most `max_in_length` bytes and output `max_out_length`. Each path
    /// must start with `in_prefix` and `out_prefix` and must not contain
    /// `in_exclude` or `out_exclude`, unless they are empty. `obey_flags`
    /// drops the paths flag diacritics forbid, and `print_flags` keeps the
    /// flags in the paths.
    #[pyo3(signature = (
        *,
        max_strings = None,
        nbest = None,
        random = None,
        cycles = None,
        beam = None,
        obey_flags = false,
        print_flags = false,
        max_in_length = None,
        max_out_length = None,
        in_prefix = "",
        out_prefix = "",
        in_exclude = "",
        out_exclude = "",
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a keyword-only option of hfst fst2strings"
    )]
    fn extract_paths<'py>(
        &self,
        py: Python<'py>,
        max_strings: Option<i32>,
        nbest: Option<i32>,
        random: Option<i32>,
        cycles: Option<i32>,
        beam: Option<f32>,
        obey_flags: bool,
        print_flags: bool,
        max_in_length: Option<u32>,
        max_out_length: Option<u32>,
        in_prefix: &str,
        out_prefix: &str,
        in_exclude: &str,
        out_exclude: &str,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let defaults = ExtractPathsOptions::default();
        let options = ExtractPathsOptions {
            max_strings: max_strings.unwrap_or(defaults.max_strings),
            nbest: nbest.unwrap_or(defaults.nbest),
            random: random.unwrap_or(defaults.random),
            cycles: cycles.unwrap_or(defaults.cycles),
            beam,
            obey_flags,
            print_flags,
            max_in_length: max_in_length.unwrap_or(0),
            max_out_length: max_out_length.unwrap_or(0),
            in_prefix: in_prefix.to_owned(),
            out_prefix: out_prefix.to_owned(),
            in_exclude: in_exclude.to_owned(),
            out_exclude: out_exclude.to_owned(),
        };
        let paths = self.read(py, |t| t.extract_paths(&options))?;
        paths
            .into_iter()
            .map(|path| {
                result!(py, "ExtractedPath" {
                    "input": path.input,
                    "output": path.output,
                    "pairs": PyTuple::new(py, path.pairs)?,
                    "weight": f64::from(path.weight),
                })
            })
            .collect()
    }

    /// What `hfst guess` prints for each of `words` with the transducer, a
    /// guesser `guessify` made. With `model_forms`, the text of a model form
    /// file, it generates model forms for each guess with `generator`, or
    /// with one compiled from the guesser.
    #[pyo3(signature = (
        words,
        /,
        *,
        generator = None,
        model_forms = None,
        max_number_of_guesses = 5,
        max_number_of_forms = 2,
        generate_threshold = 50.0,
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a keyword-only option of hfst guess"
    )]
    fn guess<'py>(
        &self,
        py: Python<'py>,
        words: Vec<String>,
        generator: Option<Bound<'_, Transducer>>,
        model_forms: Option<String>,
        max_number_of_guesses: usize,
        max_number_of_forms: usize,
        generate_threshold: f32,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let options = GuessOptions {
            max_number_of_guesses,
            max_number_of_forms,
            generate_threshold,
        };
        let mut transducers = vec![self];
        transducers.extend(generator.as_ref().map(Bound::get));
        let words_read: Vec<&str> = words.iter().map(String::as_str).collect();
        let reports = reading_all(py, &transducers, |ts| {
            let (guesser, generator) = match ts {
                [guesser] => (*guesser, None),
                [guesser, generator] => (*guesser, Some(*generator)),
                _ => return Err(crate::transducer::unlocked()),
            };
            embed::guess(
                guesser,
                generator,
                model_forms.as_deref(),
                &words_read,
                &options,
            )
        })?;
        words
            .into_iter()
            .zip(reports)
            .map(|(word, report)| {
                let (paradigms, guesses) = match report {
                    GuessReport::Guesses(guesses) => (false, guesses),
                    GuessReport::Paradigms(paradigms) => (true, paradigms),
                };
                result!(py, "GuessReport" {
                    "input": word,
                    "paradigms": paradigms,
                    "guesses": PyTuple::new(py, guesses)?,
                })
            })
            .collect()
    }

    /// The tokeniser `hfst tokenize` makes of a transducer that is not a
    /// pmatch container: each word of the transducer's input side a token,
    /// analysed as its outputs. Multicharacter symbols are matched longest
    /// first when the transducer has any, or when the options of a call set
    /// `tokenize_multichar`.
    fn naive_tokeniser(&self, py: Python<'_>) -> PyResult<Tokeniser> {
        let container = self.read(py, |t| embed::naive_tokeniser(t, false))?;
        Ok(Tokeniser::from_core(container.core()))
    }
}

// [spec:hfst:req:python.operations]
#[pymethods]
impl Tokeniser {
    /// What `hfst pmatch` reports for `text` with the tokeniser's container:
    /// for each piece of it, each run of lines up to an empty one or with
    /// `newline` each line, the piece with its matches marked or with
    /// `locate` its matches. The options left as None keep the container's
    /// own settings.
    #[pyo3(signature = (
        text,
        /,
        *,
        newline = false,
        locate = false,
        extract_patterns = None,
        delete_patterns = None,
        mark_patterns = None,
        max_context = None,
        max_recursion = None,
        time_cutoff = 0.0,
        weight_cutoff = None,
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a keyword-only option of hfst pmatch"
    )]
    fn pmatch<'py>(
        &self,
        py: Python<'py>,
        text: &str,
        newline: bool,
        locate: bool,
        extract_patterns: Option<bool>,
        delete_patterns: Option<bool>,
        mark_patterns: Option<bool>,
        max_context: Option<usize>,
        max_recursion: Option<usize>,
        time_cutoff: f64,
        weight_cutoff: Option<f32>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let defaults = PmatchOptions::default();
        let options = PmatchOptions {
            blankline_separated: !newline,
            locate,
            extract_patterns,
            delete_patterns,
            mark_patterns,
            max_context,
            max_recursion,
            time_cutoff,
            weight_cutoff: weight_cutoff.unwrap_or(defaults.weight_cutoff),
        };
        let core = self.core();
        let pieces = without_gil(py, || {
            let mut container = PmatchContainer::from_core(Arc::clone(core));
            embed::pmatch(&mut container, text, &options)
        })?;
        pieces
            .into_iter()
            .map(|(piece, found)| {
                let (marked, located) = match found {
                    PmatchResult::Marked(marked) => (Some(marked), None),
                    PmatchResult::Located(locations) => {
                        let located = locations
                            .into_iter()
                            .map(|l| {
                                result!(py, "PmatchLocation" {
                                    "start": l.start,
                                    "length": l.length,
                                    "output": l.output,
                                    "tag": l.tag,
                                    "weight": f64::from(l.weight),
                                })
                            })
                            .collect::<PyResult<Vec<_>>>()?;
                        (None, Some(PyTuple::new(py, located)?))
                    }
                };
                result!(py, "PmatchResult" {
                    "input": piece,
                    "marked": marked,
                    "located": located,
                })
            })
            .collect()
    }
}

// [spec:hfst:req:python.operations]
/// What `hfst lookup` reports for each of `inputs` with the stream
/// `transducers`. `input_format` reads each line as `"text"`, as symbols
/// `"spaced"` apart, or as `"apertium"` stream format. `cascade` combines the
/// results of several transducers: `"union"`, `"priority-union"` or
/// `"composition"`. Each other option is the tool's: `max_number` results a
/// word in optimized lookup, epsilon cycles followed `cycles` times, results
/// within `beam` of the lightest, `time_cutoff` seconds a word, flags obeyed
/// unless not `obey_flags` and shown with `show_flags`.
#[pyfunction]
#[pyo3(signature = (
    transducers,
    inputs,
    /,
    *,
    input_format = "text",
    epsilon_format = "",
    max_number = None,
    cycles = 5,
    beam = None,
    time_cutoff = 0.0,
    cascade = "union",
    obey_flags = true,
    show_flags = false,
))]
#[expect(
    clippy::too_many_arguments,
    reason = "each is a keyword-only option of hfst lookup"
)]
pub fn lookup_report<'py>(
    py: Python<'py>,
    transducers: Vec<Bound<'_, Transducer>>,
    inputs: Vec<String>,
    input_format: &str,
    epsilon_format: &str,
    max_number: Option<usize>,
    cycles: usize,
    beam: Option<f32>,
    time_cutoff: f64,
    cascade: &str,
    obey_flags: bool,
    show_flags: bool,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let options = LookupOptions {
        input_format: choice(
            "lookup_report()",
            "input_format",
            input_format,
            &INPUT_FORMATS,
        )?,
        epsilon_format: epsilon_format.to_owned(),
        max_number,
        cycles,
        beam,
        time_cutoff,
        cascade: choice(
            "lookup_report()",
            "cascade",
            cascade,
            &[
                ("union", Cascade::Union),
                ("priority-union", Cascade::PriorityUnion),
                ("composition", Cascade::Composition),
            ],
        )?,
        obey_flags,
        show_flags,
    };
    let transducers: Vec<&Transducer> = transducers.iter().map(Bound::get).collect();
    let lines: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let reports = reading_all(py, &transducers, |ts| {
        embed::lookup_report(ts, &lines, &options)
    })?;
    lookup_reports(py, reports)
}

// [spec:hfst:req:python.operations]
/// What `hfst flookup` reports for each of `inputs` with the stream
/// `transducers`. Like the tool, it looks up in each transducer inverted
/// unless `invert`, and then refuses optimized lookup unless `force_ol`.
/// The other options are lookup_report's.
#[pyfunction]
#[pyo3(signature = (
    transducers,
    inputs,
    /,
    *,
    input_format = "text",
    epsilon_format = "",
    cycles = 5,
    beam = None,
    time_cutoff = 0.0,
    invert = false,
    force_ol = false,
    obey_flags = true,
    show_flags = false,
))]
#[expect(
    clippy::too_many_arguments,
    reason = "each is a keyword-only option of hfst flookup"
)]
pub fn flookup_report<'py>(
    py: Python<'py>,
    transducers: Vec<Bound<'_, Transducer>>,
    inputs: Vec<String>,
    input_format: &str,
    epsilon_format: &str,
    cycles: usize,
    beam: Option<f32>,
    time_cutoff: f64,
    invert: bool,
    force_ol: bool,
    obey_flags: bool,
    show_flags: bool,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let options = FlookupOptions {
        input_format: choice(
            "flookup_report()",
            "input_format",
            input_format,
            &INPUT_FORMATS,
        )?,
        epsilon_format: epsilon_format.to_owned(),
        cycles,
        beam,
        time_cutoff,
        invert,
        force_ol,
        obey_flags,
        show_flags,
    };
    let transducers: Vec<&Transducer> = transducers.iter().map(Bound::get).collect();
    let lines: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let reports = reading_all(py, &transducers, |ts| {
        embed::flookup_report(ts, &lines, &options)
    })?;
    lookup_reports(py, reports)
}

// [spec:hfst:req:python.operations]
/// What `hfst optimized-lookup` reports for each of `words` with the union
/// of the stream `transducers`: each analysis once with `unique`, at most
/// `analyses` of them, `time_cutoff` seconds a word, only those within
/// `beam` of the lightest, and with `fast` every analysis as found.
#[pyfunction]
#[pyo3(signature = (
    transducers,
    words,
    /,
    *,
    unique = false,
    analyses = None,
    time_cutoff = 0.0,
    beam = None,
    fast = false,
))]
#[expect(
    clippy::too_many_arguments,
    reason = "each is a keyword-only option of hfst optimized-lookup"
)]
pub fn optimized_lookup_report<'py>(
    py: Python<'py>,
    transducers: Vec<Bound<'_, Transducer>>,
    words: Vec<String>,
    unique: bool,
    analyses: Option<i32>,
    time_cutoff: f64,
    beam: Option<f32>,
    fast: bool,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let defaults = OptimizedLookupOptions::default();
    let options = OptimizedLookupOptions {
        unique,
        max_analyses: analyses.unwrap_or(defaults.max_analyses),
        time_cutoff,
        beam: beam.unwrap_or(defaults.beam),
        fast,
    };
    let transducers: Vec<&Transducer> = transducers.iter().map(Bound::get).collect();
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let reports = reading_all(py, &transducers, |ts| {
        embed::optimized_lookup_report(ts, &words, &options)
    })?;
    reports
        .into_iter()
        .map(|report| {
            let analyses = report
                .analyses
                .map(|analyses| {
                    let analyses: Vec<(String, f64)> = analyses
                        .into_iter()
                        .map(|(output, weight)| (output, f64::from(weight)))
                        .collect();
                    PyTuple::new(py, analyses)
                })
                .transpose()?;
            result!(py, "OptimizedLookupReport" {
                "input": report.input,
                "analyses": analyses,
            })
        })
        .collect()
}

// [spec:hfst:req:python.operations]
/// What `hfst pair-test` reports with the twolc rule transducers `rules` on
/// `tests`: each line a pair string, a negative test with `negative_test`,
/// or with `xerox_mode` the text of a twolc source holding test cases.
/// Blank and `!` comment lines are not tests.
#[pyfunction]
#[pyo3(signature = (rules, tests, /, *, negative_test = false, xerox_mode = false))]
pub fn pair_test<'py>(
    py: Python<'py>,
    rules: Vec<Bound<'_, Transducer>>,
    tests: &str,
    negative_test: bool,
    xerox_mode: bool,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let options = PairTestOptions {
        negative_test,
        xerox_mode,
    };
    let rules: Vec<&Transducer> = rules.iter().map(Bound::get).collect();
    let found = reading_all(py, &rules, |ts| embed::pair_test(ts, tests, &options))?;
    found
        .into_iter()
        .map(|test| {
            let failures = test
                .failures
                .into_iter()
                .map(|f| {
                    result!(py, "RuleFailure" {
                        "rule": f.rule,
                        "recognized_prefix": f.recognized_prefix,
                    })
                })
                .collect::<PyResult<Vec<_>>>()?;
            result!(py, "PairTestResult" {
                "test": test.test,
                "positive": test.positive,
                "pairs": PyTuple::new(py, test.pairs)?,
                "failed": test.failed,
                "failures": PyTuple::new(py, failures)?,
            })
        })
        .collect()
}

// [spec:hfst:req:python.operations]
/// What `hfst grep` reports for `pattern`, a Xerox regular expression or
/// with `fixed_strings` a string, on `text`: the lines that match, or with
/// `invert_match` those that do not, their matches marked with `{{{ }}}` or
/// with `color` colour codes, at most `max_count` of them unless it is 0.
/// `word_regexp` matches only between spaces, `line_regexp` whole lines.
#[pyfunction]
#[pyo3(signature = (
    pattern,
    text,
    /,
    *,
    fixed_strings = false,
    color = false,
    word_regexp = false,
    line_regexp = false,
    invert_match = false,
    max_count = 0,
))]
#[expect(
    clippy::too_many_arguments,
    reason = "each is a keyword-only option of hfst grep"
)]
pub fn grep<'py>(
    py: Python<'py>,
    pattern: &str,
    text: &str,
    fixed_strings: bool,
    color: bool,
    word_regexp: bool,
    line_regexp: bool,
    invert_match: bool,
    max_count: u64,
) -> PyResult<Bound<'py, PyAny>> {
    let options = GrepOptions {
        fixed_strings,
        color,
        word_regexp,
        line_regexp,
        invert_match,
        max_count,
    };
    let report = without_gil(py, || embed::grep(pattern, text, &options))?
        .map_err(|error| hfst_error(py, error))?;
    let lines = report
        .lines
        .into_iter()
        .map(|line| {
            result!(py, "GrepLine" {
                "number": line.number,
                "text": line.text,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    result!(py, "GrepReport" {
        "lines": PyTuple::new(py, lines)?,
        "matched": report.matched,
    })
}
