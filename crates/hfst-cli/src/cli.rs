//! The clap-4 argument layer shared by every hfst tool.
//!
//! This replaces the hand-ported getopt loop (hfst-getopt + the inc/
//! getopt-cases fragments) for the tools that have been migrated: a tool
//! declares its whole command line as one clap derive struct, flattens
//! [`CommonArgs`] plus an IO group ([`UnaryIo`] / [`BinaryIo`]) into it, and
//! calls [`parse`] once. Help text is clap's, not the hand-written
//! print_usage blocks.
//!
//! # Converting a tool
//!
//! ```ignore
//! use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
//!
//! #[derive(clap::Parser)]
//! #[command(about = "Minimize a transducer")]
//! struct Args {
//!     #[command(flatten)]
//!     common: CommonArgs,
//!     #[command(flatten)]
//!     io: UnaryIo,
//!     /// Encode weights when minimizing (default is false)
//!     #[arg(short = 'E', long = "encode-weights")]
//!     encode_weights: bool,
//! }
//!
//! impl ToolArgs for Args {
//!     fn common(&self) -> &CommonArgs { &self.common }
//!     fn apply_io(&self, opts: &mut CommonOptions) { self.io.apply(opts) }
//! }
//!
//! pub fn run(args: Vec<String>) -> i32 { cli::exit_code(execute(args)) }
//!
//! fn execute(args: Vec<String>) -> ToolResult {
//!     let argv0 = args.first().cloned().unwrap_or_default();
//!     let common = hfst_set_program_name(&argv0, "0.1", "HfstMinimize");
//!     let (common, args) = cli::parse::<Args>(common, args)?;
//!     ...
//! }
//! ```
//!
//! Doc comments on the fields become the help text. Anything the C tool
//! validated inside its getopt loop goes in [`ToolArgs::validate`], which runs
//! before the parameter checks so the error ordering the spec pins is kept;
//! anything it validated after the loop belongs in the tool's own body.
//!
//! # What the argv pre-pass is for
//!
//! clap 4 already handles everything the system getopt_long did except two
//! spellings the C tools accept, so [`normalize_argv`] rewrites those before
//! clap sees them:
//!
//!   * a long option behind ONE dash ('-quiet', hfst-tokenise's '-cg'), which
//!     the old parser resolved by scanning the long table first, so a name
//!     that also reads as a short cluster is still the long option;
//!   * an optional-argument short with its value attached ('-pboth', '-S5'),
//!     which clap only accepts with an '=' between them.
//!
//! Short clusters ('-wq'), attached required arguments ('-n2', '-Wall'),
//! '--opt=val', option/operand permutation, a lone '-' operand and the numeric
//! '-1'/'-2' shorts of the binary tools are all native clap behaviour and need
//! no help — but '-1'/'-2' only work with allow_negative_numbers OFF (clap's
//! default), so no tool may turn it on.
//!
//! # Exit codes
//!
//! A tool body returns [`ToolResult`]; [`exit_code`] is the single place that
//! becomes the process status the TOOLS table's fn(Vec<String>) -> i32
//! contract wants. Argument errors are reported in HFST's own shape (the
//! "Try ... --help" hint, then "prog: Unknown option ...") and exit 1, not
//! clap's 2.

use clap::ArgAction;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use std::collections::HashSet;

use crate::globals::{ColourTristate, CommonOptions};
use crate::hfst_commandline::{error, print_short_help, print_version};
use crate::inc::check_common_params;

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

// ---------------------------------------------------------------------------
// exit-code mapping
// ---------------------------------------------------------------------------

/// A tool body's outcome: Ok is a clean run, Err carries the process status.
pub type ToolResult = Result<(), i32>;

/// The one place a tool's Result becomes its process exit code.
// [spec:hfst:req:cli.main]
pub fn exit_code(result: ToolResult) -> i32 {
    match result {
        Ok(()) => EXIT_SUCCESS,
        Err(code) => code,
    }
}

/// Lift a legacy i32-returning driver (unary_ops, binary_ops) into a
/// [`ToolResult`] so a tool body can stay in the Result idiom throughout.
pub fn from_code(code: i32) -> ToolResult {
    if code == EXIT_SUCCESS {
        Ok(())
    } else {
        Err(code)
    }
}

// ---------------------------------------------------------------------------
// the shared option groups
// ---------------------------------------------------------------------------

