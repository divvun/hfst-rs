//! Faithful 1:1 port of tools/src/hfst-dump-alphabets.cc — the alphabet dump
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::Write;

// add tools-specific variables here
// [spec:hfst:def:hfst-dump-alphabets.alphadumpformat]
#[derive(Clone, Copy, PartialEq, Eq)]
enum AlphaDumpFormat {
    Tsv,
    Vislcg3List,
    Vislcg3Tags,
}

static mut OUTPUT_FORMAT: AlphaDumpFormat = AlphaDumpFormat::Tsv;
static mut PRINT_SEEN: bool = true;
static mut PRINT_META: bool = true;
static mut ONLY_MULTICHARS: bool = false;

// [spec:hfst:def:hfst-dump-alphabets.is-multichar-fn]
// [spec:hfst:sem:hfst-dump-alphabets.is-multichar-fn]
fn is_multichar(s: &str) -> bool {
    if s.len() > 2 {
        if s.starts_with('+') || s.starts_with(' ') || s.starts_with('@') {
            return true;
        } else {
            return false;
        }
    }
    false
}

// [spec:hfst:def:hfst-dump-alphabets.print-usage-fn]
// [spec:hfst:sem:hfst-dump-alphabets.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPrint alphabets of automaton\n\n",
        globals::program_name()
    );

    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    // fprintf(message_out, (tool-specific options and short descriptions)
    let _ = write!(msg, "Alphabet dump options:\n");
    let _ = write!(
        msg,
        "  -f, --format=AFORMAT     Print alphabet in AFORAMT\n"
    );
    let _ = write!(
        msg,
        "  -1, --exclude-seen       Ignore alphabets seen in automaton\n"
    );
    let _ = write!(
        msg,
        "  -2, --exclude-metadata   Ignore alphabets from headers\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-dump-alphabets.parse-options-fn]
// [spec:hfst:sem:hfst-dump-alphabets.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            // add tool-specific cases here
            match c as u8 as char {
                'f' => {
                    let optarg = getopt::optarg();
                    if optarg == "tsv" {
                        OUTPUT_FORMAT = AlphaDumpFormat::Tsv;
                        ONLY_MULTICHARS = false;
                        verbose_printf("printing one symbol per line\n");
                    } else if optarg == "vislcg3-list" {
                        OUTPUT_FORMAT = AlphaDumpFormat::Vislcg3List;
                        ONLY_MULTICHARS = true;
                        verbose_printf("printing LIST x = x ; for VISL CG 3...\n");
                    } else if optarg == "vislcg3-tags" {
                        OUTPUT_FORMAT = AlphaDumpFormat::Vislcg3Tags;
                        ONLY_MULTICHARS = true;
                        verbose_printf("printing STRICT-TAGS += for VISL CG 3...\n");
                    } else {
                        eprintln!("Error: unrecognised format {}", optarg);
                        std::process::exit(1);
                    }
                    continue;
                }
                '1' => {
                    PRINT_SEEN = false;
                    continue;
                }
                '2' => {
                    PRINT_META = false;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-dump-alphabets.process-stream-fn]
// [spec:hfst:sem:hfst-dump-alphabets.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> i32 {
    unsafe {
        // Data output goes to a std stream (the std counterpart of the libc
        // outfile FILE*); `emit` writes a string and ignores errors, matching the
        // old fput/fputs. (print_usage's message_out path stays on FILE* until
        // the message_out chunk of io-foundation.)
        let mut out = match globals::output_writer() {
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
                verbose_printf("Alphadumping...\n");
            } else {
                verbose_printf(&format!("Alphadumping... {}\n", transducer_n));
            }
            let trans = match HfstTransducer::new_from_stream(instream) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-dump-alphabets: {e}");
                    return 1;
                }
            };
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
            if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3Tags {
                emit(
                    "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                );
                emit("## (some statistics here TODO)\n");
                emit("STRICT-TAGS +=\n");
            } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3List {
                emit(
                    "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                );
                emit("## (some statistics here TODO)\n");
            }
            if PRINT_META {
                if transducer_knows_alphabet {
                    for s in transducer_alphabet.iter() {
                        if ONLY_MULTICHARS && !is_multichar(s) {
                            continue;
                        }
                        if OUTPUT_FORMAT == AlphaDumpFormat::Tsv {
                            emit(&format!("{}\n", s));
                        } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3Tags {
                            emit(&format!("\t{}\n", s));
                        } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3List {
                            emit(&format!("LIST {} = {} ;\n", s, s));
                        }
                    }
                } else {
                    eprintln!("Error: cannot dump non-existent header alphabet");
                    std::process::exit(1);
                }
            }
            if PRINT_SEEN {
                for s in found_alphabet.iter() {
                    if ONLY_MULTICHARS && !is_multichar(s) {
                        continue;
                    }
                    if OUTPUT_FORMAT == AlphaDumpFormat::Tsv {
                        emit(&format!("{}\n", s));
                    } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3Tags {
                        emit(&format!("\t{}\n", s));
                    } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3List {
                        emit(&format!("LIST {} = {} ;\n", s, s));
                    }
                }
            }
        } // for each automaton
        if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3Tags {
            emit("\t;\n");
        }
        0
    }
}

// [spec:hfst:def:hfst-dump-alphabets.main-fn]
// [spec:hfst:sem:hfst-dump-alphabets.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // that calls error(EXIT_FAILURE, ...) is not reproduced here.)
        let instream_res = if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        let mut instream = match instream_res {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "hfst-dump-alphabets: {} is not a valid transducer file: {e}",
                    globals::input_filename()
                );
                return 1;
            }
        };
        let _retval = process_stream(&mut instream);

        0
    }
}
