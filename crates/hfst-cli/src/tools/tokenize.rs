//! Faithful 1:1 port of tools/src/hfst-tokenize.cc — a replacement for
//! hfst-proc using pmatch: perform matching/lookup/tokenization on text
//! streams. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, inc fragments) and the hfst optimized-lookup pmatch
//! tokenizer ('hfst::pmatch_tokenize', 'hfst::pmatch', 'hfst::pmatch_compiler').
//!
//! This is a unary tool (#includes inc/globals-common.h + inc/globals-unary.h),
//! but like hfst-pmatch it does not use the usual unary
//! HfstInputStream/HfstOutputStream pipeline for output: it reads its single
//! positional argument as the ruleset archive filename, reads lines of stdin
//! (via 'inputfile'), and prints to stdout.
//!
//! The tokenization engine itself (the naive-tokenizer construction and the
//! input-segmentation drivers) lives in 'hfst::pmatch_tokenize'; this binary
//! keeps only option parsing and stream opening.

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use crate::inc::{CaseResult, handle_common_case, handle_error_case};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_tokenize::{
    OutputFormat, TokenizeInputSettings, TokenizeSettings, make_naive_tokenizer,
    process_input_stream,
};
use std::io::Write;

// File-scope tool state (the C++ file-scope statics).
static mut SUPERBLANKS: bool = false; // Input is apertium-style superblanks
// (overrides blankline_separated)
static mut BLANKLINE_SEPARATED: bool = true; // Input is separated by blank lines
// (as opposed to single newlines)
static mut KEEP_NEWLINES: bool = false;
#[allow(dead_code)]
static mut TOKEN_NUMBER: i32 = 1;
static mut TOKENIZER_FILENAME: String = String::new();
const DEFAULT_FORMAT: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

// 'static TokenizeSettings settings;' — held as a process-global. Default()
// mirrors the C++ default-constructed TokenizeSettings.
static mut SETTINGS: Option<TokenizeSettings> = None;

fn settings() -> &'static mut TokenizeSettings {
    unsafe {
        let ptr = &raw mut SETTINGS;
        if (*ptr).is_none() {
            *ptr = Some(TokenizeSettings::default());
        }
        (*ptr).as_mut().expect("initialized above")
    }
}