/// The options every tool carries (tools/src/inc/getopt-cases-common.h plus
/// the '-o' of the unary/binary IO groups, which every tool shares).
///
/// '-v' against '-q'/'-s' is last-one-wins, as the C cases were: each assigned
/// BOTH the verbose and the silent flag, so whichever came last on the command
/// line decided. clap reproduces that with mutual overrides_with rather than
/// by inspecting match indices.
// [spec:hfst:req:cli.common-options]
// [spec:hfst:req:cli.arg-parse]
#[derive(clap::Args, Debug, Clone, Default)]
pub struct CommonArgs {
    /// Print version info
    #[arg(short = 'V', long = "version", action = ArgAction::SetTrue)]
    pub version: bool,

    /// Print verbosely while processing
    #[arg(short = 'v', long = "verbose", overrides_with_all = ["quiet", "silent"])]
    pub verbose: bool,

    /// Only print fatal errors and requested output
    #[arg(short = 'q', long = "quiet", overrides_with_all = ["verbose", "silent"])]
    pub quiet: bool,

    /// Alias of --quiet
    #[arg(short = 's', long = "silent", overrides_with_all = ["verbose", "quiet"])]
    pub silent: bool,

    /// Print debugging messages while processing
    #[arg(short = 'd', long = "debug")]
    pub debug: bool,

    /// Write output transducer to OUTFILE
    #[arg(
        short = 'o',
        long = "output",
        value_name = "OUTFILE",
        allow_hyphen_values = true
    )]
    pub output: Option<String>,

    /// Print in colour WHEN: always, never, auto (default)
    #[arg(
        long = "colour",
        visible_alias = "color",
        value_name = "WHEN",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "always"
    )]
    pub colour: Option<String>,
}

impl CommonArgs {
    /// Fold the parsed common options into the tool's shared state; the
    /// counterpart of the getopt-cases-common.h switch arms.
    // [spec:hfst:req:cli.common-options]
    pub fn apply(&self, opts: &mut CommonOptions) {
        opts.debug = self.debug;
        if self.verbose {
            opts.verbose = true;
            opts.silent = false;
        } else if self.quiet || self.silent {
            opts.verbose = false;
            opts.silent = true;
        }
        if let Some(name) = &self.output {
            opts.output_filename = name.clone();
            // A "-" output name means stdout; messages then go to stderr so
            // they do not corrupt the data stream.
            if opts.output_filename == "-" {
                opts.output_filename = "<stdout>".to_string();
                opts.message_to_stderr = true;
            }
            opts.output_named = true;
        }
        match self.colour.as_deref() {
            None => {}
            Some("always") => opts.colour = ColourTristate::COLOUR_ALWAYS,
            Some("never") => opts.colour = ColourTristate::COLOUR_NEVER,
            Some("auto") => opts.colour = ColourTristate::COLOUR_AUTO,
            Some(other) => {
                error(
                    opts,
                    EXIT_FAILURE,
                    0,
                    &format!("--colour must be one of always, never, auto, not {}", other),
                );
            }
        }
    }
}

/// The one-input-stream IO group: '-i INFILE' plus the positional operand
/// (tools/src/inc/getopt-cases-unary.h + check-params-unary.h).
// [spec:hfst:req:cli.unary-options]
#[derive(clap::Args, Debug, Clone, Default)]
pub struct UnaryIo {
    /// Read input transducer from INFILE
    #[arg(
        short = 'i',
        long = "input",
        value_name = "INFILE",
        allow_hyphen_values = true
    )]
    pub input: Option<String>,

    /// Input transducer file; missing or - reads the standard input
    #[arg(value_name = "INFILE", num_args = 0..)]
    pub infiles: Vec<String>,
}

impl UnaryIo {
    /// Resolve '-i' and the leftover operand into the input filename, with the
    /// too-many-files diagnostics of check-params-unary.h.
    // [spec:hfst:req:cli.unary-options]
    pub fn apply(&self, opts: &mut CommonOptions) {
        if let Some(name) = &self.input {
            opts.input_filename = stdin_if_dash(name);
            opts.input_named = true;
        }
        if !opts.input_named {
            match self.infiles.len() {
                1 => opts.input_filename = stdin_if_dash(&self.infiles[0]),
                0 => opts.input_filename = "<stdin>".to_string(),
                _ => error(
                    opts,
                    EXIT_FAILURE,
                    0,
                    "no more than one transducer file may be given",
                ),
            }
        } else if !self.infiles.is_empty() {
            error(
                opts,
                EXIT_FAILURE,
                0,
                "no more than one transducer filename may be given",
            );
        }
    }
}

