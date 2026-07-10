//! Faithful 1:1 port of tools/src/hfst-lookup.cc — the transducer lookup
//! (apply) command-line tool. Lookup is done from left to right (as opposed to
//! xfst and foma, which look up from right to left; for that behaviour use
//! hfst-flookup). Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, inc fragments).
//!
//! This is a unary tool (#includes inc/globals-unary.h, getopt-cases-unary.h,
//! check-params-unary.h); it mirrors hfst-invert's option-parsing skeleton and
//! adds the tool-specific options.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    extend_options_from_env, hfst_error, hfst_error_at_line, hfst_set_program_name, hfst_strformat,
    hfst_warning, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
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
use std::io::{BufRead, Write};

// ---------------------------------------------------------------------------
// tool-specific option state (the C++ file's static variables)
// ---------------------------------------------------------------------------

const DEFAULT_MAX_NUMBER: isize = 5; // the C++ static MAX_NUMBER = 5

/// hfst-lookup's own options (the former tool-specific `static mut`s).
struct Options {
    lookup_file_name: String,
    // The lookup-strings input. In the C this was a FILE* (a named file from -I,
    // or stdin); after the io-foundation de-C-ism it is a std::io::BufRead.
    // lookup_given records whether -I named a file (so the seekable file-size
    // progress bar and the interactive prompt know which mode they are in).
    lookup_reader: Option<Box<dyn BufRead>>,
    pipe_input: bool,
    pipe_output: bool,
    linen: usize,
    lookup_given: bool,
    infinite_cutoff: usize,
    // max_number is size_t = -1 (SIZE_MAX) by default, meaning "no limit";
    // modelled here as isize -1 (which lookup_fd / lookup_pairs treat as
    // unlimited).
    max_number: isize,
    beam: f32,
    cascade: CascadeVariant,
    input_format: LookupInputFormat,
    output_format: LookupOutputFormat,
    time_cutoff: f64,
    // XFST variables for apply
    show_flags: bool,
    obey_flags: bool,
    print_pairs: bool,
    print_space: bool,
    quote_special: bool,
    epsilon_format: String,
    space_format: String,
    // the output templates (begin/lookup/end triples for the regular, empty,
    // unknown and infinite cases), chosen from output_format in parse_options.
    formats: Option<LookupFormats>,
    print_statistics: bool,
    show_progress_bar: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            lookup_file_name: String::new(),
            lookup_reader: None,
            pipe_input: false,
            pipe_output: false,
            linen: 0,
            lookup_given: false,
            infinite_cutoff: 5,
            max_number: -1,
            beam: -1.0,
            cascade: CascadeVariant::Union,
            input_format: LookupInputFormat::Utf8TokenInput,
            output_format: LookupOutputFormat::XeroxOutput,
            time_cutoff: 0.0,
            show_flags: false,
            obey_flags: true,
            print_pairs: false,
            print_space: false,
            quote_special: false,
            epsilon_format: String::new(),
            space_format: String::new(),
            formats: None,
            print_statistics: false,
            show_progress_bar: false,
        }
    }
}

impl Options {
    fn formats(&self) -> &LookupFormats {
        self.formats
            .as_ref()
            .expect("output format templates are initialised in parse_options")
    }
}

// The two optimized-lookup table shapes the stream can produce; the fast
// lookup path runs on either ([dec:hfst:monomorphic-backends]).
enum OlTransducer {
    W(HfstTransducer<hfst::transducer::Transducer<hfst::transducer::WeightedTables>>),
    U(HfstTransducer<hfst::transducer::Transducer<hfst::transducer::UnweightedTables>>),
}

