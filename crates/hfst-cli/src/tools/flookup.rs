//! Faithful 1:1 port of tools/src/hfst-flookup.cc — the transducer lookup
//! (apply) command-line tool. Lookup is done right to left, like flookup of
//! foma and lookup of xfst. Drives the hfst-cli foundation (getopt,
//! commandline, program-options, inc fragments).
//!
//! This is a unary tool (#includes inc/globals-unary.h, getopt-cases-unary.h,
//! check-params-unary.h); it mirrors hfst-invert's option-parsing skeleton and
//! adds the tool-specific options. Following the de-globalized contract, the
//! tool's state lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields)
//! and a tool-local [`Options`] — both built by `parse_options` and threaded
//! into the processing functions. There are no `static mut` globals and no
//! `unsafe`.
//!
//! The lookup machinery itself is not here: it lives in the library, as
//! [`hfst::lookup_driver`] (the cascade and the lookup paths) and
//! [`hfst::hfst_lookup_format`] (the output templates and the input parser),
//! shared with hfst-lookup. What this file keeps is the driving: option
//! parsing, the `-R`/`-f` inversion of each transducer as it is read, the
//! writers, the interactive prompt and the stdin loop, plus the
//! [`LookupEngineOptions`] that select hfst-flookup's dialect of the engine.

use crate::CliLookupReporter;
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    convert_any, extend_options_from_env, hfst_error, hfst_error_at_line, hfst_set_program_name,
    hfst_strformat, hfst_warning, verbose_print,
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
use hfst::hfst_data_types::ImplementationType;
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
// tool-specific state (the C++ file's static variables, now tool-local)
// ---------------------------------------------------------------------------

/// hfst-flookup's own options + runtime state (the former tool-specific
/// `static mut`s). Options set in `parse_options`; the line counter and the
/// statistics counters are updated in `process_stream`.
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
    beam: f32,
    invert: bool,
    // accept also ol transducers when -R is not specified inverting is slow then
    force_ol: bool,

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

    // statistic counting
    stats: LookupStats,
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
            beam: -1.0,
            invert: false,
            force_ol: false,
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
            stats: LookupStats::new(),
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
         Perform transducer lookup (apply). Lookup is done from right to left,\n\
         in the same way as in flookup of foma and lookup of xfst.\n\
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
    let _ = writeln!(msg);
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
    let _ = writeln!(msg);

    let _ = write!(
        msg,
        "STREAM can be {{ input, output, both }}. If not given, defaults to {{both}}.\n\
         If input file is not specified with -I, input is read interactively line by\n\
         line from the user. If you redirect input from a file, use --pipe-mode=input.\n\
         --pipe-mode=output is ignored on non-windows platforms.\n"
    );
    let _ = writeln!(msg);

    let _ = write!(
        msg,
        "Known bugs:\n\
         \x20 * 'quote-special' quotes spaces that come from 'print-space'\n\
         \x20 * optimized lookup transducers are unidirectional and only support lookdown,\n\
         \x20   --force-ol forces inversion but is slow\n"
    );

    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-flookup.parse-options-fn]
// [spec:hfst:sem:hfst-flookup.parse-options-fn]
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
            b'R' => {
                options.invert = true;
            }
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
                // NOTE: C++ falls through from 'X' into 'c' (no break).
                options.infinite_cutoff = optarg.parse::<i32>().unwrap_or(0) as usize;
            }
            b'c' => {
                options.infinite_cutoff = optarg.parse::<i32>().unwrap_or(0) as usize;
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
            b'f' => {
                options.force_ol = true;
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

// [spec:hfst:def:hfst-flookup.print-prompt-fn]
// [spec:hfst:sem:hfst-flookup.print-prompt-fn]
fn print_prompt(common: &CommonOptions, options: &Options) {
    if !common.silent && !options.pipe_input && !options.lookup_given {
        eprint!("> ");
    }
}

// The renderer knobs for the library %-template engine, snapshotted from the
// tool's options.
fn render_opts(options: &Options) -> LookupRenderOptions {
    LookupRenderOptions {
        epsilon_format: options.epsilon_format.clone(),
        space_format: options.space_format.clone(),
        print_space: options.print_space,
        show_flags: options.show_flags,
        quote_special: options.quote_special,
        // hfst-flookup puts an unsplittable lookup form in %a
        unsplit_to_base: false,
        beam: options.beam,
    }
}

/// The engine knobs for the library lookup driver, snapshotted from the tool's
/// options. hfst-flookup validates flags after the lookup instead of inside
/// it, bounds an infinitely ambiguous lookup by epsilon cycles, always unions
/// a multi-transducer cascade, and lays print-pairs out xerox-style with the
/// input form echoed on the message stream.
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
        cascade: CascadeVariant::Union,
        flags: FlagPolicy::PostFilter,
        ambiguity: AmbiguityLimit::Cycles,
        pair_style: PairPrintStyle::Flookup,
    }
}

fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    inputstream: &mut HfstInputStream<'_>,
    outstream: &mut dyn Write,
) -> i32 {
    let reporter = CliLookupReporter::new(common);
    let mut cascade = LookupCascade::new();

    while inputstream.is_good() {
        // [spec:hfst:def:hfst-flookup.trans-fn]
        // [spec:hfst:sem:hfst-flookup.trans-fn]
        let mut trans = match inputstream.read() {
            Ok(t) => t,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let ty = trans.get_type();
        if is_optimized_lookup_type(ty) && !options.invert && !options.force_ol {
            hfst_error(
                common,
                1,
                0,
                "lookup not supported for optimized lookup transducers: convert to openfst format,\n\
                 invert, and convert back to optimized lookup format or specify --force-ol\n",
            );
        }

        cascade.begin_transducer(&trans, &common.input_filename, &reporter);

        if !options.invert {
            if !is_optimized_lookup_type(ty) {
                crate::for_algebra!(&mut trans, t => {
                    if let Err(e) = t.invert() {
                        hfst_error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                }, else => {
                    unreachable!("non-OL stream type paired with an OL transducer")
                });
            } else {
                // the C++ convert / invert / convert round-trip, as typed
                // conversions at this boundary
                trans = match convert_any(trans, ImplementationType::TROPICAL_OPENFST_TYPE) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if let hfst::hfst_transducer::AnyTransducer::Tropical(t) = &mut trans
                    && let Err(e) = t.invert()
                {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                trans = match convert_any(trans, ty) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
            }
        }

        if let Err(e) = cascade.push_transducer(trans, &reporter) {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }

    inputstream.close();

    if options.print_pairs && is_optimized_lookup_type(inputstream.get_type()) {
        hfst_error(
            common,
            1,
            0,
            "pair printing not supported on optimized lookup transducers",
        );
    }

    let mut line: String;

    let input_tokenizer = match HfstStrings2FstTokenizer::new(
        cascade.multichar_symbols(),
        &options.epsilon_format.clone(),
    ) {
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
        if options.lookup_given
            && let Ok(md) = std::fs::metadata(options.lookup_file_name.clone())
        {
            filesize = md.len() as i64;
        }
        eprintln!("{}... rewinding", filesize);
    }
    print_prompt(common, options);
    // C tracked the read position with ftell(LOOKUP_FILE); the std reader has no
    // tell, so accumulate the bytes consumed by read_line (the same cumulative
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

        // strip trailing '\n'/'\r'
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

        options.stats.inputs += 1;
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

        // hfst-flookup echoes a print-pairs input form on the message stream
        // while the pairs go to the result stream.
        let (kvs, infinite) = cascade.perform_lookups(
            &kv,
            unknown,
            &engine,
            &reporter,
            &mut *outstream,
            &mut std::io::stderr(),
        );

        if !options.print_pairs {
            // printing was already done in function lookup_fd
            let markup_opt = if markup.is_empty() {
                None
            } else {
                Some(markup.as_str())
            };
            let formats = options.formats().clone();
            let render = render_opts(options);
            if let Err(e) = print_lookups(
                &kvs,
                &kv,
                markup_opt,
                unknown,
                infinite,
                &formats,
                &render,
                &mut options.stats,
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
        && let Err(e) = options.stats.write_statistics(&mut *outstream)
    {
        hfst_error(common, 1, 0, &format!("{e}"));
        return 1;
    }
    0
}

// [spec:hfst:def:hfst-flookup.main-fn]
// [spec:hfst:sem:hfst-flookup.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.6", "HfstFlookup");
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
        Ok(s) => s,
        Err(e) => {
            hfst_error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-flookup: cannot open output: {e}");
            return 1;
        }
    };
    process_stream(&common, &mut options, &mut instream, &mut *out);
    let _ = out.flush();

    // (free(inputfilename)/free(outfilename) in C++ are no-ops here.)
    0
}
