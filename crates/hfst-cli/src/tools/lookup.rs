//! Faithful 1:1 port of tools/src/hfst-lookup.cc — the transducer lookup
//! (apply) command-line tool. Lookup is done from left to right (as opposed to
//! xfst and foma, which look up from right to left; for that behaviour use
//! hfst-flookup). Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, inc fragments).
//!
//! This is a unary tool (#includes inc/globals-unary.h, getopt-cases-unary.h,
//! check-params-unary.h); it mirrors hfst-invert's option-parsing skeleton and
//! adds the tool-specific options.
//!
//! The lookup machinery itself is not here: it lives in the library, as
//! [`hfst::lookup_driver`] (the cascade and the lookup paths) and
//! [`hfst::hfst_lookup_format`] (the output templates and the input parser),
//! shared with hfst-flookup. What this file keeps is the driving: option
//! parsing, the writers, the interactive prompt and the stdin loop, plus the
//! [`LookupEngineOptions`] that select hfst-lookup's dialect of the engine.

use crate::CliLookupReporter;
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
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_lookup_format::{
    CascadeVariant, LookupFormats, LookupInputFormat, LookupOutputFormat, LookupRenderOptions,
    LookupStats, parse_lookup_line, print_lookups,
};
use hfst::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use hfst::lookup_driver::{
    AmbiguityLimit, FlagPolicy, LookupCascade, LookupEngineOptions, PairPrintStyle,
    is_optimized_lookup_type,
};
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

// [spec:hfst:req:cli.help]
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
    let _ = writeln!(msg);
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
    let _ = writeln!(msg);

    let _ = msg.write_all(
        "CASCADE must be one of { union, priority-union, composition }.\n\
         If not specified, defaults to {union}.\n"
            .as_bytes(),
    );
    let _ = writeln!(msg);

    let _ = msg.write_all(
        "STREAM can be { input, output, both }. If not given, defaults to {both}.\n\
         If input file is not specified with -I, input is read interactively line by\n\
         line from the user. If you redirect input from a file, use --pipe-mode=input.\n\
         --pipe-mode=output is ignored on non-windows platforms.\n"
            .as_bytes(),
    );
    let _ = writeln!(msg);

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

    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-lookup.parse-options-fn]
// [spec:hfst:sem:hfst-lookup.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
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
                    eprintln!("Invalid argument for --beam");
                    return Err(1);
                }
            }
            b't' => {
                options.time_cutoff = optarg.parse::<f64>().unwrap_or(0.0);
                if options.time_cutoff < 0.0 {
                    eprintln!("Invalid argument for --time-cutoff");
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
                if opt.optarg_opt().is_none() || optarg == "both" || optarg == "BOTH" {
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

/// The engine knobs for the library lookup driver, snapshotted from the tool's
/// option state. hfst-lookup obeys flags inside the lookup itself, bounds an
/// infinitely ambiguous lookup by result count, and lays print-pairs out on
/// the result stream.
fn engine_opts(options: &Options) -> LookupEngineOptions {
    LookupEngineOptions {
        obey_flags: options.obey_flags,
        show_flags: options.show_flags,
        print_pairs: options.print_pairs,
        print_space: options.print_space,
        quote_special: options.quote_special,
        epsilon_format: options.epsilon_format.clone(),
        beam: options.beam,
        time_cutoff: options.time_cutoff,
        infinite_cutoff: options.infinite_cutoff,
        cascade: options.cascade,
        flags: FlagPolicy::InLookup,
        ambiguity: AmbiguityLimit::MaxResults {
            max_number: options.max_number,
            default_max: DEFAULT_MAX_NUMBER,
        },
        pair_style: PairPrintStyle::Lookup,
    }
}

fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    inputstream: &mut HfstInputStream<'_>,
    outstream: &mut dyn Write,
) -> i32 {
    let reporter = CliLookupReporter::new(common);
    let mut stats = LookupStats::new();
    let mut cascade = LookupCascade::new();

    while inputstream.is_good() {
        // [spec:hfst:def:hfst-lookup.trans-fn]
        // [spec:hfst:sem:hfst-lookup.trans-fn]
        let trans = match inputstream.read() {
            Ok(v) => v,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        cascade.begin_transducer(&trans, &common.input_filename, &reporter);
        if let Err(e) = cascade.push_transducer(trans, &reporter) {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }

    inputstream.close();

    if !options.obey_flags && is_optimized_lookup_type(inputstream.get_type()) {
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
    let input_tokenizer =
        match HfstStrings2FstTokenizer::new(cascade.multichar_symbols(), &epsilon_format) {
            Ok(t) => t,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

    let only_optimized_lookup = cascade.only_optimized_lookup();
    if !only_optimized_lookup && !common.silent {
        hfst_warning(
            common,
            0,
            0,
            &format!(
                "It is not possible to perform fast lookups with {} format automata.\n\
                 Using HFST basic transducer format and performing slow lookups",
                hfst_strformat(cascade.first_type())
            ),
        );
    }

    let engine = engine_opts(options);

    let mut filesize: i64 = -1;
    if options.show_progress_bar {
        eprintln!("Counting file size...");
        // C: fseek(END)/ftell to measure, then rewind. The std reader is read
        // from the start, so the file's metadata length is the equivalent size
        // and no rewind is needed.
        if options.lookup_given {
            let lookup_file_name = options.lookup_file_name.clone();
            if let Ok(md) = std::fs::metadata(&lookup_file_name) {
                filesize = md.len() as i64;
            }
        }
        eprintln!("{}... rewinding", filesize);
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

        // hfst-lookup prints a print-pairs input form on the result stream
        // itself, so the engine's message-stream echo is never used here.
        let (kvs, infinite) = cascade.perform_lookups(
            &kv,
            unknown,
            &engine,
            &reporter,
            &mut *outstream,
            &mut std::io::sink(),
        );

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
        eprintln!("{}/{}... Done", filepos, filesize);
    }

    if options.print_statistics
        && let Err(e) = stats.write_statistics(&mut *outstream)
    {
        hfst_error(common, 1, 0, &format!("{e}"));
        return 1;
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
