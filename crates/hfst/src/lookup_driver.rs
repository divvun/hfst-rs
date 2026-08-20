//! The lookup engine behind `hfst-lookup` and `hfst-flookup`, lifted out of
//! the two tools that carried near-verbatim copies of it: the cascade of
//! transducers a lookup stream expands into, the fast optimized-lookup and the
//! slow basic-transducer lookup paths, the infinite-ambiguity bounding, the
//! flag-diacritic handling and the `--xfst=print-pairs` renderers.
//!
//! Its companion [`crate::hfst_lookup_format`] owns the *presentation* half of
//! the same engine (the xerox/cg/apertium templates, the `%`-template
//! renderer, the input-line parser and the cascade combinator). Together they
//! leave the tools with option parsing, writers, the interactive prompt and
//! the stdin loop.
//!
//! The two tools are one engine under two dialects, and the dialects differ in
//! exactly three places, each named by a type here: [`FlagPolicy`] (are flags
//! obeyed inside the lookup, or validated and stripped afterwards),
//! [`AmbiguityLimit`] (is an infinitely ambiguous lookup bounded by a result
//! count or by an epsilon-cycle count) and [`PairPrintStyle`] (how a
//! `--xfst=print-pairs` line is laid out and where its input form is echoed).
//!
//! Nothing here touches process stdin/stdout or exits: every printer takes a
//! `&mut dyn Write`, and the tool's message printers arrive as a
//! [`LookupReporter`].

use std::io::Write;

use crate::error::Result;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPaths, ImplementationType, StringVector,
};
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_lookup_flag_diacritics::FlagDiacriticTable;
use crate::hfst_lookup_format::{
    CascadeStep, CascadeVariant, apply_cascade, get_print_format, is_possible_to_get_result,
};
use crate::hfst_symbol_defs::{StringSet, internal_identity, internal_unknown};
use crate::hfst_transducer::{AnyTransducer, HfstTransducer};
use crate::transducer::{Transducer, UnweightedTables, WeightedTables};

// ---------------------------------------------------------------------------
// the host's message streams
// ---------------------------------------------------------------------------

/// The tool-side message printers the engine needs. Every method takes `&self`
/// so several engine callbacks can hold the reporter at once.
pub trait LookupReporter {
    /// A `--verbose` progress line.
    fn verbose(&self, msg: &str);
    /// A warning. Implementations drop it when the tool is silent, which is
    /// the `if (!silent)` guard both tools wrap their warnings in.
    fn warning(&self, msg: &str);
    /// An unrecoverable error. Both tools print it and exit, so it never
    /// returns.
    fn fatal(&self, msg: &str) -> !;
}

// ---------------------------------------------------------------------------
// dialect knobs
// ---------------------------------------------------------------------------

/// How flag diacritics reach a basic-transducer lookup's results.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlagPolicy {
    /// hfst-lookup: the lookup itself obeys (or, under
    /// `--xfst=obey-flags`, ignores) flags, and its result paths are kept as
    /// found — flags included, to be filtered at print time by `show-flags`.
    InLookup,
    /// hfst-flookup: the lookup runs without obeying flags, and every result
    /// path is afterwards replayed through a flag-diacritic table; paths the
    /// table rejects are dropped and the survivors lose their flag symbols
    /// unless `show-flags` is on.
    PostFilter,
}

impl FlagPolicy {
    /// The `obey_flags` argument this policy hands to the underlying
    /// basic-transducer lookup.
    fn obey_in_lookup(self, obey_flags: bool) -> bool {
        match self {
            FlagPolicy::InLookup => obey_flags,
            // The lookup must not prune on flags: this dialect decides
            // validity itself, afterwards, over whole paths.
            FlagPolicy::PostFilter => false,
        }
    }
}

/// What bounds a lookup the transducer reports as infinitely ambiguous — and
/// with it the whole dialect's bounding model, because the same choice decides
/// what an *unbounded* lookup means and whether `--time-cutoff` displaces the
/// ambiguity check on the slow path.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AmbiguityLimit {
    /// hfst-lookup: `--max-number` caps the number of results, and an
    /// infinitely ambiguous lookup that was given no cap falls back to
    /// `default_max`. An ordinary lookup is bounded by `max_number` alone, so
    /// nothing bounds the epsilon cycles of a non-cyclic input, and
    /// `--time-cutoff` (which bounds by wall clock instead) suppresses the
    /// ambiguity check entirely.
    MaxResults {
        /// The user's `--max-number`, `-1` for "no limit".
        max_number: isize,
        /// The cap an infinitely ambiguous lookup falls back to when the user
        /// gave none.
        default_max: isize,
    },
    /// hfst-flookup: `--cycles` caps how many times an input epsilon cycle is
    /// followed, and does so on *every* basic-transducer lookup, cyclic or
    /// not. The result count is never capped, and the slow path ignores
    /// `--time-cutoff` altogether (the tool's own help says the option only
    /// works in optimized-lookup mode).
    Cycles,
}

