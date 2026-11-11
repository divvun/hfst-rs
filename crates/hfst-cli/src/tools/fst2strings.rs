//! Faithful 1:1 port of tools/src/hfst-fst2strings.cc — the transducer path
//! printing command-line tool. Option handling is clap 4 derive through
//! [`crate::cli`]; the value-checked options replay in command-line order so
//! the diagnostics keep the C getopt loop's sequencing.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, parse_u64, verbose_print, warning};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::tools::fst2strings::{
    Fst2StringsError, Fst2StringsProgress, PathFormat, PathQuery, extract_paths, extract_paths_ol,
    write_path,
};

/// hfst-fst2strings's own options (the former tool-specific `static mut`s).
struct Options {
    /// the maximum number of strings printed for each transducer
    max_strings: i32,
    cycles: i32,
    nbest_strings: i32,
    max_random_strings: i32,
    beam: f32,
    display_weights: bool,
    eval_fd: bool,
    filter_fd: bool,
    quote_special: bool,
    print_spaces: bool,
    max_input_length: u32,
    max_output_length: u32,
    input_prefix: String,
    output_prefix: String,
    input_exclude: String,
    output_exclude: String,
    print_in_pairstring_format: bool,
    epsilon_format: String,
    print_separator_after_each_transducer: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            max_strings: 0,
            cycles: -1,
            nbest_strings: -1,
            max_random_strings: -1,
            beam: -1.0,
            display_weights: false,
            eval_fd: false,
            filter_fd: true,
            quote_special: false,
            print_spaces: false,
            max_input_length: 0,
            max_output_length: 0,
            input_prefix: String::new(),
            output_prefix: String::new(),
            input_exclude: String::new(),
            output_exclude: String::new(),
            print_in_pairstring_format: false,
            epsilon_format: String::new(),
            print_separator_after_each_transducer: false,
        }
    }
}

/// hfst-fst2strings's command line.
// [spec:hfst:def:hfst-fst2strings.parse-options-fn]
// [spec:hfst:sem:hfst-fst2strings.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Display the strings recognized by a transducer",
    after_help = "If all NSTR, NBEST and NCYC are omitted, all possible paths are printed:
NSTR, NBEST and NCYC default to infinity.
NBEST overrides NSTR and NCYC
NRAND overrides NBEST, NSTR and NCYC
B must be a non-negative float
If EPS is not given, default is empty string.
Numeric options are parsed with strtod(3).
Xfst variables supported are { obey-flags, print-flags,
print-pairs, print-space, quote-special }.

Examples:
  hfst-fst2strings lexical.hfst    generates all forms of lexical.hfst
  hfst-fst2strings -P \"cat<n>\" -c 0 lexical.hfst
                   generates paradigm for cat<n> without following cycles

Known bugs:
  Does not work correctly for hfst optimized lookup format."
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// print at most NSTR strings
    #[arg(
        short = 'n',
        long = "max-strings",
        value_name = "NSTR",
        allow_hyphen_values = true
    )]
    max_strings: Option<String>,

    /// print at most NBEST best strings
    #[arg(
        short = 'N',
        long = "nbest",
        value_name = "NBEST",
        allow_hyphen_values = true
    )]
    nbest: Option<String>,

    /// print at most NRAND random strings
    #[arg(
        short = 'r',
        long = "random",
        value_name = "NRAND",
        allow_hyphen_values = true
    )]
    random: Option<String>,

    /// follow cycles at most NCYC times
    #[arg(
        short = 'c',
        long = "cycles",
        value_name = "NCYC",
        allow_hyphen_values = true
    )]
    cycles: Option<String>,

    /// display the weight for each string
    #[arg(short = 'w', long = "print-weights")]
    print_weights: bool,

    /// print separator "--" after each transducer
    #[arg(short = 'S', long = "print-separator")]
    print_separator: bool,

    /// print epsilon as EPS
    #[arg(
        short = 'e',
        long = "epsilon-format",
        value_name = "EPS",
        allow_hyphen_values = true
    )]
    epsilon_format: Option<String>,

    /// toggle xfst compatibility option VARIABLE
    #[arg(
        short = 'X',
        long = "xfst",
        value_name = "VARIABLE",
        action = clap::ArgAction::Append,
        allow_hyphen_values = true
    )]
    xfst: Vec<String>,

    /// reject output string with weight more than B away from the weight of
    /// the best output string
    #[arg(
        short = 'b',
        long = "beam",
        value_name = "B",
        allow_hyphen_values = true
    )]
    beam: Option<String>,

    /// reject input string longer than MIL
    #[arg(
        short = 'l',
        long = "max-in-length",
        value_name = "MIL",
        allow_hyphen_values = true
    )]
    max_in_length: Option<String>,

    /// reject output string longer than MOL
    #[arg(
        short = 'L',
        long = "max-out-length",
        value_name = "MOL",
        allow_hyphen_values = true
    )]
    max_out_length: Option<String>,

    /// input string must begin with IPREFIX
    #[arg(
        short = 'p',
        long = "in-prefix",
        value_name = "IPREFIX",
        allow_hyphen_values = true
    )]
    in_prefix: Option<String>,

    /// output string must begin with OPREFIX
    #[arg(
        short = 'P',
        long = "out-prefix",
        value_name = "OPREFIX",
        allow_hyphen_values = true
    )]
    out_prefix: Option<String>,

    /// input string must not contain IXSTR
    #[arg(
        short = 'u',
        long = "in-exclude",
        value_name = "IXSTR",
        allow_hyphen_values = true
    )]
    in_exclude: Option<String>,

    /// output string must not contain OXSTR
    #[arg(
        short = 'U',
        long = "out-exclude",
        value_name = "OXSTR",
        allow_hyphen_values = true
    )]
    out_exclude: Option<String>,

    /// The checked option occurrences in command-line order: the C loop
    /// validated each value (and printed the non-fatal --xfst diagnostic) as
    /// it was scanned, so the diagnostics have to replay in that order.
    #[arg(skip)]
    events: Vec<Event>,
}

