#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-flookup.cc — the transducer lookup
//! (apply) command-line tool. Lookup is done right to left, like flookup of
//! foma and lookup of xfst. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).
//!
//! This is a unary tool (#includes inc/globals-unary.h, getopt-cases-unary.h,
//! check-params-unary.h); it mirrors hfst-invert's option-parsing skeleton and
//! adds the tool-specific options.

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPaths, ImplementationType, StringPairVector,
    StringVector,
};
use hfst::hfst_flag_diacritics::FdOperation;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_lookup_flag_diacritics::FlagDiacriticTable;
use hfst::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use hfst::hfst_symbol_defs::{StringSet, internal_identity, internal_unknown, is_epsilon};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_error, hfst_error_at_line, hfst_set_program_name,
    hfst_strformat, hfst_warning, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::{BufRead, Write};

// ---------------------------------------------------------------------------
// tools-specific global state (the C++ file's static variables)
// ---------------------------------------------------------------------------

static mut LOOKUP_FILE_NAME: String = String::new();
// The lookup-strings input. In the C this was a FILE* (a named file from -I, or
// stdin); after the io-foundation de-C-ism it is a std::io::BufRead. LOOKUP_GIVEN
// records whether -I named a file (so the seekable file-size progress bar and the
// interactive prompt know which mode they are in).
static mut LOOKUP_READER: Option<Box<dyn BufRead>> = None;

fn lookup_reader() -> &'static mut Option<Box<dyn BufRead>> {
    unsafe { &mut *std::ptr::addr_of_mut!(LOOKUP_READER) }
}
static mut PIPE_INPUT: bool = false;
static mut PIPE_OUTPUT: bool = false;
static mut LINEN: usize = 0;
static mut LOOKUP_GIVEN: bool = false;
static mut INFINITE_CUTOFF: usize = 5;
static mut BEAM: f32 = -1.0;
static mut INVERT: bool = false;
static mut FORCE_OL: bool = false; // accept also ol transducers when -R is not
// specified inverting is slow then

// symbols actually seen in (non-ol) transducers
static mut CASCADE_SYMBOLS_SEEN: Vec<StringSet> = Vec::new();
static mut CASCADE_UNKNOWN_OR_IDENTITY_SEEN: Vec<bool> = Vec::new();

// [spec:hfst:def:hfst-flookup.lookup-input-format]
#[derive(Clone, Copy, PartialEq)]
enum LookupInputFormat {
    Utf8TokenInput,
    SpaceSeparatedTokenInput,
    ApertiumInput,
}

// [spec:hfst:def:hfst-flookup.lookup-output-format]
#[derive(Clone, Copy, PartialEq)]
enum LookupOutputFormat {
    XeroxOutput,
    CgOutput,
    ApertiumOutput,
}

static mut INPUT_FORMAT: LookupInputFormat = LookupInputFormat::Utf8TokenInput;
static mut OUTPUT_FORMAT: LookupOutputFormat = LookupOutputFormat::XeroxOutput;
static mut TIME_CUTOFF: f64 = 0.0;

// XFST variables for apply
static mut SHOW_FLAGS: bool = false;
static mut OBEY_FLAGS: bool = true;
static mut PRINT_PAIRS: bool = false;
static mut PRINT_SPACE: bool = false;
static mut QUOTE_SPECIAL: bool = false;

static mut EPSILON_FORMAT: String = String::new();
static mut SPACE_FORMAT: String = String::new();

// the formats for lookup cases go like so:
//  BEGIN LOOKUP LOOKUP LOOKUP... END
// for standard case of more than 0 and less than infinite results:
static mut BEGIN_SETF: String = String::new(); // print before set of lookups
static mut LOOKUPF: String = String::new(); // print before each lookup
static mut END_SETF: String = String::new(); // print for each lookup
// when there are 0 results:
static mut EMPTY_BEGIN_SETF: String = String::new();
static mut EMPTY_LOOKUPF: String = String::new();
static mut EMPTY_END_SETF: String = String::new();
// when there are 0 results and token is unrecognizable by analyser:
static mut UNKNOWN_BEGIN_SETF: String = String::new();
static mut UNKNOWN_LOOKUPF: String = String::new();
static mut UNKNOWN_END_SETF: String = String::new();
// when there are infinite results:
static mut INFINITE_BEGIN_SETF: String = String::new();
static mut INFINITE_LOOKUPF: String = String::new();
static mut INFINITE_END_SETF: String = String::new();

static mut PRINT_STATISTICS: bool = false;
static mut SHOW_PROGRESS_BAR: bool = false;

// predefined formats
// Xerox
const XEROX_BEGIN_SETF: &str = "";
const XEROX_LOOKUPF: &str = "%i\t%l\t%w%n";
const XEROX_END_SETF: &str = "%n";
const XEROX_EMPTY_BEGIN_SETF: &str = "";
const XEROX_EMPTY_LOOKUPF: &str = "%i\t%i+?\t%w%n";
const XEROX_EMPTY_END_SETF: &str = "%n";
const XEROX_UNKNOWN_BEGIN_SETF: &str = "";
const XEROX_UNKNOWN_LOOKUPF: &str = "%i\t%i+?\t%w%n";
const XEROX_UNKNOWN_END_SETF: &str = "%n";
const XEROX_INFINITE_BEGIN_SETF: &str = "";
const XEROX_INFINITE_LOOKUPF: &str = "%i\t%l\t%w%n";
const XEROX_INFINITE_END_SETF: &str = "%i\t[...cyclic...]%n%n";
// CG
const CG_BEGIN_SETF: &str = "\"<%i>\"%n";
const CG_LOOKUPF: &str = "\t\"%b\"%a\t%w%n";
const CG_END_SETF: &str = "%n";
const CG_EMPTY_BEGIN_SETF: &str = "\"<%i>\"%n";
const CG_EMPTY_LOOKUPF: &str = "\t\"%i\" ?\tInf%n";
const CG_EMPTY_END_SETF: &str = "%n";
const CG_UNKNOWN_BEGIN_SETF: &str = "\"<%i>\"%n";
const CG_UNKNOWN_LOOKUPF: &str = "\t\"%i\"\t ?\tInf%n";
const CG_UNKNOWN_END_SETF: &str = "%n";
const CG_INFINITE_BEGIN_SETF: &str = "\"<%i>\"%n";
const CG_INFINITE_LOOKUPF: &str = "\t\"%b\"%a\t%w%n";
const CG_INFINITE_END_SETF: &str = "\t\"%i\"...cyclic...%n%n";
// apertium
const APERTIUM_BEGIN_SETF: &str = "^%i";
const APERTIUM_LOOKUPF: &str = "/%l";
const APERTIUM_END_SETF: &str = "$%m%n";
const APERTIUM_EMPTY_BEGIN_SETF: &str = "^%i";
const APERTIUM_EMPTY_LOOKUPF: &str = "/*%i";
const APERTIUM_EMPTY_END_SETF: &str = "$%m%n";
const APERTIUM_UNKNOWN_BEGIN_SETF: &str = " ";
const APERTIUM_UNKNOWN_LOOKUPF: &str = "%i%m";
const APERTIUM_UNKNOWN_END_SETF: &str = " ";
const APERTIUM_INFINITE_BEGIN_SETF: &str = "^%i";
const APERTIUM_INFINITE_LOOKUPF: &str = "/%l";
const APERTIUM_INFINITE_END_SETF: &str = "/...$%n";

