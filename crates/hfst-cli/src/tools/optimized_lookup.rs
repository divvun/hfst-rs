//! hfst-optimized-lookup: run an optimized-lookup transducer on standard input
//! (one word per line) and print analyses in Xerox format.
//!
//! The presentation layer (option parsing, Xerox/weighted output formatting) is
//! a faithful port of tools/src/hfst-optimized-lookup.cc; the lookup engine is
//! provided by the hfst library crate's optimized-lookup runtime
//! (`HfstTransducer::lookup_fd_string`), so the tool no longer carries its own
//! binary-format reader or traversal code.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared program-name/version/verbosity fields) and a tool-local [`Options`] —
//! both built by `parse_options` and threaded into the processing functions.
//! There are no `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
use crate::hfst_getopt::{self as getopt, Getopt};
use std::io::Write;

// ---------------------------------------------------------------------------
// config.h-defined constants
// ---------------------------------------------------------------------------
const PACKAGE_STRING: &str = "hfst-optimized-lookup 1.2";

// ---------------------------------------------------------------------------
// typedefs
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.weight]
type Weight = f32;
// [spec:hfst:def:hfst-optimized-lookup.display-vector]
type DisplayVector = Vec<String>;
// [spec:hfst:def:hfst-optimized-lookup.display-set]
type DisplaySet = std::collections::BTreeSet<String>;
// [spec:hfst:def:hfst-optimized-lookup.display-multi-map]
// std::multimap<Weight, std::string>: ordered, allows duplicate keys; modelled
// as a sorted vector of (Weight, String) pairs.
type DisplayMultiMap = Vec<(Weight, String)>;
// [spec:hfst:def:hfst-optimized-lookup.display-map]
type DisplayMap = std::collections::BTreeMap<String, Weight>;

const MAX_IO_STRING: usize = 5000;

// ---------------------------------------------------------------------------
// enums
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.output-type]
#[derive(PartialEq, Clone, Copy)]
enum OutputType {
    #[allow(dead_code)]
    Hfst,
    Xerox,
}
use OutputType::Xerox;

