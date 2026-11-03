//! Faithful 1:1 port of tools/src/parsers/hfst-xfst.cc — the command-line
//! program for compiling XFST scripts or executing XFST commands
//! interactively, driving the hfst XfstCompiler. On a terminal the shell
//! reads its lines through a rustyline editor in place of the C++ readline
//! branch.
//!
//! Option handling is clap 4 derive through [`crate::cli`], but the shared
//! [`crate::cli::CommonArgs`] group is NOT flattened in: the C++ copies the
//! common getopt cases inline "with exceptions" — there is no '-o' case at
//! all, and '--colour=auto' maps to COLOUR_NEVER — so the common options are
//! re-declared here with those exceptions preserved bug-for-bug.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::{ColourTristate, CommonOptions};
use crate::hfst_commandline::{
    error, hfst_error, hfst_parse_format_name, hfst_set_program_name, parse_format_name_quiet,
    print_version, skip_elapsed, verbose_print, version_line,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::xfst_compiler::{Completion, Flow, LineSource, XfstCompiler, complete};
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Editor, Helper};
use std::io::{IsTerminal, Write};

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

/// hfst-xfst's own options (the former tool-specific `static mut`s).
struct Options {
    output_format: ImplementationType,
    scriptfilename: Option<String>,
    startupfilename: Option<String>,
    execute_commands: Vec<String>,
    execute_command_and_quit: Option<String>,
    pipe_input: bool,
    pipe_output: bool, // this has no effect on non-windows platforms
    restricted_mode: bool,
    /// '-r': read the shell's lines plainly, even on a terminal.
    no_readline: bool,
    print_weight: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            output_format: ImplementationType::UNSPECIFIED_TYPE,
            scriptfilename: None,
            startupfilename: None,
            execute_commands: Vec::new(),
            execute_command_and_quit: None,
            pipe_input: false,
            pipe_output: false,
            restricted_mode: false,
            no_readline: false,
            print_weight: false,
        }
    }
}

/// hfst-xfst's command line.
//
// The 'k' arm of the C switch (set pipe_output) is unreachable: no long-table
// entry carries the value 'k', so '-k' has always reported an unknown option.
// It is therefore not declared here either.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Compile XFST scripts or execute XFST commands interactively",
    after_help = "Option --execute can be invoked many times.
If FMT is not given, OpenFst's tropical format will be used.
The possible values for FMT are { foma, openfst-tropical, sfst }.
On a terminal the shell reads lines with a line editor: the arrow keys move
and recall this session's lines, Ctrl-C clears the line, Ctrl-D quits.
With -r it reads plain lines instead.
Input files are always treated as UTF-8.

STREAM can be { input, output, both }. If not given, defaults to {both}.
If input file is not specified with -F, input is read interactively line by
line from the user. If you redirect input from a file, use --pipe-mode=input.
--pipe-mode=output is ignored on non-windows platforms."
)]
struct Args {
    /// Never populated: this tool's switch copies the common cases inline
    /// "with exceptions" instead of chaining getopt-cases-common.h.
    #[arg(skip)]
    common: CommonArgs,

    /// Print version info
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Print verbosely while processing
    #[arg(short = 'v', long = "verbose", overrides_with_all = ["quiet", "silent"])]
    verbose: bool,

    /// Only print fatal errors and requested output
    #[arg(short = 'q', long = "quiet", overrides_with_all = ["verbose", "silent"])]
    quiet: bool,

    /// Alias of --quiet
    #[arg(short = 's', long = "silent", overrides_with_all = ["verbose", "quiet"])]
    silent: bool,

    /// Print debugging messages while processing
    #[arg(short = 'd', long = "debug")]
    debug: bool,