// [spec:hfst:def:hfst-tokenize.print-usage-fn]
// [spec:hfst:sem:hfst-tokenize.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [--segment | --xerox | --cg | --giella-cg] [OPTIONS...] RULESET\nperform matching/lookup on text streams\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "  -n, --newline            Newline as input separator (default is blank line)\n\
         \x20 -a, --print-all          Print nonmatching text\n\
         \x20 -w, --print-weight       Print weights (overrides earlier -W option)\n\
         \x20 -W, --no-weights         Don't print weights (default; overrides earlier -w, or -w implied by -g, options)\n\
         \x20 -m, --tokenize-multichar Tokenize multicharacter symbols\n\
         \x20                          (by default only one grapheme is tokenized at a time\n\
         \x20                          regardless of what is present in the alphabet)\n\
         \x20 -b, --beam=B             Output only analyses whose weight is within B from best result\n\
         \x20 -tS, --time-cutoff=S     Limit search after having used S seconds per input\n\
         \x20 -lN, --weight-classes=N  Output no more than N best weight classes\n\
         \x20                          (where analyses with equal weight constitute a class\n\
         \x20 -u, --unique             Remove duplicate analyses\n\
         \x20 -z, --segment            Segmenting / tokenization mode (default)\n\
         \x20 -i, --space-separated    Tokenization with one sentence per line, space-separated tokens\n\
         \x20 -x, --xerox              Xerox output\n\
         \x20 -c, --cg                 Constraint Grammar output\n\
         \x20 -S, --superblanks        Ignore contents of unescaped [] (cf. apertium-destxt); flush on NUL\n\
         \x20 -g, --giella-cg          CG format used in Giella infrastructure (implies -w and -l2,\n\
         \x20                          treats @PMATCH_INPUT_MARK@ as subreading separator,\n\
         \x20                          expects tags to be Multichar_symbols, flush on NUL)\n\
         \x20 -C  --conllu             CoNLL-U format\n\
         \x20 -f, --finnpos            FinnPos output\n\
         \x20 -L, --visl               VISL input and output (implies -W, handles <s> as blocks and <STYLE> inline)\n",
    );
    let _ = write!(
        msg,
        "Use standard streams for input and output (for now).\n\n"
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-tokenize.parse-options-fn]
// [spec:hfst:sem:hfst-tokenize.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            // tool-specific options
            let names: &[(&str, i32, i32)] = &[
                ("newline", getopt::NO_ARGUMENT, b'n' as i32),
                ("keep-newline", getopt::NO_ARGUMENT, b'k' as i32),
                ("print-all", getopt::NO_ARGUMENT, b'a' as i32),
                ("print-weights", getopt::NO_ARGUMENT, b'w' as i32),
                ("no-weights", getopt::NO_ARGUMENT, b'W' as i32),
                ("tokenize-multichar", getopt::NO_ARGUMENT, b'm' as i32),
                ("beam", getopt::REQUIRED_ARGUMENT, b'b' as i32),
                ("time-cutoff", getopt::REQUIRED_ARGUMENT, b't' as i32),
                ("weight-classes", getopt::REQUIRED_ARGUMENT, b'l' as i32),
                ("unique", getopt::NO_ARGUMENT, b'u' as i32),
                ("segment", getopt::NO_ARGUMENT, b'z' as i32),
                ("space-separated", getopt::NO_ARGUMENT, b'd' as i32),
                ("xerox", getopt::NO_ARGUMENT, b'x' as i32),
                ("cg", getopt::NO_ARGUMENT, b'c' as i32),
                ("superblanks", getopt::NO_ARGUMENT, b'S' as i32),
                ("giella-cg", getopt::NO_ARGUMENT, b'g' as i32),
                ("gtd", getopt::NO_ARGUMENT, b'g' as i32),
                ("conllu", getopt::NO_ARGUMENT, b'C' as i32),
                ("finnpos", getopt::NO_ARGUMENT, b'f' as i32),
                ("visl", getopt::NO_ARGUMENT, b'L' as i32),
            ];
            for &(name, has_arg, val) in names {
                long_options.push(getopt::GetOpt { name, has_arg, val });
            }
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'k' as i32 {
                KEEP_NEWLINES = true;
                BLANKLINE_SEPARATED = false;
            } else if c == b'n' as i32 {
                BLANKLINE_SEPARATED = false;
            } else if c == b'a' as i32 {
                settings().print_all = true;
            } else if c == b'w' as i32 {
                settings().print_weights = true;
            } else if c == b'W' as i32 {
                settings().print_weights = false;
            } else if c == b'm' as i32 {
                settings().tokenize_multichar = true;
            } else if c == b't' as i32 {
                settings().time_cutoff = getopt::optarg().trim().parse::<f64>().unwrap_or(0.0);
                if settings().time_cutoff < 0.0 {
                    eprint!("Invalid argument for --time-cutoff\n");
                    return 1;
                }
            } else if c == b'u' as i32 {
                settings().dedupe = true;
            } else if c == b'b' as i32 {
                settings().beam = getopt::optarg().trim().parse::<f64>().unwrap_or(0.0) as f32;
                if settings().beam < 0.0 {
                    eprint!("Invalid argument for --beam\n");
                    return 1;
                }
            } else if c == b'l' as i32 {
                settings().max_weight_classes = getopt::optarg().trim().parse::<i32>().unwrap_or(0);
                if settings().max_weight_classes < 1 {
                    eprint!("Invalid or no argument --weight-classes count\n");
                    return 1;
                }
            } else if c == b'z' as i32 {
                settings().output_format = OutputFormat::tokenize;
            } else if c == b'i' as i32 {
                settings().output_format = OutputFormat::space_separated;
            } else if c == b'x' as i32 {
                settings().output_format = OutputFormat::xerox;
            } else if c == b'c' as i32 {
                settings().output_format = OutputFormat::cg;
            } else if c == b'C' as i32 {
                settings().output_format = OutputFormat::conllu;
            } else if c == b'S' as i32 {
                SUPERBLANKS = true;
            } else if c == b'g' as i32 {
                settings().output_format = OutputFormat::giellacg;
                settings().print_weights = true;
                settings().print_all = true;
                settings().dedupe = true;
                settings().hack_uncompose = true;
                settings().verbose = false;
                if settings().max_weight_classes == i32::MAX {
                    settings().max_weight_classes = 2;
                }
            } else if c == b'L' as i32 {
                settings().output_format = OutputFormat::visl;
                settings().print_weights = false;
                settings().print_all = true;
                settings().dedupe = true;
                settings().verbose = false;
            } else if c == b'f' as i32 {
                settings().output_format = OutputFormat::finnpos;
            } else {
                return handle_error_case(c);
            }

            if globals::VERBOSE {
                settings().verbose = true;
            }
        }

        // no more options, we should now be at the input filename
        let argc = args.len();
        if (getopt::OPTIND + 1) < argc {
            eprint!("More than one input file given\n");
            1
        } else if (getopt::OPTIND + 1) == argc {
            *std::ptr::addr_of_mut!(TOKENIZER_FILENAME) = args[getopt::OPTIND].clone();
            EXIT_CONTINUE
        } else {
            eprint!("No input file given\n");
            1
        }
    }
}