// ---------------------------------------------------------------------------
// tool-local option state (the former tool-specific `static mut`s)
// ---------------------------------------------------------------------------
/// hfst-optimized-lookup's own options. `impl Default` matches the old
/// `static mut` initializers (not all are the type default).
struct Options {
    output_type: OutputType,
    #[allow(dead_code)]
    verbose_flag: bool,
    display_weights_flag: bool,
    display_unique_flag: bool,
    echo_inputs_flag: bool,
    be_fast: bool,
    max_analyses: i32,
    time_cutoff: f64,
    beam: f32,
    #[allow(dead_code)]
    pipe_input: bool,
    #[allow(dead_code)]
    pipe_output: bool,
    /// The single positional operand (the transducer path), resolved from the
    /// leftover free argument once the getopt loop finishes. `None` when no
    /// input file was given.
    input_path: Option<String>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            output_type: OutputType::Xerox,
            verbose_flag: false,
            display_weights_flag: false,
            display_unique_flag: false,
            echo_inputs_flag: false,
            be_fast: false,
            max_analyses: i32::MAX,
            time_cutoff: 0.0,
            beam: -1.0,
            pipe_input: false,
            pipe_output: false,
            input_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// small i/o helpers
// ---------------------------------------------------------------------------
fn print_out(s: &str) {
    let mut o = std::io::stdout();
    let _ = o.write_all(s.as_bytes());
}

fn print_err(s: &str) {
    let mut e = std::io::stderr();
    let _ = e.write_all(s.as_bytes());
}

// ---------------------------------------------------------------------------
// Display sink: the various transducer variants differ only in how they
// collect and emit analyses. This enum captures the C++ class hierarchy's
// virtual note_analysis / printAnalyses behaviour. The library lookup engine
// already filters flag diacritics, so Plain/Uniq/WPlain/WUniq cover all output
// (the Fd variants format identically to their non-Fd counterparts).
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum Variant {
    Plain,
    Uniq,
    Fd,
    FdUniq,
    WPlain,
    WUniq,
    WFd,
    WFdUniq,
}

#[allow(dead_code)]
fn variant_weighted(v: Variant) -> bool {
    matches!(
        v,
        Variant::WPlain | Variant::WUniq | Variant::WFd | Variant::WFdUniq
    )
}

#[allow(dead_code)]
fn variant_has_fd(v: Variant) -> bool {
    matches!(
        v,
        Variant::Fd | Variant::FdUniq | Variant::WFd | Variant::WFdUniq
    )
}

// ---------------------------------------------------------------------------
// The Transducer wraps the hfst library transducer plus the per-variant output
// sinks; analysis is delegated to the library's optimized-lookup engine.
// ---------------------------------------------------------------------------
/// The two optimized-lookup table shapes the stream can produce; the lookup
/// call is the only surface this tool needs ([dec:hfst:monomorphic-backends]).
enum OlInner {
    W(
        hfst::hfst_transducer::HfstTransducer<
            hfst::transducer::Transducer<hfst::transducer::WeightedTables>,
        >,
    ),
    U(
        hfst::hfst_transducer::HfstTransducer<
            hfst::transducer::Transducer<hfst::transducer::UnweightedTables>,
        >,
    ),
}

impl OlInner {
    fn lookup_fd_string(
        &mut self,
        s: &str,
        limit: isize,
        time_cutoff: f64,
    ) -> hfst::error::Result<hfst::hfst_data_types::HfstOneLevelPaths> {
        match self {
            OlInner::W(t) => t.lookup_fd_string(s, limit, time_cutoff),
            OlInner::U(t) => t.lookup_fd_string(s, limit, time_cutoff),
        }
    }
}

struct Transducer {
    inner: OlInner,
    variant: Variant,
    display_vector: DisplayVector,     // Plain
    display_set: DisplaySet,           // Uniq / FdUniq
    display_multimap: DisplayMultiMap, // WPlain
    display_map: DisplayMap,           // WUniq / WFdUniq
}

impl Transducer {
    // Run the library optimized-lookup engine on the input and route each path
    // into the variant's display sink, mirroring the C++ note_analysis hooks.
    fn analyze(&mut self, options: &Options, input: &str) {
        let limit: isize = -1; // no max-lookups cap (preserve current behaviour)
        let time_cutoff: f64 = options.time_cutoff;
        let paths = match self.inner.lookup_fd_string(input, limit, time_cutoff) {
            Ok(p) => p,
            Err(e) => {
                print_err(&format!("{e}\n"));
                return;
            }
        };
        for path in &paths {
            let weight = path.first;
            let output: String = path
                .second
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .concat();
            match self.variant {
                Variant::Plain | Variant::Fd => self.display_vector.push(output),
                Variant::Uniq | Variant::FdUniq => {
                    self.display_set.insert(output);
                }
                Variant::WPlain | Variant::WFd => self.display_multimap.push((weight, output)),
                Variant::WUniq | Variant::WFdUniq => {
                    // mirror the original display_map population: keep the lowest
                    // weight per output. The original guard treats a missing
                    // entry or a stored weight greater than the current one as
                    // "lower", then uses entry().or_insert(), which only writes
                    // when the key is absent.
                    let lower = match self.display_map.get(&output) {
                        None => true,
                        Some(&w) => w > weight,
                    };
                    if lower {
                        self.display_map.entry(output).or_insert(weight);
                    }
                }
            }
        }
    }

    // ---- printing ----