// statistic counting
static mut INPUTS: u64 = 0;
static mut NO_ANALYSES: u64 = 0;
static mut ANALYSED: u64 = 0;
static mut ANALYSES: u64 = 0;

// which transducer in the cascade we are handling
static mut TRANSDUCER_NUMBER: u32 = 0;

// [spec:hfst:def:hfst-flookup.print-usage-fn]
// [spec:hfst:sem:hfst-flookup.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\n\
         Perform transducer lookup (apply). Lookup is done from right to left,\n\
         in the same way as in flookup of foma and lookup of xfst.\n\
         \n",
        globals::program_name()
    );

    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Input/Output options:\n\
         \x20 -i, --input=INFILE       Read input transducer from INFILE\n\
         \x20 -o, --output=OUTFILE     Write output to OUTFILE\n\
         \x20 -p, --pipe-mode[=STREAM] Control input and output streams\n"
    );

    let _ = write!(
        msg,
        "Lookup options:\n\
         \x20 -R, --invert                     Do lookdown instead of lookup\n\
         \x20 -I, --input-strings=SFILE        Read lookup strings from SFILE\n\
         \x20 -O, --output-format=OFORMAT      Use OFORMAT printing results sets\n\
         \x20 -e, --epsilon-format=EPS         Print epsilon as EPS\n\
         \x20 -F, --input-format=IFORMAT       Use IFORMAT parsing input\n\
         \x20 -x, --statistics                 Print statistics\n\
         \x20 -X, --xfst=VARIABLE              Toggle xfst VARIABLE\n\
         \x20 -c, --cycles=INT                 How many times to follow input epsilon cycles\n\
         \x20 -b, --beam=B                     Output only analyses whose weight is within B from\n\
         \x20                                  the best analysis\n\
         \x20 -t, --time-cutoff=S              Limit search after having used S seconds per input\n\
         \x20                                  (currently only works in optimized-lookup mode\n\
         \x20 -P, --progress                   Show neat progress bar if possible\n\
         \x20 -f, --force-ol                   Force lookup of optimized lookup transducers (slow)\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "OFORMAT is one of {{xerox,cg,apertium}}, xerox being default\n\
         IFORMAT is one of {{text,spaced,apertium}}, default being text,\n\
         unless OFORMAT is apertium\n\
         VARIABLEs relevant to lookup are {{print-pairs,print-space,\n\
         quote-special,show-flags,obey-flags}}\n\
         Input epsilon cycles are followed by default INT=5 times.\n\
         Epsilon is printed by default as an empty string.\n\
         B must be a non-negative float.\n\
         S must be a non-negative float. The default, 0.0, indicates no cutoff.\n\
         If the input contains several transducers, a set containing\n\
         results from all transducers is printed for each input string.\n"
    );
    let _ = write!(msg, "\n");

    let _ = write!(
        msg,
        "STREAM can be {{ input, output, both }}. If not given, defaults to {{both}}.\n\
         If input file is not specified with -I, input is read interactively line by\n\
         line from the user. If you redirect input from a file, use --pipe-mode=input.\n\
         --pipe-mode=output is ignored on non-windows platforms.\n"
    );
    let _ = write!(msg, "\n");

    let _ = write!(
        msg,
        "Known bugs:\n\
         \x20 * 'quote-special' quotes spaces that come from 'print-space'\n\
         \x20 * optimized lookup transducers are unidirectional and only support lookdown,\n\
         \x20   --force-ol forces inversion but is slow\n"
    );

    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-flookup.parse-options-fn]