/// The two-input-stream IO group: '-1'/'-2'/'-C' plus up to two positional
/// operands (tools/src/inc/getopt-cases-binary.h + check-params-binary.h).
///
/// The numeric shorts parse natively as long as no command turns
/// allow_negative_numbers on.
///
/// '--input2' is an alias of '--input1', not the long name of '-2'. That is
/// the HFST_GETOPT_BINARY_LONG macro's own wiring — its 'input2' entry carries
/// the '1' option value — so the long spelling has always filled the FIRST
/// slot while '-2' filled the second. Preserved bug-for-bug; only the help
/// text stops advertising the two as the same option.
// [spec:hfst:req:cli.binary-options]
#[derive(clap::Args, Debug, Clone, Default)]
pub struct BinaryIo {
    /// Read first input transducer from INFILE1
    #[arg(
        short = '1',
        long = "input1",
        alias = "input2",
        value_name = "INFILE1",
        allow_hyphen_values = true
    )]
    pub input1: Option<String>,

    /// Read second input transducer from INFILE2
    #[arg(short = '2', value_name = "INFILE2", allow_hyphen_values = true)]
    pub input2: Option<String>,

    /// Do not allow transducers to be converted into the same type
    #[arg(short = 'C', long = "do-not-convert")]
    pub do_not_convert: bool,

    /// Input transducer files; a missing or - operand reads the standard input
    #[arg(value_name = "INFILE", num_args = 0..)]
    pub infiles: Vec<String>,
}

impl BinaryIo {
    /// Resolve '-1'/'-2' and the leftover operands into the two input
    /// filenames, reproducing check-params-binary.h case for case.
    // [spec:hfst:req:cli.binary-options]
    pub fn apply(&self, opts: &mut CommonOptions) {
        if self.do_not_convert {
            opts.allow_transducer_conversion = false;
        }
        if let Some(name) = &self.input1 {
            opts.first_filename = stdin_if_dash(name);
            opts.first_named = true;
            if name == "-" {
                opts.is_input_stdin = true;
            }
        }
        if let Some(name) = &self.input2 {
            opts.second_filename = stdin_if_dash(name);
            opts.second_named = true;
            if name == "-" {
                opts.is_input_stdin = true;
            }
        }
        let files = &self.infiles;
        let remaining = files.len();
        if opts.first_named && opts.second_named {
            if remaining > 0 {
                error(
                    opts,
                    EXIT_FAILURE,
                    0,
                    "No more than two transducer files may be given",
                );
            }
        } else if !opts.first_named && !opts.second_named {
            match remaining {
                2 => {
                    opts.first_filename = stdin_if_dash(&files[0]);
                    opts.second_filename = stdin_if_dash(&files[1]);
                    opts.is_input_stdin = false;
                }
                1 => {
                    opts.second_filename = stdin_if_dash(&files[0]);
                    opts.first_filename = "<stdin>".to_string();
                    opts.is_input_stdin = true;
                }
                0 => error(
                    opts,
                    EXIT_FAILURE,
                    0,
                    "at least one input must be from a named file",
                ),
                _ => error(
                    opts,
                    EXIT_FAILURE,
                    0,
                    "no more than two transducer filenames may be given",
                ),
            }
        } else if !opts.first_named {
            match remaining {
                1 => {
                    opts.first_filename = stdin_if_dash(&files[0]);
                    opts.is_input_stdin = false;
                }
                0 => {
                    opts.first_filename = "<stdin>".to_string();
                    opts.is_input_stdin = true;
                }
                _ => error(
                    opts,
                    EXIT_FAILURE,
                    0,
                    "no more than two transducer filenames may be given",
                ),
            }
        } else {
            match remaining {
                1 => {
                    opts.second_filename = stdin_if_dash(&files[0]);
                    opts.is_input_stdin = false;
                }
                0 => {
                    opts.second_filename = "<stdin>".to_string();
                    opts.is_input_stdin = true;
                }
                _ => error(
                    opts,
                    EXIT_FAILURE,
                    0,
                    "no more than two transducer filenames may be given",
                ),
            }
        }
    }
}

/// A filename operand of "-" selects the standard stream, matching the C
/// hfst_fopen, which returned stdin for "-".
fn stdin_if_dash(name: &str) -> String {
    if name == "-" {
        "<stdin>".to_string()
    } else {
        name.to_string()
    }
}

// ---------------------------------------------------------------------------
// the tool contract
// ---------------------------------------------------------------------------

