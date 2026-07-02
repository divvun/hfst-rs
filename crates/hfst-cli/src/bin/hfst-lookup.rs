#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-lookup.cc — the transducer lookup
//! (apply) command-line tool. Lookup is done from left to right (as opposed to
//! xfst and foma, which look up from right to left; for that behaviour use
//! hfst-flookup). Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, inc fragments).
//!
//! This is a unary tool (#includes inc/globals-unary.h, getopt-cases-unary.h,
//! check-params-unary.h); it mirrors hfst-invert's option-parsing skeleton and
//! adds the tool-specific options.

use hfst::error::ErrorKind;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPaths, ImplementationType, StringVector,
};
use hfst::hfst_flag_diacritics::FdOperation;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_lookup_format::{
    self as lookup_format, CascadeStep, CascadeVariant, LookupFormats, LookupInputFormat,
    LookupOutputFormat, LookupRenderOptions, LookupStats, apply_cascade, is_possible_to_get_result,
    parse_lookup_line, print_lookups,
};
use hfst::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use hfst::hfst_symbol_defs::StringSet;
use hfst::hfst_symbol_defs::{internal_identity, internal_unknown};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_from_env, hfst_error, hfst_error_at_line, hfst_set_program_name,
    hfst_strformat, hfst_warning, print_more_info, print_report_bugs, verbose_print,
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
// max_number is size_t = -1 (SIZE_MAX) by default, meaning "no limit"; modelled
// here as isize -1 (which lookup_fd / lookup_pairs treat as unlimited).
static mut MAX_NUMBER: isize = -1;
const DEFAULT_MAX_NUMBER: isize = 5; // the C++ static MAX_NUMBER = 5
static mut BEAM: f32 = -1.0;

static mut CASCADE: CascadeVariant = CascadeVariant::Union;

// symbols actually seen in (non-ol) transducers
static mut CASCADE_SYMBOLS_SEEN: Vec<StringSet> = Vec::new();
static mut CASCADE_UNKNOWN_OR_IDENTITY_SEEN: Vec<bool> = Vec::new();

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

// the output templates (begin/lookup/end triples for the regular, empty,
// unknown and infinite cases), chosen from OUTPUT_FORMAT in parse_options.
static mut FORMATS: Option<LookupFormats> = None;

fn formats() -> &'static LookupFormats {
    unsafe {
        (*std::ptr::addr_of!(FORMATS))
            .as_ref()
            .expect("output format templates are initialised in parse_options")
    }
}

static mut PRINT_STATISTICS: bool = false;
static mut SHOW_PROGRESS_BAR: bool = false;

// statistic counting
static mut STATS: LookupStats = LookupStats::new();

// which transducer in the cascade we are handling
static mut TRANSDUCER_NUMBER: u32 = 0;

