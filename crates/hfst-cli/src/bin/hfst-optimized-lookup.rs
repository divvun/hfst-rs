//! hfst-optimized-lookup: run an optimized-lookup transducer on standard input
//! (one word per line) and print analyses in Xerox format.
//!
//! The presentation layer (option parsing, Xerox/weighted output formatting) is
//! a faithful port of tools/src/hfst-optimized-lookup.cc; the lookup engine is
//! provided by the hfst library crate's optimized-lookup runtime
//! (`HfstTransducer::lookup_fd_string`), so the tool no longer carries its own
//! binary-format reader or traversal code.

use hfst_cli::hfst_getopt as getopt;
use libc::{c_char, c_int};
use std::ffi::CString;

// ---------------------------------------------------------------------------
// config.h-defined constants
// ---------------------------------------------------------------------------
const PACKAGE_NAME: &str = "hfst-optimized-lookup";
const PACKAGE_BUGREPORT: &str = "hfst-bugs@helsinki.fi";
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
// global mutable tool state (the C++ globals)
// ---------------------------------------------------------------------------
static mut OUTPUT_TYPE: OutputType = OutputType::Xerox;
#[allow(dead_code)]
static mut VERBOSE_FLAG: bool = false;
static mut DISPLAY_WEIGHTS_FLAG: bool = false;
static mut DISPLAY_UNIQUE_FLAG: bool = false;
static mut ECHO_INPUTS_FLAG: bool = false;
static mut BE_FAST: bool = false;
static mut MAX_ANALYSES: c_int = c_int::MAX;
static mut TIME_CUTOFF: f64 = 0.0;

static mut BEAM: f32 = -1.0;
#[allow(dead_code)]
static mut PIPE_INPUT: bool = false;
#[allow(dead_code)]
static mut PIPE_OUTPUT: bool = false;

// ---------------------------------------------------------------------------
// small i/o helpers
// ---------------------------------------------------------------------------
fn print_out(s: &str) {
    use std::io::Write;
    let mut o = std::io::stdout();
    let _ = o.write_all(s.as_bytes());
}

