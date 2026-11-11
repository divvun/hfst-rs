//! The queries of the compile surface: what each query tool reports, as
//! data. Each calls the library function the tool calls; the tool prints what
//! these return.

use std::borrow::Cow;
use std::io::Cursor;

use hfst::hfst_data_types::{HfstTwoLevelPath, ImplementationType};
use hfst::hfst_flag_diacritics::FdOperation;
use hfst::hfst_lookup_format::{CascadeVariant, LookupInputFormat, reported_paths};
use hfst::hfst_symbol_defs::{StringSet, is_epsilon};
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use hfst::lookup_driver::{
    AmbiguityLimit, FlagPolicy, LookupCascade, LookupEngineOptions, LookupReporter, PairPrintStyle,
    is_optimized_lookup_type,
};
use hfst::pair_test_driver::PairTestGrammar;
use hfst::pmatch::PmatchContainer;
use hfst::tools::fst2strings::{Fst2StringsError, PathQuery, extract_paths, extract_paths_ol};
use hfst::tools::grep::{Dialect, Matcher, PatternError};
use hfst::tools::lookup::{Lookup, LookupError};
use hfst::tools::optimized_lookup::{OptimizedLookup, OptimizedLookupOutcome};
use hfst::tools::pair_test::{PairTestError, PairTester, ReportOptions, read_test_cases};
use hfst::tools::pmatch::{InputSplitter, reported_locations};
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

use super::transducer::Transducer;
use crate::error::{Code, Error};
use crate::format::Format;
use crate::lookup::LookupResult;

pub use hfst::tools::guess::{GuessOptions, GuessReport};
pub use hfst::tools::optimized_lookup::OptimizedLookupOptions;

fn strings(set: &StringSet) -> Vec<String> {
    set.iter().map(|s| s.to_string()).collect()
}

fn format_of(ty: ImplementationType) -> Option<Format> {
    match ty {
        ImplementationType::TROPICAL_OPENFST_TYPE => Some(Format::OpenFstTropical),
        ImplementationType::FOMA_TYPE => Some(Format::Foma),
        ImplementationType::HFST_OLW_TYPE | ImplementationType::THFST_TYPE => {
            Some(Format::OptimizedLookupWeighted)
        }
        ImplementationType::HFST_OL_TYPE => Some(Format::OptimizedLookupUnweighted),
        ImplementationType::SFST_TYPE
        | ImplementationType::XFSM_TYPE
        | ImplementationType::HFST2_TYPE
        | ImplementationType::UNSPECIFIED_TYPE
        | ImplementationType::ERROR_TYPE => None,
    }
}

// -----------------------------------------------------------------------------
// summarize
// -----------------------------------------------------------------------------

/// What `hfst summarize` reports on a transducer.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub name: String,
    pub format: Format,
    pub states: usize,
    pub final_states: usize,
    pub arcs: usize,
    /// Arcs with an epsilon on both sides.
    pub io_epsilons: usize,
    pub input_epsilons: usize,
    pub output_epsilons: usize,
    /// The most arcs a state has.
    pub densest_arcs: usize,
    /// The fewest arcs a state has.
    pub sparsest_arcs: usize,
    pub unique_input_arcs: usize,
    pub unique_output_arcs: usize,
    /// The input symbol with the most arcs out of one state, and how many.
    pub most_ambiguous_input: (String, u32),
    pub most_ambiguous_output: (String, u32),
    /// The symbols the arcs use.
    pub found_alphabet: Vec<String>,
    /// Each pair of symbols an arc carries, input then output, and how many
    /// arcs carry it.
    pub symbol_pairs: Vec<(String, String, u32)>,
    pub acceptor: bool,
    pub input_deterministic: bool,
    pub output_deterministic: bool,
    pub cyclic: bool,
    pub cyclic_at_initial_state: bool,
    /// The alphabet the transducer declares.
    pub alphabet: Vec<String>,
    /// What `--verbose` measures besides.
    pub details: Option<SummaryDetails>,
}

/// What `hfst summarize --verbose` measures besides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryDetails {
    pub infinitely_ambiguous: bool,
    /// The symbols a path can start with; `None` for optimized lookup, which
    /// has no graph to walk.
    pub first_input_symbols: Option<Vec<String>>,
}

/// `hfst check-alpha`'s report on two transducers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlphabetComparison {
    /// The symbols the first transducer's arcs use.
    pub first_used: Vec<String>,
    pub second_used: Vec<String>,
    /// The alphabet the first transducer declares.
    pub first_declared: Vec<String>,
    pub second_declared: Vec<String>,
    /// Whether either has a symbol, used or declared, the other lacks.
    pub mismatch: bool,
}

/// `hfst dump-alphabets`' report on a transducer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alphabets {
    pub declared: Vec<String>,
    /// The symbols its arcs use.
    pub seen: Vec<String>,
}

/// `hfst compare`'s options.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompareOptions {
    pub do_not_harmonize: bool,
    pub eliminate_flags: bool,
}

