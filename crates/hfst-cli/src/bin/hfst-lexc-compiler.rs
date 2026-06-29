//! Faithful 1:1 port of tools/src/hfst-lexc-compiler.cc — the lexc compilation
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments) and the now-
//! available hfst::lexc::LexcCompiler library API.
//!
//! Compile lexc files into a transducer.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{
    get_encode_weights, set_encode_weights, set_flag_is_epsilon_in_composition,
    set_xerox_composition,
};
use hfst::lexc::LexcCompiler;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_parse_format_name, hfst_set_program_name,
    hfst_warning, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, hfst_getopt_common_long, print_common_program_options,
};
use hfst_cli::hfst_tool_metadata::{hfst_set_formula, hfst_set_name};
use hfst_cli::inc::{CaseResult, check_common_params, handle_common_case, handle_error_case};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};
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
static mut ENC: bool = false;
static mut SPLIT_CHARACTERS: bool = false;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

fn fput(f: &mut dyn std::io::Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

fn eput(s: &str) {
    let _ = std::io::stderr().write_all(s.as_bytes());
}

// [spec:hfst:def:hfst-lexc-compiler.print-usage-fn]
// [spec:hfst:sem:hfst-lexc-compiler.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE1...]]\nCompile lexc files into transducer\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Input/Output options:\n  -f, --format=FORMAT     compile into FORMAT transducer\n  -o, --output=OUTFILE    write result into OUTFILE\n",
        );
        fput(
            &mut *msg,
            "Lexc options:\n  -A, --alignStrings      align characters in input and output strings\n  -E, --encode-weights    encode weights when minimizing (default is false)\n  -F, --withFlags         use flags to hyperminimize result\n  -M, --minimizeFlags     if --withFlags is used, minimize the number of flags\n  -R, --renameFlags       if --withFlags and --minimizeFlags are used, rename\n                          flags (for testing)\n  -x,\n  --xerox-composition=BOOL   Whether flag diacritics are treated as ordinary\n                             symbols in composition (default is true).\n  -X, --xfst=VARIABLE     toggle xfst compatibility option VARIABLE.\n   --split-characters     disable unicode character parsing for multichars\n   -Wall                  enable all warnings:\n   -Wone-sided-flags      warn about one sided flag diacritics\n   -Wrepeated-lexicons    warn about repeat lexicon names\n   -Wmissing-lexicons     warn about lexicons used but missing\n   -Wunused-lexicons      warn about lexicons defined but unused\n   -Wmissing-alphabets    warn about implicit alphabets\n   -Wunnecessary-escapes  warn about unneeded %-escapes\n   -Werror                treat warnings as errors\n",
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            "If INFILE or OUTFILE are omitted or -, standard streams will be used\nThe possible values for FORMAT are { sfst, openfst-tropical, openfst-log,\nfoma, optimized-lookup-unweighted, optimized-lookup-weighted }.\nBOOL is one of {true,ON,yes} or {false,OFF,no}.\nXfst variables are {flag-is-epsilon (default OFF)}.\n",
        );
        fput(
            &mut *msg,
            &format!(
                "\nExamples:\n  {} -o cat.hfst cat.lexc               Compile single-file lexicon\n  {} -o L.hfst Root.lexc 2.lexc 3.lexc  Compile multi-file lexicon\n\nUsing weights:\n  LEXICON Root\n  cat # \"weight: 2\" ;    Define weight for a word\n  <[dog::1]+> # ;        Use weights in regular expressions\n\nUsing weights has an effect only if FORMAT is weighted, i.e.\n{{ openfst-tropical, openfst-log, optimized-lookup-weighted }}.\n\n",
                program_name, program_name
            ),
        );
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-lexc-compiler.parse-options-fn]
// [spec:hfst:sem:hfst-lexc-compiler.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            let encode_weights_name = CString::new("encode-weights").unwrap();
            let format_name = CString::new("format").unwrap();
            let output_name = CString::new("output").unwrap();
            let align_strings_name = CString::new("alignStrings").unwrap();
            let with_flags_name = CString::new("withFlags").unwrap();
            let minimize_flags_name = CString::new("minimizeFlags").unwrap();
            let rename_flags_name = CString::new("renameFlags").unwrap();
            let xerox_composition_name = CString::new("xerox-composition").unwrap();
            let xfst_name = CString::new("xfst").unwrap();
            let werror_name = CString::new("Werror").unwrap();
            let wstuff_name = CString::new("Wstuff").unwrap();
            let split_characters_name = CString::new("split-characters").unwrap();
            long_options.push(getopt::Option {
                name: encode_weights_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'E' as c_int,
            });
            long_options.push(getopt::Option {
                name: format_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: output_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'o' as c_int,
            });
            long_options.push(getopt::Option {
                name: align_strings_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'A' as c_int,
            });
            long_options.push(getopt::Option {
                name: with_flags_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'F' as c_int,
            });
            long_options.push(getopt::Option {
                name: minimize_flags_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'M' as c_int,
            });
            long_options.push(getopt::Option {
                name: rename_flags_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'R' as c_int,
            });
            long_options.push(getopt::Option {
                name: xerox_composition_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'x' as c_int,
            });
            long_options.push(getopt::Option {
                name: xfst_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'X' as c_int,
            });
            long_options.push(getopt::Option {
                name: werror_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'Q' as c_int,
            });
            long_options.push(getopt::Option {
                name: wstuff_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'W' as c_int,
            });
            long_options.push(getopt::Option {
                name: split_characters_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: '9' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short =
                CString::new(format!("{}Ef:o:AFMRx:X:QW:9", HFST_GETOPT_COMMON_SHORT)).unwrap();
            let mut option_index: c_int = 0;
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then the tool's own, then the terminal error arm.
            match handle_common_case(c, || print_usage()) {
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
                    FORMAT = hfst_parse_format_name(&cstr(getopt::OPTARG));
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
                    let argument = cstr(getopt::OPTARG);
                    if argument == "yes" || argument == "true" || argument == "ON" {
                        XEROX_COMPOSITION = true;
                    } else if argument == "no" || argument == "false" || argument == "OFF" {
                        XEROX_COMPOSITION = false;
                    } else {
                        eput(&format!(
                            "Error: unknown option to --xerox-composition: '{}'\n",
                            cstr(getopt::OPTARG)
                        ));
                        return libc::EXIT_FAILURE;
                    }
                    continue;
                }
                'X' => {
                    let argument = cstr(getopt::OPTARG);
                    if argument == "flag-is-epsilon" {
                        set_flag_is_epsilon_in_composition(true);
                    } else {
                        eput(&format!(
                            "Error: unknown option to --xfst: '{}'\n",
                            cstr(getopt::OPTARG)
                        ));
                        return libc::EXIT_FAILURE;
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
                    let optarg = cstr(getopt::OPTARG);
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
                        return libc::EXIT_FAILURE;
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

        if argc - getopt::OPTIND > 0 {
            while getopt::OPTIND < argc {
                let arg = *argv.offset(getopt::OPTIND as isize);
                let name = cstr(arg);
                // C: lexcfiles.push(hfst_fopen(name, "r")); a "-" resolved to stdin,
                // otherwise the named file was opened (erroring on failure). The
                // content is read by filename later, so only validate openability and
                // record "<stdin>" for "-".
                if name == "-" {
                    (*std::ptr::addr_of_mut!(LEXCFILENAMES)).push("<stdin>".to_string());
                } else {
                    if std::fs::File::open(&name).is_err() {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!("Could not open '{}'. ", name),
                        );
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
unsafe fn lexc_streams(lexc: &mut LexcCompiler, outstream: &mut HfstOutputStream) -> c_int {
    unsafe {
        let lexcfilenames = &*std::ptr::addr_of!(LEXCFILENAMES);
        for i in 0..(LEXCCOUNT as usize) {
            verbose_printf(&format!("Parsing lexc file {}\n", lexcfilenames[i]));
            if lexcfilenames[i] == "<stdin>" {
                // The new Rust LexcCompiler::parse takes the source text, so we
                // read the whole of standard input into a string (mirroring the
                // C++ 'lexc.parse(stdin)').
                let mut source = String::new();
                use std::io::Read;
                let _ = std::io::stdin().read_to_string(&mut source);
                lexc.parse(&source);
            } else {
                // Read the named file's contents into a string (mirroring the
                // C++ 'lexc.parse(filename)').
                let source = std::fs::read_to_string(&lexcfilenames[i]).unwrap_or_default();
                lexc.parse(&source);
            }
        }
        verbose_printf("Compiling... ");
        let Some(mut res) = lexc.compile_lexical() else {
            if LEXCCOUNT == 1 {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    &format!(
                        "The file {} did not compile cleanly.\n(if there are no error messages above, try -v or -d to get more info)",
                        lexcfilenames[0]
                    ),
                );
            } else {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    &format!(
                        "The files {}... did not compile cleanly.\n(if there are no error messages above, try -v or -d to get more info)",
                        lexcfilenames[0]
                    ),
                );
            }
            return libc::EXIT_FAILURE;
        };
        hfst_set_name(&mut res, &lexcfilenames[0], "lexc");
        hfst_set_formula(&mut res, &lexcfilenames[0], "L");
        verbose_printf("\nWriting... ");
        outstream.redirect(&mut res);
        verbose_printf("done\n");
        // C++ 'delete res' — owned value drops at end of scope.
        outstream.close();

        if ENCODE_WEIGHTS {
            set_encode_weights(ENC);
        }

        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-lexc-compiler.main-fn]
// [spec:hfst:sem:hfst-lexc-compiler.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt and
        // extend_options_getenv reorder/replace it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstLexc");

        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        ENC = get_encode_weights();
        if ENCODE_WEIGHTS {
            set_encode_weights(true);
        }

        verbose_printf("Reading from ");
        let lexcfilenames = &*std::ptr::addr_of!(LEXCFILENAMES);
        for i in 0..(LEXCCOUNT as usize) {
            verbose_printf(&format!("{}, ", lexcfilenames[i]));
        }
        verbose_printf(&format!("writing to {}\n", cstr(globals::OUTFILENAME)));
        // here starts the buffer handling part
        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), FORMAT, true)
        } else {
            HfstOutputStream::new(FORMAT, true)
        };
        set_xerox_composition(XEROX_COMPOSITION);
        let mut lexc = LexcCompiler::new_with_flags(FORMAT, WITH_FLAGS, ALIGN_STRINGS);
        lexc.set_minimize_flags(MINIMIZE_FLAGS);
        lexc.set_rename_flags(RENAME_FLAGS);
        // lexc.with_flags_ = with_flags;
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
            let c = CString::new(line).unwrap_or_default();
            libc::printf(c"%s".as_ptr(), c.as_ptr());
        }
        if SPLIT_CHARACTERS {
            eput("Warningn: Disabling unicode character tokenisation\n");
            lexc.set_split_characters(true);
        }
        let retval = lexc_streams(&mut lexc, &mut outstream);
        // The C++ also frees the filename buffers here; the Rust owners drop
        // automatically.
        retval
    }
}
