//! Faithful 1:1 port of tools/src/hfst-check-alpha.cc — the tool that compares
//! the compatibility of alphabets within and between automata. Drives the
//! hfst-cli foundation (getopt, commandline, program-options, tool-metadata,
//! inc fragments). A binary tool (two input streams).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-1/-2/…` fields) built by `parse_options` and threaded into
//! the processing functions. There are no `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;

use std::io::Write;

// [spec:hfst:def:hfst-check-alpha.print-usage-fn]
// [spec:hfst:sem:hfst-check-alpha.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILEs]\nCompare the compatibility of alphabets between INFILEs\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    // (tool-specific options and short descriptions)
    let _ = writeln!(msg, "Check alpha options:");
    let _ = writeln!(msg);
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
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
//
// Parse argv into the shared options; `Err(code)` is an exit code the caller
// should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(mut common: CommonOptions, args: &mut Vec<String>) -> Result<CommonOptions, i32> {
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_binary_long());
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: binary
        // cases, then common cases, then the tool's own (none here), then the
        // terminal error arm.
        match handle_binary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_binary_params(&mut common, &opt, args);
    check_common_params(&mut common);
    Ok(common)
}

// [spec:hfst:def:hfst-check-alpha.process-stream-fn]
// [spec:hfst:sem:hfst-check-alpha.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut out = match common.output_writer() {
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
            verbose_print(common, "Checking alphas...\n");
        } else {
            verbose_print(common, &format!("Checking alphas... {}\n", transducer_n));
        }
        // read first alphas
        let first = match firststream.read() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // one dispatch per read ([dec:hfst:monomorphic-backends]); the
        // alphabet queries are backend-independent values.
        let (mutt, first_transducer_alphabet): (HfstBasicTransducer, StringSet) = crate::for_any!(&first, t => {
            let mutt = match HfstBasicTransducer::try_from_transducer(t) {
                Ok(m) => m,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let alpha = match t.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            (mutt, alpha)
        });
        let transducer_knows_alphabet = true;
        let first_found_alphabet: StringSet = mutt.symbols_used();
        // read second alphas
        let second = match secondstream.read() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let (secondmutt, second_transducer_alphabet): (HfstBasicTransducer, StringSet) = crate::for_any!(&second, t => {
            let mutt = match HfstBasicTransducer::try_from_transducer(t) {
                Ok(m) => m,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let alpha = match t.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            (mutt, alpha)
        });
        let second_found_alphabet: StringSet = secondmutt.symbols_used();
        // match
        let _ = writeln!(out, "Actual alphabet differences:");
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
        let _ = writeln!(out);
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
        let _ = writeln!(out);
        if common.verbose {
            let _ = write!(out, "{} alphabet:", first.get_name());
            fprint_stringset(&mut *out, &first_found_alphabet);
            let _ = writeln!(out);
            let _ = write!(out, "{} alphabet:", second.get_name());
            fprint_stringset(&mut *out, &second_found_alphabet);
            let _ = writeln!(out);
        }
        if transducer_knows_alphabet {
            let _ = writeln!(out, "sigma set difference:");
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
            let _ = writeln!(out);
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
            let _ = writeln!(out);
            if common.verbose {
                let _ = write!(out, "First ({}):", first.get_name());
                fprint_stringset(&mut *out, &first_transducer_alphabet);
                let _ = writeln!(out);
                let _ = write!(out, "Second ({}):", second.get_name());
                fprint_stringset(&mut *out, &second_transducer_alphabet);
                let _ = writeln!(out);
            }
        } else {
            let _ = writeln!(out, "No internal alphabets to compare in this format");
        } // FSTs know their alphas
        continue_reading = firststream.is_good() && secondstream.is_good();
    }

    let _ = write!(out, "\nRead {} transducers in total.\n", transducer_n);
    mismatch
}

// [spec:hfst:def:hfst-check-alpha.main-fn]
// [spec:hfst:sem:hfst-check-alpha.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstALphaFix");
    let common = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let first_opened = common.first_filename != "<stdin>";
    let second_opened = common.second_filename != "<stdin>";
    verbose_print(
        &common,
        &format!(
            "Reading from {} and {}, writing to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps each ctor in try/catch on HfstException, calling error()
    // and returning EXIT_FAILURE; the Rust ctors now return a Result, so the
    // error path and message are preserved via a match on that Result.)
    let firststream = if first_opened {
        let name = common.first_filename.clone();
        match HfstInputStream::new_filename(&name) {
            Ok(s) => s,
            Err(_) => {
                error(
                    &common,
                    1,
                    0,
                    &format!("{} is not a valid transducer file", name),
                );
                return 1;
            }
        }
    } else {
        match HfstInputStream::new() {
            Ok(s) => s,
            Err(_) => {
                error(
                    &common,
                    1,
                    0,
                    &format!("{} is not a valid transducer file", common.first_filename),
                );
                return 1;
            }
        }
    };
    let secondstream = if second_opened {
        let name = common.second_filename.clone();
        match HfstInputStream::new_filename(&name) {
            Ok(s) => s,
            Err(_) => {
                error(
                    &common,
                    1,
                    0,
                    &format!("{} is not a valid transducer file", name),
                );
                return 1;
            }
        }
    } else {
        match HfstInputStream::new() {
            Ok(s) => s,
            Err(_) => {
                error(
                    &common,
                    1,
                    0,
                    &format!("{} is not a valid transducer file", common.second_filename),
                );
                return 1;
            }
        }
    };
    let mut firststream = firststream;
    let mut secondstream = secondstream;

    let _retval = process_stream(&common, &mut firststream, &mut secondstream);

    0
}