impl OlTransducer {
    fn lookup_fd_string_vector(
        &mut self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> hfst::error::Result<HfstOneLevelPaths> {
        match self {
            OlTransducer::W(t) => t.lookup_fd_string_vector(s, limit, time_cutoff),
            OlTransducer::U(t) => t.lookup_fd_string_vector(s, limit, time_cutoff),
        }
    }

    fn lookup_pairs(&mut self, s: &str, limit: isize, time_cutoff: f64) -> HfstTwoLevelPaths {
        match self {
            OlTransducer::W(t) => t.lookup_pairs(s, limit, time_cutoff),
            OlTransducer::U(t) => t.lookup_pairs(s, limit, time_cutoff),
        }
    }

    fn is_lookup_infinitely_ambiguous_string_vector(&mut self, s: &StringVector) -> bool {
        match self {
            OlTransducer::W(t) => t.is_lookup_infinitely_ambiguous_string_vector(s),
            OlTransducer::U(t) => t.is_lookup_infinitely_ambiguous_string_vector(s),
        }
    }
}

// The runtime lookup accumulators (the C++ file's non-option static state): the
// per-transducer symbol tables the basic-lookup path consults, and the cascade
// index of the transducer currently being handled. These are mutated during
// process_stream and threaded into the lookup functions.
struct LookupState {
    // symbols actually seen in (non-ol) transducers
    cascade_symbols_seen: Vec<StringSet>,
    cascade_unknown_or_identity_seen: Vec<bool>,
    // which transducer in the cascade we are handling
    transducer_number: u32,
}

impl LookupState {
    fn new() -> LookupState {
        LookupState {
            cascade_symbols_seen: Vec::new(),
            cascade_unknown_or_identity_seen: Vec::new(),
            transducer_number: 0,
        }
    }
}

// [spec:hfst:def:hfst-lookup.print-usage-fn]
// [spec:hfst:sem:hfst-lookup.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\n\
         perform transducer lookup (apply)\n\
         NOTE: hfst-lookup does lookup from left to right as opposed to xfst and foma\n\
         \x20     lookup which is carried out from right to left. In order to do lookup\n\
         \x20     in a similar way as xfst and foma, use 'hfst-flookup' instead.\n\
         \n",
        common.program_name
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
}

// [spec:hfst:def:hfst-lookup.parse-options-fn]
// [spec:hfst:sem:hfst-lookup.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }

        // add tool-specific cases here
        let optarg = opt.optarg();
        match c as u8 {
            b'I' => {
                options.lookup_file_name = optarg.clone();
                // C: lookup_file = fopen(lookup_file_name, "r"); open the named
                // file as a buffered std reader instead.
                match std::fs::File::open(&optarg) {
                    Ok(f) => options.lookup_reader = Some(Box::new(std::io::BufReader::new(f))),
                    Err(_) => options.lookup_reader = None,
                }
                options.lookup_given = true;
            }
            b'O' => {
                if optarg == "xerox" {
                    options.output_format = LookupOutputFormat::XeroxOutput;
                } else if optarg == "cg" {
                    options.output_format = LookupOutputFormat::CgOutput;
                } else if optarg == "apertium" {
                    options.output_format = LookupOutputFormat::ApertiumOutput;
                    options.input_format = LookupInputFormat::ApertiumInput;
                } else {
                    hfst_error(
                        &common,
                        1,
                        0,
                        &format!(
                            "Unknown output format {}; valid values are: xerox, cg, apertium\n",
                            optarg
                        ),
                    );
                    return Err(1);
                }
            }
            b'F' => {
                if optarg == "text" {
                    options.input_format = LookupInputFormat::Utf8TokenInput;
                } else if optarg == "spaced" {
                    options.input_format = LookupInputFormat::SpaceSeparatedTokenInput;
                } else if optarg == "apertium" {
                    options.input_format = LookupInputFormat::ApertiumInput;
                } else {
                    hfst_error(
                        &common,
                        1,
                        0,
                        &format!(
                            "Unknown input format {}; valid values are:utf8, spaced, apertium\n",
                            optarg
                        ),
                    );
                    return Err(1);
                }
            }
            b'e' | b'E' => {
                options.epsilon_format = optarg.clone();
            }
            b'b' => {
                options.beam = optarg.parse::<f32>().unwrap_or(0.0);
                if options.beam < 0.0 {
                    eprint!("Invalid argument for --beam\n");
                    return Err(1);
                }
            }
            b't' => {
                options.time_cutoff = optarg.parse::<f64>().unwrap_or(0.0);
                if options.time_cutoff < 0.0 {
                    eprint!("Invalid argument for --time-cutoff\n");
                    return Err(1);
                }
            }
            b'x' => {
                options.print_statistics = true;
            }
            b'X' => {
                if optarg == "print-pairs" {
                    options.print_pairs = true;
                } else if optarg == "print-space" {
                    options.print_space = true;
                    options.space_format = " ".to_string();
                } else if optarg == "show-flags" {
                    options.show_flags = true;
                } else if optarg == "quote-special" {
                    options.quote_special = true;
                } else if optarg == "obey-flags" {
                    options.obey_flags = false;
                } else {
                    hfst_error(
                        &common,
                        1,
                        0,
                        &format!("Xfst variable {} unrecognised", optarg),
                    );
                }
            }
            b'c' => {
                options.infinite_cutoff = optarg.parse::<i32>().unwrap_or(0) as usize;
            }
            b'n' => {
                options.max_number = optarg.parse::<i32>().unwrap_or(0) as isize;
            }
            b'p' => {
                if opt.optarg_opt().is_none() {
                    options.pipe_input = true;
                    options.pipe_output = true;
                } else if optarg == "both" || optarg == "BOTH" {
                    options.pipe_input = true;
                    options.pipe_output = true;
                } else if optarg == "input" || optarg == "INPUT" || optarg == "in" || optarg == "IN"
                {
                    options.pipe_input = true;
                } else if optarg == "output"
                    || optarg == "OUTPUT"
                    || optarg == "out"
                    || optarg == "OUT"
                {
                    options.pipe_output = true;
                } else {
                    hfst_error(
                        &common,
                        1,
                        0,
                        &format!("--pipe-mode argument {} unrecognised", optarg),
                    );
                }
            }
            b'P' => {
                options.show_progress_bar = true;
            }
            b'C' => {
                if optarg == "union" {
                    options.cascade = CascadeVariant::Union;
                } else if optarg == "priority-union" {
                    options.cascade = CascadeVariant::PriorityUnion;
                } else if optarg == "composition" {
                    options.cascade = CascadeVariant::Composition;
                } else {
                    hfst_error(
                        &common,
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
                return Err(handle_error_case(&common, &opt, c));
            }
        }
    }

    options.formats = Some(LookupFormats::for_output_format(options.output_format));

    if !options.lookup_given {
        options.lookup_reader = Some(Box::new(std::io::BufReader::new(std::io::stdin())));
        options.lookup_file_name = "<stdin>".to_string();
    }
    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-lookup.print-prompt-fn]