impl AmbiguityLimit {
    /// The result limit an optimized-lookup call is given.
    fn ol_limit(self, infinite: bool, infinite_cutoff: usize) -> isize {
        match self {
            AmbiguityLimit::MaxResults {
                max_number,
                default_max,
            } => {
                if infinite && max_number == -1 {
                    default_max
                } else {
                    max_number
                }
            }
            AmbiguityLimit::Cycles => {
                if infinite {
                    infinite_cutoff as isize
                } else {
                    -1
                }
            }
        }
    }

    /// The warning an infinitely ambiguous optimized lookup prints, or `None`
    /// when this dialect has nothing to say about it.
    fn ol_infinite_warning(self, infinite_cutoff: usize) -> Option<String> {
        match self {
            AmbiguityLimit::MaxResults {
                max_number,
                default_max,
            } => {
                let maxnum = if max_number == -1 {
                    default_max
                } else {
                    max_number
                };
                if max_number == -1 {
                    Some(format!(
                        "Got infinite results, number of results limited to {}\n\
                         (can be controlled with --max-number=N)",
                        maxnum
                    ))
                } else {
                    Some(format!(
                        "Got infinite results, number of results limited to {}",
                        maxnum
                    ))
                }
            }
            // A cycle cap of 0 bounds nothing worth announcing.
            AmbiguityLimit::Cycles if infinite_cutoff == 0 => None,
            AmbiguityLimit::Cycles => Some(format!(
                "Got infinite results, number of cycles limited to {}",
                infinite_cutoff
            )),
        }
    }

    /// The epsilon-cycle cap a basic-transducer lookup is given: whatever the
    /// caller worked out for a cyclic input under `MaxResults`, always the
    /// configured cutoff under `Cycles`.
    fn basic_epsilon_cycles(self, limit: Option<isize>, infinite_cutoff: usize) -> Option<usize> {
        match self {
            AmbiguityLimit::MaxResults { .. } => limit.map(|l| l as usize),
            AmbiguityLimit::Cycles => Some(infinite_cutoff),
        }
    }

    /// Whether a `--time-cutoff` displaces the slow path's ambiguity check.
    fn basic_respects_time_cutoff(self) -> bool {
        matches!(self, AmbiguityLimit::MaxResults { .. })
    }
}

/// How a `--xfst=print-pairs` result set is laid out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PairPrintStyle {
    /// hfst-lookup: every line is `INPUT TAB PAIRS TAB WEIGHT` on the result
    /// stream, the weight of the input path is folded into the printed weight,
    /// flag pairs are dropped unless `show-flags` is on, a result-less lookup
    /// prints an `INPUT TAB INPUT+? TAB inf` failure line when the caller asks
    /// for one, and a composition step can suppress the set-terminating blank
    /// line.
    Lookup,
    /// hfst-flookup: the input form is echoed on the *message* stream while
    /// only the pairs and the weight go to the result stream, the input path's
    /// weight is not folded in, flag pairs are always printed, and a
    /// result-less lookup always prints the bare input form. Failure lines,
    /// composition-origin echoes and newline suppression have no counterpart
    /// here, so this style ignores them.
    Flookup,
}

/// Every knob the engine reads, snapshotted from the tool's options.
#[derive(Clone, Debug)]
pub struct LookupEngineOptions {
    /// xfst variable obey-flags.
    pub obey_flags: bool,
    /// xfst variable show-flags.
    pub show_flags: bool,
    /// xfst variable print-pairs.
    pub print_pairs: bool,
    /// xfst variable print-space.
    pub print_space: bool,
    /// xfst variable quote-special.
    pub quote_special: bool,
    /// What to print for an epsilon symbol (-e).
    pub epsilon_format: String,
    /// Only results whose weight is within this distance of the best result
    /// are printed; negative means no limit (-b).
    pub beam: f32,
    /// Seconds of search per input, 0.0 for no cutoff (-t).
    pub time_cutoff: f64,
    /// How many times an input epsilon cycle is followed (-c).
    pub infinite_cutoff: usize,
    /// How results from several transducers are combined (-C).
    pub cascade: CascadeVariant,
    /// The dialect's flag-diacritic handling.
    pub flags: FlagPolicy,
    /// The dialect's infinite-ambiguity bounding.
    pub ambiguity: AmbiguityLimit,
    /// The dialect's `--xfst=print-pairs` layout.
    pub pair_style: PairPrintStyle,
}