    /// Print in colour WHEN: always, never, auto ('auto' maps to never,
    /// preserved bug-for-bug from the C source)
    #[arg(
        long = "colour",
        visible_alias = "color",
        value_name = "WHEN",
        num_args = 0..=1,
        require_equals = true
    )]
    colour: Option<Option<String>>,

    /// Execute command CMD on startup
    #[arg(
        short = 'e',
        long = "execute",
        value_name = "CMD",
        action = clap::ArgAction::Append,
        allow_hyphen_values = true
    )]
    execute: Vec<String>,

    /// Execute command CMD, and quit
    #[arg(
        short = 'E',
        long = "execute-and-quit",
        value_name = "CMD",
        allow_hyphen_values = true
    )]
    execute_and_quit: Option<String>,

    /// Write result using FMT as backend format
    #[arg(
        short = 'f',
        long = "format",
        value_name = "FMT",
        allow_hyphen_values = true
    )]
    format: Option<String>,

    /// Read commands from FILE, and quit
    #[arg(
        short = 'F',
        long = "scriptfile",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    scriptfile: Option<String>,

    /// Read commands from FILE on startup
    #[arg(
        short = 'l',
        long = "startupfile",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    startupfile: Option<String>,

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

    /// Read the shell's lines plainly, without the line editor
    #[arg(short = 'r', long = "no-readline")]
    no_readline: bool,

    /// Print weights for each operation
    #[arg(short = 'w', long = "print-weight")]
    print_weight: bool,

    /// Allow read and write operations only in current directory, do not
    /// allow system calls
    #[arg(short = 'R', long = "restricted-mode")]
    restricted_mode: bool,
}

impl Args {
    /// The 'p' case's STREAM vocabulary, replayed per occurrence; unknown
    /// values were fatal inside the C loop.
    fn pipe_flags(&self, opts: &CommonOptions) -> Result<(bool, bool), i32> {
        let mut pipe_input = false;
        let mut pipe_output = false;
        for stream in &self.pipe_mode {
            match stream.as_str() {
                "both" | "BOTH" => {
                    pipe_input = true;
                    pipe_output = true;
                }
                "input" | "INPUT" | "in" | "IN" => {
                    pipe_input = true;
                }
                "output" | "OUTPUT" | "out" | "OUT" => {
                    pipe_output = true;
                }
                other => {
                    error(
                        opts,
                        EXIT_FAILURE,
                        0,
                        &format!("--pipe-mode argument {} unrecognised", other),
                    );
                    return Err(EXIT_FAILURE);
                }
            }
        }
        Ok((pipe_input, pipe_output))
    }

    /// The hand-copied colour case: bare '--colour' and 'always' select
    /// ALWAYS; 'never' AND 'auto' select NEVER (the preserved bug); anything
    /// else was fatal inside the C loop.
    fn colour(&self, opts: &CommonOptions) -> Result<Option<ColourTristate>, i32> {
        match &self.colour {
            None => Ok(None),
            Some(None) => Ok(Some(ColourTristate::COLOUR_ALWAYS)),
            Some(Some(when)) => match when.as_str() {
                "always" => Ok(Some(ColourTristate::COLOUR_ALWAYS)),
                "never" | "auto" => Ok(Some(ColourTristate::COLOUR_NEVER)),
                other => {
                    hfst_error(
                        opts,
                        EXIT_FAILURE,
                        0,
                        &format!(
                            "--colour must be one of always, never, or auto, not {}",
                            other
                        ),
                    );
                    Err(EXIT_FAILURE)
                }
            },
        }
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

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // '-V' was answered inside the C loop; the vocabulary rejections
        // (colour, pipe-mode) and the format parse also ran there.
        if self.version {
            print_version(opts);
            return Err(EXIT_SUCCESS);
        }
        self.colour(opts)?;
        self.pipe_flags(opts)?;
        if let Some(name) = &self.format {
            hfst_parse_format_name(opts, name);
        }
        Ok(())
    }
}