    // [spec:hfst:def:hfst-optimized-lookup.transducer.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.print-analyses-fn]
    fn print_analyses(&mut self, options: &Options, prepend: &str) {
        let output_type = options.output_type;
        let display_weights = options.display_weights_flag;
        let max_analyses = options.max_analyses;
        let beam = options.beam;
        match self.variant {
            Variant::Plain | Variant::Fd => {
                // Transducer::printAnalyses (Fd inherits it). beFast -> nothing.
                if options.be_fast {
                    return;
                }
                if output_type == Xerox && self.display_vector.is_empty() {
                    print_out(&format!("{}\t{}\t+?\n\n", prepend, prepend));
                    return;
                }
                let mut i = 0;
                for it in self.display_vector.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if output_type == Xerox {
                        print_out(&format!("{}\t", prepend));
                    }
                    print_out(&format!("{}\n", it));
                    i += 1;
                }
                self.display_vector.clear(); // purge the display vector
                print_out("\n");
            }
            Variant::Uniq | Variant::FdUniq => {
                // TransducerUniq/TransducerFdUniq::printAnalyses
                if output_type == Xerox && self.display_set.is_empty() {
                    print_out(&format!("{}\t{}\t+?\n\n", prepend, prepend));
                    return;
                }
                let mut i = 0;
                for it in self.display_set.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if output_type == Xerox {
                        print_out(&format!("{}\t", prepend));
                    }
                    print_out(&format!("{}\n", it));
                    i += 1;
                }
                self.display_set.clear(); // purge the display set
                print_out("\n");
            }
            Variant::WPlain => {
                // TransducerW::printAnalyses
                if output_type == Xerox && self.display_multimap.is_empty() {
                    print_out(&format!("{}\t{}\t+?\n\n", prepend, prepend));
                    return;
                }
                // C++ iterates a std::multimap<Weight,string>, ascending by key.
                let mut sorted = self.display_multimap.clone();
                sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut i = 0;
                let mut lowest_weight: f32 = -1.0;
                let mut first = true;
                for (weight, value) in sorted.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if first {
                        lowest_weight = *weight;
                        first = false;
                    }
                    // if beam is not set (negative), only maxAnalyses constrains
                    if beam < 0.0 || *weight <= (lowest_weight + beam) {
                        if output_type == Xerox {
                            print_out(&format!("{}\t", prepend));
                        }
                        print_out(value);
                        if display_weights {
                            print_out(&format!("\t{}", fmt_weight(*weight)));
                        }
                        print_out("\n");
                    }
                    i += 1;
                }
                self.display_multimap.clear();
                print_out("\n");
            }
            Variant::WUniq | Variant::WFdUniq => {
                // TransducerWUniq/TransducerWFdUniq::printAnalyses
                if output_type == Xerox && self.display_map.is_empty() {
                    // NOTE: the WUniq/WFdUniq empty-case prints a single blank
                    // line (one std::endl), unlike WPlain's two.
                    print_out(&format!("{}\t{}\t+?\n", prepend, prepend));
                    print_out("\n");
                    return;
                }
                let mut lowest_weight: f32 = -1.0;
                let mut weight_sorted: Vec<(Weight, String)> = Vec::new();
                let mut first = true;
                // C++ iterates display_map (std::map<string,Weight>) in key order.
                for (key, weight) in self.display_map.iter() {
                    if first {
                        lowest_weight = *weight;
                        first = false;
                    }
                    if beam < 0.0 || *weight <= (lowest_weight + beam) {
                        weight_sorted.push((*weight, key.clone()));
                    }
                }
                weight_sorted
                    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut i = 0;
                for (weight, value) in weight_sorted.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if output_type == Xerox {
                        print_out(&format!("{}\t", prepend));
                    }
                    print_out(value);
                    if display_weights {
                        print_out(&format!("\t{}", fmt_weight(*weight)));
                    }
                    print_out("\n");
                    i += 1;
                }
                self.display_map.clear();
                print_out("\n");
            }
            Variant::WFd => {
                // TransducerWFd has no own printAnalyses: inherits TransducerW.
                if output_type == Xerox && self.display_multimap.is_empty() {
                    print_out(&format!("{}\t{}\t+?\n\n", prepend, prepend));
                    return;
                }
                let mut sorted = self.display_multimap.clone();
                sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut i = 0;
                let mut lowest_weight: f32 = -1.0;
                let mut first = true;
                for (weight, value) in sorted.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if first {
                        lowest_weight = *weight;
                        first = false;
                    }
                    if beam < 0.0 || *weight <= (lowest_weight + beam) {
                        if output_type == Xerox {
                            print_out(&format!("{}\t", prepend));
                        }
                        print_out(value);
                        if display_weights {
                            print_out(&format!("\t{}", fmt_weight(*weight)));
                        }
                        print_out("\n");
                    }
                    i += 1;
                }
                self.display_multimap.clear();
                print_out("\n");
            }
        }
    }
}