impl LookupEngineOptions {
    fn print_format(&self, s: &str) -> String {
        get_print_format(s, &self.epsilon_format, self.quote_special)
    }

    fn within_beam(&self, weight: f32, lowest_weight: f32) -> bool {
        self.beam < 0.0 || weight <= (lowest_weight + self.beam)
    }
}

// ---------------------------------------------------------------------------
// the optimized-lookup handle
// ---------------------------------------------------------------------------

/// The two optimized-lookup table shapes a lookup stream can produce; the fast
/// lookup path runs on either ([dec:hfst:monomorphic-backends]).
pub enum OlLookup {
    W(HfstTransducer<Transducer<WeightedTables>>),
    U(HfstTransducer<Transducer<UnweightedTables>>),
}

impl OlLookup {
    fn lookup_fd_string_vector(
        &mut self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> Result<HfstOneLevelPaths> {
        match self {
            OlLookup::W(t) => t.lookup_fd_string_vector(s, limit, time_cutoff),
            OlLookup::U(t) => t.lookup_fd_string_vector(s, limit, time_cutoff),
        }
    }

    fn lookup_pairs(&mut self, s: &str, limit: isize, time_cutoff: f64) -> HfstTwoLevelPaths {
        match self {
            OlLookup::W(t) => t.lookup_pairs(s, limit, time_cutoff),
            OlLookup::U(t) => t.lookup_pairs(s, limit, time_cutoff),
        }
    }

    fn is_lookup_infinitely_ambiguous_string_vector(&mut self, s: &StringVector) -> bool {
        match self {
            OlLookup::W(t) => t.is_lookup_infinitely_ambiguous_string_vector(s),
            OlLookup::U(t) => t.is_lookup_infinitely_ambiguous_string_vector(s),
        }
    }
}

/// Whether a stream/transducer type belongs to the optimized-lookup family.
/// THFST is a member of it (the weighted directory format), so it counts here
/// alongside HFST_OL and HFST_OLW.
pub fn is_optimized_lookup_type(ty: ImplementationType) -> bool {
    ty == ImplementationType::HFST_OL_TYPE
        || ty == ImplementationType::HFST_OLW_TYPE
        || ty == ImplementationType::THFST_TYPE
}

// ---------------------------------------------------------------------------
// printing helpers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-lookup.print-lookup-string-fn]
// [spec:hfst:sem:hfst-lookup.print-lookup-string-fn]
// [spec:hfst:def:hfst-flookup.print-lookup-string-fn]
// [spec:hfst:sem:hfst-flookup.print-lookup-string-fn]
fn print_lookup_string(opts: &LookupEngineOptions, s: &StringVector, out: &mut dyn Write) {
    for it in s.iter() {
        let _ = out.write_all(opts.print_format(it).as_bytes());
    }
}

// [spec:hfst:def:hfst-lookup.get-lookup-string-fn]
// [spec:hfst:sem:hfst-lookup.get-lookup-string-fn]
fn get_lookup_string(opts: &LookupEngineOptions, s: &StringVector) -> String {
    let mut retval = String::new();
    for it in s.iter() {
        retval += &opts.print_format(it);
    }
    retval
}

// [spec:hfst:def:hfst-flookup.is-valid-flag-diacritic-path-fn]
// [spec:hfst:sem:hfst-flookup.is-valid-flag-diacritic-path-fn]
fn is_valid_flag_diacritic_path(arcs: &StringVector, reporter: &dyn LookupReporter) -> bool {
    let mut fd_t = FlagDiacriticTable::new();
    let res = fd_t.is_valid_string(arcs);
    if !res {
        reporter.verbose("blocked by flags: ");
        for s in arcs.iter() {
            reporter.verbose(&format!("{} ", s));
        }
    }
    res
}

// ---------------------------------------------------------------------------
// the lookup itself
// ---------------------------------------------------------------------------

/// One transducer of the slow (basic) cascade, together with the symbol
/// bookkeeping that decides whether a lookup against it can succeed at all.
#[derive(Clone, Copy)]
struct BasicTarget<'a> {
    transducer: &'a HfstBasicTransducer,
    symbols_seen: &'a StringSet,
    unknown_or_identity_seen: bool,
}