// [spec:hfst:def:hfst-lookup.print-usage-fn]
// [spec:hfst:sem:hfst-lookup.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\n\
         perform transducer lookup (apply)\n\
         NOTE: hfst-lookup does lookup from left to right as opposed to xfst and foma\n\
         \x20     lookup which is carried out from right to left. In order to do lookup\n\
         \x20     in a similar way as xfst and foma, use 'hfst-flookup' instead.\n\
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
         \x20 -I, --input-strings=SFILE        Read lookup strings from SFILE\n\
         \x20 -O, --output-format=OFORMAT      Use OFORMAT printing results sets\n\
         \x20 -e, --epsilon-format=EPS         Print epsilon as EPS\n\
         \x20 -F, --input-format=IFORMAT       Use IFORMAT parsing input\n\
         \x20 -x, --statistics                 Print statistics\n\
         \x20 -X, --xfst=VARIABLE              Toggle xfst VARIABLE\n\
         \x20 -c, --cycles=INT                 How many times to follow input epsilon cycles\n\
         \x20                                  (only for non-lookup-optimized transducers)\n\
         \x20 -n, --max-number=INT             Maximum number of results printed for each input\n\
         \x20                                  (only for lookup-optimized transducers)\n\
         \x20 -b, --beam=B                     Output only analyses whose weight is within B from\n\
         \x20                                  the best analysis\n\
         \x20 -t, --time-cutoff=S              Limit search after having used S seconds per input\n\
         \x20                                  (only for lookup-optimized transducers)\n\
         \x20 -C, --cascade=CASCADE            How multiple transducers in input are handled\n\
         \x20 -P, --progress                   Show neat progress bar if possible\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = msg.write_all(
        "OFORMAT is one of {xerox,cg,apertium}, xerox being default\n\
         IFORMAT is one of {text,spaced,apertium}, default being text,\n\
         unless OFORMAT is apertium\n\
         VARIABLEs relevant to lookup are {print-pairs,print-space,\n\
         quote-special,show-flags,obey-flags}\n\
         Input epsilon cycles are followed by default INT=5 times.\n\
         Epsilon is printed by default as an empty string.\n\
         B must be a non-negative float.\n\
         S must be a non-negative float. The default, 0.0, indicates no cutoff.\n\
         If the input contains several transducers, a set containing\n\
         results from all transducers is printed for each input string.\n"
            .as_bytes(),
    );
    let _ = write!(msg, "\n");

    let _ = msg.write_all(
        "CASCADE must be one of { union, priority-union, composition }.\n\
         If not specified, defaults to {union}.\n"
            .as_bytes(),
    );
    let _ = write!(msg, "\n");

    let _ = msg.write_all(
        "STREAM can be { input, output, both }. If not given, defaults to {both}.\n\
         If input file is not specified with -I, input is read interactively line by\n\
         line from the user. If you redirect input from a file, use --pipe-mode=input.\n\
         --pipe-mode=output is ignored on non-windows platforms.\n"
            .as_bytes(),
    );
    let _ = write!(msg, "\n");

    let _ = write!(
        msg,
        "Todo:\n\
         \x20 Support --xfst=obey-flags for optimized lookup format.\n\
         \x20 Support --cycles for optimized lookup format.\n"
    );

    let _ = write!(
        msg,
        "\n\
         Known bugs:\n\
         \x20 'quote-special' quotes spaces that come from 'print-space'\n"
    );

    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-lookup.parse-options-fn]
// [spec:hfst:sem:hfst-lookup.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            for (name, has_arg, val) in [
                ("input-strings", getopt::REQUIRED_ARGUMENT, b'I'),
                ("output-format", getopt::REQUIRED_ARGUMENT, b'O'),
                ("input-format", getopt::REQUIRED_ARGUMENT, b'F'),
                ("statistics", getopt::NO_ARGUMENT, b'x'),
                ("cycles", getopt::REQUIRED_ARGUMENT, b'c'),
                ("max-number", getopt::REQUIRED_ARGUMENT, b'n'),
                ("xfst", getopt::REQUIRED_ARGUMENT, b'X'),
                ("epsilon-format", getopt::REQUIRED_ARGUMENT, b'e'),
                ("epsilon-format2", getopt::REQUIRED_ARGUMENT, b'E'),
                ("beam", getopt::REQUIRED_ARGUMENT, b'b'),
                ("time-cutoff", getopt::REQUIRED_ARGUMENT, b't'),
                ("pipe-mode", getopt::OPTIONAL_ARGUMENT, b'p'),
                ("progress", getopt::NO_ARGUMENT, b'P'),
                ("cascade", getopt::REQUIRED_ARGUMENT, b'C'),
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
                }
                b'c' => {
                    INFINITE_CUTOFF = optarg.parse::<i32>().unwrap_or(0) as usize;
                }
                b'n' => {
                    MAX_NUMBER = optarg.parse::<i32>().unwrap_or(0) as isize;
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
                b'C' => {
                    if optarg == "union" {
                        CASCADE = CascadeVariant::Union;
                    } else if optarg == "priority-union" {
                        CASCADE = CascadeVariant::PriorityUnion;
                    } else if optarg == "composition" {
                        CASCADE = CascadeVariant::Composition;
                    } else {
                        hfst_error(
                            1,
                            0,
                            &format!(
                                "--cascade argument {} unrecognised, possible values are\n\
                                 {{ union, priority-union, composition }}",
                                optarg
                            ),
                        );
                    }
                }
                _ => {
                    return handle_error_case(c);
                }
            }
        }

        FORMATS = Some(LookupFormats::for_output_format(OUTPUT_FORMAT));

        if !LOOKUP_GIVEN {
            *lookup_reader() = Some(Box::new(std::io::BufReader::new(std::io::stdin())));
            LOOKUP_FILE_NAME = "<stdin>".to_string();
        }
        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-lookup.print-prompt-fn]
// [spec:hfst:sem:hfst-lookup.print-prompt-fn]
unsafe fn print_prompt() {
    unsafe {
        if !globals::SILENT && !PIPE_INPUT && !LOOKUP_GIVEN {
            eprint!("> ");
        }
    }
}

