//! Faithful 1:1 port of tools/src/parsers/hfst-xfst.cc — the command-line
//! program for compiling XFST scripts or executing XFST commands
//! interactively. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options) plus the hfst XfstCompiler. The readline
//! branch (HAVE_READLINE) is not compiled in this port, so input always goes
//! through the plain line-reading interactive branch.

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, GETOPT_COLOUR, error, extend_options_from_env, hfst_error,
    hfst_parse_format_name, hfst_set_program_name, print_version, verbose_print,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use crate::inc::handle_error_case;
use hfst::hfst_data_types::ImplementationType;
use hfst::xfst_compiler::XfstCompiler;
use std::io::{BufRead, Read, Write};

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

// File-scope tool state, mirroring the static globals in the C++ source.
static mut OUTPUT_FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;
static mut SCRIPTFILENAME: Option<String> = None;
static mut STARTUPFILENAME: Option<String> = None;
static mut EXECUTE_COMMANDS: Vec<String> = Vec::new();
static mut EXECUTE_COMMAND_AND_QUIT: Option<String> = None;
static mut PIPE_INPUT: bool = false;
static mut PIPE_OUTPUT: bool = false; // this has no effect on non-windows platforms
static mut RESTRICTED_MODE: bool = false;
// HAVE_READLINE is not defined in this port.
static mut USE_READLINE: bool = false;
static mut PRINT_WEIGHT: bool = false;

fn execute_commands() -> &'static mut Vec<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(EXECUTE_COMMANDS) }
}

