//! Faithful 1:1 port of tools/src/hfst-twolc/src/hfst-twolc.cc — the twolc
//! two-level grammar compiling command-line tool — together with its bespoke
//! option parser libhfst/src/parsers/commandline_src/CommandLine.{h,cc}.
//! Drives the hfst TwolcCompiler (which replaces the three htwolcpre
//! Flex/Bison preprocessor passes with the nfst-twolc parser + AST walk).

use crate::hfst_getopt::{self as getopt, Getopt};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::twolc::TwolcCompiler;
use std::io::{Read, Write};

// The 'PROGRAM_NAME' macro of the C++ CommandLine ("hfst-twolc"): the name
// baked into the usage/version texts, independent of argv[0].
const PROGRAM_NAME: &str = "hfst-twolc";
// PACKAGE_STRING expands to "" when config.h is absent (as elsewhere in the
// CLI port).
const PACKAGE_STRING: &str = "";

/// The parsed command line, mirroring the C++ 'class CommandLine' data
/// members (input_file/output_file stream handles excluded — streams are
/// opened where they are used).
// [spec:hfst:def:command-line.command-line]
struct CommandLine {
    be_verbose: bool,
    be_quiet: bool,
    has_input_file: bool,
    input_file_name: String,
    has_output_file: bool,
    output_file_name: String,
    resolve_left_conflicts: bool,
    resolve_right_conflicts: bool,
    help: bool,
    version: bool,
    usage: bool,
    has_debug_file: bool,
    format: ImplementationType,
}