// [spec:hfst:sem:hfst-flookup.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            for (name, has_arg, val) in [
                ("input-strings", 1, b'I'),
                ("output-format", 1, b'O'),
                ("input-format", 1, b'F'),
                ("statistics", 0, b'x'),
                ("cycles", 1, b'c'),
                ("xfst", 1, b'X'),
                ("epsilon-format", 1, b'e'),
                ("epsilon-format2", 1, b'E'),
                ("beam", 1, b'b'),
                ("time-cutoff", 1, b't'),
                ("pipe-mode", 2, b'p'),
                ("progress", 0, b'P'),
                ("invert", 0, b'R'),
                ("force-ol", 0, b'f'),
            ] {
                long_options.push(getopt::GetOpt {
                    name,
                    has_arg,
                    val: val as i32,
                });
            }
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }

            // add tool-specific cases here
            let optarg = getopt::optarg();
            match c as u8 {
                b'R' => {
                    INVERT = true;
                }
                b'I' => {
                    LOOKUP_FILE_NAME = optarg.clone();
                    // C: lookup_file = fopen(lookup_file_name, "r"); open the named
                    // file as a buffered std reader instead.
                    match std::fs::File::open(&optarg) {
                        Ok(f) => *lookup_reader() = Some(Box::new(std::io::BufReader::new(f))),
                        Err(_) => *lookup_reader() = None,
                    }
                    LOOKUP_GIVEN = true;
                }
                b'O' => {
                    if optarg == "xerox" {
                        OUTPUT_FORMAT = LookupOutputFormat::XeroxOutput;
                    } else if optarg == "cg" {
                        OUTPUT_FORMAT = LookupOutputFormat::CgOutput;
                    } else if optarg == "apertium" {
                        OUTPUT_FORMAT = LookupOutputFormat::ApertiumOutput;
                        INPUT_FORMAT = LookupInputFormat::ApertiumInput;
                    } else {
                        hfst_error(
                            1,
                            0,
                            &format!(
                                "Unknown output format {}; valid values are: xerox, cg, apertium\n",
                                optarg
                            ),
                        );
                        return 1;
                    }
                }
                b'F' => {
                    if optarg == "text" {
                        INPUT_FORMAT = LookupInputFormat::Utf8TokenInput;
                    } else if optarg == "spaced" {
                        INPUT_FORMAT = LookupInputFormat::SpaceSeparatedTokenInput;
                    } else if optarg == "apertium" {
                        INPUT_FORMAT = LookupInputFormat::ApertiumInput;
                    } else {
                        hfst_error(
                            1,
                            0,
                            &format!(
                                "Unknown input format {}; valid values are:utf8, spaced, apertium\n",
                                optarg
                            ),
                        );
                        return 1;
                    }
                }
                b'e' | b'E' => {
                    EPSILON_FORMAT = optarg.clone();
                }
                b'b' => {
                    BEAM = optarg.parse::<f32>().unwrap_or(0.0);
                    if BEAM < 0.0 {
                        eprint!("Invalid argument for --beam\n");
                        return 1;
                    }
                }
                b't' => {
                    TIME_CUTOFF = optarg.parse::<f64>().unwrap_or(0.0);
                    if TIME_CUTOFF < 0.0 {
                        eprint!("Invalid argument for --time-cutoff\n");
                        return 1;
                    }
                }
                b'x' => {
                    PRINT_STATISTICS = true;
                }
                b'X' => {
                    if optarg == "print-pairs" {
                        PRINT_PAIRS = true;
                    } else if optarg == "print-space" {
                        PRINT_SPACE = true;
                        SPACE_FORMAT = " ".to_string();
                    } else if optarg == "show-flags" {
                        SHOW_FLAGS = true;
                    } else if optarg == "quote-special" {
                        QUOTE_SPECIAL = true;
                    } else if optarg == "obey-flags" {
                        OBEY_FLAGS = false;
                    } else {
                        hfst_error(1, 0, &format!("Xfst variable {} unrecognised", optarg));
                    }
                    // NOTE: C++ falls through from 'X' into 'c' (no break).
                    INFINITE_CUTOFF = optarg.parse::<i32>().unwrap_or(0) as usize;
                }
                b'c' => {
                    INFINITE_CUTOFF = optarg.parse::<i32>().unwrap_or(0) as usize;
                }
                b'p' => {
                    if getopt::optarg_opt().is_none() {
                        PIPE_INPUT = true;
                        PIPE_OUTPUT = true;
                    } else if optarg == "both" || optarg == "BOTH" {
                        PIPE_INPUT = true;
                        PIPE_OUTPUT = true;
                    } else if optarg == "input"
                        || optarg == "INPUT"
                        || optarg == "in"
                        || optarg == "IN"
                    {
                        PIPE_INPUT = true;
                    } else if optarg == "output"
                        || optarg == "OUTPUT"
                        || optarg == "out"
                        || optarg == "OUT"
                    {
                        PIPE_OUTPUT = true;
                    } else {
                        hfst_error(
                            1,
                            0,
                            &format!("--pipe-mode argument {} unrecognised", optarg),
                        );
                    }
                }
                b'P' => {
                    SHOW_PROGRESS_BAR = true;
                }
                b'f' => {
                    FORCE_OL = true;
                }
                _ => {
                    return handle_error_case(c);
                }
            }
        }

        match OUTPUT_FORMAT {
            LookupOutputFormat::XeroxOutput => {
                BEGIN_SETF = XEROX_BEGIN_SETF.to_string();
                LOOKUPF = XEROX_LOOKUPF.to_string();
                END_SETF = XEROX_END_SETF.to_string();
                EMPTY_BEGIN_SETF = XEROX_EMPTY_BEGIN_SETF.to_string();
                EMPTY_LOOKUPF = XEROX_EMPTY_LOOKUPF.to_string();
                EMPTY_END_SETF = XEROX_EMPTY_END_SETF.to_string();
                UNKNOWN_BEGIN_SETF = XEROX_UNKNOWN_BEGIN_SETF.to_string();
                UNKNOWN_LOOKUPF = XEROX_UNKNOWN_LOOKUPF.to_string();
                UNKNOWN_END_SETF = XEROX_UNKNOWN_END_SETF.to_string();
                INFINITE_BEGIN_SETF = XEROX_INFINITE_BEGIN_SETF.to_string();
                INFINITE_LOOKUPF = XEROX_INFINITE_LOOKUPF.to_string();
                INFINITE_END_SETF = XEROX_INFINITE_END_SETF.to_string();
            }
            LookupOutputFormat::CgOutput => {
                BEGIN_SETF = CG_BEGIN_SETF.to_string();
                LOOKUPF = CG_LOOKUPF.to_string();
                END_SETF = CG_END_SETF.to_string();
                EMPTY_BEGIN_SETF = CG_EMPTY_BEGIN_SETF.to_string();
                EMPTY_LOOKUPF = CG_EMPTY_LOOKUPF.to_string();
                EMPTY_END_SETF = CG_EMPTY_END_SETF.to_string();
                UNKNOWN_BEGIN_SETF = CG_UNKNOWN_BEGIN_SETF.to_string();
                UNKNOWN_LOOKUPF = CG_UNKNOWN_LOOKUPF.to_string();
                UNKNOWN_END_SETF = CG_UNKNOWN_END_SETF.to_string();
                INFINITE_BEGIN_SETF = CG_INFINITE_BEGIN_SETF.to_string();
                INFINITE_LOOKUPF = CG_INFINITE_LOOKUPF.to_string();
                INFINITE_END_SETF = CG_INFINITE_END_SETF.to_string();
            }
            LookupOutputFormat::ApertiumOutput => {
                BEGIN_SETF = APERTIUM_BEGIN_SETF.to_string();
                LOOKUPF = APERTIUM_LOOKUPF.to_string();
                END_SETF = APERTIUM_END_SETF.to_string();
                EMPTY_BEGIN_SETF = APERTIUM_EMPTY_BEGIN_SETF.to_string();
                EMPTY_LOOKUPF = APERTIUM_EMPTY_LOOKUPF.to_string();
                EMPTY_END_SETF = APERTIUM_EMPTY_END_SETF.to_string();
                UNKNOWN_BEGIN_SETF = APERTIUM_UNKNOWN_BEGIN_SETF.to_string();
                UNKNOWN_LOOKUPF = APERTIUM_UNKNOWN_LOOKUPF.to_string();
                UNKNOWN_END_SETF = APERTIUM_UNKNOWN_END_SETF.to_string();
                INFINITE_BEGIN_SETF = APERTIUM_INFINITE_BEGIN_SETF.to_string();
                INFINITE_LOOKUPF = APERTIUM_INFINITE_LOOKUPF.to_string();
                INFINITE_END_SETF = APERTIUM_INFINITE_END_SETF.to_string();
            }
        }

        if !LOOKUP_GIVEN {
            *lookup_reader() = Some(Box::new(std::io::BufReader::new(std::io::stdin())));
            LOOKUP_FILE_NAME = "<stdin>".to_string();
        }
        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-flookup.print-prompt-fn]
