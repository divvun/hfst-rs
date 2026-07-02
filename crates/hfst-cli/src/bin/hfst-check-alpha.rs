//! Faithful 1:1 port of tools/src/hfst-check-alpha.cc — the tool that compares
//! the compatibility of alphabets within and between automata. Drives the
//! hfst-cli foundation (globals, getopt, commandline, program-options,
//! tool-metadata, inc fragments). A binary tool (two input streams).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_print,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use std::io::Write;

// [spec:hfst:def:hfst-check-alpha.print-usage-fn]
// [spec:hfst:sem:hfst-check-alpha.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILEs]\nCompare the compatibility of alphabets between INFILEs\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    // (tool-specific options and short descriptions)
    let _ = write!(msg, "Check alpha options:\n");
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-check-alpha.fprint-stringset-fn]
// [spec:hfst:sem:hfst-check-alpha.fprint-stringset-fn]
fn fprint_stringset(outfile: &mut dyn Write, strings: &StringSet) {
    let mut first = true;
    for s in strings {
        if !first {
            let _ = write!(outfile, ", ");
        }
        let _ = write!(outfile, "{}", s);
        first = false;
    }
}

// [spec:hfst:def:hfst-check-alpha.parse-options-fn]
// [spec:hfst:sem:hfst-check-alpha.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own (none here), then the
            // terminal error arm.
            match handle_binary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            return handle_error_case(c);
        }

        check_binary_params(args);
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-check-alpha.process-stream-fn]
// [spec:hfst:sem:hfst-check-alpha.process-stream-fn]
unsafe fn process_stream(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
) -> i32 {
    unsafe {
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-check-alpha: cannot open output: {e}");
                return 1;
            }
        };
        let mut continue_reading = firststream.is_good() && secondstream.is_good();
        let mut transducer_n: usize = 0;
        let mut mismatch = 0;
        while continue_reading {
            transducer_n += 1;

            if transducer_n < 2 {
                verbose_print("Checking alphas...\n");
            } else {
                verbose_print(&format!("Checking alphas... {}\n", transducer_n));
            }
            // read first alphas
            let first = match HfstTransducer::new_from_stream(firststream) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let mutt: HfstBasicTransducer = match first.get_basic_transducer() {
                Ok(m) => m,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let first_transducer_alphabet: StringSet = match first.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let transducer_knows_alphabet = true;
            let first_found_alphabet: StringSet = mutt.symbols_used();
            // read second alphas
            let second = match HfstTransducer::new_from_stream(secondstream) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let secondmutt: HfstBasicTransducer = match second.get_basic_transducer() {
                Ok(m) => m,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let second_transducer_alphabet: StringSet = match second.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let second_found_alphabet: StringSet = secondmutt.symbols_used();
            // match
            let _ = write!(out, "Actual alphabet differences:\n");
            let first_minus_second: StringSet = first_found_alphabet
                .difference(&second_found_alphabet)
                .cloned()
                .collect();
            if !first_minus_second.is_empty() {
                mismatch = 1;
                let _ = write!(
                    out,
                    "In first {} but not in second {}:",
                    first.get_name(),
                    second.get_name()
                );
                fprint_stringset(&mut *out, &first_minus_second);
            } else {
                let _ = write!(
                    out,
                    "First {} alpha is superset of second {}.",
                    first.get_name(),
                    second.get_name()
                );
            }
            let _ = write!(out, "\n");
            let second_minus_first: StringSet = second_found_alphabet
                .difference(&first_found_alphabet)
                .cloned()
                .collect();
            if !second_minus_first.is_empty() {
                mismatch = 1;
                let _ = write!(
                    out,
                    "In second {} but not in first {}:",
                    second.get_name(),
                    second.get_name()
                );
                fprint_stringset(&mut *out, &second_minus_first);
            } else {
                let _ = write!(
                    out,
                    "Second {} alpha is superset of second {}.",
                    second.get_name(),
                    second.get_name()
                );
            }
            let _ = write!(out, "\n");
            if globals::VERBOSE {
                let _ = write!(out, "{} alphabet:", first.get_name());
                fprint_stringset(&mut *out, &first_found_alphabet);
                let _ = write!(out, "\n");
                let _ = write!(out, "{} alphabet:", second.get_name());
                fprint_stringset(&mut *out, &second_found_alphabet);
                let _ = write!(out, "\n");
            }
            if transducer_knows_alphabet {
                let _ = write!(out, "sigma set difference:\n");
                let first_minus_second: StringSet = first_transducer_alphabet
                    .difference(&second_transducer_alphabet)
                    .cloned()
                    .collect();
                let second_minus_first: StringSet = second_transducer_alphabet
                    .difference(&first_transducer_alphabet)
                    .cloned()
                    .collect();
                if !first_minus_second.is_empty() {
                    mismatch = 1;
                    let _ = write!(
                        out,
                        "First {} has but second {} does not: ",
                        first.get_name(),
                        second.get_name()
                    );
                    fprint_stringset(&mut *out, &first_minus_second);
                } else {
                    let _ = write!(
                        out,
                        "First {} alpha is superset of second {}.",
                        first.get_name(),
                        second.get_name()
                    );
                }
                let _ = write!(out, "\n");
                if !second_minus_first.is_empty() {
                    mismatch = 1;
                    let _ = write!(
                        out,
                        "Second {} has but first {} does not: ",
                        second.get_name(),
                        first.get_name()
                    );
                    fprint_stringset(&mut *out, &second_minus_first);
                } else {
                    let _ = write!(
                        out,
                        "Second {} alpha is superset of first {}.",
                        second.get_name(),
                        first.get_name()
                    );
                }
                let _ = write!(out, "\n");
                if globals::VERBOSE {
                    let _ = write!(out, "First ({}):", first.get_name());
                    fprint_stringset(&mut *out, &first_transducer_alphabet);
                    let _ = write!(out, "\n");
                    let _ = write!(out, "Second ({}):", second.get_name());
                    fprint_stringset(&mut *out, &second_transducer_alphabet);
                    let _ = write!(out, "\n");
                }
            } else {
                let _ = write!(out, "No internal alphabets to compare in this format\n");
            } // FSTs know their alphas
            continue_reading = firststream.is_good() && secondstream.is_good();
        }

        let _ = write!(out, "\nRead {} transducers in total.\n", transducer_n);
        mismatch
    }
}