impl CommandLine {
    // [spec:hfst:def:command-line.command-line.print-version-fn]
    // [spec:hfst:sem:command-line.command-line.print-version-fn]
    fn print_version(&self) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dversion
        let f = &mut std::io::stderr();
        let _ = write!(
            f,
            "\n{} {} ({})\n\
             Copyright (C) 2010 University of Helsinki,\n\
             License GPLv3: GNU GPL version 3 \n\
             <http://gnu.org/licenses/gpl.html>\n\
             This is free software: you are free to change and \n\
             redistribute it.\n\
             There is NO WARRANTY, to the extent permitted by law.\n\n",
            PROGRAM_NAME, 0, PACKAGE_STRING
        );
    }

    // [spec:hfst:def:command-line.command-line.print-usage-fn]
    // [spec:hfst:sem:command-line.command-line.print-usage-fn]
    fn print_usage(&self) {
        let f = &mut std::io::stderr();
        let _ = write!(
            f,
            "\nUsage: {0} [OPTIONS...] INFILE\n\
             Usage: {0} [OPTIONS...] -i INFILE\n\
             Usage: {0} [OPTIONS...] --input=INFILE\n\
             Usage: cat INFILE | {0} [OPTIONS...]\n\
             An input file has to be given either using the option -i or\n\
             --input, as the last commandline argument or from STDIN.\n\n",
            PROGRAM_NAME
        );
    }

    // [spec:hfst:def:command-line.command-line.print-help-fn]
    // [spec:hfst:sem:command-line.command-line.print-help-fn]
    fn print_help(&self) {
        self.print_usage();
        let f = &mut std::io::stderr();
        let _ = write!(
            f,
            "\nRead a twolc grammar, compile it and store it. If INFILE is \n\
             missing, the grammar is read from STDIN. If there is no output\n\
             file given using -o or --output, the compiled grammar is\n\
             written to STDOUT.\n\n"
        );
        let _ = write!(
            f,
            "Common options:\n\
             \x20 -h, --help               Print help message\n\
             \x20 -V, --version            Print version info\n\
             \x20 -u, --usage              Print usage\n\
             \x20 -v, --verbose            Print verbosely while processing\n\
             \x20 -q, --quiet              Do not print output\n\
             \x20 -s, --silent             Alias of --quiet\n"
        );
        let _ = write!(
            f,
            "Input/Output options:\n\
             \x20 -i, --input=INFILE       Read input transducer from INFILE\n\
             \x20 -o, --output=OUTFILE     Write output transducer to OUTFILE\n"
        );
        let _ = write!(
            f,
            "TwolC grammar options:\n\
             \x20 -R, --resolve            Resolve left-arrow conflicts.\n\
             \x20 -D, --dont-resolve-right Don't resolve right-arrow conflicts.\n\
             \x20 -f, --format=FORMAT      Store result in format FORMAT.\n\n"
        );
        let _ = write!(
            f,
            "Format may be one of openfst-log, openfst-tropical, foma or sfst.\n\n"
        );
        let _ = write!(
            f,
            "By default format is openfst-tropical. By default right arrow \n\
             conflicts are resolved and left arrow conflicts are not resolved.\n\n"
        );
    }

    // [spec:hfst:def:command-line.command-line.parse-options-fn]
    // [spec:hfst:sem:command-line.command-line.parse-options-fn]
    //
    // The C++ error paths call 'exit(1)' directly; here they return
    // Err(1) and 'run' propagates the exit code.
    fn parse_options(&mut self, args: &mut Vec<String>) -> Result<(), i32> {
        let mut resolve_left = false;
        let mut resolve_right = true;
        let mut verbose = false;
        let mut silent = false;
        let mut outfilename: Option<String> = None;
        let mut output_named = false;
        let mut input_named = false;
        let mut is_debug = false;
        let mut infilename: Option<String> = None;
        let mut debug_file_name: Option<String> = None;
        let mut form = ImplementationType::TROPICAL_OPENFST_TYPE;

        // The getopt parser state (was the file-scope static-mut globals) lives
        // in this owned value and is threaded through the loop.
        let mut opt = Getopt::new();
        loop {
            // The C long-option table names '--resolve-left' where the help
            // text (and the Giella build macros) say '--resolve'; both names
            // are accepted here, mapping to the same 'R'.
            let long_options: [(&'static str, i32, i32); 13] = [
                ("help", getopt::NO_ARGUMENT, 'h' as i32),
                ("version", getopt::NO_ARGUMENT, 'V' as i32),
                ("verbose", getopt::NO_ARGUMENT, 'v' as i32),
                ("quiet", getopt::NO_ARGUMENT, 'q' as i32),
                ("silent", getopt::NO_ARGUMENT, 's' as i32),
                ("usage", getopt::NO_ARGUMENT, 'u' as i32),
                ("input", getopt::REQUIRED_ARGUMENT, 'i' as i32),
                ("output", getopt::REQUIRED_ARGUMENT, 'o' as i32),
                ("resolve", getopt::NO_ARGUMENT, 'R' as i32),
                ("resolve-left", getopt::NO_ARGUMENT, 'R' as i32),
                ("dont-resolve-right", getopt::NO_ARGUMENT, 'D' as i32),
                ("debug_file", getopt::REQUIRED_ARGUMENT, 'd' as i32),
                ("format", getopt::REQUIRED_ARGUMENT, 'f' as i32),
            ];
            let table: Vec<getopt::GetOpt> = long_options
                .iter()
                .map(|&(name, has_arg, val)| getopt::GetOpt { name, has_arg, val })
                .collect();
            let c = opt.getopt_long(args, &table);
            if -1 == c {
                break;
            }

            match c as u8 as char {
                'h' => {
                    self.help = true;
                }
                'V' => {
                    self.version = true;
                }
                'u' => {
                    self.usage = true;
                }
                'v' => {
                    verbose = true;
                }
                'q' => {
                    silent = true;
                }
                's' => {
                    silent = true;
                }
                'R' => {
                    resolve_left = true;
                }
                'D' => {
                    resolve_right = false;
                }
                'i' => {
                    input_named = true;
                    infilename = Some(opt.optarg());
                }
                'd' => {
                    is_debug = true;
                    debug_file_name = Some(opt.optarg());
                }
                'o' => {
                    output_named = true;
                    outfilename = Some(opt.optarg());
                }
                'f' => {
                    let optarg = opt.optarg();
                    // The two leading standalone 'if's are preserved
                    // bug-for-bug from the C: "tropical-weight" and
                    // "tropical" set the format but still fall into the
                    // else-if chain's terminal error arm.
                    if optarg == "tropical-weight" {
                        form = ImplementationType::TROPICAL_OPENFST_TYPE;
                    }
                    if optarg == "tropical" {
                        form = ImplementationType::TROPICAL_OPENFST_TYPE;
                    }
                    if optarg == "log" {
                        form = ImplementationType::LOG_OPENFST_TYPE;
                    } else if optarg == "tropical-openfst" {
                        form = ImplementationType::TROPICAL_OPENFST_TYPE;
                    } else if optarg == "openfst-tropical" {
                        form = ImplementationType::TROPICAL_OPENFST_TYPE;
                    } else if optarg == "log-weight" {
                        form = ImplementationType::LOG_OPENFST_TYPE;
                    } else if optarg == "log-openfst" {
                        form = ImplementationType::LOG_OPENFST_TYPE;
                    } else if optarg == "openfst-log" {
                        form = ImplementationType::LOG_OPENFST_TYPE;
                    } else if optarg == "openfst" {
                        form = ImplementationType::TROPICAL_OPENFST_TYPE;
                    } else if optarg == "weighted" {
                        form = ImplementationType::TROPICAL_OPENFST_TYPE;
                    } else if optarg == "weight" {
                        form = ImplementationType::TROPICAL_OPENFST_TYPE;
                    } else if optarg == "sfst" {
                        form = ImplementationType::SFST_TYPE;
                    } else if optarg == "foma" {
                        form = ImplementationType::FOMA_TYPE;
                    } else if optarg == "unweighted" {
                        form = ImplementationType::FOMA_TYPE;
                    } else {
                        eprintln!(
                            "Unknown format \"{}\".Try running with option -h or --help.",
                            optarg
                        );
                        return Err(1);
                    }
                }
                ':' => {
                    let optopt = opt.optopt;
                    eprintln!(
                        "Missing argument for -{}. Try using --help.",
                        optopt as u8 as char
                    );
                    return Err(1);
                }
                _ => {
                    let optopt = opt.optopt;
                    eprintln!(
                        "Unknown commandline option: -{}. Try using --help.",
                        optopt as u8 as char
                    );
                    return Err(1);
                }
            }
        }

        let optind = opt.optind;
        if !input_named {
            if (args.len() - optind) == 1 {
                input_named = true;
                infilename = Some(args[optind].clone());
            } else if (args.len() - optind) > 1 {
                eprintln!("no more than one input rule file may be given");
                return Err(1);
            }
        } else if (args.len() - optind) > 0 {
            eprintln!("no more than one input rule file may be given");
            return Err(1);
        }

        self.be_verbose = verbose;
        self.be_quiet = silent;
        self.has_input_file = input_named;
        self.has_output_file = output_named;
        self.resolve_left_conflicts = resolve_left;
        self.resolve_right_conflicts = resolve_right;
        if self.has_input_file {
            self.input_file_name = infilename.unwrap_or_default();
        }
        if self.has_output_file {
            self.output_file_name = outfilename.unwrap_or_default();
        }
        self.format = form;

        if is_debug {
            self.has_debug_file = true;
            self.has_input_file = true;
            self.input_file_name = debug_file_name.unwrap_or_default();
        }

        Ok(())
    }

    // [spec:hfst:def:command-line.command-line.command-line-fn]
    // [spec:hfst:sem:command-line.command-line.command-line-fn]
    fn new(args: &mut Vec<String>) -> Result<Self, i32> {
        let mut cl = CommandLine {
            be_verbose: false,
            be_quiet: false,
            has_input_file: false,
            input_file_name: String::new(),
            has_output_file: false,
            output_file_name: String::new(),
            resolve_left_conflicts: false,
            resolve_right_conflicts: true,
            help: false,
            version: false,
            usage: false,
            has_debug_file: false,
            format: ImplementationType::TROPICAL_OPENFST_TYPE,
        };
        cl.parse_options(args)?;
        Ok(cl)
    }

    /// 'CommandLine::set_input_file': the whole grammar source, from the named
    /// file or stdin. The C++ returned a stream; the Rust compiler front end
    /// takes the source as one string.
    fn read_input(&self) -> Result<String, i32> {
        if self.has_input_file {
            match std::fs::read_to_string(&self.input_file_name) {
                Ok(s) => Ok(s),
                Err(_) => {
                    eprintln!("File {} could not be opened!", self.input_file_name);
                    // The C++ printed the __HFST_TWOLC_DIE token to stdout for
                    // the driver script; preserved.
                    print!("__HFST_TWOLC_DIE");
                    Err(1)
                }
            }
        } else {
            let mut s = String::new();
            match std::io::stdin().read_to_string(&mut s) {
                Ok(_) => Ok(s),
                Err(_) => {
                    eprintln!("File <stdin> could not be opened!");
                    print!("__HFST_TWOLC_DIE");
                    Err(1)
                }
            }
        }
    }
}

