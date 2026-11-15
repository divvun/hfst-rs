//! The queries: what the `hfst` tools that read a transducer and report on
//! it report, as records. A query on one transducer is a method of it; one
//! over several, or none, is a function.

use std::sync::Arc;

use hfst_embed::compile::{
    self as embed, Cascade, CompareOptions, ExtractPathsOptions, FlookupOptions, GrepOptions,
    GuessOptions, GuessReport as Guessed, LookupInput, LookupOptions, LookupOutcome,
    OptimizedLookupOptions, PairTestOptions, PmatchOptions, PmatchResult, Transducer as Machine,
};
use hfst_swift_runtime::GlueError;
use hfst_swift_runtime::error::invalid;
use hfst_swift_runtime::tokenise::Tokeniser;
use hfst_swift_runtime::transducer::LookupResult;

use crate::operations::choice;
use crate::transducer::{Transducer, reading_all, unlocked};

/// A count the caller gives, 0 or more.
fn count(what: &str, value: i64) -> Result<usize, GlueError> {
    usize::try_from(value).map_err(|_| invalid(format!("{what} is 0 or more, not {value}")))
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct SymbolCount {
    pub symbol: String,
    pub count: u32,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct SymbolPairCount {
    pub input: String,
    pub output: String,
    pub count: u32,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct SymbolPair {
    pub input: String,
    pub output: String,
}

fn pairs(pairs: Vec<(String, String)>) -> Vec<SymbolPair> {
    pairs
        .into_iter()
        .map(|(input, output)| SymbolPair { input, output })
        .collect()
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct SummaryDetails {
    pub infinitely_ambiguous: bool,
    pub first_input_symbols: Option<Vec<String>>,
}

// [spec:hfst:req:swift.operations]
/// What `hfst summarize` reports on a transducer.
#[derive(uniffi::Record)]
pub struct Summary {
    pub name: String,
    pub format: String,
    pub states: u64,
    pub final_states: u64,
    pub arcs: u64,
    pub io_epsilons: u64,
    pub input_epsilons: u64,
    pub output_epsilons: u64,
    pub densest_arcs: u64,
    pub sparsest_arcs: u64,
    pub unique_input_arcs: u64,
    pub unique_output_arcs: u64,
    pub most_ambiguous_input: SymbolCount,
    pub most_ambiguous_output: SymbolCount,
    pub found_alphabet: Vec<String>,
    pub symbol_pairs: Vec<SymbolPairCount>,
    pub acceptor: bool,
    pub input_deterministic: bool,
    pub output_deterministic: bool,
    pub cyclic: bool,
    pub cyclic_at_initial_state: bool,
    pub alphabet: Vec<String>,
    pub details: Option<SummaryDetails>,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct AlphabetComparison {
    pub first_used: Vec<String>,
    pub second_used: Vec<String>,
    pub first_declared: Vec<String>,
    pub second_declared: Vec<String>,
    pub mismatch: bool,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct Alphabets {
    pub declared: Vec<String>,
    pub seen: Vec<String>,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct ExtractedPath {
    pub input: String,
    pub output: String,
    pub pairs: Vec<SymbolPair>,
    pub weight: f32,
}

// [spec:hfst:req:swift.operations]
/// The options of `hfst fst2strings`; each limit `None` sets none.
#[derive(uniffi::Record)]
pub struct ExtractSettings {
    pub max_strings: Option<i32>,
    pub nbest: Option<i32>,
    pub random: Option<i32>,
    pub cycles: Option<i32>,
    pub beam: Option<f32>,
    pub obey_flags: bool,
    pub print_flags: bool,
    pub max_in_length: Option<u32>,
    pub max_out_length: Option<u32>,
    pub in_prefix: String,
    pub out_prefix: String,
    pub in_exclude: String,
    pub out_exclude: String,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct GuessReport {
    pub input: String,
    pub paradigms: bool,
    pub guesses: Vec<String>,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct PmatchLocation {
    pub start: u32,
    pub length: u32,
    pub output: String,
    pub tag: String,
    pub weight: f32,
}

// [spec:hfst:req:swift.operations]
/// What `hfst pmatch` reports for a piece of its input: the piece marked up,
/// or with `locate` its matches.
#[derive(uniffi::Record)]
pub struct PmatchPiece {
    pub input: String,
    pub marked: Option<String>,
    pub located: Option<Vec<PmatchLocation>>,
}

// [spec:hfst:req:swift.operations]
/// The options of `hfst pmatch`; each left `None` keeps the container's
/// setting.
#[derive(uniffi::Record)]
pub struct PmatchRunSettings {
    pub newline: bool,
    pub locate: bool,
    pub extract_patterns: Option<bool>,
    pub delete_patterns: Option<bool>,
    pub mark_patterns: Option<bool>,
    pub max_context: Option<i64>,
    pub max_recursion: Option<i64>,
    pub time_cutoff: f64,
    pub weight_cutoff: Option<f32>,
}

// [spec:hfst:req:swift.operations]
// [spec:hfst:req:swift.threads]
#[uniffi::export]
impl Transducer {
    pub fn summarize(&self, verbose: bool) -> Result<Summary, GlueError> {
        let s = self.read(|t| t.summarize(verbose))?;
        let count = |(symbol, count): (String, u32)| SymbolCount { symbol, count };
        Ok(Summary {
            name: s.name,
            format: s.format.name().to_owned(),
            states: s.states as u64,
            final_states: s.final_states as u64,
            arcs: s.arcs as u64,
            io_epsilons: s.io_epsilons as u64,
            input_epsilons: s.input_epsilons as u64,
            output_epsilons: s.output_epsilons as u64,
            densest_arcs: s.densest_arcs as u64,
            sparsest_arcs: s.sparsest_arcs as u64,
            unique_input_arcs: s.unique_input_arcs as u64,
            unique_output_arcs: s.unique_output_arcs as u64,
            most_ambiguous_input: count(s.most_ambiguous_input),
            most_ambiguous_output: count(s.most_ambiguous_output),
            found_alphabet: s.found_alphabet,
            symbol_pairs: s
                .symbol_pairs
                .into_iter()
                .map(|(input, output, count)| SymbolPairCount {
                    input,
                    output,
                    count,
                })
                .collect(),
            acceptor: s.acceptor,
            input_deterministic: s.input_deterministic,
            output_deterministic: s.output_deterministic,
            cyclic: s.cyclic,
            cyclic_at_initial_state: s.cyclic_at_initial_state,
            alphabet: s.alphabet,
            details: s.details.map(|d| SummaryDetails {
                infinitely_ambiguous: d.infinitely_ambiguous,
                first_input_symbols: d.first_input_symbols,
            }),
        })
    }

    pub fn check_alpha(
        self: Arc<Self>,
        other: Arc<Transducer>,
    ) -> Result<AlphabetComparison, GlueError> {
        let c = reading_all(&[self, other], |ts| match ts {
            [first, second] => first.check_alpha(second),
            [one] => one.check_alpha(one),
            _ => Err(unlocked()),
        })?;
        Ok(AlphabetComparison {
            first_used: c.first_used,
            second_used: c.second_used,
            first_declared: c.first_declared,
            second_declared: c.second_declared,
            mismatch: c.mismatch,
        })
    }

    pub fn compare(
        self: Arc<Self>,
        other: Arc<Transducer>,
        do_not_harmonize: bool,
        eliminate_flags: bool,
    ) -> Result<bool, GlueError> {
        let options = CompareOptions {
            do_not_harmonize,
            eliminate_flags,
        };
        reading_all(&[self, other], |ts| match ts {
            [first, second] => first.compare(second, &options),
            [one] => one.compare(one, &options),
            _ => Err(unlocked()),
        })
    }

    pub fn dump_alphabets(&self) -> Result<Alphabets, GlueError> {
        let a = self.read(Machine::dump_alphabets)?;
        Ok(Alphabets {
            declared: a.declared,
            seen: a.seen,
        })
    }

    pub fn extract_paths(
        &self,
        settings: ExtractSettings,
    ) -> Result<Vec<ExtractedPath>, GlueError> {
        let defaults = ExtractPathsOptions::default();
        let options = ExtractPathsOptions {
            max_strings: settings.max_strings.unwrap_or(defaults.max_strings),
            nbest: settings.nbest.unwrap_or(defaults.nbest),
            random: settings.random.unwrap_or(defaults.random),
            cycles: settings.cycles.unwrap_or(defaults.cycles),
            beam: settings.beam,
            obey_flags: settings.obey_flags,
            print_flags: settings.print_flags,
            max_in_length: settings.max_in_length.unwrap_or(0),
            max_out_length: settings.max_out_length.unwrap_or(0),
            in_prefix: settings.in_prefix,
            out_prefix: settings.out_prefix,
            in_exclude: settings.in_exclude,
            out_exclude: settings.out_exclude,
        };
        let paths = self.read(|t| t.extract_paths(&options))?;
        Ok(paths
            .into_iter()
            .map(|path| ExtractedPath {
                input: path.input,
                output: path.output,
                pairs: pairs(path.pairs),
                weight: path.weight,
            })
            .collect())
    }

    pub fn guess(
        self: Arc<Self>,
        words: Vec<String>,
        generator: Option<Arc<Transducer>>,
        model_forms: Option<String>,
        max_number_of_guesses: i64,
        max_number_of_forms: i64,
        generate_threshold: f32,
    ) -> Result<Vec<GuessReport>, GlueError> {
        let options = GuessOptions {
            max_number_of_guesses: count("maxNumberOfGuesses", max_number_of_guesses)?,
            max_number_of_forms: count("maxNumberOfForms", max_number_of_forms)?,
            generate_threshold,
        };
        let mut transducers = vec![self];
        transducers.extend(generator);
        let read: Vec<&str> = words.iter().map(String::as_str).collect();
        let reports = reading_all(&transducers, |ts| {
            let (guesser, generator) = match ts {
                [guesser] => (*guesser, None),
                [guesser, generator] => (*guesser, Some(*generator)),
                _ => return Err(unlocked()),
            };
            embed::guess(guesser, generator, model_forms.as_deref(), &read, &options)
        })?;
        Ok(words
            .into_iter()
            .zip(reports)
            .map(|(input, report)| {
                let (paradigms, guesses) = match report {
                    Guessed::Guesses(guesses) => (false, guesses),
                    Guessed::Paradigms(paradigms) => (true, paradigms),
                };
                GuessReport {
                    input,
                    paradigms,
                    guesses,
                }
            })
            .collect())
    }

    /// The tokeniser `hfst tokenize` makes of a transducer that is not a
    /// pmatch container.
    pub fn naive_tokeniser(&self) -> Result<Arc<Tokeniser>, GlueError> {
        let container = self.read(|t| embed::naive_tokeniser(t, false))?;
        Ok(Arc::new(Tokeniser::from_core(container.core())))
    }
}

// [spec:hfst:req:swift.operations]
/// What `hfst pmatch` reports for `text` with `tokeniser`'s container.
#[uniffi::export]
pub fn pmatch(
    tokeniser: Arc<Tokeniser>,
    text: String,
    settings: PmatchRunSettings,
) -> Result<Vec<PmatchPiece>, GlueError> {
    let defaults = PmatchOptions::default();
    let options = PmatchOptions {
        blankline_separated: !settings.newline,
        locate: settings.locate,
        extract_patterns: settings.extract_patterns,
        delete_patterns: settings.delete_patterns,
        mark_patterns: settings.mark_patterns,
        max_context: settings
            .max_context
            .map(|n| count("maxContext", n))
            .transpose()?,
        max_recursion: settings
            .max_recursion
            .map(|n| count("maxRecursion", n))
            .transpose()?,
        time_cutoff: settings.time_cutoff,
        weight_cutoff: settings.weight_cutoff.unwrap_or(defaults.weight_cutoff),
    };
    let pieces = embed::pmatch(&mut tokeniser.run(), &text, &options);
    Ok(pieces
        .into_iter()
        .map(|(input, found)| match found {
            PmatchResult::Marked(marked) => PmatchPiece {
                input,
                marked: Some(marked),
                located: None,
            },
            PmatchResult::Located(locations) => PmatchPiece {
                input,
                marked: None,
                located: Some(
                    locations
                        .into_iter()
                        .map(|l| PmatchLocation {
                            start: l.start,
                            length: l.length,
                            output: l.output,
                            tag: l.tag,
                            weight: l.weight,
                        })
                        .collect(),
                ),
            },
        })
        .collect())
}

const INPUT_FORMATS: [(&str, LookupInput); 3] = [
    ("text", LookupInput::Text),
    ("spaced", LookupInput::Spaced),
    ("apertium", LookupInput::Apertium),
];

// [spec:hfst:req:swift.operations]
/// What `hfst lookup` or `hfst flookup` reports for one line: its results,
/// or `None` for a line with a symbol no transducer knows.
#[derive(uniffi::Record)]
pub struct LookupReport {
    pub input: String,
    pub markup: String,
    pub results: Option<Vec<LookupResult>>,
    pub infinite: bool,
}

fn lookup_reports(reports: Vec<embed::LookupReport>) -> Vec<LookupReport> {
    reports
        .into_iter()
        .map(|report| {
            let (results, infinite) = match report.outcome {
                LookupOutcome::OutsideAlphabet => (None, false),
                LookupOutcome::Found { infinite, results } => (
                    Some(results.into_iter().map(LookupResult::from).collect()),
                    infinite,
                ),
            };
            LookupReport {
                input: report.input,
                markup: report.markup,
                results,
                infinite,
            }
        })
        .collect()
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct LookupSettings {
    pub input_format: String,
    pub epsilon_format: String,
    pub max_number: Option<i64>,
    pub cycles: i64,
    pub beam: Option<f32>,
    pub time_cutoff: f64,
    pub cascade: String,
    pub obey_flags: bool,
    pub show_flags: bool,
}

// [spec:hfst:req:swift.operations]
/// What `hfst lookup` reports for each of `inputs` with `transducers`.
#[uniffi::export]
pub fn lookup_report(
    transducers: Vec<Arc<Transducer>>,
    inputs: Vec<String>,
    settings: LookupSettings,
) -> Result<Vec<LookupReport>, GlueError> {
    let options = LookupOptions {
        input_format: choice("inputFormat", &settings.input_format, &INPUT_FORMATS)?,
        epsilon_format: settings.epsilon_format,
        max_number: settings
            .max_number
            .map(|n| count("maxNumber", n))
            .transpose()?,
        cycles: count("cycles", settings.cycles)?,
        beam: settings.beam,
        time_cutoff: settings.time_cutoff,
        cascade: choice(
            "cascade",
            &settings.cascade,
            &[
                ("union", Cascade::Union),
                ("priority-union", Cascade::PriorityUnion),
                ("composition", Cascade::Composition),
            ],
        )?,
        obey_flags: settings.obey_flags,
        show_flags: settings.show_flags,
    };
    let lines: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let reports = reading_all(&transducers, |ts| {
        embed::lookup_report(ts, &lines, &options)
    })?;
    Ok(lookup_reports(reports))
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct FlookupSettings {
    pub input_format: String,
    pub epsilon_format: String,
    pub cycles: i64,
    pub beam: Option<f32>,
    pub time_cutoff: f64,
    pub invert: bool,
    pub force_ol: bool,
    pub obey_flags: bool,
    pub show_flags: bool,
}

// [spec:hfst:req:swift.operations]
/// What `hfst flookup` reports for each of `inputs` with `transducers`.
#[uniffi::export]
pub fn flookup_report(
    transducers: Vec<Arc<Transducer>>,
    inputs: Vec<String>,
    settings: FlookupSettings,
) -> Result<Vec<LookupReport>, GlueError> {
    let options = FlookupOptions {
        input_format: choice("inputFormat", &settings.input_format, &INPUT_FORMATS)?,
        epsilon_format: settings.epsilon_format,
        cycles: count("cycles", settings.cycles)?,
        beam: settings.beam,
        time_cutoff: settings.time_cutoff,
        invert: settings.invert,
        force_ol: settings.force_ol,
        obey_flags: settings.obey_flags,
        show_flags: settings.show_flags,
    };
    let lines: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let reports = reading_all(&transducers, |ts| {
        embed::flookup_report(ts, &lines, &options)
    })?;
    Ok(lookup_reports(reports))
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct Analysis {
    pub output: String,
    pub weight: f32,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct OptimizedLookupReport {
    pub input: String,
    pub analyses: Option<Vec<Analysis>>,
}

// [spec:hfst:req:swift.operations]
/// What `hfst optimized-lookup` reports for each of `words` with the union
/// of `transducers`.
#[uniffi::export]
pub fn optimized_lookup_report(
    transducers: Vec<Arc<Transducer>>,
    words: Vec<String>,
    unique: bool,
    analyses: Option<i32>,
    time_cutoff: f64,
    beam: Option<f32>,
    fast: bool,
) -> Result<Vec<OptimizedLookupReport>, GlueError> {
    let defaults = OptimizedLookupOptions::default();
    let options = OptimizedLookupOptions {
        unique,
        max_analyses: analyses.unwrap_or(defaults.max_analyses),
        time_cutoff,
        beam: beam.unwrap_or(defaults.beam),
        fast,
    };
    let read: Vec<&str> = words.iter().map(String::as_str).collect();
    let reports = reading_all(&transducers, |ts| {
        embed::optimized_lookup_report(ts, &read, &options)
    })?;
    Ok(reports
        .into_iter()
        .map(|report| OptimizedLookupReport {
            input: report.input,
            analyses: report.analyses.map(|analyses| {
                analyses
                    .into_iter()
                    .map(|(output, weight)| Analysis { output, weight })
                    .collect()
            }),
        })
        .collect())
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct RuleFailure {
    pub rule: String,
    pub recognized_prefix: u64,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct PairTestResult {
    pub test: String,
    pub positive: bool,
    pub pairs: Vec<SymbolPair>,
    pub failed: bool,
    pub failures: Vec<RuleFailure>,
}

// [spec:hfst:req:swift.operations]
/// What `hfst pair-test` reports with the rule transducers `rules` on
/// `tests`.
#[uniffi::export]
pub fn pair_test(
    rules: Vec<Arc<Transducer>>,
    tests: String,
    negative_test: bool,
    xerox_mode: bool,
) -> Result<Vec<PairTestResult>, GlueError> {
    let options = PairTestOptions {
        negative_test,
        xerox_mode,
    };
    let found = reading_all(&rules, |ts| embed::pair_test(ts, &tests, &options))?;
    Ok(found
        .into_iter()
        .map(|test| PairTestResult {
            test: test.test,
            positive: test.positive,
            pairs: pairs(test.pairs),
            failed: test.failed,
            failures: test
                .failures
                .into_iter()
                .map(|f| RuleFailure {
                    rule: f.rule,
                    recognized_prefix: f.recognized_prefix as u64,
                })
                .collect(),
        })
        .collect())
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct GrepLine {
    pub number: u64,
    pub text: String,
}

// [spec:hfst:req:swift.operations]
#[derive(uniffi::Record)]
pub struct GrepReport {
    pub lines: Vec<GrepLine>,
    pub matched: bool,
}

// [spec:hfst:req:swift.operations]
/// What `hfst grep` reports for `pattern` on `text`.
#[uniffi::export]
#[expect(clippy::too_many_arguments, reason = "each is an option of hfst grep")]
pub fn grep(
    pattern: String,
    text: String,
    fixed_strings: bool,
    color: bool,
    word_regexp: bool,
    line_regexp: bool,
    invert_match: bool,
    max_count: u64,
) -> Result<GrepReport, GlueError> {
    let options = GrepOptions {
        fixed_strings,
        color,
        word_regexp,
        line_regexp,
        invert_match,
        max_count,
    };
    let report = embed::grep(&pattern, &text, &options)?;
    Ok(GrepReport {
        lines: report
            .lines
            .into_iter()
            .map(|line| GrepLine {
                number: line.number,
                text: line.text,
            })
            .collect(),
        matched: report.matched,
    })
}