// The renderer knobs for the library %-template engine, snapshotted from the
// tool's option globals.
unsafe fn render_opts() -> LookupRenderOptions {
    unsafe {
        LookupRenderOptions {
            epsilon_format: EPSILON_FORMAT.clone(),
            space_format: SPACE_FORMAT.clone(),
            print_space: PRINT_SPACE,
            show_flags: SHOW_FLAGS,
            quote_special: QUOTE_SPECIAL,
            // hfst-lookup puts an unsplittable lookup form in %b
            unsplit_to_base: true,
            beam: BEAM,
        }
    }
}

unsafe fn get_print_format(s: &str) -> String {
    unsafe { lookup_format::get_print_format(s, &EPSILON_FORMAT, QUOTE_SPECIAL) }
}

// [spec:hfst:def:hfst-lookup.print-lookup-string-fn]
// [spec:hfst:sem:hfst-lookup.print-lookup-string-fn]
unsafe fn print_lookup_string(s: &StringVector, out: &mut dyn Write) {
    unsafe {
        for it in s.iter() {
            let _ = out.write_all(get_print_format(it).as_bytes());
        }
    }
}

// [spec:hfst:def:hfst-lookup.get-lookup-string-fn]
// [spec:hfst:sem:hfst-lookup.get-lookup-string-fn]
unsafe fn get_lookup_string(s: &StringVector) -> String {
    unsafe {
        let mut retval = String::new();
        for it in s.iter() {
            retval += &get_print_format(it);
        }
        retval
    }
}