// [spec:hfst:sem:hfst-lookup.print-prompt-fn]
fn print_prompt(common: &CommonOptions, options: &Options) {
    if !common.silent && !options.pipe_input && !options.lookup_given {
        eprint!("> ");
    }
}

// The renderer knobs for the library %-template engine, snapshotted from the
// tool's option state.
fn render_opts(options: &Options) -> LookupRenderOptions {
    LookupRenderOptions {
        epsilon_format: options.epsilon_format.clone(),
        space_format: options.space_format.clone(),
        print_space: options.print_space,
        show_flags: options.show_flags,
        quote_special: options.quote_special,
        // hfst-lookup puts an unsplittable lookup form in %b
        unsplit_to_base: true,
        beam: options.beam,
    }
}

fn get_print_format(options: &Options, s: &str) -> String {
    lookup_format::get_print_format(s, &options.epsilon_format, options.quote_special)
}

// [spec:hfst:def:hfst-lookup.print-lookup-string-fn]
// [spec:hfst:sem:hfst-lookup.print-lookup-string-fn]
fn print_lookup_string(options: &Options, s: &StringVector, out: &mut dyn Write) {
    for it in s.iter() {
        let _ = out.write_all(get_print_format(options, it).as_bytes());
    }
}

// [spec:hfst:def:hfst-lookup.get-lookup-string-fn]
// [spec:hfst:sem:hfst-lookup.get-lookup-string-fn]
fn get_lookup_string(options: &Options, s: &StringVector) -> String {
    let mut retval = String::new();
    for it in s.iter() {
        retval += &get_print_format(options, it);
    }
    retval
}

