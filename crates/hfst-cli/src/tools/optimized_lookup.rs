//! hfst-optimized-lookup: run an optimized-lookup transducer on standard input
//! (one word per line) and print analyses in Xerox format.
//!
//! The presentation layer (option parsing, Xerox/weighted output formatting) is
//! a faithful port of tools/src/hfst-optimized-lookup.cc; the lookup engine is
//! provided by the hfst library crate's optimized-lookup runtime
//! (`HfstTransducer::lookup_fd_string`), so the tool no longer carries its own
//! binary-format reader or traversal code.
//!
//! Option handling is clap 4 derive through [`crate::cli`], but the shared
//! [`crate::cli::CommonArgs`] group is NOT flattened in: this tool carries its
//! own option table (no '-o', no '-d', no '--colour') and its own
//! `-h/-V/-v/-q/-s` semantics — '-q'/'-s' turn weight display ON, '-v' changes
//! nothing, and a parse failure dumps the full usage
//! ([`crate::cli::ErrorStyle::UsageDump`]).

use crate::cli::{self, CommonArgs, ErrorStyle, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use crate::hfst_commandline::{VERSION_COPYRIGHT_BLOCK, version_line};
use hfst::tools::optimized_lookup::{
    OptimizedLookup, OptimizedLookupOptions, OptimizedLookupOutcome,
};
use std::io::Write;

const MAX_IO_STRING: usize = 5000;

// ---------------------------------------------------------------------------
// enums
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.output-type]
#[derive(PartialEq, Clone, Copy)]
enum OutputType {
    #[allow(
        dead_code,
        reason = "`HFST` is a dead enumerator upstream too: no option assigns it \
                  and nothing compares against it, only `xerox` is ever used. The \
                  def rule this enum transcribes names both members, so dropping \
                  it would put the annotation at odds with the type it describes."
    )]
    Hfst,
    Xerox,
}
use OutputType::Xerox;

// ---------------------------------------------------------------------------
// tool-local option state (the former tool-specific `static mut`s)
// ---------------------------------------------------------------------------
/// hfst-optimized-lookup's own options. `impl Default` matches the old
/// `static mut` initializers (not all are the type default).
///
/// Three fields here are written by an option handler and read by nothing.
/// That is upstream's shape, not an oversight in the port: `verboseFlag` is
/// assigned and never tested anywhere in hfst-optimized-lookup.cc, and every
/// read of `pipe_input`/`pipe_output` sits inside `#ifdef WINDOWS`. Each keeps
/// a reasoned `allow` rather than being deleted, so a future reader can tell
/// "upstream ignores this too" apart from "we forgot to wire this up".
struct Options {
    output_type: OutputType,
    #[allow(
        dead_code,
        reason = "-v/-q/-s assign it and nothing consults it, exactly as upstream \
                  does; verbosity has no observable effect in this tool. The \
                  visible half of -q/-s is display_weights_flag, which is wired."
    )]
    verbose_flag: bool,
    display_weights_flag: bool,
    display_unique_flag: bool,
    echo_inputs_flag: bool,
    be_fast: bool,
    max_analyses: i32,
    time_cutoff: f64,
    beam: f32,
    #[allow(
        dead_code,
        reason = "--pipe-mode is a Windows-only console switch: upstream reads it \
                  only under #ifdef WINDOWS, to pick console reads over cin. On \
                  the platforms this port builds for there is nothing to select, \
                  but the flag must still parse and validate its STREAM argument."
    )]
    pipe_input: bool,
    #[allow(
        dead_code,
        reason = "As pipe_input: upstream's reads are all #ifdef WINDOWS, choosing \
                  hfst_fprintf_console over cout. The help text already says \
                  --pipe-mode=output is ignored on non-windows platforms."
    )]
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
// runTransducer
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.run-transducer-fn]
// [spec:hfst:sem:hfst-optimized-lookup.run-transducer-fn]
fn run_transducer(options: &Options, t: &OptimizedLookup<'_>) {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let lookup_options = OptimizedLookupOptions {
        unique: options.display_unique_flag,
        max_analyses: options.max_analyses,
        time_cutoff: options.time_cutoff,
        beam: options.beam,
        fast: options.be_fast,
    };
    let mut out = std::io::stdout();

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

        let outcome = t.look_up(&str_display, &lookup_options, &mut |e| {
            print_err(&format!("{e}\n"));
        });
        // An untokenizable word skips the lookup, and C++ printed the echo's
        // blank line for it alone.
        if outcome == OptimizedLookupOutcome::Untokenizable && options.echo_inputs_flag {
            print_out("\n");
        }
        if options.output_type == Xerox {
            let _ = t.write(
                &mut out,
                &str_display,
                &outcome,
                &lookup_options,
                options.display_weights_flag,
            );
        }
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
    // Every transducer of the stream, looked up in as their union.
    let mut lookup = OptimizedLookup::default();
    let mut read = 0;
    while instream.is_good() {
        let any = match instream.read() {
            Ok(v) => v,
            Err(_) => break,
        };
        if let Err(e) = lookup.push(any) {
            print_err(&format!("{e}\n"));
            return 1;
        }
        read += 1;
    }
    if read == 0 {
        print_err("Transducer file contains no transducers\n");
        return 1;
    }
    run_transducer(options, &lookup);
    0
}

