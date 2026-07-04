//! Faithful 1:1 port of tools/src/hfst-lexc-compiler.cc — the lexc compilation
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments) and the now-
//! available hfst::lexc::LexcCompiler library API.
//!
//! Compile lexc files into a transducer.

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_parse_format_name, hfst_set_program_name,
    hfst_warning, redirect_converting, verbose_print,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use crate::hfst_tool_metadata::{hfst_set_formula, hfst_set_name};
use crate::inc::{CaseResult, check_common_params, handle_common_case, handle_error_case};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::lexc::LexcCompiler;
use std::io::Write;

// ---------------------------------------------------------------------------
// Tool-global state. C: file-scope static variables (#include globals-common.h
// plus the lexc-specific statics).
// ---------------------------------------------------------------------------

static mut LEXCFILENAMES: Vec<String> = Vec::new();
// The C kept a parallel FILE* array (LEXCFILES) of fopen'd lexc inputs; but the
// file content is read by filename via std::fs::read_to_string in lexc_streams,
// and the only thing the FILE* was used for was the stdin sentinel. After the
// io-foundation de-C-ism that array is gone — the "<stdin>" filename serves as the
// sentinel directly.
static mut LEXCCOUNT: u32 = 0;
static mut IS_INPUT_STDIN: bool = true;
static mut FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;
static mut ALIGN_STRINGS: bool = false;
static mut WITH_FLAGS: bool = false;
static mut MINIMIZE_FLAGS: bool = false;
static mut RENAME_FLAGS: bool = false;
static mut TREAT_WARNINGS_AS_ERRORS: bool = false;
static mut WARN_EVERYTHING: bool = false;
static mut WARN_MISSING_LEXICONS: bool = false;
static mut WARN_UNUSED_LEXICONS: bool = false;
static mut WARN_REPEATED_LEXICONS: bool = false;
static mut WARN_ONE_SIDED_FLAGS: bool = false;
static mut WARN_MISSING_ALPHABETS: bool = false;
static mut WARN_UNNECESSARY_ESCAPES: bool = false;
// Compatibility with Xerox tools is the default
static mut XEROX_COMPOSITION: bool = true;
static mut ENCODE_WEIGHTS: bool = false;
// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition' file-static
// global; now threaded into the lexc compiler via 'set_flag_is_epsilon').
static mut FLAG_IS_EPSILON: bool = false;
static mut SPLIT_CHARACTERS: bool = false;

fn eput(s: &str) {
    let _ = std::io::stderr().write_all(s.as_bytes());
}

// [spec:hfst:def:hfst-lexc-compiler.print-usage-fn]
// [spec:hfst:sem:hfst-lexc-compiler.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1...]]\nCompile lexc files into transducer\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Input/Output options:\n  -f, --format=FORMAT     compile into FORMAT transducer\n  -o, --output=OUTFILE    write result into OUTFILE\n"
    );
    let _ = write!(
        msg,
        "Lexc options:\n  -A, --alignStrings      align characters in input and output strings\n  -E, --encode-weights    encode weights when minimizing (default is false)\n  -F, --withFlags         use flags to hyperminimize result\n  -M, --minimizeFlags     if --withFlags is used, minimize the number of flags\n  -R, --renameFlags       if --withFlags and --minimizeFlags are used, rename\n                          flags (for testing)\n  -x,\n  --xerox-composition=BOOL   Whether flag diacritics are treated as ordinary\n                             symbols in composition (default is true).\n  -X, --xfst=VARIABLE     toggle xfst compatibility option VARIABLE.\n   --split-characters     disable unicode character parsing for multichars\n   -Wall                  enable all warnings:\n   -Wone-sided-flags      warn about one sided flag diacritics\n   -Wrepeated-lexicons    warn about repeat lexicon names\n   -Wmissing-lexicons     warn about lexicons used but missing\n   -Wunused-lexicons      warn about lexicons defined but unused\n   -Wmissing-alphabets    warn about implicit alphabets\n   -Wunnecessary-escapes  warn about unneeded %-escapes\n   -Werror                treat warnings as errors\n"
    );
    let _ = write!(msg, "\n");
    let _ = msg.write_all(
        "If INFILE or OUTFILE are omitted or -, standard streams will be used\nThe possible values for FORMAT are { sfst, openfst-tropical, openfst-log,\nfoma, optimized-lookup-unweighted, optimized-lookup-weighted }.\nBOOL is one of {true,ON,yes} or {false,OFF,no}.\nXfst variables are {flag-is-epsilon (default OFF)}.\n"
            .as_bytes(),
    );
    let _ = write!(
        msg,
        "\nExamples:\n  {0} -o cat.hfst cat.lexc               Compile single-file lexicon\n  {0} -o L.hfst Root.lexc 2.lexc 3.lexc  Compile multi-file lexicon\n\nUsing weights:\n  LEXICON Root\n  cat # \"weight: 2\" ;    Define weight for a word\n  <[dog::1]+> # ;        Use weights in regular expressions\n\nUsing weights has an effect only if FORMAT is weighted, i.e.\n{{ openfst-tropical, openfst-log, optimized-lookup-weighted }}.\n\n",
        globals::program_name()
    );
}