//
// Parse file 'filename' using compiler 'comp'.
// Filename "<stdin>" reads the compiler's input, which is stdin.
fn parse_file<B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer>(
    common: &CommonOptions,
    filename: &str,
    comp: &mut XfstCompiler<B>,
) -> i32 {
    // hfst_file_to_mem(filename): the whole file (or stdin) as one string.
    let line = if filename == "<stdin>" {
        let mut s = String::new();
        match comp.input().read_to_string(&mut s) {
            Ok(_) => Some(s),
            Err(_) => None,
        }
    } else {
        std::fs::read_to_string(filename).ok()
    };
    let Some(line) = line else {
        hfst_error(
            common,
            EXIT_FAILURE,
            0,
            &format!("error when reading file {}\n", filename),
        );
        return EXIT_FAILURE;
    };

    comp.set_source_name(filename);
    match comp.parse(&line) {
        Ok(_) => EXIT_SUCCESS,
        // Already reported against the script.
        Err(_) => EXIT_FAILURE,
    }
}

pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstXfst2Fst");
    let (mut common, args) = cli::parse::<Args>(common, args)?;

    // The hand-copied common cases ("inc/getopt-cases-common.h" with
    // exceptions): verbosity is last-one-wins, colour has the auto->NEVER
    // mapping, and there is no '-o' at all.
    common.debug = args.debug;
    if args.verbose {
        common.verbose = true;
        common.silent = false;
    } else if args.quiet || args.silent {
        common.verbose = false;
        common.silent = true;
    }
    if let Some(colour) = args.colour(&common)? {
        common.colour = colour;
    }
    let (pipe_input, pipe_output) = args.pipe_flags(&common)?;
    let mut options = Options {
        output_format: match &args.format {
            // validate() already ran the loud parse; this pass is quiet so
            // the ambiguous-name warning is not repeated.
            Some(name) => parse_format_name_quiet(name),
            None => ImplementationType::UNSPECIFIED_TYPE,
        },
        scriptfilename: args.scriptfile.clone(),
        startupfilename: args.startupfile.clone(),
        execute_commands: args.execute.clone(),
        execute_command_and_quit: args.execute_and_quit.clone(),
        pipe_input,
        pipe_output,
        restricted_mode: args.restricted_mode,
        no_readline: args.no_readline,
        print_weight: args.print_weight,
    };
    if options.output_format == ImplementationType::UNSPECIFIED_TYPE {
        options.output_format = ImplementationType::TROPICAL_OPENFST_TYPE;
        verbose_print(
            &common,
            "Using default output format OpenFst \
             with tropical weight class\n",
        );
    }

    match options.output_format {
        ImplementationType::SFST_TYPE => {
            verbose_print(&common, "Using SFST as output handler\n");
        }
        ImplementationType::TROPICAL_OPENFST_TYPE => {
            verbose_print(&common, "Using OpenFst's tropical weights as output\n");
        }
        ImplementationType::FOMA_TYPE => {
            verbose_print(&common, "Using foma as output handler\n");
        }
        ImplementationType::HFST_OL_TYPE => {
            verbose_print(&common, "Using optimized lookup output\n");
        }
        ImplementationType::HFST_OLW_TYPE => {
            verbose_print(&common, "Using optimized lookup weighted output\n");
        }
        ImplementationType::THFST_TYPE => {
            verbose_print(&common, "Using thfst (directory) output\n");
        }
        ImplementationType::XFSM_TYPE
        | ImplementationType::HFST2_TYPE
        | ImplementationType::UNSPECIFIED_TYPE
        | ImplementationType::ERROR_TYPE => {
            error(
                &common,
                EXIT_FAILURE,
                0,
                "Unknown format cannot be used as output\n",
            );
            return Err(EXIT_FAILURE);
        }
    }

    if options.pipe_input && options.scriptfilename.is_some() {
        hfst_error(
            &common,
            EXIT_FAILURE,
            0,
            "--pipe-mode and --scriptfile cannot be used simultaneously\n",
        );
        return Err(EXIT_FAILURE);
    }

    if options.startupfilename.is_some() && options.scriptfilename.is_some() {
        hfst_error(
            &common,
            EXIT_FAILURE,
            0,
            "--startupfile and --scriptfile cannot be used simultaneously\n",
        );
        return Err(EXIT_FAILURE);
    }

    // Create XfstCompiler: the parsed --format is matched ONCE into the
    // compiler's backend type parameter ([dec:hfst:monomorphic-backends]).
    // A session is monomorphic in its backend: choosing foma here builds a
    // foma-native compiler, so `read regex @"foma"` stays foma end-to-end
    // (no convert-to-tropical), matching how the C++ session keeps a
    // transducer in its own format. (Mixing formats in ONE session is the
    // only thing this cannot express; even C++ rejects combining them.)
    cli::from_code(match options.output_format {
        ImplementationType::TROPICAL_OPENFST_TYPE => {
            run_compiler::<hfst_openfst::StdVectorFst>(&common, &options)
        }
        ImplementationType::FOMA_TYPE => {
            #[cfg(feature = "foma")]
            {
                run_compiler::<hfst::backend_foma::FomaTransducer>(&common, &options)
            }
            #[cfg(not(feature = "foma"))]
            {
                error(
                    &common,
                    EXIT_FAILURE,
                    0,
                    "the foma backend is not available in this build\n",
                );
                EXIT_FAILURE
            }
        }
        // Exhaustive backstop: no format may silently fall through to a
        // tropical compiler. None of these can back an xfst session in this
        // port — SFST and XFSM are unimplemented backends; the optimized-
        // lookup formats are lookup-only (not an algebra backend);
        // HFST2/UNSPECIFIED/ERROR are metadata/sentinel types (UNSPECIFIED
        // was already resolved to tropical in parse_options, and the first
        // match above emits the C++ 'Unknown format cannot be used as
        // output' error for the truly-unknown ones before reaching here).
        ImplementationType::SFST_TYPE
        | ImplementationType::XFSM_TYPE
        | ImplementationType::HFST_OL_TYPE
        | ImplementationType::HFST_OLW_TYPE
        | ImplementationType::THFST_TYPE
        | ImplementationType::HFST2_TYPE
        | ImplementationType::UNSPECIFIED_TYPE
        | ImplementationType::ERROR_TYPE => {
            error(
                &common,
                EXIT_FAILURE,
                0,
                &format!(
                    "format {} cannot be used as an xfst backend\n",
                    hfst::hfst_data_types::implementation_type_to_format(options.output_format)
                ),
            );
            EXIT_FAILURE
        }
    })
}