/// The per-call knobs of the pair printer: whether this call is the one that
/// prints (a cascade step that is not the printing step passes `false`),
/// whether a result-less lookup prints a failure line, an alternative input
/// form to echo (a composition step echoes the original input rather than the
/// intermediate one) and whether the set-terminating newline is suppressed.
#[derive(Clone, Copy)]
struct PairPrintContext<'a> {
    at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&'a HfstOneLevelPath>,
    no_newline: bool,
}

impl PairPrintContext<'static> {
    /// The context of a lookup that is the whole job: it prints, it reports
    /// failure, it echoes its own input and it terminates the set.
    fn whole_lookup() -> PairPrintContext<'static> {
        PairPrintContext {
            at_this_point: true,
            print_fail: true,
            input_to_print: None,
            no_newline: false,
        }
    }
}

// [spec:hfst:def:hfst-lookup.lookup-fd-and-print-fn]
// [spec:hfst:sem:hfst-lookup.lookup-fd-and-print-fn]
// [spec:hfst:def:hfst-flookup.lookup-fd-and-print-fn]
// [spec:hfst:sem:hfst-flookup.lookup-fd-and-print-fn]
#[allow(clippy::too_many_arguments)]
fn lookup_fd_and_print(
    opts: &LookupEngineOptions,
    basic: Option<BasicTarget<'_>>,
    transducer: Option<&mut OlLookup>,
    results: &mut HfstOneLevelPaths,
    s: &HfstOneLevelPath,
    limit: Option<isize>,
    print: PairPrintContext<'_>,
    reporter: &dyn LookupReporter,
    out: &mut dyn Write,
    echo: &mut dyn Write,
) {
    // If we want a StringPairVector representation
    let mut results_spv: HfstTwoLevelPaths = HfstTwoLevelPaths::new();

    if let Some(t) = basic {
        if is_possible_to_get_result(s, t.symbols_seen, t.unknown_or_identity_seen) {
            t.transducer.lookup(
                &s.second,
                &mut results_spv,
                opts.ambiguity
                    .basic_epsilon_cycles(limit, opts.infinite_cutoff),
                // no weight limit, variable 'beam' defines which paths are printed
                None,
                -1,
                opts.flags.obey_in_lookup(opts.obey_flags),
            );
        }
    } else if let Some(big_t) = transducer {
        let mut lookup_str = String::new();
        for it in s.second.iter() {
            lookup_str += it;
        }
        results_spv = big_t.lookup_pairs(&lookup_str, limit.unwrap_or(-1), opts.time_cutoff);
    }

    if print.at_this_point && opts.print_pairs {
        match opts.pair_style {
            PairPrintStyle::Lookup => print_pairs_lookup(opts, &results_spv, s, print, out),
            PairPrintStyle::Flookup => print_pairs_flookup(opts, &results_spv, s, out, echo),
        }
    }

    // Convert HfstTwoLevelPaths into HfstOneLevelPaths
    for it in results_spv.iter() {
        let mut sv: StringVector = Vec::new();
        for spv_it in it.second.iter() {
            sv.push(spv_it.1.clone());
        }
        results.insert(HfstOneLevelPath {
            first: it.first,
            second: sv,
        });
    }

    if opts.flags == FlagPolicy::PostFilter {
        let mut filtered: HfstOneLevelPaths = HfstOneLevelPaths::new();
        for res in results.iter() {
            if is_valid_flag_diacritic_path(&res.second, reporter) || !opts.obey_flags {
                let mut unflagged: StringVector = Vec::new();
                for arc in res.second.iter() {
                    if opts.show_flags || !FdOperation::is_diacritic(arc) {
                        unflagged.push(arc.clone());
                    }
                }
                filtered.insert(HfstOneLevelPath {
                    first: res.first,
                    second: unflagged,
                });
            }
        }
        *results = filtered;
    }
}