/// `hfst fst2strings`' options that select paths. Each count is the tool's
/// own: 0 or less sets no limit.
#[derive(Clone, Debug)]
pub struct ExtractPathsOptions {
    pub max_strings: i32,
    pub nbest: i32,
    pub random: i32,
    /// Follow a cycle at most this many times; negative for no limit.
    pub cycles: i32,
    /// Only paths within this much of the lightest.
    pub beam: Option<f32>,
    /// Drop the paths flag diacritics forbid, as `--xfst obey-flags`.
    pub obey_flags: bool,
    /// Keep flag diacritics in the paths, as `--xfst print-flags`.
    pub print_flags: bool,
    /// Only paths whose input is at most this many bytes; 0 for no limit.
    pub max_in_length: u32,
    pub max_out_length: u32,
    pub in_prefix: String,
    pub out_prefix: String,
    pub in_exclude: String,
    pub out_exclude: String,
}

impl Default for ExtractPathsOptions {
    fn default() -> Self {
        let query = PathQuery::default();
        ExtractPathsOptions {
            max_strings: query.max_strings,
            nbest: query.nbest,
            random: query.random,
            cycles: query.cycles,
            beam: None,
            obey_flags: query.obey_flags,
            print_flags: query.show_flags,
            max_in_length: 0,
            max_out_length: 0,
            in_prefix: String::new(),
            out_prefix: String::new(),
            in_exclude: String::new(),
            out_exclude: String::new(),
        }
    }
}

/// One path `hfst fst2strings` prints.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtractedPath {
    /// The input side as the tool prints it: no epsilons, and flag
    /// diacritics only with `print_flags`.
    pub input: String,
    pub output: String,
    /// The path's symbol pairs, input then output, an epsilon as `""`; a
    /// pair of flag diacritics only with `print_flags`.
    pub pairs: Vec<(String, String)>,
    pub weight: f32,
}

fn extracted(path: &HfstTwoLevelPath, print_flags: bool) -> ExtractedPath {
    let shown = |s: &str| print_flags || !FdOperation::is_diacritic(s);
    let printed = |s: &str| {
        if is_epsilon(s) {
            String::new()
        } else {
            s.to_string()
        }
    };
    let side = |output: bool| -> String {
        path.second
            .iter()
            .map(|pair| if output { &pair.1 } else { &pair.0 })
            .filter(|s| shown(s))
            .map(|s| printed(s))
            .collect()
    };
    ExtractedPath {
        input: side(false),
        output: side(true),
        pairs: path
            .second
            .iter()
            .filter(|pair| shown(&pair.0) || shown(&pair.1))
            .map(|pair| (printed(&pair.0), printed(&pair.1)))
            .collect(),
        weight: path.first,
    }
}