/// What [`parse`] needs from a tool's derive struct.
pub trait ToolArgs: clap::Parser {
    /// The flattened [`CommonArgs`].
    fn common(&self) -> &CommonArgs;

    /// Fold the tool's IO group into the shared state; normally one call to
    /// [`UnaryIo::apply`] or [`BinaryIo::apply`].
    fn apply_io(&self, opts: &mut CommonOptions);

    /// Tool-specific option validation the C ran INSIDE its getopt loop (a
    /// bad --project direction, an out-of-vocabulary --push) or immediately
    /// after it, i.e. before the parameter checks. Checks the C ran after the
    /// parameter checks belong in the tool body instead.
    fn validate(&self, _opts: &CommonOptions) -> ToolResult {
        Ok(())
    }

    /// Recover state that depends on the ORDER two different options were
    /// written in — the one thing a derive struct cannot carry, because a C
    /// getopt loop let both write the same variable and the last write won
    /// (hfst-edit-metadata's '-a' and bare '-p' both set
    /// print_all_properties). Defaulted to nothing; a tool that needs it
    /// reads [`clap::ArgMatches::index_of`] here.
    fn absorb_matches(&mut self, _matches: &clap::ArgMatches) {}

    /// Whether the tool's switch chained getopt-cases-common.h and ran
    /// check-params-common.h afterwards. Every tool did except hfst-info,
    /// whose switch reads only its own version/feature options: there
    /// '-v/-q/-s/-d/-o/--colour' are accepted and discarded and no output
    /// file is resolved, which is why its report goes to stdout instead of
    /// the messages-to-stderr default. '-V' is answered either way.
    fn applies_common_options(&self) -> bool {
        true
    }
}

/// Parse a tool's argv into its derive struct and the shared
/// [`CommonOptions`], reporting help / version / argument errors the way the
/// HFST tools do.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
// [spec:hfst:req:cli.version]
pub fn parse<T: ToolArgs>(
    mut common: CommonOptions,
    argv: Vec<String>,
) -> Result<(CommonOptions, T), i32> {
    let mut cmd = T::command().bin_name(common.program_name.clone());
    let argv = normalize_argv(&cmd, argv);
    let matches = match cmd.try_get_matches_from_mut(argv) {
        Ok(m) => m,
        Err(e) => return Err(report_clap_error(&common, &cmd, e)),
    };
    let mut args = match T::from_arg_matches(&matches) {
        Ok(a) => a,
        Err(e) => return Err(report_clap_error(&common, &cmd, e)),
    };
    args.absorb_matches(&matches);

    let shares_common = args.applies_common_options();
    if shares_common {
        args.common().apply(&mut common);
    }
    if args.common().version {
        print_version(&common);
        return Err(EXIT_SUCCESS);
    }
    args.validate(&common)?;
    if shares_common {
        check_common_params(&mut common);
    }
    args.apply_io(&mut common);
    Ok((common, args))
}

// ---------------------------------------------------------------------------
// argv pre-normalisation
// ---------------------------------------------------------------------------

/// Rewrite the two spellings the C tools accept and clap does not: a long
/// option behind one dash, and an optional-argument short with its value
/// attached. Everything else passes through untouched.
///
/// The long-name lookup runs first, so a single-dash token that ALSO reads as
/// a cluster stays the long option, exactly as the old parser resolved it.
// [spec:hfst:req:cli.arg-parse]
pub fn normalize_argv(cmd: &clap::Command, argv: Vec<String>) -> Vec<String> {
    let mut longs: HashSet<&str> = HashSet::new();
    // Shorts whose value is optional, with the long name to rewrite through.
    let mut optional_shorts: Vec<(char, Option<&str>)> = Vec::new();
    for arg in cmd.get_arguments() {
        if let Some(long) = arg.get_long() {
            longs.insert(long);
        }
        for alias in arg.get_all_aliases().into_iter().flatten() {
            longs.insert(alias);
        }
        if let Some(short) = arg.get_short()
            && arg
                .get_num_args()
                .is_some_and(|n| n.min_values() == 0 && n.max_values() >= 1)
        {
            optional_shorts.push((short, arg.get_long()));
        }
    }

    let mut out: Vec<String> = Vec::with_capacity(argv.len());
    let mut iter = argv.into_iter();
    if let Some(program) = iter.next() {
        out.push(program);
    }
    let mut past_end_of_options = false;
    for token in iter {
        if past_end_of_options {
            out.push(token);
            continue;
        }
        if token == "--" {
            past_end_of_options = true;
            out.push(token);
            continue;
        }
        // A lone "-" is the stdin operand, "--long" is already a long option,
        // and anything not starting with a dash is an operand.
        if token.len() < 3 || !token.starts_with('-') || token.starts_with("--") {
            out.push(token);
            continue;
        }
        let body = &token[1..];
        let name = body.split('=').next().unwrap_or(body);
        if name.chars().count() > 1 && longs.contains(name) {
            out.push(format!("-{}", token));
            continue;
        }
        // '-pboth' for an optional-argument short: clap needs the '=' spelled.
        let mut chars = body.chars();
        let first = chars.next().unwrap_or('\0');
        let rest = chars.as_str();
        if !rest.is_empty()
            && !rest.starts_with('=')
            && let Some((_, long)) = optional_shorts.iter().find(|(c, _)| *c == first)
        {
            match long {
                Some(long) => out.push(format!("--{}={}", long, rest)),
                None => out.push(format!("-{}={}", first, rest)),
            }
            continue;
        }
        out.push(token);
    }
    out
}

