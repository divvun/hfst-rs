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
use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    convert_any, hfst_error, hfst_error_at_line, hfst_set_program_name, hfst_strformat,
    hfst_warning, verbose_print,
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

/// hfst-flookup's command line.
// [spec:hfst:def:hfst-flookup.parse-options-fn]
// [spec:hfst:sem:hfst-flookup.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Perform transducer lookup (apply). Lookup is done from right to left,\n\
             in the same way as in flookup of foma and lookup of xfst.",
    after_help = "OFORMAT is one of {xerox,cg,apertium}, xerox being default
IFORMAT is one of {text,spaced,apertium}, default being text,
unless OFORMAT is apertium
VARIABLEs relevant to lookup are {print-pairs,print-space,
quote-special,show-flags,obey-flags}
Input epsilon cycles are followed by default INT=5 times.
Epsilon is printed by default as an empty string.
B must be a non-negative float.
S must be a non-negative float. The default, 0.0, indicates no cutoff.
If the input contains several transducers, a set containing
results from all transducers is printed for each input string.

STREAM can be { input, output, both }. If not given, defaults to {both}.
If input file is not specified with -I, input is read interactively line by
line from the user. If you redirect input from a file, use --pipe-mode=input.
--pipe-mode=output is ignored on non-windows platforms.

Known bugs:
  * 'quote-special' quotes spaces that come from 'print-space'
  * optimized lookup transducers are unidirectional and only support lookdown,
    --force-ol forces inversion but is slow"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Do lookdown instead of lookup
    #[arg(short = 'R', long = "invert")]
    invert: bool,

    /// Read lookup strings from SFILE
    #[arg(
        short = 'I',
        long = "input-strings",
        value_name = "SFILE",
        allow_hyphen_values = true
    )]
    input_strings: Option<String>,

    /// Use OFORMAT printing results sets
    #[arg(
        short = 'O',
        long = "output-format",
        value_name = "OFORMAT",
        allow_hyphen_values = true
    )]
    output_format: Option<String>,

    /// Use IFORMAT parsing input
    #[arg(
        short = 'F',
        long = "input-format",
        value_name = "IFORMAT",
        allow_hyphen_values = true
    )]
    input_format: Option<String>,

    /// Print epsilon as EPS
    #[arg(
        short = 'e',
        long = "epsilon-format",
        value_name = "EPS",
        allow_hyphen_values = true
    )]
    epsilon_format: Option<String>,

    /// Alias of --epsilon-format
    #[arg(
        short = 'E',
        long = "epsilon-format2",
        value_name = "EPS",
        allow_hyphen_values = true
    )]
    epsilon_format2: Option<String>,

    /// Print statistics
    #[arg(short = 'x', long = "statistics")]
    statistics: bool,

    /// Toggle xfst VARIABLE
    #[arg(
        short = 'X',
        long = "xfst",
        value_name = "VARIABLE",
        action = clap::ArgAction::Append,
        allow_hyphen_values = true
    )]
    xfst: Vec<String>,

    /// How many times to follow input epsilon cycles
    #[arg(
        short = 'c',
        long = "cycles",
        value_name = "INT",
        allow_hyphen_values = true
    )]
    cycles: Option<String>,

    /// Output only analyses whose weight is within B from the best analysis
    #[arg(
        short = 'b',
        long = "beam",
        value_name = "B",
        allow_hyphen_values = true
    )]
    beam: Option<String>,

    /// Limit search after having used S seconds per input (currently only
    /// works in optimized-lookup mode)
    #[arg(
        short = 't',
        long = "time-cutoff",
        value_name = "S",
        allow_hyphen_values = true
    )]
    time_cutoff: Option<String>,

    /// Control input and output streams
    #[arg(
        short = 'p',
        long = "pipe-mode",
        value_name = "STREAM",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "both",
        action = clap::ArgAction::Append
    )]
    pipe_mode: Vec<String>,

    /// Show neat progress bar if possible
    #[arg(short = 'P', long = "progress")]
    progress: bool,

    /// Force lookup of optimized lookup transducers (slow)
    #[arg(short = 'f', long = "force-ol")]
    force_ol: bool,

    /// The tool-specific option occurrences in command-line order: the C
    /// loop's arms overwrite shared settings ('-O apertium' and '-F' both
    /// write the input format; '-e'/'-E' both write the epsilon format; the
    /// 'X' arm FALLS THROUGH into 'c' and overwrites the cycle cutoff), so
    /// the LAST writer wins and the diagnostics fire in occurrence order.
    #[arg(skip)]
    events: Vec<Event>,
}

/// One checked iteration of the C option loop, in occurrence order.
#[derive(Clone, Copy)]
enum Event {
    OutputFormat,
    InputFormat,
    EpsilonFormat,
    EpsilonFormat2,
    Beam,
    TimeCutoff,
    /// Index into the `xfst` occurrence vector.
    Xfst(usize),
    Cycles,
    /// Index into the `pipe_mode` occurrence vector.
    PipeMode(usize),
}