fn print_err(s: &str) {
    use std::io::Write;
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
struct Transducer {
    inner: hfst::hfst_transducer::HfstTransducer,
    variant: Variant,
    display_vector: DisplayVector,     // Plain
    display_set: DisplaySet,           // Uniq / FdUniq
    display_multimap: DisplayMultiMap, // WPlain
    display_map: DisplayMap,           // WUniq / WFdUniq
}

impl Transducer {
    // Run the library optimized-lookup engine on the input and route each path
    // into the variant's display sink, mirroring the C++ note_analysis hooks.
    fn analyze(&mut self, input: &str) {
        let limit: isize = -1; // no max-lookups cap (preserve current behaviour)
        let time_cutoff: f64 = unsafe { TIME_CUTOFF };
        let paths = self.inner.lookup_fd_string(input, limit, time_cutoff);
        for path in &paths {
            let weight = path.first;
            let output: String = path
                .second
                .iter()
                .cloned()
                .collect::<Vec<String>>()
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
    fn print_analyses(&mut self, prepend: &str) {
        let output_type = unsafe { OUTPUT_TYPE };
        let display_weights = unsafe { DISPLAY_WEIGHTS_FLAG };
        let max_analyses = unsafe { MAX_ANALYSES };
        let beam = unsafe { BEAM };
        match self.variant {
            Variant::Plain | Variant::Fd => {
                // Transducer::printAnalyses (Fd inherits it). beFast -> nothing.
                if unsafe { BE_FAST } {
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
fn run_transducer(t: &mut Transducer) {
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

        if unsafe { ECHO_INPUTS_FLAG } {
            print_out(&format!("{}\n", str_display));
        }

        t.analyze(&str_display);
        t.print_analyses(&str_display);
    }
}

// ---------------------------------------------------------------------------
// setup
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.setup-fn]
// [spec:hfst:sem:hfst-optimized-lookup.setup-fn]
fn setup(path: &str) -> c_int {
    let mut instream = hfst::hfst_input_stream::HfstInputStream::new_filename(path);
    let t = hfst::hfst_transducer::HfstTransducer::new_from_stream(&mut instream);
    let weighted = t.get_type() == hfst::hfst_data_types::ImplementationType::HFST_OLW_TYPE;
    let unique = unsafe { DISPLAY_UNIQUE_FLAG };
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
    run_transducer(&mut tr);
    0
}

// ---------------------------------------------------------------------------
// print_usage / print_version / print_short_help
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.print-usage-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-usage-fn]
fn print_usage() -> bool {
    print_out(&format!(
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
\n\
Report bugs to {}\n\
\n",
        PACKAGE_NAME, PACKAGE_BUGREPORT
    ));
    true
}

// [spec:hfst:def:hfst-optimized-lookup.print-version-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-version-fn]
fn print_version() -> bool {
    print_out(&format!(
        "\n{}\ncopyright (C) 2009 University of Helsinki\n",
        PACKAGE_STRING
    ));
    true
}

// [spec:hfst:def:hfst-optimized-lookup.print-short-help-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-short-help-fn]
fn print_short_help() -> bool {
    print_usage();
    true
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.main-fn]
// [spec:hfst:sem:hfst-optimized-lookup.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) for getopt_long.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();

        loop {
            let long_options: [getopt::Option; 15] = [
                // first the hfst-mandated options
                opt("help", 0, b'h'),
                opt("version", 0, b'V'),
                opt("verbose", 0, b'v'),
                opt("quiet", 0, b'q'),
                opt("silent", 0, b's'),
                // the hfst-optimized-lookup-specific options
                opt("echo-inputs", 0, b'e'),
                opt("show-weights", 0, b'w'),
                opt("beam", 1, b'b'),
                opt("time-cutoff", 1, b't'),
                opt("unique", 0, b'u'),
                opt("xerox", 0, b'x'),
                opt("fast", 0, b'f'),
                opt("pipe-mode", 2, b'p'),
                opt("analyses", 1, b'n'),
                getopt::Option {
                    name: std::ptr::null(),
                    has_arg: 0,
                    flag: std::ptr::null_mut(),
                    val: 0,
                },
            ];

            let short = CString::new("hVvqsewb:t:uxfn:p::").unwrap();
            let mut option_index: c_int = 0;
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );

            if c == -1 {
                // no more options to look at
                break;
            }

            match c as u8 {
                b'h' => {
                    print_usage();
                    return libc::EXIT_SUCCESS;
                }
                b'V' => {
                    print_version();
                    return libc::EXIT_SUCCESS;
                }
                b'v' => {
                    VERBOSE_FLAG = true;
                }
                b'q' | b's' => {
                    VERBOSE_FLAG = false;
                }
                b'e' => {
                    ECHO_INPUTS_FLAG = true;
                }
                b'w' => {
                    DISPLAY_WEIGHTS_FLAG = true;
                }
                b'u' => {
                    DISPLAY_UNIQUE_FLAG = true;
                }
                b'b' => {
                    BEAM = atof(getopt::OPTARG) as f32;
                    if BEAM < 0.0 {
                        print_err("Invalid argument for --beam\n");
                        return libc::EXIT_FAILURE;
                    }
                }
                b't' => {
                    TIME_CUTOFF = atof(getopt::OPTARG);
                    if TIME_CUTOFF < 0.0 {
                        print_err("Invalid argument for --time-cutoff\n");
                        return libc::EXIT_FAILURE;
                    }
                }
                b'n' => {
                    MAX_ANALYSES = atoi(getopt::OPTARG);
                    if MAX_ANALYSES < 1 {
                        print_err("Invalid or no argument for analyses count\n");
                        return libc::EXIT_FAILURE;
                    }
                }
                b'x' => {
                    OUTPUT_TYPE = Xerox;
                }
                b'f' => {
                    BE_FAST = true;
                }
                b'p' => {
                    let arg = getopt::OPTARG;
                    if arg.is_null() {
                        PIPE_INPUT = true;
                        PIPE_OUTPUT = true;
                    } else {
                        let a = cstr(arg);
                        if a == "both" || a == "BOTH" {
                            PIPE_INPUT = true;
                            PIPE_OUTPUT = true;
                        } else if a == "input" || a == "INPUT" || a == "in" || a == "IN" {
                            PIPE_INPUT = true;
                        } else if a == "output" || a == "OUTPUT" || a == "out" || a == "OUT" {
                            PIPE_OUTPUT = true;
                        } else {
                            print_err(&format!("--pipe-mode argument {} unrecognised\n\n", a));
                            return libc::EXIT_FAILURE;
                        }
                    }
                }
                _ => {
                    print_err("Invalid option\n\n");
                    print_short_help();
                    return libc::EXIT_FAILURE;
                }
            }
        }

        // no more options, we should now be at the input filename
        let optind = getopt::OPTIND;
        if (optind + 1) < argc {
            print_err("More than one input file given\n");
            libc::EXIT_FAILURE
        } else if (optind + 1) == argc {
            let path = *argv.offset(optind as isize);
            let pathstr = cstr(path);
            setup(&pathstr)
        } else {
            print_err("No input file given\n");
            libc::EXIT_FAILURE
        }
    }
}

// helpers -------------------------------------------------------------------
fn opt(name: &str, has_arg: c_int, val: u8) -> getopt::Option {
    // leak the CString so the pointer stays valid for getopt's lifetime (the
    // long_options table is rebuilt every loop iteration in the C++ too, via a
    // static array of string literals).
    let c = CString::new(name).unwrap();
    let ptr = c.into_raw() as *const c_char;
    getopt::Option {
        name: ptr,
        has_arg,
        flag: std::ptr::null_mut(),
        val: val as c_int,
    }
}

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

// atof / atoi over a possibly-NULL C string, matching the C library semantics
// the tool relies on (atof(optarg) / atoi(optarg)).
unsafe fn atof(ptr: *const c_char) -> f64 {
    unsafe {
        if ptr.is_null() {
            return 0.0;
        }
        let s = cstr(ptr);
        parse_leading_f64(&s)
    }
}

unsafe fn atoi(ptr: *const c_char) -> c_int {
    unsafe {
        if ptr.is_null() {
            return 0;
        }
        let s = cstr(ptr);
        parse_leading_i32(&s)
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

fn parse_leading_i32(s: &str) -> c_int {
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
    t[..end].parse::<c_int>().unwrap_or(0)
}
