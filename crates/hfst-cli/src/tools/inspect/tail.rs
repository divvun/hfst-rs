//! Faithful 1:1 port of tools/src/hfst-tail.cc — the transducer archive
//! tailing command-line tool. Option handling is clap 4 derive through
//! [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, parse_i64, verbose_print};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::AnyTransducer;
use std::collections::VecDeque;

/// hfst-tail's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-n, --n-last=[+]K': how many trailing transducers to keep.
    tail_count: i64,
}

/// hfst-tail's command line.
// [spec:hfst:def:hfst-tail.parse-options-fn]
// [spec:hfst:sem:hfst-tail.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Get last transducers from an archive",
    after_help = "K must be an integer, as parsed by strtoul base 10, and not 0.
if K is omitted, it defaults to +1 (all except the first)"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Print the last K transducers; use +K to print transducers starting
    /// from the Kth
    #[arg(
        short = 'n',
        long = "n-last",
        value_name = "[+]K",
        allow_hyphen_values = true
    )]
    n_last: Option<String>,
}

impl Args {
    /// Case 'n': a leading '+' negates the parsed count, which is what
    /// selects the skip-the-first-K mode. Without -n the count stays at the
    /// C initialiser of -1, i.e. '+1'.
    fn tail_count(&self, common: &CommonOptions) -> i64 {
        match self.n_last.as_deref() {
            // swap sign haha lol
            Some(k) if k.starts_with('+') => -parse_i64(common, k, 10),
            Some(k) => parse_i64(common, k, 10),
            None => -1,
        }
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The C rejected a non-numeric K inside the getopt loop, before the
        // parameter checks; run it here for the same ordering.
        self.tail_count(opts);
        Ok(())
    }
}

// [spec:hfst:def:hfst-tail.process-stream-fn]
// [spec:hfst:sem:hfst-tail.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
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
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.2", "HfstTail");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options {
        tail_count: args.tail_count(&common),
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
            return Err(1);
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
            return Err(1);
        }
    };

    cli::from_code(process_stream(
        &common,
        &options,
        &mut instream,
        &mut outstream,
    ))
}