// [spec:hfst:def:hfst-lexc-compiler.parse-options-fn]
// [spec:hfst:sem:hfst-lexc-compiler.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.push(getopt::GetOpt {
                name: "encode-weights",
                has_arg: getopt::NO_ARGUMENT,
                val: 'E' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "format",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'f' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "output",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'o' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "alignStrings",
                has_arg: getopt::NO_ARGUMENT,
                val: 'A' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "withFlags",
                has_arg: getopt::NO_ARGUMENT,
                val: 'F' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "minimizeFlags",
                has_arg: getopt::NO_ARGUMENT,
                val: 'M' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "renameFlags",
                has_arg: getopt::NO_ARGUMENT,
                val: 'R' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "xerox-composition",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'x' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "xfst",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'X' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "Werror",
                has_arg: getopt::NO_ARGUMENT,
                val: 'Q' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "Wstuff",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'W' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "split-characters",
                has_arg: getopt::NO_ARGUMENT,
                val: '9' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then the tool's own, then the terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            // tool-specific cases
            let cc = c as u8 as char;
            match cc {
                'A' => {
                    ALIGN_STRINGS = true;
                    continue;
                }
                'E' => {
                    ENCODE_WEIGHTS = true;
                    continue;
                }
                'f' => {
                    FORMAT = hfst_parse_format_name(&getopt::optarg());
                    continue;
                }
                'F' => {
                    WITH_FLAGS = true;
                    continue;
                }
                'M' => {
                    MINIMIZE_FLAGS = true;
                    continue;
                }
                'R' => {
                    RENAME_FLAGS = true;
                    continue;
                }
                'x' => {
                    let argument = getopt::optarg();
                    if argument == "yes" || argument == "true" || argument == "ON" {
                        XEROX_COMPOSITION = true;
                    } else if argument == "no" || argument == "false" || argument == "OFF" {
                        XEROX_COMPOSITION = false;
                    } else {
                        eput(&format!(
                            "Error: unknown option to --xerox-composition: '{}'\n",
                            getopt::optarg()
                        ));
                        return 1;
                    }
                    continue;
                }
                'X' => {
                    let argument = getopt::optarg();
                    if argument == "flag-is-epsilon" {
                        FLAG_IS_EPSILON = true;
                    } else {
                        eput(&format!(
                            "Error: unknown option to --xfst: '{}'\n",
                            getopt::optarg()
                        ));
                        return 1;
                    }
                    continue;
                }
                'Q' => {
                    TREAT_WARNINGS_AS_ERRORS = true;
                    WARN_ONE_SIDED_FLAGS = true;
                    WARN_MISSING_LEXICONS = true;
                    WARN_UNUSED_LEXICONS = true;
                    WARN_REPEATED_LEXICONS = true;
                    // compatibility?? might change later:
                    WARN_UNNECESSARY_ESCAPES = false;
                    WARN_MISSING_ALPHABETS = false;
                    eput("Warning: --Werror is deprecated, use -Werror -Wall instead\n");
                    continue;
                }
                'W' => {
                    let optarg = getopt::optarg();
                    if optarg == "error" {
                        TREAT_WARNINGS_AS_ERRORS = true;
                    } else if optarg == "all" {
                        WARN_ONE_SIDED_FLAGS = true;
                        WARN_EVERYTHING = true;
                        WARN_MISSING_LEXICONS = true;
                        WARN_UNUSED_LEXICONS = true;
                        WARN_REPEATED_LEXICONS = true;
                        WARN_MISSING_ALPHABETS = true;
                        WARN_UNNECESSARY_ESCAPES = true;
                        WARN_MISSING_ALPHABETS = true;
                    } else if optarg == "one-sided-flags" {
                        WARN_ONE_SIDED_FLAGS = true;
                    } else if optarg == "no-one-sided-flags" {
                        WARN_ONE_SIDED_FLAGS = false;
                    } else if optarg == "unused-lexicons" {
                        WARN_UNUSED_LEXICONS = true;
                    } else if optarg == "no-unused-lexicons" {
                        WARN_UNUSED_LEXICONS = false;
                    } else if optarg == "repeated-lexicons" {
                        WARN_REPEATED_LEXICONS = true;
                    } else if optarg == "no-repeated-lexicons" {
                        WARN_REPEATED_LEXICONS = false;
                    } else if optarg == "missing-lexicons" {
                        WARN_MISSING_LEXICONS = true;
                    } else if optarg == "no-missing-lexicons" {
                        WARN_MISSING_LEXICONS = false;
                    } else if optarg == "missing-alphabets" {
                        WARN_MISSING_ALPHABETS = true;
                    } else if optarg == "no-missing-alphabets" {
                        WARN_MISSING_ALPHABETS = false;
                    } else if optarg == "unnecessary-escapes" {
                        WARN_UNNECESSARY_ESCAPES = true;
                    } else if optarg == "no-unnecessary-escapes" {
                        WARN_UNNECESSARY_ESCAPES = false;
                    } else {
                        eput(&format!("Unknown warning option {}\n", optarg));
                        return 1;
                    }
                    continue;
                }
                '9' => {
                    SPLIT_CHARACTERS = true;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        if FORMAT == ImplementationType::UNSPECIFIED_TYPE {
            if !globals::SILENT {
                hfst_warning(0, 0, "Defaulting to OpenFst tropical type");
            }
            FORMAT = ImplementationType::TROPICAL_OPENFST_TYPE;
        }

        if args.len() > getopt::OPTIND {
            while getopt::OPTIND < args.len() {
                let name = args[getopt::OPTIND].clone();
                // C: lexcfiles.push(hfst_fopen(name, "r")); a "-" resolved to stdin,
                // otherwise the named file was opened (erroring on failure). The
                // content is read by filename later, so only validate openability and
                // record "<stdin>" for "-".
                if name == "-" {
                    (*std::ptr::addr_of_mut!(LEXCFILENAMES)).push("<stdin>".to_string());
                } else {
                    if std::fs::File::open(&name).is_err() {
                        error(1, 0, &format!("Could not open '{}'. ", name));
                    }
                    (*std::ptr::addr_of_mut!(LEXCFILENAMES)).push(name.clone());
                }
                LEXCCOUNT += 1;
                getopt::OPTIND += 1;
            }
            IS_INPUT_STDIN = false;
        } else {
            (*std::ptr::addr_of_mut!(LEXCFILENAMES)).push("<stdin>".to_string());
            IS_INPUT_STDIN = true;
            LEXCCOUNT += 1;
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-lexc-compiler.lexc-streams-fn]
// [spec:hfst:sem:hfst-lexc-compiler.lexc-streams-fn]
unsafe fn lexc_streams<B: hfst::backend::AlgebraBackend>(
    lexc: &mut LexcCompiler<B>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    unsafe {
        let lexcfilenames = &*std::ptr::addr_of!(LEXCFILENAMES);
        for i in 0..(LEXCCOUNT as usize) {
            verbose_print(&format!("Parsing lexc file {}\n", lexcfilenames[i]));
            if lexcfilenames[i] == "<stdin>" {
                // The new Rust LexcCompiler::parse takes the source text, so we
                // read the whole of standard input into a string (mirroring the
                // C++ 'lexc.parse(stdin)').
                let mut source = String::new();
                use std::io::Read;
                let _ = std::io::stdin().read_to_string(&mut source);
                if let Err(e) = lexc.parse(&source) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            } else {
                // Read the named file's contents into a string (mirroring the
                // C++ 'lexc.parse(filename)').
                let source = std::fs::read_to_string(&lexcfilenames[i]).unwrap_or_default();
                if let Err(e) = lexc.parse(&source) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            }
        }
        verbose_print("Compiling... ");
        let compiled = match lexc.compile_lexical() {
            Ok(c) => c,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        let Some(mut res) = compiled else {
            if LEXCCOUNT == 1 {
                error(
                    1,
                    0,
                    &format!(
                        "The file {} did not compile cleanly.\n(if there are no error messages above, try -v or -d to get more info)",
                        lexcfilenames[0]
                    ),
                );
            } else {
                error(
                    1,
                    0,
                    &format!(
                        "The files {}... did not compile cleanly.\n(if there are no error messages above, try -v or -d to get more info)",
                        lexcfilenames[0]
                    ),
                );
            }
            return 1;
        };
        hfst_set_name(&mut res, &lexcfilenames[0], "lexc");
        hfst_set_formula(&mut res, &lexcfilenames[0], "L");
        verbose_print("\nWriting... ");
        if let Err(e) = redirect_converting(outstream, &mut res) {
            error(1, 0, &format!("{e}"));
            return 1;
        }
        verbose_print("done\n");
        // C++ 'delete res' — owned value drops at end of scope.
        outstream.close();

        0
    }
}

// [spec:hfst:def:hfst-lexc-compiler.main-fn]
// [spec:hfst:sem:hfst-lexc-compiler.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstLexc");

        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        verbose_print("Reading from ");
        let lexcfilenames = &*std::ptr::addr_of!(LEXCFILENAMES);
        for i in 0..(LEXCCOUNT as usize) {
            verbose_print(&format!("{}, ", lexcfilenames[i]));
        }
        verbose_print(&format!("writing to {}\n", globals::output_filename()));
        // here starts the buffer handling part
        let output_opened = globals::output_filename() != "<stdout>";
        let outstream_res = if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), FORMAT, true)
        } else {
            HfstOutputStream::new(FORMAT, true)
        };
        let mut outstream = match outstream_res {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // The parsed --format is matched ONCE into the compiler's backend
        // type ([dec:hfst:monomorphic-backends]); optimized-lookup formats
        // compile at tropical and convert at the write.
        match FORMAT {
            ImplementationType::LOG_OPENFST_TYPE => {
                run_typed::<hfst::log_weight_transducer::LogFst>(&mut outstream)
            }
            _ => run_typed::<hfst_openfst::StdVectorFst>(&mut outstream),
        }
    }
}