/// [`PairPrintStyle::Lookup`]'s renderer.
fn print_pairs_lookup(
    opts: &LookupEngineOptions,
    results_spv: &HfstTwoLevelPaths,
    s: &HfstOneLevelPath,
    print: PairPrintContext<'_>,
    out: &mut dyn Write,
) {
    // No results, print just the lookup string.
    if results_spv.is_empty() {
        if print.print_fail {
            let input = get_lookup_string(opts, &s.second);
            let _ = out.write_all(format!("{}\t{}+?\tinf\n\n", input, input).as_bytes());
            let _ = out.flush();
        }
    } else {
        let mut lowest_weight: f32 = -1.0;
        let mut first = true;
        for it in results_spv.iter() {
            if first {
                lowest_weight = it.first;
            }
            first = false;
            if opts.within_beam(it.first, lowest_weight) {
                // print the lookup string
                let echoed = print.input_to_print.unwrap_or(s);
                print_lookup_string(opts, &echoed.second, &mut *out);
                let _ = out.write_all(b"\t");
                // and the path that yielded the result string
                let mut first_pair = true;
                for it2 in it.second.iter() {
                    if opts.show_flags || !FdOperation::is_diacritic(&it2.1) {
                        if opts.print_space && !first_pair {
                            let _ = out.write_all(b" ");
                        }
                        let _ = out.write_all(
                            format!(
                                "{}:{}",
                                opts.print_format(&it2.0),
                                opts.print_format(&it2.1)
                            )
                            .as_bytes(),
                        );
                        first_pair = false;
                    }
                }
                // and the weight of that path (add the weight of input)
                let _ = out.write_all(format!("\t{:.6}\n", it.first + s.first).as_bytes());
            }
        }
        if !print.no_newline {
            let _ = out.write_all(b"\n");
        }
    }
    let _ = out.flush();
}

/// [`PairPrintStyle::Flookup`]'s renderer: the input form goes to `echo` (the
/// tool's message stream), everything else to `out`.
fn print_pairs_flookup(
    opts: &LookupEngineOptions,
    results_spv: &HfstTwoLevelPaths,
    s: &HfstOneLevelPath,
    out: &mut dyn Write,
    echo: &mut dyn Write,
) {
    if results_spv.is_empty() {
        // No results, print just the lookup string.
        print_lookup_string(opts, &s.second, &mut *echo);
        let _ = out.write_all(b"\n");
    } else {
        let mut lowest_weight: f32 = -1.0;
        let mut first = true;
        for it in results_spv.iter() {
            if first {
                lowest_weight = it.first;
            }
            first = false;
            if opts.within_beam(it.first, lowest_weight) {
                print_lookup_string(opts, &s.second, &mut *echo);
                let _ = out.write_all(b"\t");
                let mut first_pair = true;
                for it2 in it.second.iter() {
                    if opts.print_space && !first_pair {
                        let _ = out.write_all(b" ");
                    }
                    first_pair = false;
                    let _ = write!(
                        out,
                        "{}:{}",
                        opts.print_format(&it2.0),
                        opts.print_format(&it2.1)
                    );
                }
                let _ = writeln!(out, "\t{:.6}", it.first);
            }
        }
        let _ = out.write_all(b"\n");
    }
    let _ = out.flush();
}

// The optimized-lookup single-transducer lookup.
// [spec:hfst:def:hfst-lookup.lookup-simple-fn]
// [spec:hfst:sem:hfst-lookup.lookup-simple-fn]
// [spec:hfst:def:hfst-flookup.lookup-simple-fn]
// [spec:hfst:sem:hfst-flookup.lookup-simple-fn]
#[allow(clippy::too_many_arguments)]
fn lookup_simple_ol(
    opts: &LookupEngineOptions,
    s: &HfstOneLevelPath,
    t: &mut OlLookup,
    infinity: &mut bool,
    print: PairPrintContext<'_>,
    reporter: &dyn LookupReporter,
    out: &mut dyn Write,
    echo: &mut dyn Write,
) -> HfstOneLevelPaths {
    let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();
    let infinite =
        opts.time_cutoff == 0.0 && t.is_lookup_infinitely_ambiguous_string_vector(&s.second);
    if infinite && let Some(msg) = opts.ambiguity.ol_infinite_warning(opts.infinite_cutoff) {
        reporter.warning(&msg);
    }
    let limit = opts.ambiguity.ol_limit(infinite, opts.infinite_cutoff);

    if opts.print_pairs {
        lookup_fd_and_print(
            opts,
            None,
            Some(&mut *t),
            &mut results,
            s,
            Some(limit),
            print,
            reporter,
            out,
            echo,
        );
    } else {
        results = match t.lookup_fd_string_vector(&s.second, limit, opts.time_cutoff) {
            Ok(r) => r,
            Err(e) => reporter.fatal(&format!("{e}")),
        };
    }
    if infinite {
        *infinity = true;
    }

    if results.is_empty() {
        reporter.verbose("Got no results\n");
    }
    results
}