// [spec:hfst:sem:hfst-flookup.print-prompt-fn]
unsafe fn print_prompt() {
    unsafe {
        if !globals::SILENT && !PIPE_INPUT && !LOOKUP_GIVEN {
            eprint!("> ");
        }
    }
}

// [spec:hfst:def:hfst-flookup.is-valid-flag-diacritic-path-fn]
// [spec:hfst:sem:hfst-flookup.is-valid-flag-diacritic-path-fn]
unsafe fn is_valid_flag_diacritic_path(arcs: StringVector) -> bool {
    unsafe {
        let mut fd_t = FlagDiacriticTable::new();
        let res = fd_t.is_valid_string(&arcs);
        if !res {
            verbose_printf("blocked by flags: ");
            for s in arcs.iter() {
                verbose_printf(&format!("{} ", s));
            }
        }
        res
    }
}

// [spec:hfst:def:hfst-flookup.lookup-printf-fn]
// [spec:hfst:sem:hfst-flookup.lookup-printf-fn]
unsafe fn lookup_printf(
    format: &str,
    input: Option<&HfstOneLevelPath>,
    result: Option<&HfstOneLevelPath>,
    markup: Option<&str>,
    ofile: &mut dyn Write,
) -> i32 {
    unsafe {
        let epsilon_format = EPSILON_FORMAT.clone();
        let space_format = SPACE_FORMAT.clone();

        // Build the lookupform string (the result side).
        let lookupform: Option<String> = result.map(|r| {
            let mut p = String::new();
            let mut first = true;
            for s in r.second.iter() {
                if !first && PRINT_SPACE {
                    p.push_str(&space_format);
                }
                if is_epsilon(s) {
                    p.push_str(&epsilon_format);
                } else if FdOperation::is_diacritic(s) {
                    if SHOW_FLAGS {
                        p.push_str(s);
                    }
                } else {
                    p.push_str(s);
                }
                first = false;
            }
            p
        });

        // Build the inputform string.
        let inputform: String = match input {
            Some(inp) => {
                let mut p = String::new();
                let mut first = true;
                for s in inp.second.iter() {
                    if !first && PRINT_SPACE {
                        p.push_str(&space_format);
                    }
                    if is_epsilon(s) {
                        p.push_str(&epsilon_format);
                    } else if FdOperation::is_diacritic(s) {
                        if SHOW_FLAGS {
                            p.push_str(s);
                        }
                    } else {
                        p.push_str(s);
                    }
                    first = false;
                }
                p
            }
            None => String::new(),
        };

        // weight
        let w: f32 = match result {
            Some(r) => r.first,
            None => f32::INFINITY,
        };

        // %i, %l, %b, %a, %m substitution sources
        let i = inputform.clone();
        let (l, b, a) = match &lookupform {
            Some(lf) => {
                let l = lf.clone();
                // find the analysis split point (first of '+', ' ', '<', '[')
                let split = lf
                    .find('+')
                    .or_else(|| lf.find(' '))
                    .or_else(|| lf.find('<'))
                    .or_else(|| lf.find('['))
                    .unwrap_or(0);
                let b = lf[..split].to_string();
                let a = lf[split..].to_string();
                (l, b, a)
            }
            None => (String::new(), String::new(), String::new()),
        };
        let m = markup.map(|s| s.to_string()).unwrap_or_default();

        // Walk the format string, substituting %-escapes.
        let format_s = format;
        let mut res = String::new();
        let mut percent = false;
        for ch in format_s.chars() {
            if percent {
                match ch {
                    'b' => res.push_str(&b),
                    'l' => res.push_str(&l),
                    'i' => res.push_str(&i),
                    'a' => res.push_str(&a),
                    'm' => res.push_str(&m),
                    'n' => res.push('\n'),
                    'w' => {
                        // On non-MSC, the C++ never prints "inf" (the test is
                        // 'if (false)'), always uses %f.
                        res.push_str(&format!("{:.6}", w));
                    }
                    other => {
                        // unknown format, retain % as well
                        res.push('%');
                        res.push(other);
                    }
                }
                percent = false;
            } else if ch == '%' {
                percent = true;
            } else {
                res.push(ch);
            }
        }

        let printed = if !QUOTE_SPECIAL {
            res
        } else {
            get_print_format(&res)
        };
        let _ = ofile.write_all(printed.as_bytes());
        printed.len() as i32
    }
}

