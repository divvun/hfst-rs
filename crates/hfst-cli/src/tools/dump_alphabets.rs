//! Faithful 1:1 port of tools/src/hfst-dump-alphabets.cc — the alphabet dump
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name, verbose_print};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use std::io::Write;

// add tools-specific variables here
// [spec:hfst:def:hfst-dump-alphabets.alphadumpformat]
#[derive(Clone, Copy, PartialEq, Eq)]
enum AlphaDumpFormat {
    Tsv,
    Vislcg3List,
    Vislcg3Tags,
}

/// hfst-dump-alphabets's own options (the former tool-specific `static mut`s).
struct Options {
    output_format: AlphaDumpFormat,
    print_seen: bool,
    print_meta: bool,
    only_multichars: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            output_format: AlphaDumpFormat::Tsv,
            print_seen: true,
            print_meta: true,
            only_multichars: false,
        }
    }
}

// [spec:hfst:def:hfst-dump-alphabets.is-multichar-fn]
// [spec:hfst:sem:hfst-dump-alphabets.is-multichar-fn]
fn is_multichar(s: &str) -> bool {
    if s.len() > 2 {
        return s.starts_with('+') || s.starts_with(' ') || s.starts_with('@');
    }
    false
}

// [spec:hfst:def:hfst-dump-alphabets.print-usage-fn]
// [spec:hfst:sem:hfst-dump-alphabets.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPrint alphabets of automaton\n\n",
        common.program_name
    );

    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    // fprintf(message_out, (tool-specific options and short descriptions)
    let _ = writeln!(msg, "Alphabet dump options:");
    let _ = writeln!(msg, "  -f, --format=AFORMAT     Print alphabet in AFORAMT");
    let _ = writeln!(
        msg,
        "  -1, --exclude-seen       Ignore alphabets seen in automaton"
    );
    let _ = writeln!(
        msg,
        "  -2, --exclude-metadata   Ignore alphabets from headers"
    );
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-dump-alphabets.parse-options-fn]
// [spec:hfst:sem:hfst-dump-alphabets.parse-options-fn]
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
        long_options.extend(hfst_getopt_unary_long());
        // add tool-specific options here
        long_options.push(getopt::GetOpt {
            name: "format",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'f' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "include-seen",
            has_arg: getopt::NO_ARGUMENT,
            val: '1' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "include-metadata",
            has_arg: getopt::NO_ARGUMENT,
            val: '2' as i32,
        });
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own, then the terminal
        // error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        // add tool-specific cases here
        match c as u8 as char {
            'f' => {
                let optarg = opt.optarg();
                if optarg == "tsv" {
                    options.output_format = AlphaDumpFormat::Tsv;
                    options.only_multichars = false;
                    verbose_print(&common, "printing one symbol per line\n");
                } else if optarg == "vislcg3-list" {
                    options.output_format = AlphaDumpFormat::Vislcg3List;
                    options.only_multichars = true;
                    verbose_print(&common, "printing LIST x = x ; for VISL CG 3...\n");
                } else if optarg == "vislcg3-tags" {
                    options.output_format = AlphaDumpFormat::Vislcg3Tags;
                    options.only_multichars = true;
                    verbose_print(&common, "printing STRICT-TAGS += for VISL CG 3...\n");
                } else {
                    eprintln!("Error: unrecognised format {}", optarg);
                    std::process::exit(1);
                }
                continue;
            }
            '1' => {
                options.print_seen = false;
                continue;
            }
            '2' => {
                options.print_meta = false;
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-dump-alphabets.process-stream-fn]
// [spec:hfst:sem:hfst-dump-alphabets.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
) -> i32 {
    // Data output goes to a std stream (the std counterpart of the libc
    // outfile FILE*); `emit` writes a string and ignores errors, matching the
    // old fput/fputs. (print_usage's message_out path stays on FILE* until
    // the message_out chunk of io-foundation.)
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-dump-alphabets: could not open output: {e}");
            return 1;
        }
    };
    let mut emit = |s: &str| {
        let _ = out.write_all(s.as_bytes());
    };
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        if transducer_n < 2 {
            verbose_print(common, "Alphadumping...\n");
        } else {
            verbose_print(common, &format!("Alphadumping... {}\n", transducer_n));
        }
        let any = match instream.read() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("hfst-dump-alphabets: {e}");
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mutt = HfstBasicTransducer::new_from_transducer(&trans);
            // unsigned int initial_state = 0; // mutt.get_initial_state();
            let transducer_alphabet = match trans.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("hfst-dump-alphabets: {e}");
                    return 1;
                }
            };
            let transducer_knows_alphabet = true;
            let found_alphabet: StringSet = mutt.symbols_used();
            if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                emit(
                    "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                );
                emit("## (some statistics here TODO)\n");
                emit("STRICT-TAGS +=\n");
            } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                emit(
                    "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                );
                emit("## (some statistics here TODO)\n");
            }
            if options.print_meta {
                if transducer_knows_alphabet {
                    for s in transducer_alphabet.iter() {
                        if options.only_multichars && !is_multichar(s) {
                            continue;
                        }
                        if options.output_format == AlphaDumpFormat::Tsv {
                            emit(&format!("{}\n", s));
                        } else if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                            emit(&format!("\t{}\n", s));
                        } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                            emit(&format!("LIST {} = {} ;\n", s, s));
                        }
                    }
                } else {
                    eprintln!("Error: cannot dump non-existent header alphabet");
                    std::process::exit(1);
                }
            }
            if options.print_seen {
                for s in found_alphabet.iter() {
                    if options.only_multichars && !is_multichar(s) {
                        continue;
                    }
                    if options.output_format == AlphaDumpFormat::Tsv {
                        emit(&format!("{}\n", s));
                    } else if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                        emit(&format!("\t{}\n", s));
                    } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                        emit(&format!("LIST {} = {} ;\n", s, s));
                    }
                }
            }
        });
    } // for each automaton
    if options.output_format == AlphaDumpFormat::Vislcg3Tags {
        emit("\t;\n");
    }
    0
}

// [spec:hfst:def:hfst-dump-alphabets.main-fn]
// [spec:hfst:sem:hfst-dump-alphabets.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
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
    // that calls error(EXIT_FAILURE, ...) is not reproduced here.)
    let instream_res = if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    };
    let mut instream = match instream_res {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "hfst-dump-alphabets: {} is not a valid transducer file: {e}",
                common.input_filename
            );
            return 1;
        }
    };
    let _retval = process_stream(&common, &options, &mut instream);

    0
}