// The basic-transducer single-transducer lookup.
#[allow(clippy::too_many_arguments)]
fn lookup_simple_basic(
    opts: &LookupEngineOptions,
    s: &HfstOneLevelPath,
    t: BasicTarget<'_>,
    infinity: &mut bool,
    print: PairPrintContext<'_>,
    reporter: &dyn LookupReporter,
    out: &mut dyn Write,
    echo: &mut dyn Write,
) -> HfstOneLevelPaths {
    let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();

    let possible = is_possible_to_get_result(s, t.symbols_seen, t.unknown_or_identity_seen);
    let unbounded_by_time = !opts.ambiguity.basic_respects_time_cutoff() || opts.time_cutoff == 0.0;

    if possible
        && unbounded_by_time
        && t.transducer
            .is_lookup_infinitely_ambiguous_path(s, opts.flags.obey_in_lookup(opts.obey_flags))
    {
        if opts.infinite_cutoff > 0 {
            reporter.warning(&format!(
                "Got infinite results, number of cycles limited to {}",
                opts.infinite_cutoff
            ));
        }
        lookup_fd_and_print(
            opts,
            Some(t),
            None,
            &mut results,
            s,
            Some(opts.infinite_cutoff as isize),
            print,
            reporter,
            out,
            echo,
        );
        *infinity = true;
    } else {
        lookup_fd_and_print(
            opts,
            Some(t),
            None,
            &mut results,
            s,
            None,
            print,
            reporter,
            out,
            echo,
        );
    }

    if results.is_empty() {
        reporter.verbose("Got no results\n");
    }
    results
}

// The optimized-lookup cascade: the library cascade combinator driving the
// single-transducer optimized lookup above.
fn lookup_cascading_ol(
    opts: &LookupEngineOptions,
    s: &HfstOneLevelPath,
    cascade: &mut [OlLookup],
    infinity: &mut bool,
    reporter: &dyn LookupReporter,
    out: &mut dyn Write,
    echo: &mut dyn Write,
) -> HfstOneLevelPaths {
    let result = apply_cascade(
        s,
        cascade.len(),
        opts.cascade,
        opts.print_pairs,
        &mut |msg: &str| reporter.verbose(msg),
        &mut |input: &HfstOneLevelPath, step: &CascadeStep<'_>, out: &mut dyn Write| {
            // if last transducer in cascade, print results if --print-pairs
            // is requested
            let print = PairPrintContext {
                at_this_point: step.composed_from.is_some() && step.is_last,
                print_fail: false,
                input_to_print: step.composed_from,
                no_newline: step.composed_from.is_some(),
            };
            lookup_simple_ol(
                opts,
                input,
                &mut cascade[step.index],
                infinity,
                print,
                reporter,
                out,
                &mut *echo,
            )
        },
        out,
    );
    match result {
        Ok(r) => r,
        Err(e) => reporter.fatal(&format!("{e}")),
    }
}

// The basic-transducer cascade: the library cascade combinator driving the
// single-transducer basic lookup above.
fn lookup_cascading_basic(
    opts: &LookupEngineOptions,
    s: &HfstOneLevelPath,
    cascade: &LookupCascade,
    infinity: &mut bool,
    reporter: &dyn LookupReporter,
    out: &mut dyn Write,
    echo: &mut dyn Write,
) -> HfstOneLevelPaths {
    let result = apply_cascade(
        s,
        cascade.basic.len(),
        opts.cascade,
        opts.print_pairs,
        &mut |msg: &str| reporter.verbose(msg),
        &mut |input: &HfstOneLevelPath, step: &CascadeStep<'_>, out: &mut dyn Write| {
            // if last transducer in cascade, print results if --print-pairs
            // is requested
            let print = match step.composed_from {
                Some(origin) => PairPrintContext {
                    at_this_point: step.is_last,
                    print_fail: false,
                    input_to_print: Some(origin),
                    no_newline: true,
                },
                None => PairPrintContext {
                    at_this_point: opts.cascade != CascadeVariant::Composition,
                    print_fail: false,
                    input_to_print: None,
                    no_newline: false,
                },
            };
            lookup_simple_basic(
                opts,
                input,
                cascade.basic_target(step.index),
                infinity,
                print,
                reporter,
                out,
                &mut *echo,
            )
        },
        out,
    );
    match result {
        Ok(r) => r,
        Err(e) => reporter.fatal(&format!("{e}")),
    }
}