impl Transducer {
    // [spec:hfst:req:embed.operations]
    /// `hfst summarize`, in detail when `verbose`.
    pub fn summarize(&self, verbose: bool) -> Result<Summary, Error> {
        let summary = hfst::tools::summarize::Summary::of(&self.inner, verbose)?;
        let stats = summary.stats;
        Ok(Summary {
            name: summary.name,
            format: format_of(summary.implementation).unwrap_or_else(|| self.format()),
            states: stats.states,
            final_states: stats.final_states,
            arcs: stats.arcs,
            io_epsilons: stats.io_epsilons,
            input_epsilons: stats.input_epsilons,
            output_epsilons: stats.output_epsilons,
            densest_arcs: stats.densest_arcs,
            sparsest_arcs: stats.sparsest_arcs,
            unique_input_arcs: stats.uniq_input_arcs,
            unique_output_arcs: stats.uniq_output_arcs,
            most_ambiguous_input: (
                stats.most_ambiguous_input.0.to_string(),
                stats.most_ambiguous_input.1,
            ),
            most_ambiguous_output: (
                stats.most_ambiguous_output.0.to_string(),
                stats.most_ambiguous_output.1,
            ),
            found_alphabet: stats.found_alphabet.iter().map(|s| s.to_string()).collect(),
            symbol_pairs: stats
                .symbol_pairs
                .iter()
                .map(|((i, o), n)| (i.to_string(), o.to_string(), *n))
                .collect(),
            acceptor: stats.acceptor,
            input_deterministic: stats.input_deterministic,
            output_deterministic: stats.output_deterministic,
            cyclic: stats.cyclic,
            cyclic_at_initial_state: stats.cyclic_at_initial_state,
            alphabet: strings(&summary.alphabet),
            details: summary.details.map(|d| SummaryDetails {
                infinitely_ambiguous: d.infinitely_ambiguous,
                first_input_symbols: d.first_input_symbols.as_ref().map(strings),
            }),
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn check_alpha(&self, other: &Transducer) -> Result<AlphabetComparison, Error> {
        let c = hfst::tools::check_alpha::AlphabetComparison::of(&self.inner, &other.inner)?;
        Ok(AlphabetComparison {
            mismatch: c.mismatch(),
            first_used: strings(&c.first_used),
            second_used: strings(&c.second_used),
            first_declared: strings(&c.first_declared),
            second_declared: strings(&c.second_declared),
        })
    }

    // [spec:hfst:req:embed.operations]
    /// `hfst compare`: whether this transducer and `other` are equivalent.
    /// Like the tool, it compares two transducers of one algebra type only.
    pub fn compare(&self, other: &Transducer, options: &CompareOptions) -> Result<bool, Error> {
        let library = hfst::tools::compare::CompareOptions {
            harmonize: !options.do_not_harmonize,
            eliminate_flags: options.eliminate_flags,
        };
        match (&self.inner, &other.inner) {
            (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                compare_typed(f, s, &library)
            }
            (AnyTransducer::Foma(f), AnyTransducer::Foma(s)) => compare_typed(f, s, &library),
            (first, second) => Err(Error::new(
                Code::TypeMismatch,
                format!(
                    "the formats {} and {} are not compatible for comparison",
                    hfst::tools::format::format_description(first.get_type()),
                    hfst::tools::format::format_description(second.get_type())
                ),
            )),
        }
    }

    // [spec:hfst:req:embed.operations]
    pub fn dump_alphabets(&self) -> Result<Alphabets, Error> {
        let a = hfst::tools::dump_alphabets::Alphabets::of(&self.inner)?;
        Ok(Alphabets {
            declared: strings(&a.declared),
            seen: strings(&a.seen),
        })
    }

    // [spec:hfst:req:embed.operations]
    /// `hfst fst2strings`: the paths `options` select, in the order the tool
    /// prints them.
    pub fn extract_paths(
        &self,
        options: &ExtractPathsOptions,
    ) -> Result<Vec<ExtractedPath>, Error> {
        let query = PathQuery {
            max_strings: options.max_strings,
            nbest: options.nbest,
            random: options.random,
            cycles: options.cycles,
            beam: options.beam.unwrap_or(-1.0),
            obey_flags: options.obey_flags,
            show_flags: options.print_flags,
            max_in_length: options.max_in_length,
            max_out_length: options.max_out_length,
            in_prefix: options.in_prefix.clone(),
            out_prefix: options.out_prefix.clone(),
            in_exclude: options.in_exclude.clone(),
            out_exclude: options.out_exclude.clone(),
        };
        let mut paths = Vec::new();
        let mut each = |path: &HfstTwoLevelPath| paths.push(extracted(path, options.print_flags));
        let mut progress = |_| {};
        let found = match &self.inner {
            AnyTransducer::Tropical(t) => extract_paths(t, &query, &mut each, &mut progress),
            AnyTransducer::Foma(t) => extract_paths(t, &query, &mut each, &mut progress),
            AnyTransducer::OlW(t) => extract_paths_ol(t, &query, &mut each, &mut progress),
            AnyTransducer::OlU(t) => extract_paths_ol(t, &query, &mut each, &mut progress),
            AnyTransducer::Thfst(t) => extract_paths_ol(t, &query, &mut each, &mut progress),
        };
        match found {
            Ok(_) => Ok(paths),
            Err(Fst2StringsError::Operation(e)) => Err(e.into()),
            Err(Fst2StringsError::Cyclic) => Err(Error::new(
                Code::Cyclic,
                Fst2StringsError::Cyclic.to_string(),
            )),
            Err(e @ Fst2StringsError::NotForOptimizedLookup(_)) => {
                Err(Error::new(Code::TypeMismatch, e.to_string()))
            }
            Err(e @ Fst2StringsError::NotOneBestPath) => {
                Err(Error::new(Code::Failed, e.to_string()))
            }
        }
    }

    // [spec:hfst:req:python.transducer]
    /// The results of looking `input` up, as the read-only transducer's
    /// lookup gives them: for optimized lookup, the library's flag-aware
    /// lookup, stopping at `limit` paths or after `time_limit` seconds; for
    /// the other types, what `hfst lookup` finds, at most `limit` of them.
    pub fn lookup(
        &self,
        input: &str,
        limit: Option<usize>,
        time_limit: Option<f64>,
    ) -> Result<Vec<LookupResult>, Error> {
        let cap = limit.map_or(-1, |limit| isize::try_from(limit).unwrap_or(isize::MAX));
        let seconds = time_limit.unwrap_or(0.0);
        let paths = match &self.inner {
            AnyTransducer::OlW(t) => t.lookup_fd_string(input, cap, seconds)?,
            AnyTransducer::OlU(t) => t.lookup_fd_string(input, cap, seconds)?,
            AnyTransducer::Thfst(_) | AnyTransducer::Tropical(_) | AnyTransducer::Foma(_) => {
                let options = LookupOptions {
                    time_cutoff: seconds,
                    ..LookupOptions::default()
                };
                let report = lookup_report(&[self], &[input], &options)?;
                let mut results = match report.into_iter().next().map(|r| r.outcome) {
                    Some(LookupOutcome::Found { results, .. }) => results,
                    Some(LookupOutcome::OutsideAlphabet) | None => Vec::new(),
                };
                if let Some(limit) = limit {
                    results.truncate(limit);
                }
                return Ok(results);
            }
        };
        Ok(crate::lookup::lookup_results(paths))
    }
}

fn compare_typed<B: hfst::backend::AlgebraBackend>(
    first: &HfstTransducer<B>,
    second: &HfstTransducer<B>,
    options: &hfst::tools::compare::CompareOptions,
) -> Result<bool, Error> {
    if !options.eliminate_flags {
        return Ok(first.compare(second, options.harmonize)?);
    }
    // Eliminating flags changes both, so both are copied.
    let mut first = HfstTransducer::new_copy(first)?;
    let mut second = hfst::tools::operations::Operand::Borrowed(second);
    Ok(hfst::tools::compare::compare(
        &mut first,
        &mut second,
        options,
        &mut || {},
    )?)
}

// -----------------------------------------------------------------------------
// lookup, flookup and optimized-lookup
// -----------------------------------------------------------------------------

/// How a lookup tool reads its input lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LookupInput {
    /// As text, split into the transducers' symbols: `--input-format text`.
    #[default]
    Text,
    /// Symbols separated by spaces: `spaced`.
    Spaced,
    /// Apertium stream format, escapes and markup and all: `apertium`.
    Apertium,
}

impl LookupInput {
    fn library(self) -> LookupInputFormat {
        match self {
            LookupInput::Text => LookupInputFormat::Utf8TokenInput,
            LookupInput::Spaced => LookupInputFormat::SpaceSeparatedTokenInput,
            LookupInput::Apertium => LookupInputFormat::ApertiumInput,
        }
    }
}

/// How the results of several transducers are combined.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cascade {
    /// All of them: `--cascade union`.
    #[default]
    Union,
    /// The first transducer's that has any.
    PriorityUnion,
    /// Each transducer's results looked up in the next.
    Composition,
}

impl Cascade {
    fn library(self) -> CascadeVariant {
        match self {
            Cascade::Union => CascadeVariant::Union,
            Cascade::PriorityUnion => CascadeVariant::PriorityUnion,
            Cascade::Composition => CascadeVariant::Composition,
        }
    }
}

/// `hfst lookup`'s options that change what it finds.
#[derive(Clone, Debug)]
pub struct LookupOptions {
    pub input_format: LookupInput,
    /// What the input spells the epsilon as: `--epsilon-format`.
    pub epsilon_format: String,
    /// At most this many results a word, optimized lookup only:
    /// `--max-number`.
    pub max_number: Option<usize>,
    /// How many times an input epsilon cycle is followed: `--cycles`.
    pub cycles: usize,
    /// Only results within this much of the lightest: `--beam`.
    pub beam: Option<f32>,
    /// Seconds of search a word, 0 for no limit: `--time-cutoff`.
    pub time_cutoff: f64,
    pub cascade: Cascade,
    /// Obey flag diacritics; `--xfst obey-flags` turns it off.
    pub obey_flags: bool,
    /// Keep flag diacritics in the results: `--xfst show-flags`.
    pub show_flags: bool,
}

impl Default for LookupOptions {
    fn default() -> Self {
        LookupOptions {
            input_format: LookupInput::Text,
            epsilon_format: String::new(),
            max_number: None,
            cycles: 5,
            beam: None,
            time_cutoff: 0.0,
            cascade: Cascade::Union,
            obey_flags: true,
            show_flags: false,
        }
    }
}

/// `hfst flookup`'s options that change what it finds.
#[derive(Clone, Debug)]
pub struct FlookupOptions {
    pub input_format: LookupInput,
    pub epsilon_format: String,
    /// How many times an input epsilon cycle is followed: `--cycles`.
    pub cycles: usize,
    pub beam: Option<f32>,
    /// Seconds of search a word, optimized lookup only: `--time-cutoff`.
    pub time_cutoff: f64,
    /// Look up as the transducers are, rather than inverted: `--invert`.
    pub invert: bool,
    /// Look up in optimized lookup without `invert`, as `--force-ol`, which
    /// looks up as the transducer is.
    pub force_ol: bool,
    pub obey_flags: bool,
    pub show_flags: bool,
}

impl Default for FlookupOptions {
    fn default() -> Self {
        FlookupOptions {
            input_format: LookupInput::Text,
            epsilon_format: String::new(),
            cycles: 5,
            beam: None,
            time_cutoff: 0.0,
            invert: false,
            force_ol: false,
            obey_flags: true,
            show_flags: false,
        }
    }
}

/// What a lookup tool finds for a line.
#[derive(Clone, Debug, PartialEq)]
pub enum LookupOutcome {
    /// The line has a symbol no transducer knows, so it was not looked up.
    OutsideAlphabet,
    /// The results the tool prints, in weight order: none for a word with
    /// no analysis. `infinite` when the lookup was infinitely ambiguous and
    /// so cut short.
    Found {
        infinite: bool,
        results: Vec<LookupResult>,
    },
}

/// What a lookup tool reports for one line.
#[derive(Clone, Debug, PartialEq)]
pub struct LookupReport {
    /// The line as it was looked up; an Apertium line loses its escapes and
    /// markup.
    pub input: String,
    /// An Apertium line's bracketed markup, or `""`.
    pub markup: String,
    pub outcome: LookupOutcome,
}

/// The warnings a lookup gives, passed on to `tracing`.
struct Logged;

impl LookupReporter for Logged {
    fn verbose(&self, msg: &str) {
        tracing::debug!(target: "hfst_embed::compile", "{}", msg.trim_end());
    }