// [spec:hfst:req:xfst-cmd.shell-cancel]
/// Make Ctrl-C stop the running command instead of the process: SIGINT sets
/// the session's interrupt flag, and a second Ctrl-C while the first is still
/// pending ends the process, for a loop that does not poll the flag.
#[cfg(unix)]
fn cancel_on_ctrl_c<B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer>(
    common: &CommonOptions,
    comp: &mut XfstCompiler<B>,
) {
    use signal_hook::consts::SIGINT;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    // The session borrows the flag for as long as the process runs.
    let flag: &'static Arc<AtomicBool> = Box::leak(Box::new(Arc::new(AtomicBool::new(false))));
    let registered =
        signal_hook::flag::register_conditional_shutdown(SIGINT, 130, Arc::clone(flag))
            .and_then(|_| signal_hook::flag::register(SIGINT, Arc::clone(flag)));
    match registered {
        Ok(_) => {
            comp.set_interrupt(flag);
        }
        Err(e) => verbose_print(common, &format!("Ctrl-C will end the session ({e})\n")),
    }
}

#[cfg(not(unix))]
fn cancel_on_ctrl_c<B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer>(
    _common: &CommonOptions,
    _comp: &mut XfstCompiler<B>,
) {
}

// [spec:hfst:req:xfst-cmd.shell-line-editing]
/// The terminal line editor the shell reads every line through: cursor
/// keys, deletion and up/down recall of this session's lines.
struct LineEditor(Editor<Completions, DefaultHistory>);

impl LineEditor {
    fn new() -> rustyline::Result<LineEditor> {
        let mut editor = Editor::new()?;
        editor.set_helper(Some(Completions {
            defined: Vec::new(),
            files: FilenameCompleter::new(),
        }));
        Ok(LineEditor(editor))
    }
}