// ---------------------------------------------------------------------------
// the cascade
// ---------------------------------------------------------------------------

/// Every transducer of one lookup stream, in the two shapes a lookup runs on:
/// the optimized-lookup tables that carry the fast path, and the basic
/// transducers the algebra backends are flattened into for the slow path. It
/// also accumulates what the tools need afterwards — the multicharacter
/// symbols their input tokenizer must know, and the per-transducer symbol sets
/// that let an impossible lookup be skipped.
pub struct LookupCascade {
    ol: Vec<OlLookup>,
    basic: Vec<HfstBasicTransducer>,
    // symbols actually seen in (non-ol) transducers
    symbols_seen: Vec<StringSet>,
    unknown_or_identity_seen: Vec<bool>,
    mc_symbols: StringVector,
    // set to false if a non-ol transducer is pushed into the cascade
    only_optimized_lookup: bool,
    first_type: ImplementationType,
    transducer_n: usize,
}

impl Default for LookupCascade {
    fn default() -> LookupCascade {
        LookupCascade::new()
    }
}

impl LookupCascade {
    pub fn new() -> LookupCascade {
        LookupCascade {
            ol: Vec::new(),
            basic: Vec::new(),
            symbols_seen: Vec::new(),
            unknown_or_identity_seen: Vec::new(),
            mc_symbols: Vec::new(),
            only_optimized_lookup: true,
            first_type: ImplementationType::UNSPECIFIED_TYPE,
            transducer_n: 0,
        }
    }

    /// Whether every transducer read so far can use the fast optimized-lookup
    /// path.
    pub fn only_optimized_lookup(&self) -> bool {
        self.only_optimized_lookup
    }

    /// The type of the first transducer read, which is the one the tools name
    /// in their "cannot do fast lookups with %s automata" warning.
    pub fn first_type(&self) -> ImplementationType {
        self.first_type
    }

    /// The multicharacter symbols seen across the cascade, which the tools
    /// hand to their input tokenizer.
    pub fn multichar_symbols(&self) -> &StringVector {
        &self.mc_symbols
    }

