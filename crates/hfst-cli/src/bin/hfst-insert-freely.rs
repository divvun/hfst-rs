#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-insert-freely.cc — the freely-insert
//! a symbol (pair) command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_data_types::StringPair;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::Write;

// add tools-specific variables here
static mut LABEL: Option<String> = None;
static mut HARMONISE_FLAGS: bool = false;
static mut SYMBOL_PAIR: Option<StringPair> = None;

// FMT: Copied from hfst-substitute.cc ... should probably go in a library function

// [spec:hfst:def:hfst-insert-freely.print-usage-fn]
// [spec:hfst:sem:hfst-insert-freely.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nFreely insert a symbol (pair)\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Option:\n  -a, --symbol-pair=SYM   symbol pair SYM\n  -H, --harmonise   harmonise \n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "SYM must be either a single alphabeticsymbol or two symbols separated by a colon, :\n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-insert-freely.parse-options-fn]
// [spec:hfst:sem:hfst-insert-freely.parse-options-fn]
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
                name: "symbol-pair",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'a' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "harmonise",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'H' as i32,
            });
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
            match c as u8 {
                b'a' => {
                    // This will probably break for unicode
                    let mut lbl = getopt::optarg();
                    if lbl == "@0@" {
                        lbl = internal_epsilon.to_string();
                    }
                    SYMBOL_PAIR = label_to_stringpair(&lbl);
                    if lbl.is_empty() {
                        error(
                            1,
                            0,
                            &format!(
                                "argument of source label option is empty;\nif you REALLY want to replace epsilons with something, use @0@ or {}",
                                internal_epsilon
                            ),
                        );
                    }
                    LABEL = Some(lbl);
                    continue;
                }
                b'H' => {
                    HARMONISE_FLAGS = true;
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

// [spec:hfst:def:hfst-insert-freely.process-stream-fn]
// [spec:hfst:sem:hfst-insert-freely.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let _inputname = hfst_get_name(&trans, &globals::input_filename());
            if transducer_n == 1 {
                // If harmonize is true, then identity and unknown symbols in the
                // transducer will be expanded by the symbols in symbol pair.
                // Otherwise they aren't.
                let pair = SYMBOL_PAIR.as_ref().expect("symbol pair must be set");
                trans.insert_freely_pair(pair, HARMONISE_FLAGS);
                // C: hfst_set_name(trans, trans, "insert-freely") and
                // hfst_set_formula(trans, trans, "Id"); dest and src are the
                // same object, so the read side is taken from a copy.
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "insert-freely");
                hfst_set_formula_unary(&mut trans, &src, "Id");
            }
            outstream.redirect(&mut trans);
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-insert-freely.main-fn]
// [spec:hfst:sem:hfst-insert-freely.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstPush");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let type_ = instream.get_type();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        if is_input_stream_in_ol_format(&instream, "hfst-insert-freely") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
