//! Faithful 1:1 port of tools/src/parsers/hfst-xfst.cc — the command-line
//! program for compiling XFST scripts or executing XFST commands
//! interactively. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options) plus the hfst XfstCompiler. The readline
//! branch (HAVE_READLINE) is not compiled in this port, so input always goes
//! through the plain line-reading interactive branch.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::{ColourTristate, CommonOptions};
use crate::hfst_commandline::{
    GETOPT_COLOUR, error, extend_options_from_env, hfst_error, hfst_parse_format_name,
    hfst_set_program_name, print_version, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use crate::inc::handle_error_case;
use hfst::hfst_data_types::ImplementationType;
use hfst::xfst_compiler::XfstCompiler;
use std::io::{BufRead, Read, Write};

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
    // HAVE_READLINE is not defined in this port.
    use_readline: bool,
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
            use_readline: false,
            print_weight: false,
        }
    }
}

// [spec:hfst:def:hfst-xfst.print-usage-fn]
// [spec:hfst:sem:hfst-xfst.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...]\n\
         Compile XFST scripts or execute XFST commands interactively\n\
         \n",
        common.program_name
    );

    print_common_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(msg, "Xfst-specific options:\n");
    let _ = write!(
        msg,
        "\x20 -e, --execute=CMD          Execute command CMD on startup\n\
         \x20 -E, --execute-and-quit=CMD Execute command CMD, and quit\n\
         \x20 -f, --format=FMT           Write result using FMT as backend format\n\
         \x20 -F, --scriptfile=FILE      Read commands from FILE, and quit\n\
         \x20 -l, --startupfile=FILE     Read commands from FILE on startup\n\
         \x20 -p, --pipe-mode[=STREAM]   Control input and output streams\n\
         \x20 -r, --no-readline          Do not use readline library for input\n\
         \x20 -w, --print-weight         Print weights for each operation\n\
         \x20 -R, --restricted-mode      Allow read and write operations only in current\n\
         \x20                            directory, do not allow system calls\n\
         \n\
         Option --execute can be invoked many times.\n\
         If FMT is not given, OpenFst's tropical format will be used.\n\
         The possible values for FMT are {{ foma, openfst-tropical, openfst-log, sfst }}.\n\
         Readline library, if enabled when configuring, is used for input by default.\n\
         Input files are always treated as UTF-8.\n\
         \n\
         STREAM can be {{ input, output, both }}. If not given, defaults to {{both}}.\n\
         If input file is not specified with -F, input is read interactively line by\n\
         line from the user. If you redirect input from a file, use --pipe-mode=input.\n\
         --pipe-mode=output is ignored on non-windows platforms.\n"
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-xfst.parse-options-fn]
// [spec:hfst:sem:hfst-xfst.parse-options-fn]
//
// The C++ copies the common getopt cases inline "with exceptions" (no '-o'
// case; '--colour=auto' maps to COLOUR_NEVER) rather than #include'ing them,
// so the cases are hand-written here too. `Err(code)` is an exit code the
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
        // add tool-specific options here
        let tool_opts: [(&'static str, i32, i32); 9] = [
            ("format", getopt::REQUIRED_ARGUMENT, 'f' as i32),
            ("scriptfile", getopt::REQUIRED_ARGUMENT, 'F' as i32),
            ("execute", getopt::REQUIRED_ARGUMENT, 'e' as i32),
            ("execute-and-quit", getopt::REQUIRED_ARGUMENT, 'E' as i32),
            ("startupfile", getopt::REQUIRED_ARGUMENT, 'l' as i32),
            ("pipe-mode", getopt::OPTIONAL_ARGUMENT, 'p' as i32),
            ("no-readline", getopt::NO_ARGUMENT, 'r' as i32),
            ("print-weight", getopt::NO_ARGUMENT, 'w' as i32),
            ("restricted-mode", getopt::NO_ARGUMENT, 'R' as i32),
        ];
        for (name, has_arg, val) in tool_opts {
            long_options.push(getopt::GetOpt { name, has_arg, val });
        }
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // copied from "inc/getopt-cases-common.h" (with exceptions)
        if c == 'd' as i32 {
            common.debug = true;
            continue;
        } else if c == 'h' as i32 {
            print_usage(&common);
            return Err(EXIT_SUCCESS);
        } else if c == 'V' as i32 {
            print_version(&common);
            return Err(EXIT_SUCCESS);
        } else if c == 'v' as i32 {
            common.verbose = true;
            common.silent = false;
            continue;
        } else if c == 'q' as i32 || c == 's' as i32 {
            common.verbose = false;
            common.silent = true;
            continue;
        } else if c == GETOPT_COLOUR {
            match opt.optarg_opt().as_deref() {
                None | Some("always") => common.colour = ColourTristate::COLOUR_ALWAYS,
                // "auto" mapping to COLOUR_NEVER is preserved bug-for-bug
                // from the C source.
                Some("never") | Some("auto") => common.colour = ColourTristate::COLOUR_NEVER,
                Some(other) => {
                    hfst_error(
                        &common,
                        EXIT_FAILURE,
                        0,
                        &format!(
                            "--colour must be one of always, never, or auto, not {}",
                            other
                        ),
                    );
                }
            }
            continue;
        }
        match c as u8 as char {
            'f' => {
                options.output_format = hfst_parse_format_name(&common, &opt.optarg());
                continue;
            }
            'F' => {
                options.scriptfilename = Some(opt.optarg());
                continue;
            }
            'e' => {
                options.execute_commands.push(opt.optarg());
                continue;
            }
            'E' => {
                options.execute_command_and_quit = Some(opt.optarg());
                continue;
            }
            'l' => {
                options.startupfilename = Some(opt.optarg());
                continue;
            }
            'p' => {
                match opt.optarg_opt().as_deref() {
                    None => {
                        options.pipe_input = true;
                        options.pipe_output = true;
                    }
                    Some("both") | Some("BOTH") => {
                        options.pipe_input = true;
                        options.pipe_output = true;
                    }
                    Some("input") | Some("INPUT") | Some("in") | Some("IN") => {
                        options.pipe_input = true;
                    }
                    Some("output") | Some("OUTPUT") | Some("out") | Some("OUT") => {
                        options.pipe_output = true;
                    }
                    Some(other) => {
                        error(
                            &common,
                            EXIT_FAILURE,
                            0,
                            &format!("--pipe-mode argument {} unrecognised", other),
                        );
                    }
                }
                continue;
            }
            'r' => {
                options.use_readline = false;
                continue;
            }
            'w' => {
                options.print_weight = true;
                continue;
            }
            'R' => {
                options.restricted_mode = true;
                continue;
            }
            'k' => {
                options.pipe_output = true;
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    if options.output_format == ImplementationType::UNSPECIFIED_TYPE {
        options.output_format = ImplementationType::TROPICAL_OPENFST_TYPE;
        verbose_print(
            &common,
            "Using default output format OpenFst \
             with tropical weight class\n",
        );
    }

    Ok((common, options))
}

// [spec:hfst:def:hfst-xfst.parse-file-fn]
// [spec:hfst:sem:hfst-xfst.parse-file-fn]
//
// Parse file 'filename' using compiler 'comp'.
// Filename "<stdin>" uses stdin for reading.
fn parse_file<B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer>(
    common: &CommonOptions,
    filename: &str,
    comp: &mut XfstCompiler<B>,
) -> i32 {
    // hfst_file_to_mem(filename): the whole file (or stdin) as one string.
    let line = if filename == "<stdin>" {
        let mut s = String::new();
        match std::io::stdin().read_to_string(&mut s) {
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

    if 0 != comp.parse_line(line) {
        hfst_error(
            common,
            EXIT_FAILURE,
            0,
            &format!("error when parsing file {}\n", filename),
        );
        return EXIT_FAILURE;
    }
    0
}

// [spec:hfst:def:hfst-xfst.expression-continues-fn]
// [spec:hfst:sem:hfst-xfst.expression-continues-fn]
fn expression_continues(expr: &mut String) -> bool {
    // get rid of extra newlines...
    if expr.ends_with('\n') {
        expr.pop();
    }
    // and carriage returns
    if expr.ends_with('\r') {
        expr.pop();
    }
    if expr.ends_with('\\') {
        expr.pop();
        expr.push('\n');
        return true;
    }
    false
}

// [spec:hfst:def:hfst-xfst.main-fn]
// [spec:hfst:sem:hfst-xfst.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstXfst2Fst");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    match options.output_format {
        ImplementationType::SFST_TYPE => {
            verbose_print(&common, "Using SFST as output handler\n");
        }
        ImplementationType::TROPICAL_OPENFST_TYPE => {
            verbose_print(&common, "Using OpenFst's tropical weights as output\n");
        }
        ImplementationType::LOG_OPENFST_TYPE => {
            verbose_print(&common, "Using OpenFst's log weight output\n");
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
            return EXIT_FAILURE;
        }
    }

    if options.pipe_input && options.scriptfilename.is_some() {
        hfst_error(
            &common,
            EXIT_FAILURE,
            0,
            "--pipe-mode and --scriptfile cannot be used simultaneously\n",
        );
        return EXIT_FAILURE;
    }

    if options.startupfilename.is_some() && options.scriptfilename.is_some() {
        hfst_error(
            &common,
            EXIT_FAILURE,
            0,
            "--startupfile and --scriptfile cannot be used simultaneously\n",
        );
        return EXIT_FAILURE;
    }

    // Create XfstCompiler: the parsed --format is matched ONCE into the
    // compiler's backend type parameter ([dec:hfst:monomorphic-backends]).
    // A session is monomorphic in its backend: choosing foma here builds a
    // foma-native compiler, so `read regex @"foma"` stays foma end-to-end
    // (no convert-to-tropical), matching how the C++ session keeps a
    // transducer in its own format. (Mixing formats in ONE session is the
    // only thing this cannot express; even C++ rejects combining them.)
    match options.output_format {
        ImplementationType::TROPICAL_OPENFST_TYPE => {
            run_compiler::<hfst_openfst::StdVectorFst>(&common, &options)
        }
        ImplementationType::LOG_OPENFST_TYPE => {
            run_compiler::<hfst::log_weight_transducer::LogFst>(&common, &options)
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
    }
}

fn run_compiler<B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer>(
    common: &CommonOptions,
    options: &Options,
) -> i32 {
    let mut comp = XfstCompiler::<B>::new_with_impl();
    // HAVE_READLINE is not defined in this port.
    comp.set_readline(false);
    comp.set_verbosity(!common.silent);

    if options.print_weight {
        comp.set_prompt_verbosity(false);
        comp.set("print-weight", "ON");
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
        if 0 != comp.parse_line(cmd.clone()) {
            hfst_error(
                common,
                EXIT_FAILURE,
                0,
                &format!("command '{}' could not be parsed\n", cmd),
            );
            return EXIT_FAILURE;
        }
    }
    // If needed, execute script given in command line, and quit
    if let Some(cmd) = options.execute_command_and_quit.clone() {
        verbose_print(
            common,
            &format!("Executing xfst command '{}' given on command line\n", cmd),
        );
        if 0 != comp.parse_line(cmd.clone()) {
            hfst_error(
                common,
                EXIT_FAILURE,
                0,
                &format!("command '{}' could not be parsed\n", cmd),
            );
            return EXIT_FAILURE;
        }
        return EXIT_SUCCESS;
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
        comp.set_prompt_verbosity(!common.silent);
        comp.set_read_interactive_text_from_stdin(true);
        if !common.silent {
            comp.prompt();
            let _ = std::io::stdout().flush();
        }
        // support for backspace

        let mut expression = String::new();
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let mut line = String::new();
        loop {
            line.clear();
            if input.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            expression.push_str(line.trim_end_matches('\n'));
            // C: std::cin.getline strips the newline; expression_continues
            // then handles a trailing '\r' / '\\'.
            if expression_continues(&mut expression) {
                if !common.silent {
                    comp.prompt();
                    let _ = std::io::stdout().flush();
                }
                continue;
            }

            if 0 != comp.parse_line(format!("{}\n", expression)) {
                eprintln!("expression '{}' could not be parsed", expression);
                if comp.get("quit-on-fail") == "ON" {
                    return EXIT_FAILURE;
                }
                if !common.silent {
                    comp.prompt();
                    let _ = std::io::stdout().flush();
                }
            }
            if comp.quit_requested() {
                break;
            }

            expression = String::new();
        }
    }
    EXIT_SUCCESS
}