// [spec:hfst:def:hfst-lookup.lookup-fd-and-print-fn]
// [spec:hfst:sem:hfst-lookup.lookup-fd-and-print-fn]
#[allow(clippy::too_many_arguments)]
unsafe fn lookup_fd_and_print(
    tr: Option<&HfstBasicTransducer>,
    transducer: Option<&HfstTransducer>,
    results: &mut HfstOneLevelPaths,
    s: &HfstOneLevelPath,
    limit: Option<isize>,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
    out: &mut dyn Write,
) {
    unsafe {
        // If we want a StringPairVector representation
        let mut results_spv: HfstTwoLevelPaths = HfstTwoLevelPaths::new();

        if let Some(t) = tr {
            if is_possible_to_get_result(
                s,
                &CASCADE_SYMBOLS_SEEN[TRANSDUCER_NUMBER as usize],
                CASCADE_UNKNOWN_OR_IDENTITY_SEEN[TRANSDUCER_NUMBER as usize],
            ) {
                t.lookup(
                    &s.second,
                    &mut results_spv,
                    limit.map(|l| l as usize),
                    // no weight limit, variable 'beam' defines which paths are printed
                    None,
                    -1,
                    OBEY_FLAGS,
                );
            }
        } else if let Some(big_t) = transducer {
            // TODO: is copying slow?
            let mut lookup_str = String::new();
            for it in s.second.iter() {
                lookup_str += it;
            }
            results_spv = big_t.lookup_pairs(&lookup_str, limit.unwrap_or(-1), TIME_CUTOFF);
        }

        if print_pairs_at_this_point && PRINT_PAIRS {
            // No results, print just the lookup string.
            if results_spv.is_empty() {
                if print_fail {
                    let input = get_lookup_string(&s.second);
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
                    if BEAM < 0.0 || it.first <= (lowest_weight + BEAM) {
                        // print the lookup string
                        if let Some(itp) = input_to_print {
                            print_lookup_string(&itp.second, &mut *out);
                        } else {
                            print_lookup_string(&s.second, &mut *out);
                        }
                        let _ = out.write_all(b"\t");
                        // and the path that yielded the result string
                        let mut first_pair = true;
                        for it2 in it.second.iter() {
                            if SHOW_FLAGS || !FdOperation::is_diacritic(&it2.1) {
                                if PRINT_SPACE && !first_pair {
                                    let _ = out.write_all(b" ");
                                }
                                let _ = out.write_all(
                                    format!(
                                        "{}:{}",
                                        get_print_format(&it2.0),
                                        get_print_format(&it2.1)
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
                if !no_newline {
                    let _ = out.write_all(b"\n");
                }
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
    }
}

// HfstTransducer (optimized-lookup) variant.
// [spec:hfst:def:hfst-lookup.lookup-simple-fn]
// [spec:hfst:sem:hfst-lookup.lookup-simple-fn]
#[allow(clippy::too_many_arguments)]
unsafe fn lookup_simple_ol(
    s: &HfstOneLevelPath,
    t: &HfstTransducer,
    infinity: &mut bool,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();
        if TIME_CUTOFF == 0.0 && t.is_lookup_infinitely_ambiguous_string_vector(&s.second) {
            let maxnum: isize = if MAX_NUMBER == -1 {
                DEFAULT_MAX_NUMBER
            } else {
                MAX_NUMBER
            };
            if !globals::SILENT {
                if MAX_NUMBER == -1 {
                    hfst_warning(
                        0,
                        0,
                        &format!(
                            "Got infinite results, number of results limited to {}\n\
                             (can be controlled with --max-number=N)",
                            maxnum
                        ),
                    );
                } else {
                    hfst_warning(
                        0,
                        0,
                        &format!(
                            "Got infinite results, number of results limited to {}",
                            maxnum
                        ),
                    );
                }
            }
            if PRINT_PAIRS {
                lookup_fd_and_print(
                    None,
                    Some(t),
                    &mut results,
                    s,
                    Some(maxnum),
                    print_pairs_at_this_point,
                    print_fail,
                    input_to_print,
                    no_newline,
                    &mut *out,
                );
            } else {
                results = match t.lookup_fd_string_vector(&s.second, maxnum, TIME_CUTOFF) {
                    Ok(r) => r,
                    Err(e) => {
                        hfst_error(1, 0, &format!("{e}"));
                        unreachable!()
                    }
                };
            }
            *infinity = true;
        } else if PRINT_PAIRS {
            lookup_fd_and_print(
                None,
                Some(t),
                &mut results,
                s,
                Some(MAX_NUMBER),
                print_pairs_at_this_point,
                print_fail,
                input_to_print,
                no_newline,
                &mut *out,
            );
        } else {
            results = match t.lookup_fd_string_vector(&s.second, MAX_NUMBER, TIME_CUTOFF) {
                Ok(r) => r,
                Err(e) => {
                    hfst_error(1, 0, &format!("{e}"));
                    unreachable!()
                }
            };
        }

        if results.is_empty() {
            verbose_print("Got no results\n");
        }
        results
    }
}

// HfstBasicTransducer variant.
#[allow(clippy::too_many_arguments)]
unsafe fn lookup_simple_basic(
    s: &HfstOneLevelPath,
    t: &HfstBasicTransducer,
    infinity: &mut bool,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();

        let possible = is_possible_to_get_result(
            s,
            &CASCADE_SYMBOLS_SEEN[TRANSDUCER_NUMBER as usize],
            CASCADE_UNKNOWN_OR_IDENTITY_SEEN[TRANSDUCER_NUMBER as usize],
        );

        if possible && TIME_CUTOFF == 0.0 && t.is_lookup_infinitely_ambiguous_path(s, OBEY_FLAGS) {
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
            lookup_fd_and_print(
                Some(t),
                None,
                &mut results,
                s,
                Some(INFINITE_CUTOFF as isize),
                print_pairs_at_this_point,
                print_fail,
                input_to_print,
                no_newline,
                &mut *out,
            );
            *infinity = true;
        } else {
            lookup_fd_and_print(
                Some(t),
                None,
                &mut results,
                s,
                None,
                print_pairs_at_this_point,
                print_fail,
                input_to_print,
                no_newline,
                &mut *out,
            );
        }

        if results.is_empty() {
            verbose_print("Got no results\n");
        }
        results
    }
}

// HfstTransducer (optimized-lookup) cascade variant: the library cascade
// engine driving this tool's optimized-lookup single-transducer lookup.
unsafe fn lookup_cascading_ol(
    s: &HfstOneLevelPath,
    cascade: &[HfstTransducer],
    infinity: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    unsafe {
        let result = apply_cascade(
            s,
            cascade.len(),
            CASCADE,
            PRINT_PAIRS,
            &mut |msg: &str| verbose_print(msg),
            &mut |input: &HfstOneLevelPath, step: &CascadeStep, out: &mut dyn Write| {
                if step.composed_from.is_some() {
                    lookup_simple_ol(
                        input,
                        &cascade[step.index],
                        infinity,
                        step.is_last,
                        false,
                        step.composed_from,
                        true,
                        out,
                    )
                } else {
                    lookup_simple_ol(
                        input,
                        &cascade[step.index],
                        infinity,
                        false,
                        false,
                        None,
                        false,
                        out,
                    )
                }
            },
            out,
        );
        match result {
            Ok(r) => r,
            Err(e) => {
                hfst_error(1, 0, &format!("{e}"));
                unreachable!()
            }
        }
    }
}

// HfstBasicTransducer cascade variant: the library cascade engine driving this
// tool's basic-transducer single-transducer lookup.
unsafe fn lookup_cascading_basic(
    s: &HfstOneLevelPath,
    cascade: &[HfstBasicTransducer],
    infinity: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    unsafe {
        let result = apply_cascade(
            s,
            cascade.len(),
            CASCADE,
            PRINT_PAIRS,
            &mut |msg: &str| verbose_print(msg),
            &mut |input: &HfstOneLevelPath, step: &CascadeStep, out: &mut dyn Write| {
                TRANSDUCER_NUMBER = step.index as u32; // needed for lookup_simple
                if let Some(origin) = step.composed_from {
                    // if last transducer in cascade, print results if
                    // --print-pairs is requested
                    lookup_simple_basic(
                        input,
                        &cascade[step.index],
                        infinity,
                        step.is_last,
                        false,
                        Some(origin),
                        true,
                        out,
                    )
                } else {
                    lookup_simple_basic(
                        input,
                        &cascade[step.index],
                        infinity,
                        CASCADE != CascadeVariant::Composition,
                        false,
                        None,
                        false,
                        out,
                    )
                }
            },
            out,
        );
        match result {
            Ok(r) => r,
            Err(e) => {
                hfst_error(1, 0, &format!("{e}"));
                unreachable!()
            }
        }
    }
}

unsafe fn perform_lookups_ol(
    origin: &HfstOneLevelPath,
    cascade: &[HfstTransducer],
    unknown: bool,
    infinite: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    unsafe {
        if !unknown {
            if cascade.len() == 1 {
                lookup_simple_ol(
                    origin,
                    &cascade[0],
                    infinite,
                    true,
                    true,
                    None,
                    false,
                    &mut *out,
                )
            } else {
                lookup_cascading_ol(origin, cascade, infinite, &mut *out)
            }
        } else {
            HfstOneLevelPaths::new()
        }
    }
}

// [spec:hfst:def:hfst-lookup.perform-lookups-fn]
// [spec:hfst:sem:hfst-lookup.perform-lookups-fn]
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
                lookup_simple_basic(
                    origin,
                    &cascade[0],
                    infinite,
                    true,
                    true,
                    None,
                    false,
                    &mut *out,
                )
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
            // [spec:hfst:def:hfst-lookup.trans-fn]
            // [spec:hfst:sem:hfst-lookup.trans-fn]
            let trans = match HfstTransducer::new_from_stream(inputstream) {
                Ok(v) => v,
                Err(e) => {
                    hfst_error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let ty = trans.get_type();
            let mut symbols_seen: StringSet = StringSet::new();

            if ty != ImplementationType::HFST_OL_TYPE && ty != ImplementationType::HFST_OLW_TYPE {
                only_optimized_lookup = false;
            }

            let mut inputname = trans.get_name();
            if inputname.is_empty() {
                inputname = globals::input_filename();
            }
            if transducer_n == 1 {
                verbose_print(&format!("Reading {}...\n", inputname));
            } else {
                verbose_print(&format!("Reading {}...{}\n", inputname, transducer_n));
            }

            // add multicharacter symbols to mc_symbols
            if ty == ImplementationType::SFST_TYPE
                || ty == ImplementationType::TROPICAL_OPENFST_TYPE
                || ty == ImplementationType::LOG_OPENFST_TYPE
                || ty == ImplementationType::FOMA_TYPE
            {
                // [spec:hfst:def:hfst-lookup.basic-fn]
                // [spec:hfst:sem:hfst-lookup.basic-fn]
                let basic = match trans.get_basic_transducer() {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                for it in basic.iter() {
                    for tr_it in it.iter() {
                        let mcs = tr_it.get_input_symbol(basic.coder());
                        symbols_seen.insert(mcs.clone());
                        if mcs == internal_unknown || mcs == internal_identity {
                            id_or_unk_seen = true;
                        }
                        if mcs.chars().count() > 1 {
                            mc_symbols.push(mcs.clone());
                            verbose_print(&format!("multicharacter symbol: {}\n", mcs));
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

        if !OBEY_FLAGS
            && (inputstream.get_type() == ImplementationType::HFST_OL_TYPE
                || inputstream.get_type() == ImplementationType::HFST_OLW_TYPE)
        {
            hfst_error(
                1,
                0,
                "not obeying flags not supported on optimized lookup transducers",
            );
        }

        // if transducer type is other than optimized_lookup,
        // convert to HfstBasicTransducer
        let mut line: String;

        let epsilon_format = EPSILON_FORMAT.clone();
        let input_tokenizer = match HfstStrings2FstTokenizer::new(&mc_symbols, &epsilon_format) {
            Ok(t) => t,
            Err(e) => {
                hfst_error(1, 0, &format!("{e}"));
                return 1;
            }
        };

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
                let lookup_file_name = LOOKUP_FILE_NAME.clone();
                if let Ok(md) = std::fs::metadata(&lookup_file_name) {
                    filesize = md.len() as i64;
                }
            }
            eprint!("{}... rewinding\n", filesize);
        }
        print_prompt();
        // C tracked the read position with ftell(LOOKUP_FILE); the std reader has no
        // tell, so accumulate the bytes consumed by read_until (the same cumulative
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

            // strip trailing '\n'/'\r' ('\r' is possible on Windows)
            if let Some(pos) = line.find(['\n', '\r']) {
                line.truncate(pos);
            }
            verbose_print(&format!("Looking up {}...\n", line));
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

            STATS.inputs += 1;
            let kv = match parse_lookup_line(
                &mut line,
                &input_tokenizer,
                &mut markup,
                &mut unknown,
                only_optimized_lookup,
                INPUT_FORMAT,
            ) {
                Ok(kv) => kv,
                Err(e) => {
                    if e.kind == ErrorKind::IncorrectUtf8Coding {
                        hfst_error_at_line(
                            1,
                            0,
                            &globals::input_filename(),
                            LINEN as u32,
                            e.message.as_deref().unwrap_or(""),
                        );
                    }
                    hfst_error(1, 0, &format!("{e}"));
                    return 1;
                }
            };

            if globals::VERBOSE {
                verbose_print("Tokenized to: ");
                for s in kv.second.iter() {
                    verbose_print(&format!("{} ", s));
                }
                verbose_print("\n");
            }

            let kvs = if only_optimized_lookup {
                perform_lookups_ol(&kv, &cascade, unknown, &mut infinite, &mut *outstream)
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
                if let Err(e) = print_lookups(
                    &kvs,
                    &kv,
                    markup_opt,
                    unknown,
                    infinite,
                    formats(),
                    &render_opts(),
                    &mut STATS,
                    &mut *outstream,
                ) {
                    hfst_error(1, 0, &format!("{e}"));
                    return 1;
                }
                let _ = outstream.flush();
            }

            print_prompt();
        } // while lines in input

        if SHOW_PROGRESS_BAR {
            eprint!("{}/{}... Done\n", filepos, filesize);
        }

        if PRINT_STATISTICS {
            if let Err(e) = STATS.write_statistics(&mut *outstream) {
                hfst_error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        0
    }
}

// [spec:hfst:def:hfst-lookup.main-fn]
// [spec:hfst:sem:hfst-lookup.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        // initialise default formats (the C++ does this at static init time)
        EPSILON_FORMAT = String::new();
        SPACE_FORMAT = String::new();

        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.6", "HfstLookup");

        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        // close buffers, we use streams
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        let f = formats();
        verbose_print(&format!(
            "Output formats:\n\
             \x20 regular:'{}''{}...''{}',\n\
             \x20 unanalysed:'{}''{}''{}',\n\
             \x20 untokenised:'{}''{}''{}',\n\
             \x20 infinite:'{}''{}''{}\n\
             \x20 epsilon: '{}', space: '{}', flags: {}\n",
            f.begin_setf,
            f.lookupf,
            f.end_setf,
            f.empty_begin_setf,
            f.empty_lookupf,
            f.empty_end_setf,
            f.unknown_begin_setf,
            f.unknown_lookupf,
            f.unknown_end_setf,
            f.infinite_begin_setf,
            f.infinite_lookupf,
            f.infinite_end_setf,
            EPSILON_FORMAT.clone(),
            SPACE_FORMAT.clone(),
            SHOW_FLAGS as i32
        ));

        // here starts the buffer handling part
        // (C++ wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // emitting "%s is not a valid transducer file" is not reproduced here.)
        let mut instream = match if globals::input_filename() != "<stdin>" {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                hfst_error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-lookup: cannot open output: {e}");
                return 1;
            }
        };
        process_stream(&mut instream, &mut *out);
        let _ = out.flush();

        // (free(inputfilename)/free(outfilename) in C++ are no-ops here.)
        0
    }
}