// ---------------------------------------------------------------------------
// the command line
// ---------------------------------------------------------------------------

/// hfst-optimized-lookup's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Run a transducer on standard input (one word per line) and print analyses\n\
             NOTE: hfst-optimized-lookup does lookup from left to right as opposed to xfst\n\
             and foma lookup which is carried out from right to left. In order to do\n\
             lookup in a similar way as xfst and foma, invert the transducer first.",
    after_help = "N must be a positive integer. B must be a non-negative float.
S must be a non-negative float. The default, 0.0, indicates no cutoff.
Options -n and -b are combined with AND, i.e. they both restrict the output.

STREAM can be { input, output, both }. If not given, defaults to {both}.
Input is read interactively line by line from the user. If you redirect input
from a file, use --pipe-mode=input. --pipe-mode=output is ignored on non-windows
platforms."
)]
struct Args {
    /// Never populated: this tool carries its own option table with its own
    /// `-v/-q/-s` semantics instead of the shared common cases.
    #[arg(skip)]
    common: CommonArgs,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Be verbose
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Don't be verbose (default)
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Same as quiet
    #[arg(short = 's', long = "silent")]
    silent: bool,

    /// Echo inputs (useful if redirecting lots of output to a file)
    #[arg(short = 'e', long = "echo-inputs")]
    echo_inputs: bool,

    /// Print final analysis weights (if any)
    #[arg(short = 'w', long = "show-weights")]
    show_weights: bool,

    /// Suppress duplicate analyses
    #[arg(short = 'u', long = "unique")]
    unique: bool,

    /// Output no more than N analyses (if the transducer is weighted, the N
    /// best analyses)
    #[arg(
        short = 'n',
        long = "analyses",
        value_name = "N",
        allow_hyphen_values = true
    )]
    analyses: Option<String>,

    /// Output only analyses whose weight is within B from the best analysis
    #[arg(
        short = 'b',
        long = "beam",
        value_name = "B",
        allow_hyphen_values = true
    )]
    beam: Option<String>,

    /// Limit search after having used S seconds per input
    #[arg(
        short = 't',
        long = "time-cutoff",
        value_name = "S",
        allow_hyphen_values = true
    )]
    time_cutoff: Option<String>,

    /// Xerox output format (default)
    #[arg(short = 'x', long = "xerox")]
    xerox: bool,

    /// Be as fast as possible (with this option enabled -u and -n don't work
    /// and output won't be ordered by weight)
    #[arg(short = 'f', long = "fast")]
    fast: bool,

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

    /// The transducer file
    #[arg(value_name = "TRANSDUCER", num_args = 0..)]
    infiles: Vec<String>,

    /// The checked option occurrences in command-line order: the C loop
    /// answered '-V' and rejected bad '-b/-t/-n/-p' values as it scanned, so
    /// the diagnostics replay in that order.
    #[arg(skip)]
    events: Vec<Event>,
}

/// One checked iteration of the C option loop, in occurrence order.
#[derive(Clone, Copy)]
enum Event {
    Version,
    Beam,
    TimeCutoff,
    Analyses,
    /// Index into the `pipe_mode` occurrence vector.
    PipeMode(usize),
}