// C++ std::cout << float uses %g-like default formatting (6 significant
// digits); the weighted printAnalyses uses '\t' << (*it).first.
fn fmt_weight(w: Weight) -> String {
    // mimic ostream default float formatting (up to 6 significant digits)
    let s = format!("{:.6}", w);
    // trim trailing zeros but keep ostream-ish output
    s
}

// ---------------------------------------------------------------------------
// runTransducer
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.run-transducer-fn]
// [spec:hfst:sem:hfst-optimized-lookup.run-transducer-fn]
fn run_transducer(options: &Options, t: &mut Transducer) {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();

    loop {
        // std::cin.getline(str, MAX_IO_STRING): read a line, drop the newline.
        let mut line_bytes: Vec<u8> = Vec::new();
        let n = handle.read_until(b'\n', &mut line_bytes).unwrap_or(0);
        if n == 0 {
            break; // EOF / read failure
        }
        // strip trailing newline (and a CR if present) — getline drops '\n'.
        if line_bytes.last() == Some(&b'\n') {
            line_bytes.pop();
        }
        // getline reads at most MAX_IO_STRING-1 chars into the buffer.
        line_bytes.truncate(MAX_IO_STRING - 1);

        let str_display = String::from_utf8_lossy(&line_bytes).into_owned();

        if options.echo_inputs_flag {
            print_out(&format!("{}\n", str_display));
        }

        t.analyze(options, &str_display);
        t.print_analyses(options, &str_display);
    }
}

// ---------------------------------------------------------------------------
// setup
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.setup-fn]
// [spec:hfst:sem:hfst-optimized-lookup.setup-fn]
fn setup(options: &Options, path: &str) -> i32 {
    let mut instream = match hfst::hfst_input_stream::HfstInputStream::new_filename(path) {
        Ok(v) => v,
        Err(e) => {
            print_err(&format!("{e}\n"));
            return 1;
        }
    };
    let any = match instream.read() {
        Ok(v) => v,
        Err(e) => {
            print_err(&format!("{e}\n"));
            return 1;
        }
    };
    let weighted = any.get_type() == hfst::hfst_data_types::ImplementationType::HFST_OLW_TYPE;
    // the one dispatch per stream read ([dec:hfst:monomorphic-backends]):
    // the two OL table shapes move in as-is; anything else converts to the
    // weighted OL tables the lookup engine runs on.
    let t = match any {
        hfst::hfst_transducer::AnyTransducer::OlW(t) => OlInner::W(t),
        hfst::hfst_transducer::AnyTransducer::OlU(t) => OlInner::U(t),
        other @ hfst::hfst_transducer::AnyTransducer::Tropical(_) => match other.into_typed() {
            Ok(t) => OlInner::W(t),
            Err(e) => {
                print_err(&format!("{e}\n"));
                return 1;
            }
        },
    };
    let unique = options.display_unique_flag;
    let variant = match (weighted, unique) {
        (false, false) => Variant::Plain,
        (false, true) => Variant::Uniq,
        (true, false) => Variant::WPlain,
        (true, true) => Variant::WUniq,
    };
    let mut tr = Transducer {
        inner: t,
        variant,
        display_vector: DisplayVector::new(),
        display_set: DisplaySet::new(),
        display_multimap: DisplayMultiMap::new(),
        display_map: DisplayMap::new(),
    };
    run_transducer(options, &mut tr);
    0
}