/// One value-checked iteration of the C option loop, in occurrence order.
#[derive(Clone, Copy)]
enum Event {
    MaxStrings,
    Nbest,
    Random,
    Beam,
    Cycles,
    MaxInLength,
    MaxOutLength,
    /// Index into the `xfst` occurrence vector.
    Xfst(usize),
}

impl Args {
    /// Replay the checked occurrences into the tool options. `print` guards
    /// the non-fatal --xfst diagnostic so the second (post-validate) pass
    /// does not repeat it.
    fn resolve(&self, common: &CommonOptions, print: bool) -> Result<Options, i32> {
        let mut options = Options::default();
        for event in &self.events {
            match event {
                Event::MaxStrings => {
                    let text = self.max_strings.as_deref().unwrap_or_default();
                    options.max_strings = parse_u64(common, text, 10) as i32;
                }
                Event::Nbest => {
                    let text = self.nbest.as_deref().unwrap_or_default();
                    options.nbest_strings = parse_u64(common, text, 10) as i32;
                }
                Event::Random => {
                    let text = self.random.as_deref().unwrap_or_default();
                    options.max_random_strings = parse_u64(common, text, 10) as i32;
                }
                Event::Beam => {
                    let text = self.beam.as_deref().unwrap_or_default();
                    options.beam = text.trim().parse::<f32>().unwrap_or(0.0);
                    if options.beam < 0.0 {
                        eprintln!("Invalid argument for --beam");
                        return Err(1);
                    }
                }
                Event::Cycles => {
                    let text = self.cycles.as_deref().unwrap_or_default();
                    options.cycles = parse_u64(common, text, 10) as i32;
                }
                Event::MaxInLength => {
                    let text = self.max_in_length.as_deref().unwrap_or_default();
                    options.max_input_length = parse_u64(common, text, 10) as u32;
                }
                Event::MaxOutLength => {
                    let text = self.max_out_length.as_deref().unwrap_or_default();
                    options.max_output_length = parse_u64(common, text, 10) as u32;
                }
                Event::Xfst(k) => {
                    let optarg = self.xfst[*k].as_str();
                    if optarg == "obey-flags" {
                        options.eval_fd = true;
                    } else if optarg == "print-flags" {
                        options.filter_fd = false;
                    } else if optarg == "quote-special" {
                        options.quote_special = true;
                    } else if optarg == "print-pairs" {
                        options.print_in_pairstring_format = true;
                    } else if optarg == "print-space" {
                        options.print_spaces = true;
                    } else if print {
                        error(
                            common,
                            0,
                            1,
                            "Unrecognised xfst option. available options are obey-flags, print-flags\n",
                        );
                    }
                }
            }
        }
        options.display_weights = self.print_weights;
        options.print_separator_after_each_transducer = self.print_separator;
        if let Some(prefix) = &self.in_prefix {
            options.input_prefix = prefix.clone();
        }
        if let Some(prefix) = &self.out_prefix {
            options.output_prefix = prefix.clone();
        }
        if let Some(exclude) = &self.in_exclude {
            options.input_exclude = exclude.clone();
        }
        if let Some(exclude) = &self.out_exclude {
            options.output_exclude = exclude.clone();
        }
        if let Some(eps) = &self.epsilon_format {
            options.epsilon_format = eps.clone();
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
            ("max_strings", Event::MaxStrings),
            ("nbest", Event::Nbest),
            ("random", Event::Random),
            ("beam", Event::Beam),
            ("cycles", Event::Cycles),
            ("max_in_length", Event::MaxInLength),
            ("max_out_length", Event::MaxOutLength),
        ];
        let mut ordered: Vec<(usize, Event)> = ids
            .iter()
            .filter(|(id, _)| {
                matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine)
            })
            .filter_map(|(id, event)| matches.index_of(id).map(|i| (i, *event)))
            .collect();
        if matches.value_source("xfst") == Some(clap::parser::ValueSource::CommandLine)
            && let Some(indices) = matches.indices_of("xfst")
        {
            for (k, i) in indices.enumerate() {
                ordered.push((i, Event::Xfst(k)));
            }
        }
        ordered.sort_by_key(|(i, _)| *i);
        self.events = ordered.into_iter().map(|(_, event)| event).collect();
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The value rejections (and the non-fatal --xfst diagnostic) happened
        // inside the C loop, before the parameter checks.
        self.resolve(opts, true)?;
        Ok(())
    }
}