// [spec:hfst:def:hfst-xfst.print-usage-fn]
// [spec:hfst:sem:hfst-xfst.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...]\n\
         Compile XFST scripts or execute XFST commands interactively\n\
         \n",
        "hfst-xfst" /*program_name*/
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
// so the cases are hand-written here too.
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // copied from "inc/getopt-cases-common.h" (with exceptions)
            if c == 'd' as i32 {
                globals::DEBUG = true;
                continue;
            } else if c == 'h' as i32 {
                print_usage();
                return EXIT_SUCCESS;
            } else if c == 'V' as i32 {
                print_version();
                return EXIT_SUCCESS;
            } else if c == 'v' as i32 {
                globals::VERBOSE = true;
                globals::SILENT = false;
                continue;
            } else if c == 'q' as i32 || c == 's' as i32 {
                globals::VERBOSE = false;
                globals::SILENT = true;
                continue;
            } else if c == GETOPT_COLOUR {
                match getopt::optarg_opt().as_deref() {
                    None | Some("always") => {
                        globals::COLOUR = globals::ColourTristate::COLOUR_ALWAYS
                    }
                    // "auto" mapping to COLOUR_NEVER is preserved bug-for-bug
                    // from the C source.
                    Some("never") | Some("auto") => {
                        globals::COLOUR = globals::ColourTristate::COLOUR_NEVER
                    }
                    Some(other) => {
                        hfst_error(
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
                    OUTPUT_FORMAT = hfst_parse_format_name(&getopt::optarg());
                    continue;
                }
                'F' => {
                    SCRIPTFILENAME = Some(getopt::optarg());
                    continue;
                }
                'e' => {
                    execute_commands().push(getopt::optarg());
                    continue;
                }
                'E' => {
                    EXECUTE_COMMAND_AND_QUIT = Some(getopt::optarg());
                    continue;
                }
                'l' => {
                    STARTUPFILENAME = Some(getopt::optarg());
                    continue;
                }
                'p' => {
                    match getopt::optarg_opt().as_deref() {
                        None => {
                            PIPE_INPUT = true;
                            PIPE_OUTPUT = true;
                        }
                        Some("both") | Some("BOTH") => {
                            PIPE_INPUT = true;
                            PIPE_OUTPUT = true;
                        }
                        Some("input") | Some("INPUT") | Some("in") | Some("IN") => {
                            PIPE_INPUT = true;
                        }
                        Some("output") | Some("OUTPUT") | Some("out") | Some("OUT") => {
                            PIPE_OUTPUT = true;
                        }
                        Some(other) => {
                            error(
                                EXIT_FAILURE,
                                0,
                                &format!("--pipe-mode argument {} unrecognised", other),
                            );
                        }
                    }
                    continue;
                }
                'r' => {
                    USE_READLINE = false;
                    continue;
                }
                'w' => {
                    PRINT_WEIGHT = true;
                    continue;
                }
                'R' => {
                    RESTRICTED_MODE = true;
                    continue;
                }
                'k' => {
                    PIPE_OUTPUT = true;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        if OUTPUT_FORMAT == ImplementationType::UNSPECIFIED_TYPE {
            OUTPUT_FORMAT = ImplementationType::TROPICAL_OPENFST_TYPE;
            verbose_print(
                "Using default output format OpenFst \
                 with tropical weight class\n",
            );
        }

        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-xfst.parse-file-fn]
// [spec:hfst:sem:hfst-xfst.parse-file-fn]
//
// Parse file 'filename' using compiler 'comp'.
// Filename "<stdin>" uses stdin for reading.
fn parse_file(filename: &str, comp: &mut XfstCompiler) -> i32 {
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
            EXIT_FAILURE,
            0,
            &format!("error when reading file {}\n", filename),
        );
        return EXIT_FAILURE;
    };

    if 0 != comp.parse_line(line) {
        hfst_error(
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
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstXfst2Fst");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        match OUTPUT_FORMAT {
            ImplementationType::SFST_TYPE => {
                verbose_print("Using SFST as output handler\n");
            }
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                verbose_print("Using OpenFst's tropical weights as output\n");
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                verbose_print("Using OpenFst's log weight output\n");
            }
            ImplementationType::FOMA_TYPE => {
                verbose_print("Using foma as output handler\n");
            }
            ImplementationType::HFST_OL_TYPE => {
                verbose_print("Using optimized lookup output\n");
            }
            ImplementationType::HFST_OLW_TYPE => {
                verbose_print("Using optimized lookup weighted output\n");
            }
            _ => {
                error(EXIT_FAILURE, 0, "Unknown format cannot be used as output\n");
                return EXIT_FAILURE;
            }
        }

        if PIPE_INPUT && (*std::ptr::addr_of!(SCRIPTFILENAME)).is_some() {
            hfst_error(
                EXIT_FAILURE,
                0,
                "--pipe-mode and --scriptfile cannot be used simultaneously\n",
            );
            return EXIT_FAILURE;
        }

        if (*std::ptr::addr_of!(STARTUPFILENAME)).is_some()
            && (*std::ptr::addr_of!(SCRIPTFILENAME)).is_some()
        {
            hfst_error(
                EXIT_FAILURE,
                0,
                "--startupfile and --scriptfile cannot be used simultaneously\n",
            );
            return EXIT_FAILURE;
        }

        // Create XfstCompiler
        let mut comp = XfstCompiler::new_with_impl(OUTPUT_FORMAT);
        // HAVE_READLINE is not defined in this port.
        comp.set_readline(false);
        comp.set_verbosity(!globals::SILENT);

        if PRINT_WEIGHT {
            comp.set_prompt_verbosity(false);
            comp.set("print-weight", "ON");
            comp.set_prompt_verbosity(true);
        }

        if RESTRICTED_MODE {
            comp.set_restricted_mode(true);
        }

        if !PIPE_OUTPUT {
            comp.set_output_to_console(true);
        }

        // (the C wraps the whole driving block in a try/catch on
        // TransducerTypeMismatchException; the Rust library reports that
        // condition through its own error path, so the catch arm is not
        // reproduced here.)

        // If needed, execute scripts given in command line
        for cmd in execute_commands().clone() {
            verbose_print(&format!(
                "Executing xfst command '{}' given on command line\n",
                cmd
            ));
            if 0 != comp.parse_line(cmd.clone()) {
                hfst_error(
                    EXIT_FAILURE,
                    0,
                    &format!("command '{}' could not be parsed\n", cmd),
                );
                return EXIT_FAILURE;
            }
        }
        // If needed, execute script given in command line, and quit
        if let Some(cmd) = (*std::ptr::addr_of!(EXECUTE_COMMAND_AND_QUIT)).clone() {
            verbose_print(&format!(
                "Executing xfst command '{}' given on command line\n",
                cmd
            ));
            if 0 != comp.parse_line(cmd.clone()) {
                hfst_error(
                    EXIT_FAILURE,
                    0,
                    &format!("command '{}' could not be parsed\n", cmd),
                );
                return EXIT_FAILURE;
            }
            return EXIT_SUCCESS;
        }
        // If needed, execute script in startup file
        if let Some(startupfilename) = (*std::ptr::addr_of!(STARTUPFILENAME)).clone() {
            verbose_print(&format!(
                "Executing startup file '{}'...\n",
                startupfilename
            ));
            if parse_file(&startupfilename, &mut comp) == EXIT_FAILURE {
                return EXIT_FAILURE;
            }
        }

        if PIPE_INPUT {
            verbose_print("Reading from standard input...\n");
            comp.set_read_interactive_text_from_stdin(false);
            comp.set_prompt_verbosity(globals::VERBOSE);
            if parse_file("<stdin>", &mut comp) == EXIT_FAILURE
            // if (0 != comp.parse(stdin)) segfaults with scriptfiles..
            {
                return EXIT_FAILURE;
            }
        } else if let Some(scriptfilename) = (*std::ptr::addr_of!(SCRIPTFILENAME)).clone() {
            verbose_print(&format!("Reading from script file '{}'\n", scriptfilename));
            if parse_file(&scriptfilename, &mut comp) == EXIT_FAILURE {
                return EXIT_FAILURE;
            }
        }
        // Use interactive mode (the readline branch is not compiled in this
        // port, so USE_READLINE is always false here).
        else {
            verbose_print("Starting interactive mode...\n");
            comp.set_prompt_verbosity(!globals::SILENT);
            comp.set_read_interactive_text_from_stdin(true);
            if !globals::SILENT {
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
                    if !globals::SILENT {
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
                    if !globals::SILENT {
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
}