impl Args {
    /// Replay the C option loop over the ordered occurrences. Every rejection
    /// here is fatal (hfst_error with a nonzero status exits), so re-running
    /// the replay after a successful validate prints nothing.
    fn resolve(&self, common: &CommonOptions) -> Result<Options, i32> {
        let mut options = Options {
            invert: self.invert,
            force_ol: self.force_ol,
            print_statistics: self.statistics,
            show_progress_bar: self.progress,
            ..Options::default()
        };
        for event in &self.events {
            match event {
                Event::OutputFormat => {
                    let optarg = self.output_format.as_deref().unwrap_or_default();
                    if optarg == "xerox" {
                        options.output_format = LookupOutputFormat::XeroxOutput;
                    } else if optarg == "cg" {
                        options.output_format = LookupOutputFormat::CgOutput;
                    } else if optarg == "apertium" {
                        options.output_format = LookupOutputFormat::ApertiumOutput;
                        options.input_format = LookupInputFormat::ApertiumInput;
                    } else {
                        hfst_error(
                            common,
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
                Event::InputFormat => {
                    let optarg = self.input_format.as_deref().unwrap_or_default();
                    if optarg == "text" {
                        options.input_format = LookupInputFormat::Utf8TokenInput;
                    } else if optarg == "spaced" {
                        options.input_format = LookupInputFormat::SpaceSeparatedTokenInput;
                    } else if optarg == "apertium" {
                        options.input_format = LookupInputFormat::ApertiumInput;
                    } else {
                        hfst_error(
                            common,
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
                Event::EpsilonFormat => {
                    options.epsilon_format = self.epsilon_format.clone().unwrap_or_default();
                }
                Event::EpsilonFormat2 => {
                    options.epsilon_format = self.epsilon_format2.clone().unwrap_or_default();
                }
                Event::Beam => {
                    let optarg = self.beam.as_deref().unwrap_or_default();
                    options.beam = optarg.parse::<f32>().unwrap_or(0.0);
                    if options.beam < 0.0 {
                        eprintln!("Invalid argument for --beam");
                        return Err(1);
                    }
                }
                Event::TimeCutoff => {
                    let optarg = self.time_cutoff.as_deref().unwrap_or_default();
                    options.time_cutoff = optarg.parse::<f64>().unwrap_or(0.0);
                    if options.time_cutoff < 0.0 {
                        eprintln!("Invalid argument for --time-cutoff");
                        return Err(1);
                    }
                }
                Event::Xfst(k) => {
                    let optarg = self.xfst[*k].as_str();
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
                            common,
                            1,
                            0,
                            &format!("Xfst variable {} unrecognised", optarg),
                        );
                        return Err(1);
                    }
                    // NOTE: C++ falls through from 'X' into 'c' (no break).
                    options.infinite_cutoff = optarg.parse::<i32>().unwrap_or(0) as usize;
                }
                Event::Cycles => {
                    let optarg = self.cycles.as_deref().unwrap_or_default();
                    options.infinite_cutoff = optarg.parse::<i32>().unwrap_or(0) as usize;
                }
                Event::PipeMode(k) => {
                    let optarg = self.pipe_mode[*k].as_str();
                    if optarg == "both" || optarg == "BOTH" {
                        options.pipe_input = true;
                        options.pipe_output = true;
                    } else if optarg == "input"
                        || optarg == "INPUT"
                        || optarg == "in"
                        || optarg == "IN"
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
                            common,
                            1,
                            0,
                            &format!("--pipe-mode argument {} unrecognised", optarg),
                        );
                        return Err(1);
                    }
                }
            }
        }

        options.formats = Some(LookupFormats::for_output_format(options.output_format));

        if let Some(name) = &self.input_strings {
            options.lookup_file_name = name.clone();
            // C: lookup_file = fopen(lookup_file_name, "r"); open the named
            // file as a buffered std reader instead, erroring on failure the
            // way hfst_fopen did.
            match std::fs::File::open(name) {
                Ok(f) => options.lookup_reader = Some(Box::new(std::io::BufReader::new(f))),
                Err(_) => {
                    hfst_error(common, 1, 0, &format!("Could not open '{}'", name));
                    return Err(1);
                }
            }
            options.lookup_given = true;
        } else {
            options.lookup_reader = Some(Box::new(std::io::BufReader::new(std::io::stdin())));
            options.lookup_file_name = "<stdin>".to_string();
        }
        Ok(options)
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
        let ids: &[(&str, Event)] = &[
            ("output_format", Event::OutputFormat),
            ("input_format", Event::InputFormat),
            ("epsilon_format", Event::EpsilonFormat),
            ("epsilon_format2", Event::EpsilonFormat2),
            ("beam", Event::Beam),
            ("time_cutoff", Event::TimeCutoff),
            ("cycles", Event::Cycles),
        ];
        let mut ordered: Vec<(usize, Event)> = ids
            .iter()
            .filter(|(id, _)| {
                matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine)
            })
            .filter_map(|(id, event)| matches.index_of(id).map(|i| (i, *event)))
            .collect();
        for (id, make) in [
            ("xfst", Event::Xfst as fn(usize) -> Event),
            ("pipe_mode", Event::PipeMode as fn(usize) -> Event),
        ] {
            if matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine)
                && let Some(indices) = matches.indices_of(id)
            {
                for (k, i) in indices.enumerate() {
                    ordered.push((i, make(k)));
                }
            }
        }
        ordered.sort_by_key(|(i, _)| *i);
        self.events = ordered.into_iter().map(|(_, event)| event).collect();
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The vocabulary rejections happened inside the C loop, before the
        // parameter checks.
        self.resolve(opts)?;
        Ok(())
    }
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
            .expect("lookup reader is initialised in resolve")
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
pub fn run(args: Vec<String>) -> i32 {
    cli::exit_code(execute(args))
}

fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.6", "HfstFlookup");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = args.resolve(&common)?;

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
            return Err(1);
        }
    };

    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-flookup: cannot open output: {e}");
            return Err(1);
        }
    };
    process_stream(&common, &mut options, &mut instream, &mut *out);
    let _ = out.flush();

    // (free(inputfilename)/free(outfilename) in C++ are no-ops here.)
    Ok(())
}