impl Options {
    fn query(&self) -> PathQuery {
        PathQuery {
            max_strings: self.max_strings,
            nbest: self.nbest_strings,
            random: self.max_random_strings,
            cycles: self.cycles,
            beam: self.beam,
            obey_flags: self.eval_fd,
            show_flags: !self.filter_fd,
            max_in_length: self.max_input_length,
            max_out_length: self.max_output_length,
            in_prefix: self.input_prefix.clone(),
            out_prefix: self.output_prefix.clone(),
            in_exclude: self.input_exclude.clone(),
            out_exclude: self.output_exclude.clone(),
        }
    }

    fn format(&self) -> PathFormat {
        PathFormat {
            print_weights: self.display_weights,
            print_pairs: self.print_in_pairstring_format,
            print_spaces: self.print_spaces,
            quote_special: self.quote_special,
            epsilon_format: self.epsilon_format.clone(),
            show_flags: !self.filter_fd,
        }
    }
}

/// Print the search's progress as the tool's verbose lines.
fn report_progress(common: &CommonOptions, progress: Fst2StringsProgress) {
    match progress {
        Fst2StringsProgress::FindingBestWeight => {
            verbose_print(common, "Finding the weight of the best path...\n")
        }
        Fst2StringsProgress::Pruning(n) => verbose_print(
            common,
            &format!("Pruning transducer to {} best path(s)...\n", n),
        ),
        Fst2StringsProgress::Finding {
            max_strings,
            random,
        } => {
            if max_strings > 0 {
                verbose_print(
                    common,
                    &format!("Finding at most {} path(s)...\n", max_strings),
                );
            } else if random > 0 {
                verbose_print(
                    common,
                    &format!("Finding at most {} random path(s)...\n", random),
                );
            } else {
                verbose_print(common, "Finding strings...\n");
            }
        }
        Fst2StringsProgress::Found {
            count,
            random: false,
        } => verbose_print(common, &format!("Printed {} string(s)\n", count)),
        Fst2StringsProgress::Found {
            count,
            random: true,
        } => verbose_print(common, &format!("Printed {} random string(s)\n", count)),
    }
}

/// Report a failed search as the tool reports each kind of failure.
fn report_failure(common: &CommonOptions, e: Fst2StringsError) -> i32 {
    match e {
        e @ Fst2StringsError::NotForOptimizedLookup(_) => {
            crate::hfst_commandline::hfst_error(common, 1, 0, &e.to_string())
        }
        e @ (Fst2StringsError::Cyclic
        | Fst2StringsError::NotOneBestPath
        | Fst2StringsError::Operation(_)) => error(common, 1, 0, &e.to_string()),
    }
    1
}

