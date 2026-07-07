//! Faithful 1:1 port of tools/src/hfst-tail.cc — the transducer archive
//! tailing command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, parse_i64, verbose_print,
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

/// hfst-tail's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-n, --n-last=[+]K': how many trailing transducers to keep.
    tail_count: i64,
}

impl Default for Options {
    fn default() -> Self {
        Options { tail_count: -1 }
    }
}

// [spec:hfst:def:hfst-tail.print-usage-fn]
// [spec:hfst:sem:hfst-tail.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nGet last transducers from an archive\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Archive options:\n  -n, --n-last=[+]K   Print the last K transducers;\n                      use +K to print transducers starting from the Kth\n",
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "K must be an integer, as parsed by strtoul base 10, and not 0.\nif K is omitted, it defaults to +1 (all except the first)\n",
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-tail.parse-options-fn]
// [spec:hfst:sem:hfst-tail.parse-options-fn]
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
            name: "n-last",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'n' as i32,
        });
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
            let optarg = opt.optarg();
            if optarg.starts_with('+') {
                // swap sign haha lol
                options.tail_count = -parse_i64(&common, &optarg, 10);
            } else {
                options.tail_count = parse_i64(&common, &optarg, 10);
            }
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-tail.process-stream-fn]
// [spec:hfst:sem:hfst-tail.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut last_n: VecDeque<AnyTransducer> = VecDeque::new();
    let mut transducer_n: i64 = 0;
    if options.tail_count > 0 {
        verbose_print(
            common,
            &format!("Counting last {} transducers...\n", options.tail_count),
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
            last_n.push_back(trans);
            if last_n.len() as i64 > options.tail_count {
                last_n.pop_front();
            }
        }
        if options.tail_count < transducer_n {
            transducer_n -= options.tail_count + 1;
        } else {
            transducer_n = 0;
        }
        while !last_n.is_empty() {
            transducer_n += 1;
            verbose_print(
                common,
                &format!("Forwarding {}...{}\n", common.input_filename, transducer_n),
            );
            let mut front = last_n
                .pop_front()
                .expect("last_n is non-empty per the enclosing while condition");
            if let Err(e) = front.write(outstream) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
    } else if options.tail_count < 0 {
        verbose_print(
            common,
            &format!("Skipping {} transducers...\n", -options.tail_count),
        );
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if transducer_n >= -options.tail_count {
                verbose_print(
                    common,
                    &format!("Forwarding {}...{}\n", common.input_filename, transducer_n),
                );
                if let Err(e) = trans.write(outstream) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
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

// [spec:hfst:def:hfst-tail.main-fn]
// [spec:hfst:sem:hfst-tail.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.2", "HfstTail");
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