impl Args {
    /// Replay the checked occurrences into the tool options; fatal value
    /// rejections print exactly as the C loop's arms did.
    fn resolve(&self, common: &CommonOptions, print: bool) -> Result<Options, i32> {
        let mut options = Options {
            verbose_flag: self.verbose && !(self.quiet || self.silent),
            // Quiet also turns weights on. It reads like a typo, but it is
            // what upstream does and the only half of -q/-s a user can see,
            // so dropping it would silently change every -q invocation.
            display_weights_flag: self.show_weights || self.quiet || self.silent,
            display_unique_flag: self.unique,
            echo_inputs_flag: self.echo_inputs,
            be_fast: self.fast,
            ..Options::default()
        };
        if self.xerox {
            options.output_type = Xerox;
        }
        for event in &self.events {
            match event {
                Event::Version => {
                    if print {
                        print_version(common);
                    }
                    return Err(0);
                }
                Event::Beam => {
                    options.beam =
                        parse_leading_f64(self.beam.as_deref().unwrap_or_default()) as f32;
                    if options.beam < 0.0 {
                        print_err("Invalid argument for --beam\n");
                        return Err(1);
                    }
                }
                Event::TimeCutoff => {
                    options.time_cutoff =
                        parse_leading_f64(self.time_cutoff.as_deref().unwrap_or_default());
                    if options.time_cutoff < 0.0 {
                        print_err("Invalid argument for --time-cutoff\n");
                        return Err(1);
                    }
                }
                Event::Analyses => {
                    options.max_analyses =
                        parse_leading_i32(self.analyses.as_deref().unwrap_or_default());
                    if options.max_analyses < 1 {
                        print_err("Invalid or no argument for analyses count\n");
                        return Err(1);
                    }
                }
                Event::PipeMode(k) => {
                    let a = self.pipe_mode[*k].as_str();
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
            }
        }
        // The single positional operand, resolved the way the tool's own
        // post-loop block did.
        match self.infiles.len() {
            0 => options.input_path = None,
            1 => options.input_path = Some(self.infiles[0].clone()),
            _ => {
                print_err("More than one input file given\n");
                return Err(1);
            }
        }
        Ok(options)
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, _opts: &mut CommonOptions) {}

    fn applies_common_options(&self) -> bool {
        false
    }

    fn error_style() -> ErrorStyle {
        ErrorStyle::UsageDump
    }

    fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
        let ids: &[(&str, Event)] = &[
            ("version", Event::Version),
            ("beam", Event::Beam),
            ("time_cutoff", Event::TimeCutoff),
            ("analyses", Event::Analyses),
        ];
        let mut ordered: Vec<(usize, Event)> = ids
            .iter()
            .filter(|(id, _)| {
                matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine)
            })
            .filter_map(|(id, event)| matches.index_of(id).map(|i| (i, *event)))
            .collect();
        if matches.value_source("pipe_mode") == Some(clap::parser::ValueSource::CommandLine)
            && let Some(indices) = matches.indices_of("pipe_mode")
        {
            for (k, i) in indices.enumerate() {
                ordered.push((i, Event::PipeMode(k)));
            }
        }
        ordered.sort_by_key(|(i, _)| *i);
        self.events = ordered.into_iter().map(|(_, event)| event).collect();
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        self.resolve(opts, true)?;
        Ok(())
    }
}

// [spec:hfst:def:hfst-optimized-lookup.print-version-fn+1]
// [spec:hfst:sem:hfst-optimized-lookup.print-version-fn+1]
fn print_version(common: &CommonOptions) -> bool {
    let mut msg = common.message_writer();
    let _ = write!(msg, "\n{}\n{}", version_line(), VERSION_COPYRIGHT_BLOCK);
    true
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.main-fn]
// [spec:hfst:sem:hfst-optimized-lookup.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "1.2", "HfstOptimizedLookup");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = args.resolve(&common, false)?;

    match &options.input_path {
        Some(path) => {
            let pathstr = path.clone();
            cli::from_code(setup(&options, &pathstr))
        }
        None => {
            print_err("No input file given\n");
            Err(1)
        }
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