// ---------------------------------------------------------------------------
// error reporting
// ---------------------------------------------------------------------------

/// Render a clap parse failure the way the HFST tools do: --help and
/// --version leave with status 0, everything else prints the short-help hint
/// plus a "prog: ..." line on stderr and leaves with status 1 (clap's own
/// default is 2).
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
fn report_clap_error(common: &CommonOptions, cmd: &clap::Command, err: clap::Error) -> i32 {
    use std::io::Write;
    // ErrorKind is non_exhaustive, so these are equality tests rather than a
    // match with a catch-all arm.
    let kind = err.kind();
    if kind == ErrorKind::DisplayHelp || kind == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    {
        let mut out = common.message_writer();
        let _ = write!(out, "{}", err.render());
        return EXIT_SUCCESS;
    }
    if kind == ErrorKind::DisplayVersion {
        print_version(common);
        return EXIT_SUCCESS;
    }
    print_short_help(common);
    // error() with a non-zero status exits, so the return below stands for the
    // C's 'return EXIT_FAILURE' after the error call.
    error(common, EXIT_FAILURE, 0, &hfst_error_text(cmd, &err));
    EXIT_FAILURE
}

/// The HFST-shaped one-liner for a clap parse failure.
fn hfst_error_text(cmd: &clap::Command, err: &clap::Error) -> String {
    let invalid = match err.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(s)) => Some(s.clone()),
        Some(ContextValue::Strings(v)) => v.first().cloned(),
        _ => None,
    };
    let kind = err.kind();
    // getopt-cases-error.h, the '?' arm. The C could only name a SHORT option
    // (a long one left optopt at -2 and printed a bare "Unknown option"); clap
    // hands over the token either way, so both are named.
    if kind == ErrorKind::UnknownArgument {
        return match invalid {
            Some(arg) => format!("Unknown option `{}'.\n", arg),
            None => "Unknown option".to_string(),
        };
    }
    // The ':' arm: an option whose argument is missing.
    if kind == ErrorKind::InvalidValue
        || kind == ErrorKind::TooFewValues
        || kind == ErrorKind::WrongNumberOfValues
    {
        return match invalid.as_deref().and_then(|a| option_letter(cmd, a)) {
            Some(letter) => format!("Option -{} requires an argument", letter),
            None => match invalid {
                Some(arg) => format!("Option {} requires an argument", arg),
                None => "Option requires an argument".to_string(),
            },
        };
    }
    first_line(err)
}

/// The short letter of the option a clap error names, e.g. "-o" for the
/// context string "--output <OUTFILE>".
fn option_letter(cmd: &clap::Command, context: &str) -> Option<char> {
    let token = context.split_whitespace().next()?;
    let name = token.trim_start_matches('-');
    let mut chars = name.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        return Some(first);
    }
    cmd.get_arguments()
        .find(|a| {
            a.get_long() == Some(name)
                || a.get_all_aliases()
                    .is_some_and(|aliases| aliases.contains(&name))
        })
        .and_then(|a| a.get_short())
}

/// clap's own message, stripped of its "error: " prefix — the fallback for
/// failure kinds the C parser had no counterpart for.
fn first_line(err: &clap::Error) -> String {
    let rendered = err.render().to_string();
    let line = rendered
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default();
    line.trim_start_matches("error: ").to_string()
}