    fn warning(&self, msg: &str) {
        tracing::warn!(target: "hfst_embed::compile", "{}", msg.trim_end());
    }
}

fn report_of(
    lookup: &Lookup<'_>,
    line: &str,
    beam: f32,
    show_flags: bool,
) -> Result<LookupReport, Error> {
    let found = lookup
        .look_up(line, &Logged, &mut std::io::sink(), &mut std::io::sink())
        .map_err(|e| match e {
            LookupError::Input(e) | LookupError::Lookup(e) => Error::from(e),
        })?;
    let outcome = if found.outside_alphabet {
        LookupOutcome::OutsideAlphabet
    } else {
        LookupOutcome::Found {
            infinite: found.infinite,
            results: reported_paths(&found.results, beam)
                .map(|path| {
                    let symbols = hfst::hfst_lookup_format::reported_symbols(path, show_flags);
                    LookupResult {
                        output: symbols.concat(),
                        symbols,
                        weight: path.first,
                    }
                })
                .collect(),
        }
    };
    Ok(LookupReport {
        input: found.line,
        markup: found.markup,
        outcome,
    })
}

// [spec:hfst:req:embed.operations]
/// `hfst lookup` on the stream `transducers`: what it reports for each of
/// `lines`.
pub fn lookup_report(
    transducers: &[&Transducer],
    lines: &[&str],
    options: &LookupOptions,
) -> Result<Vec<LookupReport>, Error> {
    let mut cascade = LookupCascade::new();
    for t in transducers {
        cascade.begin_transducer(&t.inner, "", &Logged);
        cascade.push_borrowed(&t.inner, &Logged)?;
    }
    let engine = LookupEngineOptions {
        obey_flags: options.obey_flags,
        show_flags: options.show_flags,
        print_pairs: false,
        print_space: false,
        quote_special: false,
        epsilon_format: options.epsilon_format.clone(),
        beam: options.beam.unwrap_or(-1.0),
        time_cutoff: options.time_cutoff,
        infinite_cutoff: options.cycles,
        cascade: options.cascade.library(),
        flags: FlagPolicy::InLookup,
        ambiguity: AmbiguityLimit::MaxResults {
            max_number: options
                .max_number
                .map_or(-1, |n| isize::try_from(n).unwrap_or(isize::MAX)),
            default_max: 5,
        },
        pair_style: PairPrintStyle::Lookup,
    };
    if !options.obey_flags
        && transducers
            .first()
            .is_some_and(|t| is_optimized_lookup_type(t.inner.get_type()))
    {
        return Err(Error::new(
            Code::TypeMismatch,
            "not obeying flags not supported on optimized lookup transducers",
        ));
    }
    let lookup = Lookup::new(cascade, engine, options.input_format.library())?;
    lines
        .iter()
        .map(|line| {
            report_of(
                &lookup,
                line,
                options.beam.unwrap_or(-1.0),
                options.show_flags,
            )
        })
        .collect()
}

// [spec:hfst:req:embed.operations]
/// `hfst flookup` on the stream `transducers`: what it reports for each of
/// `lines`. Like the tool, it looks up in each transducer inverted unless
/// `invert`, and refuses optimized lookup then unless `force_ol`.
pub fn flookup_report(
    transducers: &[&Transducer],
    lines: &[&str],
    options: &FlookupOptions,
) -> Result<Vec<LookupReport>, Error> {
    let mut held: Vec<AnyTransducer> = Vec::new();
    for t in transducers {
        let ty = t.inner.get_type();
        if is_optimized_lookup_type(ty) && !options.invert && !options.force_ol {
            return Err(Error::new(
                Code::TypeMismatch,
                "lookup not supported for optimized lookup transducers: convert to openfst format, invert, and convert back to optimized lookup format or set force_ol",
            ));
        }
        let mut copy = t.inner.copy()?;
        if !options.invert {
            copy = match copy {
                AnyTransducer::Tropical(mut t) => {
                    t.invert()?;
                    AnyTransducer::Tropical(t)
                }
                AnyTransducer::Foma(mut t) => {
                    t.invert()?;
                    AnyTransducer::Foma(t)
                }
                ol @ (AnyTransducer::OlW(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_)) => {
                    let mut tropical =
                        ol.converted(ImplementationType::TROPICAL_OPENFST_TYPE, "")?;
                    if let AnyTransducer::Tropical(t) = &mut tropical {
                        t.invert()?;
                    }
                    tropical.converted(ty, "")?
                }
            };
        }
        held.push(copy);
    }
    let mut cascade = LookupCascade::new();
    for t in &held {
        cascade.begin_transducer(t, "", &Logged);
        cascade.push_borrowed(t, &Logged)?;
    }
    let engine = LookupEngineOptions {
        obey_flags: options.obey_flags,
        show_flags: options.show_flags,
        print_pairs: false,
        print_space: false,
        quote_special: false,
        epsilon_format: options.epsilon_format.clone(),
        beam: options.beam.unwrap_or(-1.0),
        time_cutoff: options.time_cutoff,
        infinite_cutoff: options.cycles,
        cascade: CascadeVariant::Union,
        flags: FlagPolicy::PostFilter,
        ambiguity: AmbiguityLimit::Cycles,
        pair_style: PairPrintStyle::Flookup,
    };
    let lookup = Lookup::new(cascade, engine, options.input_format.library())?;
    lines
        .iter()
        .map(|line| {
            report_of(
                &lookup,
                line,
                options.beam.unwrap_or(-1.0),
                options.show_flags,
            )
        })
        .collect()
}

/// What `hfst optimized-lookup` reports for a word.
#[derive(Clone, Debug, PartialEq)]
pub struct OptimizedLookupReport {
    pub input: String,
    /// The analyses it prints, each its output and weight, in its order;
    /// `None` for a word with a symbol no transducer knows.
    pub analyses: Option<Vec<(String, f32)>>,
}

// [spec:hfst:req:embed.operations]
/// `hfst optimized-lookup` on the stream `transducers`, whose union it looks
/// up in: what it reports for each of `words`. A transducer of another type
/// is looked up in as weighted optimized lookup.
pub fn optimized_lookup_report(
    transducers: &[&Transducer],
    words: &[&str],
    options: &OptimizedLookupOptions,
) -> Result<Vec<OptimizedLookupReport>, Error> {
    let mut lookup = OptimizedLookup::default();
    for t in transducers {
        lookup.push_borrowed(&t.inner)?;
    }
    let mut failure = None;
    let reports = words
        .iter()
        .map(|word| {
            let outcome = lookup.look_up(word, options, &mut |e| {
                failure.get_or_insert_with(|| Error::from(e.clone()));
            });
            OptimizedLookupReport {
                input: word.to_string(),
                analyses: match outcome {
                    OptimizedLookupOutcome::Untokenizable => None,
                    OptimizedLookupOutcome::Analyses(analyses) => Some(analyses),
                },
            }
        })
        .collect();
    match failure {
        Some(error) => Err(error),
        None => Ok(reports),
    }
}

// -----------------------------------------------------------------------------
// guess, pair-test, grep, pmatch and tokenize
// -----------------------------------------------------------------------------

/// `t` as the weighted optimized lookup a guesser runs on: borrowed when it
/// is one, converted when it is not.
fn weighted(
    t: &Transducer,
) -> Result<Cow<'_, HfstTransducer<hfst::transducer::Transducer>>, Error> {
    Ok(match &t.inner {
        AnyTransducer::OlW(t) => Cow::Borrowed(t),
        other @ (AnyTransducer::Tropical(_)
        | AnyTransducer::OlU(_)
        | AnyTransducer::Foma(_)
        | AnyTransducer::Thfst(_)) => Cow::Owned(other.copy()?.into_typed()?),
    })
}