// [spec:hfst:def:hfst-flookup.string-to-utf8-fn]
// [spec:hfst:sem:hfst-flookup.string-to-utf8-fn]
unsafe fn string_to_utf8(p: &str) -> Vec<String> {
    unsafe {
        let mut path: Vec<String> = Vec::new();
        let bytes = p.as_bytes();
        let mut idx = 0usize;
        while idx < bytes.len() {
            let c = bytes[idx];
            let u8len: usize = if c <= 127 {
                1
            } else if (c & (128 + 64 + 32 + 16)) == (128 + 64 + 32 + 16) {
                4
            } else if (c & (128 + 64 + 32)) == (128 + 64 + 32) {
                3
            } else if (c & (128 + 64)) == (128 + 64) {
                2
            } else {
                hfst_error_at_line(
                    1,
                    0,
                    &globals::input_filename(),
                    LINEN as u32,
                    &format!("{} not valid UTF-8\n", &p[idx..]),
                );
                1
            };
            let end = (idx + u8len).min(bytes.len());
            path.push(String::from_utf8_lossy(&bytes[idx..end]).into_owned());
            idx += u8len;
        }
        path
    }
}

/* Add a '\' in front of ':', ' ' and '\'. */
// [spec:hfst:def:hfst-flookup.escape-special-characters-fn]
// [spec:hfst:sem:hfst-flookup.escape-special-characters-fn]
fn escape_special_characters(s: &str) -> String {
    let mut retval = String::new();
    for ch in s.chars() {
        if ch == ':' || ch == '\\' || ch == ' ' {
            retval.push('\\');
        }
        retval.push(ch);
    }
    retval
}

// [spec:hfst:def:hfst-flookup.line-to-lookup-path-fn]
// [spec:hfst:sem:hfst-flookup.line-to-lookup-path-fn]
unsafe fn line_to_lookup_path(
    s: &mut String,
    tok: &HfstStrings2FstTokenizer,
    markup: &mut String,
    outside_sigma: &mut bool,
    optimized_lookup: bool,
) -> HfstOneLevelPath {
    unsafe {
        let mut rv = HfstOneLevelPath {
            first: 0.0,
            second: Vec::new(),
        };
        *outside_sigma = false;
        INPUTS += 1;
        match INPUT_FORMAT {
            LookupInputFormat::SpaceSeparatedTokenInput => {
                let escaped = escape_special_characters(s);
                let spv: StringPairVector = tok.tokenize_string_pair(&escaped, true);
                for it in spv.iter() {
                    rv.second.push(it.0.clone());
                }
            }
            LookupInputFormat::Utf8TokenInput => {
                if optimized_lookup {
                    rv.second.push(s.clone());
                } else {
                    let escaped = escape_special_characters(s);
                    let spv: StringPairVector = tok.tokenize_string_pair(&escaped, false);
                    for it in spv.iter() {
                        rv.second.push(it.0.clone());
                    }
                }
            }
            LookupInputFormat::ApertiumInput => {
                let mut real_s = String::new();
                let mut m = String::new();
                let mut inbr = false;
                let chars: Vec<char> = s.chars().collect();
                let mut p = 0usize;
                while p < chars.len() {
                    let ch = chars[p];
                    if inbr {
                        if ch == ']' {
                            m.push(ch);
                            inbr = false;
                        } else if ch == '\\' && p + 1 < chars.len() && chars[p + 1] == ']' {
                            p += 1;
                            m.push(chars[p]);
                        } else {
                            m.push(ch);
                        }
                    } else if ch == '[' {
                        m.push(ch);
                        inbr = true;
                    } else if ch == ']' {
                        m.push(ch);
                        p += 1;
                        continue;
                    } else if ch == '\\' {
                        p += 1;
                        if p < chars.len() {
                            real_s.push(chars[p]);
                        }
                    } else {
                        real_s.push(ch);
                    }
                    p += 1;
                }
                let path = string_to_utf8(&real_s);
                *s = real_s;
                *markup = m;
                rv.second = path;
            }
        }
        rv
    }
}

/* Replace all strings str1 in symbol with str2. */
// [spec:hfst:def:hfst-flookup.replace-all-fn]
// [spec:hfst:sem:hfst-flookup.replace-all-fn]
fn replace_all(symbol: String, str1: &str, str2: &str) -> String {
    if str1.is_empty() {
        return symbol;
    }
    symbol.replace(str1, str2)
}

// [spec:hfst:def:hfst-flookup.get-print-format-fn]
// [spec:hfst:sem:hfst-flookup.get-print-format-fn]
unsafe fn get_print_format(s: &str) -> String {
    unsafe {
        if is_epsilon(s) {
            return EPSILON_FORMAT.clone();
        }
        if QUOTE_SPECIAL {
            return replace_all(
                replace_all(replace_all(s.to_string(), "\\", "\\\\"), ":", "\\:"),
                " ",
                "\\ ",
            );
        }
        s.to_string()
    }
}

// [spec:hfst:def:hfst-flookup.print-lookup-string-fn]
// [spec:hfst:sem:hfst-flookup.print-lookup-string-fn]
unsafe fn print_lookup_string(s: &StringVector) {
    unsafe {
        for it in s.iter() {
            eprint!("{}", get_print_format(it));
        }
    }
}

// [spec:hfst:def:hfst-flookup.is-possible-to-get-result-fn]
// [spec:hfst:sem:hfst-flookup.is-possible-to-get-result-fn]
fn is_possible_to_get_result(
    s: &HfstOneLevelPath,
    symbols_seen: &StringSet,
    unknown_or_identity_seen: bool,
) -> bool {
    if unknown_or_identity_seen {
        return true;
    }
    for it in s.second.iter() {
        if !symbols_seen.contains(it) {
            return false;
        }
    }
    true
}