// [spec:hfst:def:hfst-fst2strings.process-stream-fn]
// [spec:hfst:sem:hfst-fst2strings.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    let mut first_transducer = true;
    while instream.is_good() {
        if !first_transducer && options.print_separator_after_each_transducer {
            let _ = outstream.write_all(b"--\n");
        }
        first_transducer = false;

        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        /* Pairstring format is not supported on optimized lookup format. */
        if options.print_in_pairstring_format
            && (instream.get_type() == ImplementationType::HFST_OL_TYPE
                || instream.get_type() == ImplementationType::HFST_OLW_TYPE
                || instream.get_type() == ImplementationType::THFST_TYPE)
        {
            eprint!(
                "Error: option --print-in-pairstring-format not supported on \n       optimized lookup transducers, exiting program\n"
            );
            std::process::exit(1);
        }

        if !options.input_prefix.is_empty() {
            verbose_print(
                common,
                &format!("input_prefix: '{}'\n", options.input_prefix),
            );
        }

        // the one runtime dispatch per stream read
        // ([dec:hfst:monomorphic-backends]): the algebra-only pruning
        // options (--beam, --nbest, --random without flag evaluation)
        // become the C++ catch-FunctionNotImplemented error messages on
        // the optimized-lookup variants.
        let code = match any {
            hfst::hfst_transducer::AnyTransducer::Tropical(t) => {
                process_one_algebra(common, options, t, outstream)
            }
            hfst::hfst_transducer::AnyTransducer::OlW(t) => {
                process_one_ol(common, options, t, outstream)
            }
            hfst::hfst_transducer::AnyTransducer::OlU(t) => {
                process_one_ol(common, options, t, outstream)
            }
            hfst::hfst_transducer::AnyTransducer::Thfst(t) => {
                process_one_ol(common, options, t, outstream)
            }
            #[cfg(feature = "foma")]
            hfst::hfst_transducer::AnyTransducer::Foma(t) => {
                process_one_algebra(common, options, t, outstream)
            }
        };
        if code != 0 {
            return code;
        }
    }

    instream.close();
    0
}

// The per-transducer body: the library finds the paths and renders each
// line; the tool prints them.
fn process_one_algebra<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &mut Options,
    t: HfstTransducer<B>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    let format = options.format();
    match extract_paths(
        &t,
        &options.query(),
        &mut |path| write_path(&mut *outstream, path, &format),
        &mut |progress| report_progress(common, progress),
    ) {
        Ok(_) => 0,
        Err(e) => report_failure(common, e),
    }
}

fn process_one_ol<B: hfst::backend::LookupBackend>(
    common: &CommonOptions,
    options: &mut Options,
    t: HfstTransducer<B>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    let format = options.format();
    match extract_paths_ol(
        &t,
        &options.query(),
        &mut |path| write_path(&mut *outstream, path, &format),
        &mut |progress| report_progress(common, progress),
    ) {
        Ok(_) => 0,
        Err(e) => report_failure(common, e),
    }
}

// [spec:hfst:def:hfst-fst2strings.main-fn]
// [spec:hfst:sem:hfst-fst2strings.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstFst2Strings");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = args.resolve(&common, false)?;

    if options.max_strings > 0 && options.max_random_strings > 0 && !common.silent {
        warning(
            &common,
            0,
            0,
            "option --max_strings ignored, --random used\n",
        );
        options.max_strings = -1;
    }

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    // (C closes outfile here when it is not stdout and re-opens an ofstream
    // to outfilename inside; the foundation now models the output as a std
    // writer opened from OUTFILENAME, written to directly.)
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // printing "%s is not a valid transducer file" is not reproduced here.)
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };

    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-fst2strings: cannot open output: {e}");
            return Err(1);
        }
    };
    cli::from_code(process_stream(
        &common,
        &mut options,
        &mut instream,
        &mut *out,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:hfst:req:embed.tools-in-library/test]
    // The tool's options reach the library's query and format as they are,
    // print-flags turning the flag filter off in both.
    #[test]
    fn options_reach_the_library() {
        let options = Options {
            max_strings: 3,
            nbest_strings: 2,
            cycles: 1,
            beam: 0.5,
            eval_fd: true,
            filter_fd: false,
            print_spaces: true,
            display_weights: true,
            input_prefix: "c".to_string(),
            epsilon_format: "0".to_string(),
            ..Options::default()
        };
        let query = options.query();
        assert_eq!((query.max_strings, query.nbest, query.cycles), (3, 2, 1));
        assert_eq!(query.beam, 0.5);
        assert!(query.obey_flags && query.show_flags);
        assert_eq!(query.in_prefix, "c");
        let format = options.format();
        assert!(format.print_spaces && format.print_weights && format.show_flags);
        assert_eq!(format.epsilon_format, "0");
        let defaults = Options::default();
        assert!(!defaults.query().show_flags);
        assert!(!defaults.format().print_pairs);
    }
}