// [spec:hfst:req:xfst-cmd.shell-completion]
/// Tab completion: what to complete comes from the library, and file names
/// from the file system.
struct Completions {
    defined: Vec<String>,
    files: FilenameCompleter,
}

impl Completer for Completions {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        match complete(line, pos, &self.defined) {
            Completion::Words { start, candidates } => Ok((
                start,
                candidates
                    .into_iter()
                    .map(|c| Pair {
                        display: c.trim_end().to_string(),
                        replacement: c,
                    })
                    .collect(),
            )),
            Completion::File { .. } => self.files.complete(line, pos, ctx),
            Completion::None => Ok((pos, Vec::new())),
        }
    }
}

impl Hinter for Completions {
    type Hint = String;
}
impl Highlighter for Completions {}
impl Validator for Completions {}
impl Helper for Completions {}

impl LineSource for LineEditor {
    fn set_defined_names(&mut self, names: &[String]) {
        if let Some(completions) = self.0.helper_mut() {
            completions.defined = names.to_vec();
        }
    }

    fn read_line(&mut self, prompt: &str) -> std::io::Result<Option<String>> {
        // The editor draws on the terminal itself, after what the commands
        // wrote through stdout's buffer.
        std::io::stdout().flush()?;
        match self.0.readline(prompt) {
            Ok(line) => {
                if !line.trim().is_empty() {
                    self.0
                        .add_history_entry(line.as_str())
                        .map_err(std::io::Error::other)?;
                }
                Ok(Some(line))
            }
            Err(ReadlineError::Eof) => Ok(None),
            Err(ReadlineError::Interrupted) => Err(std::io::ErrorKind::Interrupted.into()),
            Err(ReadlineError::Io(e)) => Err(e),
            Err(e) => Err(std::io::Error::other(e)),
        }
    }
}

