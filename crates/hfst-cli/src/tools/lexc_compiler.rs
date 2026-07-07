//! Faithful 1:1 port of tools/src/hfst-lexc-compiler.cc — the lexc compilation
//! command-line tool. Drives the hfst-cli foundation (getopt, commandline,
//! program-options, tool-metadata, inc fragments) and the now-available
//! hfst::lexc::LexcCompiler library API.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.
//!
//! Compile lexc files into a transducer.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_parse_format_name, hfst_set_program_name, hfst_warning,
    redirect_converting, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use crate::hfst_tool_metadata::{hfst_set_formula, hfst_set_name};
use crate::inc::{CaseResult, check_common_params, handle_common_case, handle_error_case};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::lexc::LexcCompiler;
use std::io::Write;

/// hfst-lexc-compiler's own options (the former tool-specific `static mut`s).
struct Options {
    /// The lexc input filenames (a "<stdin>" entry is the standard-input sentinel).
    // The C kept a parallel FILE* array (LEXCFILES) of fopen'd lexc inputs; but
    // the file content is read by filename via std::fs::read_to_string in
    // lexc_streams, and the only thing the FILE* was used for was the stdin
    // sentinel. After the io-foundation de-C-ism that array is gone — the
    // "<stdin>" filename serves as the sentinel directly.
    lexcfilenames: Vec<String>,
    lexccount: u32,
    is_input_stdin: bool,
    format: ImplementationType,
    align_strings: bool,
    with_flags: bool,
    minimize_flags: bool,
    rename_flags: bool,
    treat_warnings_as_errors: bool,
    warn_everything: bool,
    warn_missing_lexicons: bool,
    warn_unused_lexicons: bool,
    warn_repeated_lexicons: bool,
    warn_one_sided_flags: bool,
    warn_missing_alphabets: bool,
    warn_unnecessary_escapes: bool,
    /// Compatibility with Xerox tools is the default.
    xerox_composition: bool,
    encode_weights: bool,
    /// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition'
    /// file-static global; now threaded into the lexc compiler via
    /// 'set_flag_is_epsilon').
    flag_is_epsilon: bool,
    split_characters: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            lexcfilenames: Vec::new(),
            lexccount: 0,
            is_input_stdin: true,
            format: ImplementationType::UNSPECIFIED_TYPE,
            align_strings: false,
            with_flags: false,
            minimize_flags: false,
            rename_flags: false,
            treat_warnings_as_errors: false,
            warn_everything: false,
            warn_missing_lexicons: false,
            warn_unused_lexicons: false,
            warn_repeated_lexicons: false,
            warn_one_sided_flags: false,
            warn_missing_alphabets: false,
            warn_unnecessary_escapes: false,
            xerox_composition: true,
            encode_weights: false,
            flag_is_epsilon: false,
            split_characters: false,
        }
    }
}

fn eput(s: &str) {
    let _ = std::io::stderr().write_all(s.as_bytes());
}

// [spec:hfst:def:hfst-lexc-compiler.print-usage-fn]
// [spec:hfst:sem:hfst-lexc-compiler.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1...]]\nCompile lexc files into transducer\n\n",
        common.program_name
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
        common.program_name
    );
}

