//! Faithful 1:1 port of tools/src/hfst-head.cc — the transducer archive head
//! splitting tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, parse_i64, verbose_print, warning,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::AnyTransducer;
use std::collections::VecDeque;
use std::io::Write;

/// hfst-head's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-n, --n-first=[-]K': number of transducers to keep from the head.
    head_count: i64,
}

impl Default for Options {
    fn default() -> Self {
        Options { head_count: 1 }
    }
}

// [spec:hfst:def:hfst-head.print-usage-fn]
// [spec:hfst:sem:hfst-head.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nGet first transducers from an archive\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Archive options:\n  -n, --n-first=[-]K   print the first K transducers;\n                       with the leading `-', print all but last K transducers\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "K must be an integer, as parsed by strtoul base 10, and not 0.\nIf K is omitted default is 1."
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-head.parse-options-fn]
// [spec:hfst:sem:hfst-head.parse-options-fn]
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
    // use of this function requires options are settable on global scope
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_unary_long());
        long_options.push(getopt::GetOpt {
            name: "n-first",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'n' as i32,
        });
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own ('n'), then the
        // terminal error arm.
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
        if c == 'n' as i32 {
            options.head_count = parse_i64(&common, &opt.optarg(), 10);
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    if options.head_count == 0 {
        warning(&common, 0, 0, "Argument 0 for count is not sensible");
    }
    Ok((common, options))
}

// [spec:hfst:def:hfst-head.process-stream-fn]
// [spec:hfst:sem:hfst-head.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    if options.head_count > 0 {
        while instream.is_good() && (transducer_n < options.head_count as usize) {
            transducer_n += 1;
            let mut trans = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let mut inputname = trans.get_name();
            if inputname.is_empty() {
                inputname = common.input_filename.clone();
            }
            verbose_print(
                common,
                &format!("Forwarding {}...{}\n", inputname, transducer_n),
            );
            if let Err(e) = trans.write(outstream) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
    } else if options.head_count < 0 {
        let mut first_but_n: VecDeque<AnyTransducer> = VecDeque::new();
        verbose_print(
            common,
            &format!("Counting all but last {}\n", options.head_count),
        );
        while instream.is_good() {
            transducer_n += 1;
            let trans = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            first_but_n.push_back(trans);
        }
        if (-options.head_count) as usize > first_but_n.len() {
            warning(
                common,
                0,
                0,
                &format!(
                    "Stream in {} has less than {} automata; Nothing will be written to output",
                    common.input_filename, -options.head_count
                ),
            );
        }
        for _ in 0..(-options.head_count) {
            if !first_but_n.is_empty() {
                first_but_n.pop_back();
            }
        }
        while !first_but_n.is_empty() {
            // C: copied the front and popped it afterwards; taking it by
            // value is the same write in one move.
            let mut trans = first_but_n
                .pop_front()
                .expect("first_but_n is non-empty per the enclosing while condition");
            let mut inputname = trans.get_name();
            if inputname.is_empty() {
                inputname = common.input_filename.clone();
            }
            verbose_print(
                common,
                &format!("Forwarding {}...{}\n", inputname, transducer_n),
            );
            if let Err(e) = trans.write(outstream) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
    }
    if let Err(e) = outstream.flush() {
        error(common, 1, 0, &format!("{e}"));
        return 1;
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-head.main-fn]
// [spec:hfst:sem:hfst-head.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.2", "HfstHead");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    // here starts the buffer handling part
    let instream_result = if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)
    let mut instream = match instream_result {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    let ty = instream.get_type();
    let outstream_result = if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    };
    let mut outstream = match outstream_result {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    process_stream(&common, &options, &mut instream, &mut outstream)
}