fn run_compiler<B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer>(
    common: &CommonOptions,
    options: &Options,
) -> i32 {
    let mut comp = XfstCompiler::<B>::new();
    comp.set_diagnostic_sink(crate::hfst_commandline::stderr_sink());
    // The session holds the stdin lock, and this driver reads its own input
    // through the session, so the commands that read input share the stream.
    comp.set_output_stream(std::io::stdout());
    comp.set_input_stream(std::io::stdin().lock());
    // HAVE_READLINE is not defined in this port.
    comp.set_readline(false);
    comp.set_verbosity(!common.silent);

    if options.print_weight {
        comp.set_prompt_verbosity(false);
        comp.set("print-weight", "ON")
            .expect("print-weight is a variable every session has");
        comp.set_prompt_verbosity(true);
    }

    if options.restricted_mode {
        comp.set_restricted_mode(true);
    }

    if !options.pipe_output {
        comp.set_output_to_console(true);
    }

    // (the C wraps the whole driving block in a try/catch on
    // TransducerTypeMismatchException; the Rust library reports that
    // condition through its own error path, so the catch arm is not
    // reproduced here.)

    // If needed, execute scripts given in command line
    for cmd in options.execute_commands.clone() {
        verbose_print(
            common,
            &format!("Executing xfst command '{}' given on command line\n", cmd),
        );
        match comp.parse(&cmd) {
            Ok(Flow::Continue) => {}
            Ok(Flow::Quit) => return EXIT_SUCCESS,
            Err(_) => return EXIT_FAILURE,
        }
    }
    // If needed, execute script given in command line, and quit
    if let Some(cmd) = options.execute_command_and_quit.clone() {
        verbose_print(
            common,
            &format!("Executing xfst command '{}' given on command line\n", cmd),
        );
        return match comp.parse(&cmd) {
            Ok(_) => EXIT_SUCCESS,
            Err(_) => EXIT_FAILURE,
        };
    }
    // If needed, execute script in startup file
    if let Some(startupfilename) = options.startupfilename.clone() {
        verbose_print(
            common,
            &format!("Executing startup file '{}'...\n", startupfilename),
        );
        if parse_file(common, &startupfilename, &mut comp) == EXIT_FAILURE {
            return EXIT_FAILURE;
        }
    }

    if options.pipe_input {
        verbose_print(common, "Reading from standard input...\n");
        comp.set_read_interactive_text_from_stdin(false);
        comp.set_prompt_verbosity(common.verbose);
        if parse_file(common, "<stdin>", &mut comp) == EXIT_FAILURE
        // if (0 != comp.parse(stdin)) segfaults with scriptfiles..
        {
            return EXIT_FAILURE;
        }
    } else if let Some(scriptfilename) = options.scriptfilename.clone() {
        verbose_print(
            common,
            &format!("Reading from script file '{}'\n", scriptfilename),
        );
        if parse_file(common, &scriptfilename, &mut comp) == EXIT_FAILURE {
            return EXIT_FAILURE;
        }
    }
    // Use interactive mode (the readline branch is not compiled in this
    // port, so USE_READLINE is always false here).
    else {
        verbose_print(common, "Starting interactive mode...\n");
        let on_terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        if on_terminal {
            // [spec:hfst:req:xfst-cmd.shell-session-edges]
            // The timing line is for build logs; at a terminal it is noise.
            skip_elapsed();
            if !common.silent {
                println!(
                    "hfst xfst, {}. Type 'help' for the commands, Ctrl-D to quit.",
                    version_line()
                );
            }
        }
        if on_terminal {
            cancel_on_ctrl_c(common, &mut comp);
        }
        if !options.no_readline && on_terminal {
            match LineEditor::new() {
                Ok(editor) => {
                    comp.set_line_source(Box::new(editor));
                }
                Err(e) => verbose_print(
                    common,
                    &format!("No line editor ({e}); reading plain lines\n"),
                ),
            }
        }
        if comp.interact().is_err() {
            return EXIT_FAILURE;
        }
    }
    EXIT_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(line: &str) -> (usize, Vec<(String, String)>) {
        let completions = Completions {
            defined: vec!["Foo".into(), "Bar".into()],
            files: FilenameCompleter::new(),
        };
        let history = DefaultHistory::new();
        let (start, pairs) = completions
            .complete(line, line.len(), &rustyline::Context::new(&history))
            .expect("completion runs");
        (
            start,
            pairs
                .into_iter()
                .map(|p| (p.display, p.replacement))
                .collect(),
        )
    }

    // [spec:hfst:req:xfst-cmd.shell-completion/test]
    // A command is listed by name and inserted with the space after it.
    #[test]
    fn a_command_candidate_inserts_its_space() {
        assert_eq!(
            candidates("ech"),
            (0, vec![("echo".into(), "echo ".into())])
        );
    }

    // [spec:hfst:req:xfst-cmd.shell-completion/test]
    #[test]
    fn a_defined_name_is_inserted_as_it_is() {
        assert_eq!(
            candidates("undefine B"),
            (9, vec![("Bar".into(), "Bar".into())])
        );
    }

    // [spec:hfst:req:xfst-cmd.shell-cancel/test]
    // One SIGINT, raised in this test's own process, sets the session's flag,
    // so the next command fails as interrupted instead of the process ending.
    #[cfg(unix)]
    #[test]
    fn ctrl_c_sets_the_sessions_interrupt() {
        let mut comp = XfstCompiler::<hfst_openfst::StdVectorFst>::new();
        cancel_on_ctrl_c(&CommonOptions::default(), &mut comp);
        signal_hook::low_level::raise(signal_hook::consts::SIGINT).expect("raise SIGINT");
        assert!(comp.parse("echo still here\n").is_err());
        assert!(
            comp.diagnostics()
                .iter()
                .any(|d| d.message() == "interrupted")
        );
    }

    // [spec:hfst:req:xfst-cmd.shell-completion/test]
    #[test]
    fn a_regex_body_completes_to_nothing() {
        assert_eq!(candidates("regex a"), (7, Vec::new()));
    }
}