// [spec:hfst:req:embed.operations]
/// `hfst guess` with `guesser`, which `hfst guessify` built: what it reports
/// for each of `words`. With `model_forms`, the text of a model form file,
/// it generates model forms with `generator`, or with one compiled from the
/// guesser.
pub fn guess(
    guesser: &Transducer,
    generator: Option<&Transducer>,
    model_forms: Option<&str>,
    words: &[&str],
    options: &GuessOptions,
) -> Result<Vec<GuessReport>, Error> {
    let generator = match generator {
        Some(g) => Some(weighted(g)?),
        None => None,
    };
    let guesser =
        hfst::tools::guess::Guesser::new(weighted(guesser)?, generator, model_forms, &mut |_| {})
            .map_err(|e| match e {
            hfst::tools::guess::GuessError::NotAGuesser => {
                Error::new(Code::InvalidArgument, "the transducer is not a guesser")
            }
            hfst::tools::guess::GuessError::ModelForms(e)
            | hfst::tools::guess::GuessError::Operation(e) => Error::from(e),
        })?;
    words
        .iter()
        .map(|word| Ok(guesser.guess(word, options)?))
        .collect()
}

/// `hfst pair-test`'s options.
#[derive(Clone, Copy, Debug, Default)]
pub struct PairTestOptions {
    /// Each pair string is a negative test: it fails if accepted.
    pub negative_test: bool,
    /// The tests are a twolc source's test cases, `!!€` and `!!$` lines.
    pub xerox_mode: bool,
}