// [spec:hfst:def:hfst-lexc-compiler.parse-options-fn]
// [spec:hfst:sem:hfst-lexc-compiler.parse-options-fn]
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then the tool's own, then the terminal error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        // tool-specific cases
        let cc = c as u8 as char;
        match cc {
            'A' => {
                options.align_strings = true;
                continue;
            }
            'E' => {
                options.encode_weights = true;
                continue;
            }
            'f' => {
                options.format = hfst_parse_format_name(&common, &opt.optarg());
                continue;
            }
            'F' => {
                options.with_flags = true;
                continue;
            }
            'M' => {
                options.minimize_flags = true;
                continue;
            }
            'R' => {
                options.rename_flags = true;
                continue;
            }
            'x' => {
                let argument = opt.optarg();
                if argument == "yes" || argument == "true" || argument == "ON" {
                    options.xerox_composition = true;
                } else if argument == "no" || argument == "false" || argument == "OFF" {
                    options.xerox_composition = false;
                } else {
                    eput(&format!(
                        "Error: unknown option to --xerox-composition: '{}'\n",
                        opt.optarg()
                    ));
                    return Err(1);
                }
                continue;
            }
            'X' => {
                let argument = opt.optarg();
                if argument == "flag-is-epsilon" {
                    options.flag_is_epsilon = true;
                } else {
                    eput(&format!(
                        "Error: unknown option to --xfst: '{}'\n",
                        opt.optarg()
                    ));
                    return Err(1);
                }
                continue;
            }
            'Q' => {
                options.treat_warnings_as_errors = true;
                options.warn_one_sided_flags = true;
                options.warn_missing_lexicons = true;
                options.warn_unused_lexicons = true;
                options.warn_repeated_lexicons = true;
                // compatibility?? might change later:
                options.warn_unnecessary_escapes = false;
                options.warn_missing_alphabets = false;
                eput("Warning: --Werror is deprecated, use -Werror -Wall instead\n");
                continue;
            }
            'W' => {
                let optarg = opt.optarg();
                if optarg == "error" {
                    options.treat_warnings_as_errors = true;
                } else if optarg == "all" {
                    options.warn_one_sided_flags = true;
                    options.warn_everything = true;
                    options.warn_missing_lexicons = true;
                    options.warn_unused_lexicons = true;
                    options.warn_repeated_lexicons = true;
                    options.warn_missing_alphabets = true;
                    options.warn_unnecessary_escapes = true;
                    options.warn_missing_alphabets = true;
                } else if optarg == "one-sided-flags" {
                    options.warn_one_sided_flags = true;
                } else if optarg == "no-one-sided-flags" {
                    options.warn_one_sided_flags = false;
                } else if optarg == "unused-lexicons" {
                    options.warn_unused_lexicons = true;
                } else if optarg == "no-unused-lexicons" {
                    options.warn_unused_lexicons = false;
                } else if optarg == "repeated-lexicons" {
                    options.warn_repeated_lexicons = true;
                } else if optarg == "no-repeated-lexicons" {
                    options.warn_repeated_lexicons = false;
                } else if optarg == "missing-lexicons" {
                    options.warn_missing_lexicons = true;
                } else if optarg == "no-missing-lexicons" {
                    options.warn_missing_lexicons = false;
                } else if optarg == "missing-alphabets" {
                    options.warn_missing_alphabets = true;
                } else if optarg == "no-missing-alphabets" {
                    options.warn_missing_alphabets = false;
                } else if optarg == "unnecessary-escapes" {
                    options.warn_unnecessary_escapes = true;
                } else if optarg == "no-unnecessary-escapes" {
                    options.warn_unnecessary_escapes = false;
                } else {
                    eput(&format!("Unknown warning option {}\n", optarg));
                    return Err(1);
                }
                continue;
            }
            '9' => {
                options.split_characters = true;
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    if options.format == ImplementationType::UNSPECIFIED_TYPE {
        if !common.silent {
            hfst_warning(&common, 0, 0, "Defaulting to OpenFst tropical type");
        }
        options.format = ImplementationType::TROPICAL_OPENFST_TYPE;
    }

    if args.len() > opt.optind {
        while opt.optind < args.len() {
            let name = args[opt.optind].clone();
            // C: lexcfiles.push(hfst_fopen(name, "r")); a "-" resolved to stdin,
            // otherwise the named file was opened (erroring on failure). The
            // content is read by filename later, so only validate openability and
            // record "<stdin>" for "-".
            if name == "-" {
                options.lexcfilenames.push("<stdin>".to_string());
            } else {
                if std::fs::File::open(&name).is_err() {
                    error(&common, 1, 0, &format!("Could not open '{}'. ", name));
                }
                options.lexcfilenames.push(name.clone());
            }
            options.lexccount += 1;
            opt.optind += 1;
        }
        options.is_input_stdin = false;
    } else {
        options.lexcfilenames.push("<stdin>".to_string());
        options.is_input_stdin = true;
        options.lexccount += 1;
    }
    Ok((common, options))
}

// [spec:hfst:def:hfst-lexc-compiler.lexc-streams-fn]
// [spec:hfst:sem:hfst-lexc-compiler.lexc-streams-fn]
fn lexc_streams<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &Options,
    lexc: &mut LexcCompiler<B>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let lexcfilenames = &options.lexcfilenames;
    for i in 0..(options.lexccount as usize) {
        verbose_print(common, &format!("Parsing lexc file {}\n", lexcfilenames[i]));
        if lexcfilenames[i] == "<stdin>" {
            // The new Rust LexcCompiler::parse takes the source text, so we
            // read the whole of standard input into a string (mirroring the
            // C++ 'lexc.parse(stdin)').
            let mut source = String::new();
            use std::io::Read;
            let _ = std::io::stdin().read_to_string(&mut source);
            lexc.set_source_name(&lexcfilenames[i]);
            if let Err(e) = lexc.parse(&source) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        } else {
            // Read the named file's contents into a string (mirroring the
            // C++ 'lexc.parse(filename)').
            let source = std::fs::read_to_string(&lexcfilenames[i]).unwrap_or_default();
            lexc.set_source_name(&lexcfilenames[i]);
            if let Err(e) = lexc.parse(&source) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
    }
    verbose_print(common, "Compiling... ");
    let compiled = match lexc.compile_lexical() {
        Ok(c) => c,
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    };
    let Some(mut res) = compiled else {
        if options.lexccount == 1 {
            error(
                common,
                1,
                0,
                &format!(
                    "The file {} did not compile cleanly.\n(if there are no error messages above, try -v or -d to get more info)",
                    lexcfilenames[0]
                ),
            );
        } else {
            error(
                common,
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
    verbose_print(common, "\nWriting... ");
    if let Err(e) = redirect_converting(outstream, &mut res) {
        error(common, 1, 0, &format!("{e}"));
        return 1;
    }
    verbose_print(common, "done\n");
    // C++ 'delete res' — owned value drops at end of scope.
    outstream.close();

    0
}

// [spec:hfst:def:hfst-lexc-compiler.main-fn]
// [spec:hfst:sem:hfst-lexc-compiler.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstLexc");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    verbose_print(&common, "Reading from ");
    for i in 0..(options.lexccount as usize) {
        verbose_print(&common, &format!("{}, ", options.lexcfilenames[i]));
    }
    verbose_print(&common, &format!("writing to {}\n", common.output_filename));
    // here starts the buffer handling part
    let output_opened = common.output_filename != "<stdout>";
    let outstream_res = if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, options.format, true)
    } else {
        HfstOutputStream::new(options.format, true)
    };
    let mut outstream = match outstream_res {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };
    // The parsed --format is matched ONCE into the compiler's backend
    // type ([dec:hfst:monomorphic-backends]); optimized-lookup formats
    // compile at tropical and convert at the write.
    match options.format {
        ImplementationType::LOG_OPENFST_TYPE => {
            run_typed::<hfst::log_weight_transducer::LogFst>(&common, &options, &mut outstream)
        }
        _ => run_typed::<hfst_openfst::StdVectorFst>(&common, &options, &mut outstream),
    }
}

fn run_typed<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &Options,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut lexc = LexcCompiler::<B>::new_with_flags(options.with_flags, options.align_strings);
    lexc.set_minimize_flags(options.minimize_flags);
    lexc.set_rename_flags(options.rename_flags);
    lexc.set_flag_is_epsilon(options.flag_is_epsilon);
    lexc.set_xerox_composition(options.xerox_composition);
    // lexc.with_flags = with_flags;
    if common.silent {
        lexc.set_verbosity(0);
    } else {
        lexc.set_verbosity(if common.verbose { 2 } else { 1 });
    }
    if options.treat_warnings_as_errors {
        lexc.set_treat_warnings_as_errors(true);
    }
    lexc.set_warning("-Wone-sided-flags", options.warn_one_sided_flags);
    lexc.set_warning("-Wunused-lexicons", options.warn_unused_lexicons);
    lexc.set_warning("-Wrepeated-lexicons", options.warn_repeated_lexicons);
    lexc.set_warning("-Wmissing-lexicons", options.warn_missing_lexicons);
    lexc.set_warning("-Wmissing-alphabets", options.warn_missing_alphabets);
    lexc.set_warning("-Wunnecessary-escapes", options.warn_unnecessary_escapes);
    if !common.silent && common.verbose {
        let mut line = String::from("Warning settings: ");
        if options.treat_warnings_as_errors {
            line.push_str(" -Werror (fail on all warnings)");
        }
        if options.warn_one_sided_flags {
            line.push_str(" -Wone-sided-flags");
        }
        if options.warn_unused_lexicons {
            line.push_str(" -Wunused-lexicons");
        }
        if options.warn_repeated_lexicons {
            line.push_str(" -Wrepeated-lexicons");
        }
        if options.warn_missing_lexicons {
            line.push_str(" -Wmissing-lexicons");
        }
        if options.warn_missing_alphabets {
            line.push_str(" -Wmissing-alphabets");
        }
        if options.warn_unnecessary_escapes {
            line.push_str(" -Wunnecessary-escapes");
        }
        line.push('\n');
        print!("{}", line);
    }
    if options.split_characters {
        eput("Warningn: Disabling unicode character tokenisation\n");
        lexc.set_split_characters(true);
    }
    // The C++ also frees the filename buffers here; the Rust owners drop
    // automatically.
    lexc_streams(common, options, &mut lexc, outstream)
}