// [spec:hfst:def:hfst-check-alpha.main-fn]
// [spec:hfst:sem:hfst-check-alpha.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstALphaFix");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let first_opened = globals::first_filename() != "<stdin>";
        let second_opened = globals::second_filename() != "<stdin>";
        verbose_print(&format!(
            "Reading from {} and {}, writing to {}\n",
            globals::first_filename(),
            globals::second_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        // (the C wraps each ctor in try/catch on HfstException, calling error()
        // and returning EXIT_FAILURE; the Rust ctors now return a Result, so the
        // error path and message are preserved via a match on that Result.)
        let firststream = if first_opened {
            let name = globals::first_filename();
            match HfstInputStream::new_filename(&name) {
                Ok(s) => s,
                Err(_) => {
                    error(1, 0, &format!("{} is not a valid transducer file", name));
                    return 1;
                }
            }
        } else {
            match HfstInputStream::new() {
                Ok(s) => s,
                Err(_) => {
                    error(
                        1,
                        0,
                        &format!(
                            "{} is not a valid transducer file",
                            globals::first_filename()
                        ),
                    );
                    return 1;
                }
            }
        };
        let secondstream = if second_opened {
            let name = globals::second_filename();
            match HfstInputStream::new_filename(&name) {
                Ok(s) => s,
                Err(_) => {
                    error(1, 0, &format!("{} is not a valid transducer file", name));
                    return 1;
                }
            }
        } else {
            match HfstInputStream::new() {
                Ok(s) => s,
                Err(_) => {
                    error(
                        1,
                        0,
                        &format!(
                            "{} is not a valid transducer file",
                            globals::second_filename()
                        ),
                    );
                    return 1;
                }
            }
        };
        let mut firststream = firststream;
        let mut secondstream = secondstream;

        let _retval = process_stream(&mut firststream, &mut secondstream);

        0
    }
}