/// Where a rule stopped accepting a test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleFailure {
    pub rule: String,
    /// How many of the test's pairs the rule accepted before it stopped.
    pub recognized_prefix: usize,
}

/// `hfst pair-test`'s report on one test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairTestResult {
    /// The test as the tool shows it: the pair string, or `INPUT : OUTPUT`.
    pub test: String,
    pub positive: bool,
    /// The test's pairs, input then output, word boundaries included.
    pub pairs: Vec<(String, String)>,
    /// A positive test some rule rejected, or a negative test none did.
    pub failed: bool,
    /// For a positive test, each rule that rejected it.
    pub failures: Vec<RuleFailure>,
}

fn pair_test_error(e: PairTestError) -> Error {
    match e {
        PairTestError::Operation(e) => Error::from(e),
        other @ (PairTestError::UnquotedColons(_)
        | PairTestError::UnescapedCaseColons { .. }
        | PairTestError::OddCases { .. }) => Error::new(Code::InvalidArgument, other.to_string()),
    }
}

fn pair_test_result(
    outcome: hfst::tools::pair_test::PairTestOutcome,
    test: String,
    positive: bool,
) -> PairTestResult {
    PairTestResult {
        test,
        positive,
        pairs: outcome
            .pairs
            .iter()
            .map(|(i, o)| (i.to_string(), o.to_string()))
            .collect(),
        failed: outcome.failed,
        failures: outcome
            .failures
            .into_iter()
            .map(|f| RuleFailure {
                rule: f.rule,
                recognized_prefix: f.recognized_prefix,
            })
            .collect(),
    }
}