// [spec:hfst:def:hfst-flookup.lookup-fd-and-print-fn]
// [spec:hfst:sem:hfst-flookup.lookup-fd-and-print-fn]
unsafe fn lookup_fd_and_print(
    t: &HfstBasicTransducer,
    results: &mut HfstOneLevelPaths,
    s: &HfstOneLevelPath,
    _limit: isize,
    out: &mut dyn Write,
) {
    unsafe {
        // If we want a StringPairVector representation
        let mut results_spv: HfstTwoLevelPaths = HfstTwoLevelPaths::new();

        if is_possible_to_get_result(
            s,
            &CASCADE_SYMBOLS_SEEN[TRANSDUCER_NUMBER as usize],
            CASCADE_UNKNOWN_OR_IDENTITY_SEEN[TRANSDUCER_NUMBER as usize],
        ) {
            t.lookup(
                &s.second,
                &mut results_spv,
                Some(INFINITE_CUTOFF),
                None,
                -1,
                false,
            );
        }

        if PRINT_PAIRS {
            if results_spv.is_empty() {
                // No results, print just the lookup string.
                print_lookup_string(&s.second);
                let _ = out.write_all(b"\n");
            } else {
                let mut lowest_weight: f32 = -1.0;
                let mut first = true;
                for it in results_spv.iter() {
                    if first {
                        lowest_weight = it.first;
                    }
                    first = false;
                    if BEAM < 0.0 || it.first <= (lowest_weight + BEAM) {
                        print_lookup_string(&s.second);
                        let _ = out.write_all(b"\t");
                        let mut first_pair = true;
                        for it2 in it.second.iter() {
                            if PRINT_SPACE && !first_pair {
                                let _ = out.write_all(b" ");
                            }
                            first_pair = false;
                            let _ = write!(
                                out,
                                "{}:{}",
                                get_print_format(&it2.0),
                                get_print_format(&it2.1)
                            );
                        }
                        let _ = write!(out, "\t{:.6}\n", it.first);
                    }
                }
                let _ = out.write_all(b"\n");
            }
            let _ = out.flush();
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

        let mut filtered: HfstOneLevelPaths = HfstOneLevelPaths::new();
        for res in results.iter() {
            if is_valid_flag_diacritic_path(res.second.clone()) || !OBEY_FLAGS {
                let mut unflagged: StringVector = Vec::new();
                for arc in res.second.iter() {
                    if SHOW_FLAGS || !FdOperation::is_diacritic(arc) {
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

// HfstTransducer (optimized-lookup) variant.
// [spec:hfst:def:hfst-flookup.lookup-simple-fn]
// [spec:hfst:sem:hfst-flookup.lookup-simple-fn]
unsafe fn lookup_simple_ol(
    s: &HfstOneLevelPath,
    t: &mut HfstTransducer,
    infinity: &mut bool,
) -> HfstOneLevelPaths {
    unsafe {
        let results: HfstOneLevelPaths;
        if TIME_CUTOFF == 0.0 && t.is_lookup_infinitely_ambiguous_string_vector(&s.second) {
            if !globals::SILENT && INFINITE_CUTOFF > 0 {
                hfst_warning(
                    0,
                    0,
                    &format!(
                        "Got infinite results, number of cycles limited to {}",
                        INFINITE_CUTOFF
                    ),
                );
            }
            results = t.lookup_fd_string_vector(&s.second, INFINITE_CUTOFF as isize, TIME_CUTOFF);
            *infinity = true;
        } else {
            results = t.lookup_fd_string_vector(&s.second, -1, TIME_CUTOFF);
        }

        if results.is_empty() {
            verbose_printf("Got no results\n");
        }
        results
    }
}

// HfstBasicTransducer variant.
unsafe fn lookup_simple_basic(
    s: &HfstOneLevelPath,
    t: &HfstBasicTransducer,
    infinity: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();

        let possible = is_possible_to_get_result(
            s,
            &CASCADE_SYMBOLS_SEEN[TRANSDUCER_NUMBER as usize],
            CASCADE_UNKNOWN_OR_IDENTITY_SEEN[TRANSDUCER_NUMBER as usize],
        );

        if possible && t.is_lookup_infinitely_ambiguous_path(s, false) {
            if !globals::SILENT && INFINITE_CUTOFF > 0 {
                hfst_warning(
                    0,
                    0,
                    &format!(
                        "Got infinite results, number of cycles limited to {}",
                        INFINITE_CUTOFF
                    ),
                );
            }
            lookup_fd_and_print(t, &mut results, s, INFINITE_CUTOFF as isize, &mut *out);
            *infinity = true;
        } else {
            lookup_fd_and_print(t, &mut results, s, -1, &mut *out);
        }

        if results.is_empty() {
            verbose_printf("Got no results\n");
        }
        results
    }
}

unsafe fn lookup_cascading_ol(
    s: &HfstOneLevelPath,
    cascade: &mut [HfstTransducer],
    infinity: &mut bool,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();
        for i in 0..cascade.len() {
            let result = lookup_simple_ol(s, &mut cascade[i], infinity);
            // (C++ tests 'if (infinity)' on the pointer — always true here.)
            verbose_printf(&format!("Inf results @ level {}\n", i));
            for it in result.iter() {
                results.insert(it.clone());
            }
        }
        results
    }
}

// [spec:hfst:def:hfst-flookup.lookup-cascading-fn]
// [spec:hfst:sem:hfst-flookup.lookup-cascading-fn]
unsafe fn lookup_cascading_basic(
    s: &HfstOneLevelPath,
    cascade: &[HfstBasicTransducer],
    infinity: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();
        for i in 0..cascade.len() {
            TRANSDUCER_NUMBER = i as u32; // needed for lookup_simple
            let result = lookup_simple_basic(s, &cascade[i], infinity, &mut *out);
            // (C++ tests 'if (infinity)' on the pointer — always true here.)
            verbose_printf(&format!("Inf results @ level {}\n", i));
            for it in result.iter() {
                results.insert(it.clone());
            }
        }
        results
    }
}

// [spec:hfst:def:hfst-flookup.print-lookups-fn]
// [spec:hfst:sem:hfst-flookup.print-lookups-fn]
unsafe fn print_lookups(
    kvs: &HfstOneLevelPaths,
    kv: &HfstOneLevelPath,
    markup: Option<&str>,
    outside_sigma: bool,
    inf: bool,
    ofile: &mut dyn Write,
) {
    unsafe {
        let mut lowest_weight: f32 = -1.0;

        if outside_sigma {
            lookup_printf(&UNKNOWN_BEGIN_SETF, Some(kv), None, markup, &mut *ofile);
            lookup_printf(&UNKNOWN_LOOKUPF, Some(kv), None, markup, &mut *ofile);
            lookup_printf(&UNKNOWN_END_SETF, Some(kv), None, markup, &mut *ofile);
            NO_ANALYSES += 1;
        } else if kvs.is_empty() {
            lookup_printf(&EMPTY_BEGIN_SETF, Some(kv), None, markup, &mut *ofile);
            lookup_printf(&EMPTY_LOOKUPF, Some(kv), None, markup, &mut *ofile);
            lookup_printf(&EMPTY_END_SETF, Some(kv), None, markup, &mut *ofile);
            NO_ANALYSES += 1;
        } else if inf {
            ANALYSED += 1;
            lookup_printf(&INFINITE_BEGIN_SETF, Some(kv), None, markup, &mut *ofile);
            let mut first = true;
            for lkv in kvs.iter() {
                if first {
                    lowest_weight = lkv.first;
                }
                first = false;
                if BEAM < 0.0 || lkv.first <= (lowest_weight + BEAM) {
                    lookup_printf(&INFINITE_LOOKUPF, Some(kv), Some(lkv), markup, &mut *ofile);
                    ANALYSES += 1;
                }
            }
            lookup_printf(&INFINITE_END_SETF, Some(kv), None, markup, &mut *ofile);
        } else {
            ANALYSED += 1;
            lookup_printf(&BEGIN_SETF, Some(kv), None, markup, &mut *ofile);
            let mut first = true;
            for lkv in kvs.iter() {
                if first {
                    lowest_weight = lkv.first;
                }
                first = false;
                if BEAM < 0.0 || lkv.first <= (lowest_weight + BEAM) {
                    lookup_printf(&LOOKUPF, Some(kv), Some(lkv), markup, &mut *ofile);
                    ANALYSES += 1;
                }
            }
            lookup_printf(&END_SETF, Some(kv), None, markup, &mut *ofile);
        }
    }
}

unsafe fn perform_lookups_ol(
    origin: &HfstOneLevelPath,
    cascade: &mut Vec<HfstTransducer>,
    unknown: bool,
    infinite: &mut bool,
) -> HfstOneLevelPaths {
    unsafe {
        if !unknown {
            if cascade.len() == 1 {
                lookup_simple_ol(origin, &mut cascade[0], infinite)
            } else {
                lookup_cascading_ol(origin, cascade, infinite)
            }
        } else {
            HfstOneLevelPaths::new()
        }
    }
}

// [spec:hfst:def:hfst-flookup.perform-lookups-fn]
// [spec:hfst:sem:hfst-flookup.perform-lookups-fn]
unsafe fn perform_lookups_basic(
    origin: &HfstOneLevelPath,
    cascade: &[HfstBasicTransducer],
    unknown: bool,
    infinite: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    unsafe {
        if !unknown {
            if cascade.len() == 1 {
                lookup_simple_basic(origin, &cascade[0], infinite, &mut *out)
            } else {
                lookup_cascading_basic(origin, cascade, infinite, &mut *out)
            }
        } else {
            HfstOneLevelPaths::new()
        }
    }
}

unsafe fn process_stream(inputstream: &mut HfstInputStream, outstream: &mut dyn Write) -> i32 {
    unsafe {
        let mut cascade: Vec<HfstTransducer> = Vec::new();
        let mut cascade_mut: Vec<HfstBasicTransducer> = Vec::new();
        // set to false if non-ol transducer is pushed into the cascade
        let mut only_optimized_lookup = true;

        let mut transducer_n: usize = 0;
        let mut mc_symbols: StringVector = Vec::new();
        let mut id_or_unk_seen = false;
        while inputstream.is_good() {
            transducer_n += 1;
            // [spec:hfst:def:hfst-flookup.trans-fn]
            // [spec:hfst:sem:hfst-flookup.trans-fn]
            let mut trans = HfstTransducer::new_from_stream(inputstream);
            let type_ = trans.get_type();
            let mut symbols_seen: StringSet = StringSet::new();

            if type_ != ImplementationType::HFST_OL_TYPE
                && type_ != ImplementationType::HFST_OLW_TYPE
            {
                only_optimized_lookup = false;
            } else if !INVERT && !FORCE_OL {
                hfst_error(
                    1,
                    0,
                    "lookup not supported for optimized lookup transducers: convert to openfst format,\n\
                     invert, and convert back to optimized lookup format or specify --force-ol\n",
                );
            }

            let mut inputname = trans.get_name();
            if inputname.is_empty() {
                inputname = globals::input_filename();
            }
            if transducer_n == 1 {
                verbose_printf(&format!("Reading {}...\n", inputname));
            } else {
                verbose_printf(&format!("Reading {}...{}\n", inputname, transducer_n));
            }

            if !INVERT {
                if type_ != ImplementationType::HFST_OL_TYPE
                    && type_ != ImplementationType::HFST_OLW_TYPE
                {
                    trans.invert();
                } else {
                    trans.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
                    trans.invert();
                    trans.convert(type_, String::new());
                }
            }

            // add multicharacter symbols to mc_symbols
            if type_ == ImplementationType::SFST_TYPE
                || type_ == ImplementationType::TROPICAL_OPENFST_TYPE
                || type_ == ImplementationType::LOG_OPENFST_TYPE
                || type_ == ImplementationType::FOMA_TYPE
            {
                // [spec:hfst:def:hfst-flookup.basic-fn]
                // [spec:hfst:sem:hfst-flookup.basic-fn]
                let basic = trans.get_basic_transducer();
                for it in basic.iter() {
                    for tr_it in it.iter() {
                        let mcs = tr_it.get_input_symbol(basic.coder());
                        symbols_seen.insert(mcs.clone());
                        if mcs == internal_unknown || mcs == internal_identity {
                            id_or_unk_seen = true;
                        }
                        if mcs.chars().count() > 1 {
                            mc_symbols.push(mcs.clone());
                            verbose_printf(&format!("multicharacter symbol: {}\n", mcs));
                        }
                    }
                }
                cascade_mut.push(basic);
                CASCADE_SYMBOLS_SEEN.push(symbols_seen);
                if id_or_unk_seen {
                    CASCADE_UNKNOWN_OR_IDENTITY_SEEN.push(true);
                } else {
                    CASCADE_UNKNOWN_OR_IDENTITY_SEEN.push(false);
                }
            }

            cascade.push(trans);
            id_or_unk_seen = false;
        }

        inputstream.close();

        if PRINT_PAIRS
            && (inputstream.get_type() == ImplementationType::HFST_OL_TYPE
                || inputstream.get_type() == ImplementationType::HFST_OLW_TYPE)
        {
            hfst_error(
                1,
                0,
                "pair printing not supported on optimized lookup transducers",
            );
        }

        let mut line: String;

        let input_tokenizer = HfstStrings2FstTokenizer::new(&mc_symbols, &EPSILON_FORMAT.clone());

        if !only_optimized_lookup && !globals::SILENT {
            hfst_warning(
                0,
                0,
                &format!(
                    "It is not possible to perform fast lookups with {} format automata.\n\
                     Using HFST basic transducer format and performing slow lookups",
                    hfst_strformat(cascade[0].get_type())
                ),
            );
        }

        let mut filesize: i64 = -1;
        if SHOW_PROGRESS_BAR {
            eprint!("Counting file size...\n");
            // C: fseek(END)/ftell to measure, then rewind. The std reader is read
            // from the start, so the file's metadata length is the equivalent size
            // and no rewind is needed.
            if LOOKUP_GIVEN {
                if let Ok(md) = std::fs::metadata(&LOOKUP_FILE_NAME.clone()) {
                    filesize = md.len() as i64;
                }
            }
            eprint!("{}... rewinding\n", filesize);
        }
        print_prompt();
        // C tracked the read position with ftell(LOOKUP_FILE); the std reader has no
        // tell, so accumulate the bytes consumed by read_line (the same cumulative
        // byte count getline+ftell would report).
        let mut filepos: i64 = 0;
        loop {
            // C: getline reads a raw line (bytes) then cstr does a lossy UTF-8
            // conversion. read_until(b'\n') mirrors getline's byte semantics.
            let mut raw_bytes: Vec<u8> = Vec::new();
            match lookup_reader()
                .as_mut()
                .unwrap()
                .read_until(b'\n', &mut raw_bytes)
            {
                Ok(0) => break,
                Ok(n) => filepos += n as i64,
                Err(_) => break,
            }
            line = String::from_utf8_lossy(&raw_bytes).into_owned();

            LINEN += 1;

            // strip trailing '\n'/'\r'
            if let Some(pos) = line.find(['\n', '\r']) {
                line.truncate(pos);
            }
            verbose_printf(&format!("Looking up {}...\n", line));
            if SHOW_PROGRESS_BAR {
                if filesize != -1 {
                    eprint!("{} / {}...\r", filepos, filesize);
                } else {
                    eprint!("{} / ?...\r", LINEN);
                }
            }

            let mut markup = String::new();
            let mut unknown = false;
            let mut infinite = false;

            let kv = line_to_lookup_path(
                &mut line,
                &input_tokenizer,
                &mut markup,
                &mut unknown,
                only_optimized_lookup,
            );

            if globals::VERBOSE {
                verbose_printf("Tokenized to: ");
                for s in kv.second.iter() {
                    verbose_printf(&format!("{} ", s));
                }
                verbose_printf("\n");
            }

            let kvs = if only_optimized_lookup {
                perform_lookups_ol(&kv, &mut cascade, unknown, &mut infinite)
            } else {
                perform_lookups_basic(&kv, &cascade_mut, unknown, &mut infinite, &mut *outstream)
            };

            if !PRINT_PAIRS {
                // printing was already done in function lookup_fd
                let markup_opt = if markup.is_empty() {
                    None
                } else {
                    Some(markup.as_str())
                };
                print_lookups(&kvs, &kv, markup_opt, unknown, infinite, &mut *outstream);
                let _ = outstream.flush();
            }

            print_prompt();
        } // while lines in input

        if SHOW_PROGRESS_BAR {
            eprint!("{}/{}... Done\n", filepos, filesize);
        }

        if PRINT_STATISTICS {
            let _ = write!(
                outstream,
                "Strings\tFound\tMissing\tResults\n{}\t{}\t{}\t{}\n",
                INPUTS, ANALYSED, NO_ANALYSES, ANALYSES
            );
            let _ = write!(
                outstream,
                "Coverage\tAmbiguity\n{:.6}\t{:.6}\n",
                ANALYSED as f32 / INPUTS as f32,
                ANALYSES as f32 / INPUTS as f32
            );
        }
        0
    }
}

// [spec:hfst:def:hfst-flookup.main-fn]
// [spec:hfst:sem:hfst-flookup.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        // initialise the format defaults (the C++ does this at static init time)
        EPSILON_FORMAT = String::new();
        SPACE_FORMAT = String::new();

        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.6", "HfstFlookup");

        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        // close buffers, we use streams
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        verbose_printf(&format!(
            "Output formats:\n\
             \x20 regular:'{}''{}...''{}',\n\
             \x20 unanalysed:'{}''{}''{}',\n\
             \x20 untokenised:'{}''{}''{}',\n\
             \x20 infinite:'{}''{}''{}\n\
             \x20 epsilon: '{}', space: '{}', flags: {}\n",
            BEGIN_SETF,
            LOOKUPF,
            END_SETF,
            EMPTY_BEGIN_SETF,
            EMPTY_LOOKUPF,
            EMPTY_END_SETF,
            UNKNOWN_BEGIN_SETF,
            UNKNOWN_LOOKUPF,
            UNKNOWN_END_SETF,
            INFINITE_BEGIN_SETF,
            INFINITE_LOOKUPF,
            INFINITE_END_SETF,
            EPSILON_FORMAT,
            SPACE_FORMAT,
            SHOW_FLAGS as i32
        ));

        // here starts the buffer handling part
        // (C++ wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // emitting "%s is not a valid transducer file" is not reproduced here.)
        let mut instream = if globals::input_filename() != "<stdin>" {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };

        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-flookup: cannot open output: {e}");
                return 1;
            }
        };
        process_stream(&mut instream, &mut *out);
        let _ = out.flush();

        // (free(inputfilename)/free(outfilename) in C++ are no-ops here.)
        0
    }
}
