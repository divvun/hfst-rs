//! Faithful 1:1 port of tools/src/hfst-head.cc — the transducer archive head
//! splitting tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and a
//! tool-local [`Options`], threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, parse_i64, verbose_print, warning};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::AnyTransducer;
use std::collections::VecDeque;

/// hfst-head's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-n, --n-first=[-]K': number of transducers to keep from the head.
    head_count: i64,
}

/// hfst-head's command line.
// [spec:hfst:def:hfst-head.parse-options-fn]
// [spec:hfst:sem:hfst-head.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Get first transducers from an archive",
    after_help = "K must be an integer, as parsed by strtoul base 10, and not 0.
If K is omitted default is 1."
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Print the first K transducers; with the leading `-', print all but
    /// the last K transducers
    #[arg(
        short = 'n',
        long = "n-first",
        value_name = "[-]K",
        allow_hyphen_values = true
    )]
    n_first: Option<String>,
}

impl Args {
    /// Case 'n': hfst_strtol(optarg, 10), fatal on anything else. Without
    /// -n the count stays at the C initialiser of 1.
    fn head_count(&self, common: &CommonOptions) -> i64 {
        match &self.n_first {
            Some(k) => parse_i64(common, k, 10),
            None => 1,
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
        // parameter checks; run it here for the same ordering. The
        // count-of-0 warning came AFTER them and stays in the tool body.
        self.head_count(opts);
        Ok(())
    }
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
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.2", "HfstHead");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options {
        head_count: args.head_count(&common),
    };
    // The C emitted this after the common + unary parameter checks.
    if options.head_count == 0 {
        warning(&common, 0, 0, "Argument 0 for count is not sensible");
    }

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