// [spec:hfst:def:hfst-tokenize.first-transducer-is-called-top-fn]
// [spec:hfst:sem:hfst-tokenize.first-transducer-is-called-top-fn]
// (Defined in the C++ source but never called there; kept for fidelity.)
#[allow(dead_code)]
fn first_transducer_is_called_top(dictionary: &HfstTransducer) -> bool {
    dictionary.get_name() == "TOP"
}

// [spec:hfst:def:hfst-tokenize.main-fn]
// [spec:hfst:sem:hfst-tokenize.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstTokenize");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        let tokenizer_filename = {
            let ptr = &raw const TOKENIZER_FILENAME;
            (*ptr).clone()
        };
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            tokenizer_filename,
            globals::output_filename()
        ));
        let mut file = match std::fs::File::open(&tokenizer_filename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("Could not open file {}", tokenizer_filename);
                return 1;
            }
        };
        // The C wraps the rest in try/catch on HfstException (and a nested catch
        // on TransducerHeaderException around parse_hfst3_header); the Rust ports
        // currently panic rather than throw, so those catch arms are not
        // reproduced here.
        //
        // To decide whether we're working with something produced by a pmatch
        // ruleset, we want to know whether the first transducer is named TOP. To
        // do this, rather than load the whole thing into a HfstTransducer, we read
        // just the header variables with parse_hfst3_header, then rewind.
        let first_header_attributes = {
            let mut hdr_stream =
                hfst::transducer::IStream::new(&mut file as &mut dyn std::io::Read);
            match PmatchContainer::parse_hfst3_header(&mut hdr_stream) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            }
        };
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::Start(0));

        let mut stdout = std::io::stdout();
        // Text input is read from the standard input stream (C: 'inputfile()').
        let mut input = match globals::input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-tokenize: cannot open input: {e}");
                return 1;
            }
        };
        // The tool-level input-mode switches, handed to the library driver.
        let input_settings = TokenizeInputSettings {
            superblanks: SUPERBLANKS,
            blankline_separated: BLANKLINE_SEPARATED,
            keep_newlines: KEEP_NEWLINES,
            verbose: globals::VERBOSE,
        };
        let mut msg = globals::message_writer();
        if first_header_attributes.get("name").map(|s| s.as_str()) != Some("TOP") {
            verbose_print("No TOP automaton found, using naive tokeniser?\n");
            let mut is = match HfstInputStream::new_filename(&tokenizer_filename) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            };
            let mut dictionary = match HfstTransducer::new_from_stream(&mut is) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            };
            let mut container = match make_naive_tokenizer(&mut dictionary, DEFAULT_FORMAT) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            };
            container.set_verbose(globals::VERBOSE);
            container.set_single_codepoint_tokenization(!settings().tokenize_multichar);
            process_input_stream(
                &mut container,
                &mut *input,
                &mut stdout,
                &mut *msg,
                settings(),
                &input_settings,
            )
        } else {
            verbose_print("TOP automaton seen, treating as pmatch script...\n");
            let mut is = hfst::transducer::IStream::new(&mut file as &mut dyn std::io::Read);
            let mut container = match PmatchContainer::new_from_stream(&mut is) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            };
            container.set_verbose(globals::VERBOSE);
            container.set_single_codepoint_tokenization(!settings().tokenize_multichar);
            process_input_stream(
                &mut container,
                &mut *input,
                &mut stdout,
                &mut *msg,
                settings(),
                &input_settings,
            )
        }
    }
}