// ---------------------------------------------------------------------------
// print_usage / print_version / print_short_help
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.print-usage-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-usage-fn]
fn print_usage(common: &CommonOptions) -> bool {
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "\nUsage: {} [OPTIONS] TRANSDUCER\n\
Run a transducer on standard input (one word per line) and print analyses\n\
NOTE: hfst-optimized-lookup does lookup from left to right as opposed to xfst\n\
      and foma lookup which is carried out from right to left. In order to do\n\
      lookup in a similar way as xfst and foma, invert the transducer first.\n\
\n\
  -h, --help                  Print this help message\n\
  -V, --version               Print version information\n\
  -v, --verbose               Be verbose\n\
  -q, --quiet                 Don't be verbose (default)\n\
  -s, --silent                Same as quiet\n\
  -e, --echo                  Echo inputs\n\
                              (useful if redirecting lots of output to a file)\n\
  -w, --show-weights          Print final analysis weights (if any)\n\
  -u, --unique                Suppress duplicate analyses\n\
  -n N, --analyses=N          Output no more than N analyses\n\
                              (if the transducer is weighted, the N best analyses)\n\
  -b, --beam=B                Output only analyses whose weight is within B from\n\
                              the best analysis\n\
  -t, --time-cutoff=S         Limit search after having used S seconds per input\n\
  -x, --xerox                 Xerox output format (default)\n\
  -f, --fast                  Be as fast as possible.\n\
                              (with this option enabled -u and -n don't work and\n\
                              output won't be ordered by weight).\n\
  -p, --pipe-mode[=STREAM]    Control input and output streams.\n\
\n\
N must be a positive integer. B must be a non-negative float.\n\
S must be a non-negative float. The default, 0.0, indicates no cutoff.\n\
Options -n and -b are combined with AND, i.e. they both restrict the output.\n\
\n\
STREAM can be {{ input, output, both }}. If not given, defaults to {{both}}.\n\
Input is read interactively line by line from the user. If you redirect input\n\
from a file, use --pipe-mode=input. --pipe-mode=output is ignored on non-windows\n\
platforms.\n\
\n",
        common.program_name
    );
    true
}

// [spec:hfst:def:hfst-optimized-lookup.print-version-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-version-fn]
fn print_version(common: &CommonOptions) -> bool {
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "\n{}\ncopyright (C) 2009 University of Helsinki\n",
        PACKAGE_STRING
    );
    true
}

// [spec:hfst:def:hfst-optimized-lookup.print-short-help-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-short-help-fn]
fn print_short_help(common: &CommonOptions) -> bool {
    print_usage(common);
    true
}