// [spec:hfst:def:hfst-lookup.lookup-fd-and-print-fn]
// [spec:hfst:sem:hfst-lookup.lookup-fd-and-print-fn]
#[allow(clippy::too_many_arguments)]
fn lookup_fd_and_print(
    options: &Options,
    state: &LookupState,
    tr: Option<&HfstBasicTransducer>,
    transducer: Option<&mut OlTransducer>,
    results: &mut HfstOneLevelPaths,
    s: &HfstOneLevelPath,
    limit: Option<isize>,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
    out: &mut dyn Write,
) {
    // If we want a StringPairVector representation
    let mut results_spv: HfstTwoLevelPaths = HfstTwoLevelPaths::new();

    if let Some(t) = tr {
        if is_possible_to_get_result(
            s,
            &state.cascade_symbols_seen[state.transducer_number as usize],
            state.cascade_unknown_or_identity_seen[state.transducer_number as usize],
        ) {
            t.lookup(
                &s.second,
                &mut results_spv,
                limit.map(|l| l as usize),
                // no weight limit, variable 'beam' defines which paths are printed
                None,
                -1,
                options.obey_flags,
            );
        }
    } else if let Some(big_t) = transducer {
        // TODO: is copying slow?
        let mut lookup_str = String::new();
        for it in s.second.iter() {
            lookup_str += it;
        }
        results_spv = big_t.lookup_pairs(&lookup_str, limit.unwrap_or(-1), options.time_cutoff);
    }

    if print_pairs_at_this_point && options.print_pairs {
        // No results, print just the lookup string.
        if results_spv.is_empty() {
            if print_fail {
                let input = get_lookup_string(options, &s.second);
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
                if options.beam < 0.0 || it.first <= (lowest_weight + options.beam) {
                    // print the lookup string
                    if let Some(itp) = input_to_print {
                        print_lookup_string(options, &itp.second, &mut *out);
                    } else {
                        print_lookup_string(options, &s.second, &mut *out);
                    }
                    let _ = out.write_all(b"\t");
                    // and the path that yielded the result string
                    let mut first_pair = true;
                    for it2 in it.second.iter() {
                        if options.show_flags || !FdOperation::is_diacritic(&it2.1) {
                            if options.print_space && !first_pair {
                                let _ = out.write_all(b" ");
                            }
                            let _ = out.write_all(
                                format!(
                                    "{}:{}",
                                    get_print_format(options, &it2.0),
                                    get_print_format(options, &it2.1)
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

// HfstTransducer (optimized-lookup) variant.
// [spec:hfst:def:hfst-lookup.lookup-simple-fn]
// [spec:hfst:sem:hfst-lookup.lookup-simple-fn]
#[allow(clippy::too_many_arguments)]
fn lookup_simple_ol(
    common: &CommonOptions,
    options: &Options,
    state: &LookupState,
    s: &HfstOneLevelPath,
    t: &mut OlTransducer,
    infinity: &mut bool,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();
    if options.time_cutoff == 0.0 && t.is_lookup_infinitely_ambiguous_string_vector(&s.second) {
        let maxnum: isize = if options.max_number == -1 {
            DEFAULT_MAX_NUMBER
        } else {
            options.max_number
        };
        if !common.silent {
            if options.max_number == -1 {
                hfst_warning(
                    common,
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
                    common,
                    0,
                    0,
                    &format!(
                        "Got infinite results, number of results limited to {}",
                        maxnum
                    ),
                );
            }
        }
        if options.print_pairs {
            lookup_fd_and_print(
                options,
                state,
                None,
                Some(&mut *t),
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
            results = match t.lookup_fd_string_vector(&s.second, maxnum, options.time_cutoff) {
                Ok(r) => r,
                Err(e) => {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    unreachable!()
                }
            };
        }
        *infinity = true;
    } else if options.print_pairs {
        lookup_fd_and_print(
            options,
            state,
            None,
            Some(&mut *t),
            &mut results,
            s,
            Some(options.max_number),
            print_pairs_at_this_point,
            print_fail,
            input_to_print,
            no_newline,
            &mut *out,
        );
    } else {
        results =
            match t.lookup_fd_string_vector(&s.second, options.max_number, options.time_cutoff) {
                Ok(r) => r,
                Err(e) => {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    unreachable!()
                }
            };
    }

    if results.is_empty() {
        verbose_print(common, "Got no results\n");
    }
    results
}

// HfstBasicTransducer variant.
#[allow(clippy::too_many_arguments)]
fn lookup_simple_basic(
    common: &CommonOptions,
    options: &Options,
    state: &LookupState,
    s: &HfstOneLevelPath,
    t: &HfstBasicTransducer,
    infinity: &mut bool,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();

    let possible = is_possible_to_get_result(
        s,
        &state.cascade_symbols_seen[state.transducer_number as usize],
        state.cascade_unknown_or_identity_seen[state.transducer_number as usize],
    );

    if possible
        && options.time_cutoff == 0.0
        && t.is_lookup_infinitely_ambiguous_path(s, options.obey_flags)
    {
        if !common.silent && options.infinite_cutoff > 0 {
            hfst_warning(
                common,
                0,
                0,
                &format!(
                    "Got infinite results, number of cycles limited to {}",
                    options.infinite_cutoff
                ),
            );
        }
        lookup_fd_and_print(
            options,
            state,
            Some(t),
            None,
            &mut results,
            s,
            Some(options.infinite_cutoff as isize),
            print_pairs_at_this_point,
            print_fail,
            input_to_print,
            no_newline,
            &mut *out,
        );
        *infinity = true;
    } else {
        lookup_fd_and_print(
            options,
            state,
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
        verbose_print(common, "Got no results\n");
    }
    results
}

// HfstTransducer (optimized-lookup) cascade variant: the library cascade
// engine driving this tool's optimized-lookup single-transducer lookup.
fn lookup_cascading_ol(
    common: &CommonOptions,
    options: &Options,
    state: &LookupState,
    s: &HfstOneLevelPath,
    cascade: &mut [OlTransducer],
    infinity: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    let result = apply_cascade(
        s,
        cascade.len(),
        options.cascade,
        options.print_pairs,
        &mut |msg: &str| verbose_print(common, msg),
        &mut |input: &HfstOneLevelPath, step: &CascadeStep<'_>, out: &mut dyn Write| {
            if step.composed_from.is_some() {
                lookup_simple_ol(
                    common,
                    options,
                    state,
                    input,
                    &mut cascade[step.index],
                    infinity,
                    step.is_last,
                    false,
                    step.composed_from,
                    true,
                    out,
                )
            } else {
                lookup_simple_ol(
                    common,
                    options,
                    state,
                    input,
                    &mut cascade[step.index],
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
            hfst_error(common, 1, 0, &format!("{e}"));
            unreachable!()
        }
    }
}

// HfstBasicTransducer cascade variant: the library cascade engine driving this
// tool's basic-transducer single-transducer lookup.
fn lookup_cascading_basic(
    common: &CommonOptions,
    options: &Options,
    state: &mut LookupState,
    s: &HfstOneLevelPath,
    cascade: &[HfstBasicTransducer],
    infinity: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    let result = apply_cascade(
        s,
        cascade.len(),
        options.cascade,
        options.print_pairs,
        &mut |msg: &str| verbose_print(common, msg),
        &mut |input: &HfstOneLevelPath, step: &CascadeStep<'_>, out: &mut dyn Write| {
            state.transducer_number = step.index as u32; // needed for lookup_simple
            if let Some(origin) = step.composed_from {
                // if last transducer in cascade, print results if
                // --print-pairs is requested
                lookup_simple_basic(
                    common,
                    options,
                    state,
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
                    common,
                    options,
                    state,
                    input,
                    &cascade[step.index],
                    infinity,
                    options.cascade != CascadeVariant::Composition,
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
            hfst_error(common, 1, 0, &format!("{e}"));
            unreachable!()
        }
    }
}

fn perform_lookups_ol(
    common: &CommonOptions,
    options: &Options,
    state: &LookupState,
    origin: &HfstOneLevelPath,
    cascade: &mut [OlTransducer],
    unknown: bool,
    infinite: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    if !unknown {
        if cascade.len() == 1 {
            lookup_simple_ol(
                common,
                options,
                state,
                origin,
                &mut cascade[0],
                infinite,
                true,
                true,
                None,
                false,
                &mut *out,
            )
        } else {
            lookup_cascading_ol(common, options, state, origin, cascade, infinite, &mut *out)
        }
    } else {
        HfstOneLevelPaths::new()
    }
}

// [spec:hfst:def:hfst-lookup.perform-lookups-fn]
// [spec:hfst:sem:hfst-lookup.perform-lookups-fn]
fn perform_lookups_basic(
    common: &CommonOptions,
    options: &Options,
    state: &mut LookupState,
    origin: &HfstOneLevelPath,
    cascade: &[HfstBasicTransducer],
    unknown: bool,
    infinite: &mut bool,
    out: &mut dyn Write,
) -> HfstOneLevelPaths {
    if !unknown {
        if cascade.len() == 1 {
            lookup_simple_basic(
                common,
                options,
                state,
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
            lookup_cascading_basic(common, options, state, origin, cascade, infinite, &mut *out)
        }
    } else {
        HfstOneLevelPaths::new()
    }
}

fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    inputstream: &mut HfstInputStream<'_>,
    outstream: &mut dyn Write,
) -> i32 {
    let mut state = LookupState::new();
    let mut stats = LookupStats::new();
    let mut cascade: Vec<OlTransducer> = Vec::new();
    // the type of the first transducer read (C: cascade[0].get_type()).
    let mut first_type = ImplementationType::UNSPECIFIED_TYPE;
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
        let trans = match inputstream.read() {
            Ok(v) => v,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let ty = trans.get_type();
        if transducer_n == 1 {
            first_type = ty;
        }
        let mut symbols_seen: StringSet = StringSet::new();

        // THFST is a member of the optimized-lookup family (weighted directory
        // format), so it counts as optimized-lookup here alongside HFST_OL/OLW.
        if ty != ImplementationType::HFST_OL_TYPE
            && ty != ImplementationType::HFST_OLW_TYPE
            && ty != ImplementationType::THFST_TYPE
        {
            only_optimized_lookup = false;
        }

        let mut inputname = trans.get_name();
        if inputname.is_empty() {
            inputname = common.input_filename.clone();
        }
        if transducer_n == 1 {
            verbose_print(common, &format!("Reading {}...\n", inputname));
        } else {
            verbose_print(
                common,
                &format!("Reading {}...{}\n", inputname, transducer_n),
            );
        }

        // add multicharacter symbols to mc_symbols
        if ty == ImplementationType::SFST_TYPE
            || ty == ImplementationType::TROPICAL_OPENFST_TYPE
            || ty == ImplementationType::FOMA_TYPE
        {
            // [spec:hfst:def:hfst-lookup.basic-fn]
            // [spec:hfst:sem:hfst-lookup.basic-fn]
            let basic = crate::for_any!(&trans, t => {
                match HfstBasicTransducer::try_from_transducer(t) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                }
            });
            for it in basic.iter() {
                for tr_it in it.iter() {
                    let mcs = tr_it.get_input_symbol(basic.coder());
                    symbols_seen.insert(mcs.clone());
                    if mcs == internal_unknown || mcs == internal_identity {
                        id_or_unk_seen = true;
                    }
                    if mcs.chars().count() > 1 {
                        mc_symbols.push(mcs.clone());
                        verbose_print(common, &format!("multicharacter symbol: {}\n", mcs));
                    }
                }
            }
            cascade_mut.push(basic);
            state.cascade_symbols_seen.push(symbols_seen);
            if id_or_unk_seen {
                state.cascade_unknown_or_identity_seen.push(true);
            } else {
                state.cascade_unknown_or_identity_seen.push(false);
            }
        }

        // one dispatch per read ([dec:hfst:monomorphic-backends]): the
        // OL variants carry the fast lookup path; the algebra variants
        // were already converted to the basic cascade above.
        match trans {
            hfst::hfst_transducer::AnyTransducer::OlW(t) => cascade.push(OlTransducer::W(t)),
            hfst::hfst_transducer::AnyTransducer::OlU(t) => cascade.push(OlTransducer::U(t)),
            hfst::hfst_transducer::AnyTransducer::Tropical(_) => {}
        }
        id_or_unk_seen = false;
    }

    inputstream.close();

    if !options.obey_flags
        && (inputstream.get_type() == ImplementationType::HFST_OL_TYPE
            || inputstream.get_type() == ImplementationType::HFST_OLW_TYPE
            || inputstream.get_type() == ImplementationType::THFST_TYPE)
    {
        hfst_error(
            common,
            1,
            0,
            "not obeying flags not supported on optimized lookup transducers",
        );
    }

    // if transducer type is other than optimized_lookup,
    // convert to HfstBasicTransducer
    let mut line: String;

    let epsilon_format = options.epsilon_format.clone();
    let input_tokenizer = match HfstStrings2FstTokenizer::new(&mc_symbols, &epsilon_format) {
        Ok(t) => t,
        Err(e) => {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    if !only_optimized_lookup && !common.silent {
        hfst_warning(
            common,
            0,
            0,
            &format!(
                "It is not possible to perform fast lookups with {} format automata.\n\
                 Using HFST basic transducer format and performing slow lookups",
                hfst_strformat(first_type)
            ),
        );
    }

    let mut filesize: i64 = -1;
    if options.show_progress_bar {
        eprint!("Counting file size...\n");
        // C: fseek(END)/ftell to measure, then rewind. The std reader is read
        // from the start, so the file's metadata length is the equivalent size
        // and no rewind is needed.
        if options.lookup_given {
            let lookup_file_name = options.lookup_file_name.clone();
            if let Ok(md) = std::fs::metadata(&lookup_file_name) {
                filesize = md.len() as i64;
            }
        }
        eprint!("{}... rewinding\n", filesize);
    }
    print_prompt(common, options);
    // C tracked the read position with ftell(LOOKUP_FILE); the std reader has no
    // tell, so accumulate the bytes consumed by read_until (the same cumulative
    // byte count getline+ftell would report).
    let mut filepos: i64 = 0;
    loop {
        // C: getline reads a raw line (bytes) then cstr does a lossy UTF-8
        // conversion. read_until(b'\n') mirrors getline's byte semantics.
        let mut raw_bytes: Vec<u8> = Vec::new();
        match options
            .lookup_reader
            .as_mut()
            .unwrap()
            .read_until(b'\n', &mut raw_bytes)
        {
            Ok(0) => break,
            Ok(n) => filepos += n as i64,
            Err(_) => break,
        }
        line = String::from_utf8_lossy(&raw_bytes).into_owned();

        options.linen += 1;

        // strip trailing '\n'/'\r' ('\r' is possible on Windows)
        if let Some(pos) = line.find(['\n', '\r']) {
            line.truncate(pos);
        }
        verbose_print(common, &format!("Looking up {}...\n", line));
        if options.show_progress_bar {
            if filesize != -1 {
                eprint!("{} / {}...\r", filepos, filesize);
            } else {
                eprint!("{} / ?...\r", options.linen);
            }
        }

        let mut markup = String::new();
        let mut unknown = false;
        let mut infinite = false;

        stats.inputs += 1;
        let kv = match parse_lookup_line(
            &mut line,
            &input_tokenizer,
            &mut markup,
            &mut unknown,
            only_optimized_lookup,
            options.input_format,
        ) {
            Ok(kv) => kv,
            Err(e) => {
                if e.kind == ErrorKind::IncorrectUtf8Coding {
                    hfst_error_at_line(
                        common,
                        1,
                        0,
                        &common.input_filename,
                        options.linen as u32,
                        e.message.as_deref().unwrap_or(""),
                    );
                }
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        if common.verbose {
            verbose_print(common, "Tokenized to: ");
            for s in kv.second.iter() {
                verbose_print(common, &format!("{} ", s));
            }
            verbose_print(common, "\n");
        }

        let kvs = if only_optimized_lookup {
            perform_lookups_ol(
                common,
                options,
                &state,
                &kv,
                &mut cascade,
                unknown,
                &mut infinite,
                &mut *outstream,
            )
        } else {
            perform_lookups_basic(
                common,
                options,
                &mut state,
                &kv,
                &cascade_mut,
                unknown,
                &mut infinite,
                &mut *outstream,
            )
        };

        if !options.print_pairs {
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
                options.formats(),
                &render_opts(options),
                &mut stats,
                &mut *outstream,
            ) {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            let _ = outstream.flush();
        }

        print_prompt(common, options);
    } // while lines in input

    if options.show_progress_bar {
        eprint!("{}/{}... Done\n", filepos, filesize);
    }

    if options.print_statistics {
        if let Err(e) = stats.write_statistics(&mut *outstream) {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }
    0
}

// [spec:hfst:def:hfst-lookup.main-fn]
// [spec:hfst:sem:hfst-lookup.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.6", "HfstLookup");
    let (common, mut options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    {
        let f = options.formats();
        verbose_print(
            &common,
            &format!(
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
                options.epsilon_format,
                options.space_format,
                options.show_flags as i32
            ),
        );
    }

    // here starts the buffer handling part
    // (C++ wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // emitting "%s is not a valid transducer file" is not reproduced here.)
    let mut instream = match if common.input_filename != "<stdin>" {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            hfst_error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-lookup: cannot open output: {e}");
            return 1;
        }
    };
    process_stream(&common, &mut options, &mut instream, &mut *out);
    let _ = out.flush();

    // (free(inputfilename)/free(outfilename) in C++ are no-ops here.)
    0
}