// [spec:hfst:def:hfst-twolc.main-fn]
// [spec:hfst:sem:hfst-twolc.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    real_main(args)
}

fn real_main(mut args: Vec<String>) -> i32 {
    // The C++ driver linked the library's warning/error streams to stderr;
    // here that is the shared tracing subscriber the other tools install via
    // hfst_set_program_name (the library's info!/error! diagnostics would
    // otherwise be dropped).
    let argv0 = args.first().cloned().unwrap_or_default();
    crate::hfst_commandline::hfst_set_program_name(&argv0, "0", "HfstTwolc");

    let command_line = match CommandLine::new(&mut args) {
        Ok(cl) => cl,
        Err(code) => return code,
    };

    if command_line.help || command_line.version {
        if command_line.version {
            command_line.print_version();
        }
        if command_line.help {
            command_line.print_help();
        }
        return 0;
    }
    if command_line.usage {
        command_line.print_usage();
        return 0;
    }
    if !command_line.be_quiet {
        if !command_line.has_input_file {
            eprintln!("Reading input from STDIN.");
        } else {
            eprintln!("Reading input from {}.", command_line.input_file_name);
        }
        if !command_line.has_output_file {
            eprintln!("Writing output to STDOUT.");
        } else {
            eprintln!("Writing output to {}.", command_line.output_file_name);
        }
    }
    if command_line.be_verbose {
        eprintln!("Verbose mode.");
    }

    let input = match command_line.read_input() {
        Ok(s) => s,
        Err(code) => return code,
    };

    // Test that the output file is okay (the C++ opened it up front before
    // running the preprocessor passes).
    let mut out = match if command_line.has_output_file {
        HfstOutputStream::new_filename(&command_line.output_file_name, command_line.format, true)
    } else {
        HfstOutputStream::new(command_line.format, true)
    } {
        Ok(s) => s,
        Err(_) => {
            eprintln!(
                "File {} could not be opened!",
                command_line.output_file_name
            );
            print!("__HFST_TWOLC_DIE");
            return 1;
        }
    };

    // The three htwolcpre parse passes + TwolCGrammar build + compile_and_store
    // collapse into the library's TwolcCompiler (nfst-twolc parse + AST walk +
    // per-rule stream store). The --format value is matched ONCE here into
    // the compiler's backend type parameter ([dec:hfst:monomorphic-backends]);
    // the rules are compiled at the requested type and stored to a same-type
    // stream (mirroring C++ htwolcpre3, whose OtherSymbolTransducer is typed by
    // the --format transducer_type). SFST/XFSM still never reach this point (the
    // output stream constructor above rejects them).
    // Name shown in source-anchored diagnostics: the named input file, or the
    // library default ("<twolc>") when reading from stdin.
    let source_name = if command_line.has_input_file {
        command_line.input_file_name.clone()
    } else {
        String::from("<twolc>")
    };
    let compiled = match command_line.format {
        ImplementationType::LOG_OPENFST_TYPE => {
            TwolcCompiler::<hfst::log_weight_transducer::LogFst>::new_with_options(
                command_line.be_quiet,
                command_line.be_verbose,
                command_line.resolve_left_conflicts,
                command_line.resolve_right_conflicts,
            )
            .set_source_name(&source_name)
            .compile_and_store(&input, &mut out)
        }
        #[cfg(feature = "foma")]
        ImplementationType::FOMA_TYPE => {
            TwolcCompiler::<hfst::backend_foma::FomaTransducer>::new_with_options(
                command_line.be_quiet,
                command_line.be_verbose,
                command_line.resolve_left_conflicts,
                command_line.resolve_right_conflicts,
            )
            .set_source_name(&source_name)
            .compile_and_store(&input, &mut out)
        }
        _ => TwolcCompiler::<hfst_openfst::StdVectorFst>::new_with_options(
            command_line.be_quiet,
            command_line.be_verbose,
            command_line.resolve_left_conflicts,
            command_line.resolve_right_conflicts,
        )
        .set_source_name(&source_name)
        .compile_and_store(&input, &mut out),
    };
    match compiled {
        Some(()) => {}
        None => {
            // A pass failing made the C++ driver exit(1).
            return 1;
        }
    }
    if command_line.has_output_file {
        if let Err(e) = out.flush() {
            eprintln!("This is an hfst interface bug:\n{}", e);
            return 1;
        }
        out.close();
    }
    0
}