// ---------------------------------------------------------------------------
// parse_options
// ---------------------------------------------------------------------------
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return. hfst-optimized-lookup carries its own long-option
// table and its own `-h/-V/-v/-q/-s` cases (it does not use the shared
// getopt-cases fragments).
fn parse_options(
    common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let long_options: [getopt::GetOpt; 14] = [
            // first the hfst-mandated options
            opt_def("help", 0, b'h'),
            opt_def("version", 0, b'V'),
            opt_def("verbose", 0, b'v'),
            opt_def("quiet", 0, b'q'),
            opt_def("silent", 0, b's'),
            // the hfst-optimized-lookup-specific options
            opt_def("echo-inputs", 0, b'e'),
            opt_def("show-weights", 0, b'w'),
            opt_def("beam", 1, b'b'),
            opt_def("time-cutoff", 1, b't'),
            opt_def("unique", 0, b'u'),
            opt_def("xerox", 0, b'x'),
            opt_def("fast", 0, b'f'),
            opt_def("pipe-mode", 2, b'p'),
            opt_def("analyses", 1, b'n'),
        ];

        let c = opt.getopt_long(args, &long_options);

        if c == -1 {
            // no more options to look at
            break;
        }

        match c as u8 {
            b'h' => {
                print_usage(&common);
                return Err(0);
            }
            b'V' => {
                print_version(&common);
                return Err(0);
            }
            b'v' => {
                options.verbose_flag = true;
            }
            b'q' | b's' => {
                options.verbose_flag = false;
            }
            b'e' => {
                options.echo_inputs_flag = true;
            }
            b'w' => {
                options.display_weights_flag = true;
            }
            b'u' => {
                options.display_unique_flag = true;
            }
            b'b' => {
                options.beam = parse_leading_f64(&opt.optarg()) as f32;
                if options.beam < 0.0 {
                    print_err("Invalid argument for --beam\n");
                    return Err(1);
                }
            }
            b't' => {
                options.time_cutoff = parse_leading_f64(&opt.optarg());
                if options.time_cutoff < 0.0 {
                    print_err("Invalid argument for --time-cutoff\n");
                    return Err(1);
                }
            }
            b'n' => {
                options.max_analyses = parse_leading_i32(&opt.optarg());
                if options.max_analyses < 1 {
                    print_err("Invalid or no argument for analyses count\n");
                    return Err(1);
                }
            }
            b'x' => {
                options.output_type = Xerox;
            }
            b'f' => {
                options.be_fast = true;
            }
            b'p' => match opt.optarg_opt() {
                None => {
                    options.pipe_input = true;
                    options.pipe_output = true;
                }
                Some(a) => {
                    if a == "both" || a == "BOTH" {
                        options.pipe_input = true;
                        options.pipe_output = true;
                    } else if a == "input" || a == "INPUT" || a == "in" || a == "IN" {
                        options.pipe_input = true;
                    } else if a == "output" || a == "OUTPUT" || a == "out" || a == "OUT" {
                        options.pipe_output = true;
                    } else {
                        print_err(&format!("--pipe-mode argument {} unrecognised\n\n", a));
                        return Err(1);
                    }
                }
            },
            _ => {
                print_err("Invalid option\n\n");
                print_short_help(&common);
                return Err(1);
            }
        }
    }

    // no more options, we should now be at the input filename. `optind` is the
    // getopt parser's index of the first free (non-option) argument after the
    // permutation.
    let optind = opt.optind;
    if (optind + 1) < args.len() {
        print_err("More than one input file given\n");
        return Err(1);
    } else if (optind + 1) == args.len() {
        options.input_path = Some(args[optind].clone());
    } else {
        // (no operand; run() reports "No input file given")
        options.input_path = None;
    }

    Ok((common, options))
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.main-fn]
// [spec:hfst:sem:hfst-optimized-lookup.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "1.2", "HfstOptimizedLookup");
    let (_common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // the single positional operand (the transducer path) was resolved in
    // parse_options while the getopt state was live.
    match &options.input_path {
        Some(path) => {
            let pathstr = path.clone();
            setup(&options, &pathstr)
        }
        None => {
            print_err("No input file given\n");
            1
        }
    }
}

// helpers -------------------------------------------------------------------
fn opt_def(name: &'static str, has_arg: i32, val: u8) -> getopt::GetOpt {
    getopt::GetOpt {
        name,
        has_arg,
        val: val as i32,
    }
}

fn parse_leading_f64(s: &str) -> f64 {
    let t = s.trim_start();
    let mut end = 0;
    let bytes = t.as_bytes();
    let mut seen_dot = false;
    let mut seen_e = false;
    while end < bytes.len() {
        let ch = bytes[end];
        let ok = ch.is_ascii_digit()
            || (end == 0 && (ch == b'+' || ch == b'-'))
            || (ch == b'.' && !seen_dot && !seen_e)
            || ((ch == b'e' || ch == b'E') && !seen_e && end > 0)
            || ((ch == b'+' || ch == b'-')
                && end > 0
                && (bytes[end - 1] == b'e' || bytes[end - 1] == b'E'));
        if ch == b'.' {
            seen_dot = true;
        }
        if ch == b'e' || ch == b'E' {
            seen_e = true;
        }
        if !ok {
            break;
        }
        end += 1;
    }
    t[..end].parse::<f64>().unwrap_or(0.0)
}

fn parse_leading_i32(s: &str) -> i32 {
    let t = s.trim_start();
    let mut end = 0;
    let bytes = t.as_bytes();
    while end < bytes.len() {
        let ch = bytes[end];
        if ch.is_ascii_digit() || (end == 0 && (ch == b'+' || ch == b'-')) {
            end += 1;
        } else {
            break;
        }
    }
    t[..end].parse::<i32>().unwrap_or(0)
}