// [spec:hfst:req:embed.operations]
/// `hfst pair-test` with the twolc rule transducers `rules` on `tests`: each
/// line a pair string, or with `xerox_mode` a twolc source holding test
/// cases. Blank and `!` comment lines are not tests.
pub fn pair_test(
    rules: &[&Transducer],
    tests: &str,
    options: &PairTestOptions,
) -> Result<Vec<PairTestResult>, Error> {
    let mut grammar = PairTestGrammar::default();
    for rule in rules {
        grammar.push_rule(rule.inner.to_basic()?, rule.inner.get_name());
    }
    if !grammar.is_empty() {
        grammar.define_known_symbols();
    }
    let report = ReportOptions::default();
    let mut results = Vec::new();
    if options.xerox_mode {
        let cases =
            read_test_cases(&mut Cursor::new(tests), &mut |_| {}).map_err(pair_test_error)?;
        let tester = PairTester::for_test_cases(&grammar, report)?;
        let all = cases
            .positive
            .iter()
            .map(|case| (case, true))
            .chain(cases.negative.iter().map(|case| (case, false)));
        for (case, positive) in all {
            let outcome = tester.check_case(case, positive).map_err(pair_test_error)?;
            results.push(pair_test_result(
                outcome,
                format!("{} : {}", case.0, case.1),
                positive,
            ));
        }
    } else {
        let tester = PairTester::for_pair_strings(&grammar, report)?;
        let positive = !options.negative_test;
        for line in tests.split_inclusive('\n') {
            let line = line.split('\n').next().unwrap_or_default();
            if let Some(outcome) = tester
                .check_pair_string(line, positive, &mut |_| {})
                .map_err(pair_test_error)?
            {
                results.push(pair_test_result(outcome, line.to_string(), positive));
            }
        }
    }
    Ok(results)
}

/// `hfst grep`'s options that change what it matches.
#[derive(Clone, Copy, Debug, Default)]
pub struct GrepOptions {
    /// The pattern is a string of characters rather than a Xerox
    /// expression, the tool's default: `--fixed-strings`.
    pub fixed_strings: bool,
    /// Mark matches with colour codes rather than `{{{ }}}`: `--color`.
    pub color: bool,
    /// Match only between spaces: `--word-regexp`.
    pub word_regexp: bool,
    /// Match whole lines: `--line-regexp`.
    pub line_regexp: bool,
    /// Report the lines that do not match: `--invert-match`.
    pub invert_match: bool,
    /// Stop after this many matching lines; 0 for no limit: `--max-count`.
    pub max_count: u64,
}

/// A line `hfst grep` prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrepLine {
    /// Its number in the text, from 1.
    pub number: u64,
    /// The line, its matches marked.
    pub text: String,
}

/// What `hfst grep` reports on a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrepReport {
    pub lines: Vec<GrepLine>,
    /// Whether the text counts as matching: some line matched, or with
    /// `invert_match`, none did.
    pub matched: bool,
}

// [spec:hfst:req:embed.operations]
/// `hfst grep` with `pattern` on `text`.
pub fn grep(pattern: &str, text: &str, options: &GrepOptions) -> Result<GrepReport, Error> {
    let library = hfst::tools::grep::GrepOptions {
        dialect: if options.fixed_strings {
            Dialect::FixedStrings
        } else {
            Dialect::Xerox
        },
        colour: options.color,
        match_word: options.word_regexp,
        match_full_line: options.line_regexp,
        invert: options.invert_match,
        max_count: options.max_count,
        print_filenames: false,
        filename_null: false,
        print_line_numbers: false,
        only_filenames: false,
        flush_lines: false,
    };
    let mut xre = XreCompiler::<StdVectorFst>::new();
    let mut matcher =
        Matcher::compile(pattern, library.dialect, &mut xre, &mut |_| {}).map_err(|e| match e {
            PatternError::Operation(e) => Error::from(e),
            other @ (PatternError::Empty | PatternError::NotCompiled(_)) => {
                Error::new(Code::InvalidArgument, other.to_string())
            }
        })?;
    matcher.extend(&library, &mut |_| {})?;
    let mut lines = Vec::new();
    let matched = matcher.select_lines(
        &mut Cursor::new(text),
        &library,
        &mut |number, printed| {
            if let Some(text) = printed {
                lines.push(GrepLine {
                    number,
                    text: text.to_string(),
                });
            }
        },
        &mut |_| {},
    )?;
    Ok(GrepReport { lines, matched })
}