    fn basic_target(&self, index: usize) -> BasicTarget<'_> {
        BasicTarget {
            transducer: &self.basic[index],
            symbols_seen: &self.symbols_seen[index],
            unknown_or_identity_seen: self.unknown_or_identity_seen[index],
        }
    }

    /// Account for the next transducer of the stream *before* the tool
    /// pre-processes it: remember the first one's type, note whether the
    /// cascade can still use the fast path, and print the "Reading ..."
    /// progress line. `fallback_name` is used when the transducer carries no
    /// name of its own (the tools pass their input filename).
    pub fn begin_transducer(
        &mut self,
        trans: &AnyTransducer,
        fallback_name: &str,
        reporter: &dyn LookupReporter,
    ) {
        self.transducer_n += 1;
        let ty = trans.get_type();
        if self.transducer_n == 1 {
            self.first_type = ty;
        }
        if !is_optimized_lookup_type(ty) {
            self.only_optimized_lookup = false;
        }

        let mut inputname = trans.get_name();
        if inputname.is_empty() {
            inputname = fallback_name.to_string();
        }
        if self.transducer_n == 1 {
            reporter.verbose(&format!("Reading {}...\n", inputname));
        } else {
            reporter.verbose(&format!("Reading {}...{}\n", inputname, self.transducer_n));
        }
    }

    /// Add a transducer to the cascade: an algebra backend is flattened into a
    /// basic transducer (harvesting its multicharacter symbols and its
    /// symbol set on the way), and an optimized-lookup table is kept as the
    /// fast lookup handle it already is.
    // [spec:hfst:def:hfst-lookup.basic-fn]
    // [spec:hfst:sem:hfst-lookup.basic-fn]
    // [spec:hfst:def:hfst-flookup.basic-fn]
    // [spec:hfst:sem:hfst-flookup.basic-fn]
    pub fn push_transducer(
        &mut self,
        trans: AnyTransducer,
        reporter: &dyn LookupReporter,
    ) -> Result<()> {
        let ty = trans.get_type();
        let mut symbols_seen: StringSet = StringSet::new();
        let mut id_or_unk_seen = false;

        // add multicharacter symbols to mc_symbols
        if ty == ImplementationType::SFST_TYPE
            || ty == ImplementationType::TROPICAL_OPENFST_TYPE
            || ty == ImplementationType::FOMA_TYPE
        {
            let basic = to_basic(&trans)?;
            for it in basic.iter() {
                for tr_it in it.iter() {
                    let mcs = tr_it.get_input_symbol(basic.coder());
                    symbols_seen.insert(mcs.clone());
                    if mcs == internal_unknown || mcs == internal_identity {
                        id_or_unk_seen = true;
                    }
                    if mcs.chars().count() > 1 {
                        self.mc_symbols.push(mcs.clone());
                        reporter.verbose(&format!("multicharacter symbol: {}\n", mcs));
                    }
                }
            }
            self.basic.push(basic);
            self.symbols_seen.push(symbols_seen);
            self.unknown_or_identity_seen.push(id_or_unk_seen);
        }

        // one dispatch per read ([dec:hfst:monomorphic-backends]): the OL
        // variants carry the fast lookup path; the algebra variants were
        // already flattened into the basic cascade above.
        match trans {
            AnyTransducer::OlW(t) => self.ol.push(OlLookup::W(t)),
            AnyTransducer::OlU(t) => self.ol.push(OlLookup::U(t)),
            // THFST is the weighted OL engine under a distinct tag; recover it
            // as the weighted lookup handle (O(1) table move).
            AnyTransducer::Thfst(t) => self.ol.push(OlLookup::W(t.into_olw())),
            AnyTransducer::Tropical(_) => {}
            // Foma is an algebra backend, already flattened into the basic
            // cascade above, like Tropical.
            #[cfg(feature = "foma")]
            AnyTransducer::Foma(_) => {}
        }
        Ok(())
    }

    /// Look one input path up in the whole cascade, returning the results and
    /// whether the lookup was infinitely ambiguous. `out` is the tool's result
    /// stream and `echo` its message stream (only
    /// [`PairPrintStyle::Flookup`] writes to the latter).
    // [spec:hfst:def:hfst-lookup.perform-lookups-fn]
    // [spec:hfst:sem:hfst-lookup.perform-lookups-fn]
    // [spec:hfst:def:hfst-flookup.perform-lookups-fn]
    // [spec:hfst:sem:hfst-flookup.perform-lookups-fn]
    pub fn perform_lookups(
        &mut self,
        origin: &HfstOneLevelPath,
        unknown: bool,
        opts: &LookupEngineOptions,
        reporter: &dyn LookupReporter,
        out: &mut dyn Write,
        echo: &mut dyn Write,
    ) -> (HfstOneLevelPaths, bool) {
        let mut infinite = false;
        if unknown {
            return (HfstOneLevelPaths::new(), infinite);
        }
        let results = if self.only_optimized_lookup {
            if self.ol.len() == 1 {
                lookup_simple_ol(
                    opts,
                    origin,
                    &mut self.ol[0],
                    &mut infinite,
                    PairPrintContext::whole_lookup(),
                    reporter,
                    out,
                    echo,
                )
            } else {
                lookup_cascading_ol(
                    opts,
                    origin,
                    &mut self.ol,
                    &mut infinite,
                    reporter,
                    out,
                    echo,
                )
            }
        } else if self.basic.len() == 1 {
            lookup_simple_basic(
                opts,
                origin,
                self.basic_target(0),
                &mut infinite,
                PairPrintContext::whole_lookup(),
                reporter,
                out,
                echo,
            )
        } else {
            lookup_cascading_basic(opts, origin, self, &mut infinite, reporter, out, echo)
        };
        (results, infinite)
    }
}

/// Flatten any backend's transducer into a basic transducer (one dispatch,
/// each arm monomorphizing separately).
fn to_basic(trans: &AnyTransducer) -> Result<HfstBasicTransducer> {
    match trans {
        AnyTransducer::Tropical(t) => HfstBasicTransducer::try_from_transducer(t),
        AnyTransducer::OlW(t) => HfstBasicTransducer::try_from_transducer(t),
        AnyTransducer::OlU(t) => HfstBasicTransducer::try_from_transducer(t),
        #[cfg(feature = "foma")]
        AnyTransducer::Foma(t) => HfstBasicTransducer::try_from_transducer(t),
        AnyTransducer::Thfst(t) => HfstBasicTransducer::try_from_transducer(t),
    }
}