unsafe fn run_typed<B: hfst::backend::AlgebraBackend>(outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut lexc = LexcCompiler::<B>::new_with_flags(WITH_FLAGS, ALIGN_STRINGS);
        lexc.set_minimize_flags(MINIMIZE_FLAGS);
        lexc.set_rename_flags(RENAME_FLAGS);
        lexc.set_flag_is_epsilon(FLAG_IS_EPSILON);
        lexc.set_xerox_composition(XEROX_COMPOSITION);
        // lexc.with_flags = with_flags;
        if globals::SILENT {
            lexc.set_verbosity(0);
        } else {
            lexc.set_verbosity(if globals::VERBOSE { 2 } else { 1 });
        }
        if TREAT_WARNINGS_AS_ERRORS {
            lexc.set_treat_warnings_as_errors(true);
        }
        lexc.set_warning("-Wone-sided-flags", WARN_ONE_SIDED_FLAGS);
        lexc.set_warning("-Wunused-lexicons", WARN_UNUSED_LEXICONS);
        lexc.set_warning("-Wrepeated-lexicons", WARN_REPEATED_LEXICONS);
        lexc.set_warning("-Wmissing-lexicons", WARN_MISSING_LEXICONS);
        lexc.set_warning("-Wmissing-alphabets", WARN_MISSING_ALPHABETS);
        lexc.set_warning("-Wunnecessary-escapes", WARN_UNNECESSARY_ESCAPES);
        if !globals::SILENT && globals::VERBOSE {
            let mut line = String::from("Warning settings: ");
            if TREAT_WARNINGS_AS_ERRORS {
                line.push_str(" -Werror (fail on all warnings)");
            }
            if WARN_ONE_SIDED_FLAGS {
                line.push_str(" -Wone-sided-flags");
            }
            if WARN_UNUSED_LEXICONS {
                line.push_str(" -Wunused-lexicons");
            }
            if WARN_REPEATED_LEXICONS {
                line.push_str(" -Wrepeated-lexicons");
            }
            if WARN_MISSING_LEXICONS {
                line.push_str(" -Wmissing-lexicons");
            }
            if WARN_MISSING_ALPHABETS {
                line.push_str(" -Wmissing-alphabets");
            }
            if WARN_UNNECESSARY_ESCAPES {
                line.push_str(" -Wunnecessary-escapes");
            }
            line.push('\n');
            print!("{}", line);
        }
        if SPLIT_CHARACTERS {
            eput("Warningn: Disabling unicode character tokenisation\n");
            lexc.set_split_characters(true);
        }
        let retval = lexc_streams(&mut lexc, outstream);
        // The C++ also frees the filename buffers here; the Rust owners drop
        // automatically.
        retval
    }
}