/// `hfst pmatch`'s options that change what it reports.
#[derive(Clone, Copy, Debug)]
pub struct PmatchOptions {
    /// Match each run of lines up to an empty one, rather than each line:
    /// `--newline` off.
    pub blankline_separated: bool,
    /// Report the matches rather than the marked-up text: `--locate`.
    pub locate: bool,
    /// Only the marked matches, not the text around them:
    /// `--extract-tags`.
    pub extract_patterns: Option<bool>,
    pub delete_patterns: Option<bool>,
    pub mark_patterns: Option<bool>,
    pub max_context: Option<usize>,
    pub max_recursion: Option<usize>,
    /// Seconds a piece of input may take, 0 for no limit.
    pub time_cutoff: f64,
    pub weight_cutoff: f32,
}

impl Default for PmatchOptions {
    fn default() -> Self {
        PmatchOptions {
            blankline_separated: true,
            locate: false,
            extract_patterns: None,
            delete_patterns: None,
            mark_patterns: None,
            max_context: None,
            max_recursion: None,
            time_cutoff: 0.0,
            weight_cutoff: hfst::transducer::INFINITE_WEIGHT,
        }
    }
}

/// A match `hfst pmatch --locate` reports.
#[derive(Clone, Debug, PartialEq)]
pub struct PmatchLocation {
    /// Where it starts in the piece, and how long it is, as the library
    /// counts them.
    pub start: u32,
    pub length: u32,
    pub output: String,
    pub tag: String,
    pub weight: f32,
}

/// What `hfst pmatch` reports for a piece of input.
#[derive(Clone, Debug, PartialEq)]
pub enum PmatchResult {
    /// The piece with its matches marked up.
    Marked(String),
    /// Its matches.
    Located(Vec<PmatchLocation>),
}

// [spec:hfst:req:embed.operations]
/// `hfst pmatch` with the pmatch container `container`, read from bytes as
/// [`crate::tokenise::read_tokeniser`] reads it, on `text`: each piece of it
/// and what the tool reports for the piece.
pub fn pmatch(
    container: &mut PmatchContainer,
    text: &str,
    options: &PmatchOptions,
) -> Vec<(String, PmatchResult)> {
    if let Some(on) = options.extract_patterns {
        container.set_extract_patterns(on);
    }
    container.set_locate_mode(options.locate);
    if let Some(on) = options.delete_patterns {
        container.set_delete_patterns(on);
    }
    if let Some(on) = options.mark_patterns {
        container.set_mark_patterns(on);
    }
    if let Some(max) = options.max_context {
        container.set_max_context(max);
    }
    if let Some(max) = options.max_recursion {
        container.set_max_recursion(max);
    }
    InputSplitter::split(text, options.blankline_separated)
        .into_iter()
        .map(|piece| {
            let result = if options.locate {
                let locations =
                    container.locate(&piece, options.time_cutoff, options.weight_cutoff);
                PmatchResult::Located(
                    reported_locations(&locations)
                        .map(|l| PmatchLocation {
                            start: l.start,
                            length: l.length,
                            output: l.output.clone(),
                            tag: l.tag.clone(),
                            weight: l.weight,
                        })
                        .collect(),
                )
            } else {
                PmatchResult::Marked(container.do_match(
                    &piece,
                    options.time_cutoff,
                    options.weight_cutoff,
                ))
            };
            (piece, result)
        })
        .collect()
}

// [spec:hfst:req:embed.operations]
/// The naive tokeniser `hfst tokenize` makes of a transducer that is not a
/// pmatch container, as [`crate::tokenise::tokens`] and
/// [`crate::tokenise::format`] take it. Multicharacter symbols are matched
/// longest first when the transducer has any, or when `tokenize_multichar`.
pub fn naive_tokeniser(
    dictionary: &Transducer,
    tokenize_multichar: bool,
) -> Result<PmatchContainer, Error> {
    let mut dictionary: HfstTransducer<StdVectorFst> = match &dictionary.inner {
        AnyTransducer::Tropical(t) => HfstTransducer::new_copy(t)?,
        other @ (AnyTransducer::OlW(_)
        | AnyTransducer::OlU(_)
        | AnyTransducer::Foma(_)
        | AnyTransducer::Thfst(_)) => other.copy()?.into_typed()?,
    };
    let mut container = hfst::pmatch_tokenize::make_naive_tokenizer(&mut dictionary)?;
    let single_codepoint = !tokenize_multichar && !container.has_multichar_input_symbols();
    container.set_single_codepoint_tokenization(single_codepoint);
    Ok(container)
}
