//! Faithful 1:1 port of tools/src/hfst-multiply.cc — the transducer archive
//! duplication tool (writes the first transducer of an archive repeatedly).
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
//! the parsed [`Args`] and threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    hfst_set_program_name, is_input_stream_in_ol_format, parse_u64, verbose_print,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-multiply's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Use first transducer of an archive repeatedly")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Duplicate each transducer NUMBER times; NUMBER must be a positive
    /// integer as parsed by strtoul base 10
    #[arg(
        short = 'n',
        long = "n-times",
        value_name = "NUMBER",
        allow_hyphen_values = true
    )]
    n_times: Option<String>,
}

impl Args {
    /// Case 'n': strtoul base 10, fatal on anything else. Without -n the
    /// count stays at the C initialiser of 1.
    fn dupe_count(&self, common: &CommonOptions) -> u64 {
        match &self.n_times {
            Some(n) => parse_u64(common, n, 10),
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
        // The C rejected a non-numeric NUMBER inside the getopt loop,
        // before the parameter checks; run it here for the same ordering.
        self.dupe_count(opts);
        Ok(())
    }
}

// [spec:hfst:def:hfst-multiply.process-stream-fn]
// [spec:hfst:sem:hfst-multiply.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    dupe_count: u64,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    // C declares 'queue<HfstTransducer> last_n;' here but never uses it.
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("hfst-multiply: {e}");
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_algebra!(any, trans => {
            let mut trans = trans;
            let mut inputname = trans.get_name();
            if inputname.is_empty() {
                inputname = common.input_filename.clone();
            }

            verbose_print(common, &format!(
                "Duplicate {} times {}...{}\n",
                inputname, dupe_count, transducer_n
            ));
            for _ in 0..dupe_count {
                if let Err(e) = outstream.write(&mut trans) {
                    eprintln!("hfst-multiply: {e}");
                    return 1;
                }
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = writeln!(
                std::io::stderr(),
                "Error: hfst-multiply cannot process transducers that are in optimized lookup format."
            );
            return 1;
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-multiply.main-fn]
// [spec:hfst:sem:hfst-multiply.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstDuplicate");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let dupe_count = args.dupe_count(&common);

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
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-multiply: cannot open input: {e}");
            return Err(1);
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    if is_input_stream_in_ol_format(&instream, "hfst-multiply") {
        return Err(1);
    }

    let ty = instream.get_type();
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-multiply: cannot open output: {e}");
            return Err(1);
        }
    };

    cli::from_code(process_stream(
        &common,
        dupe_count,
        &mut instream,
        &mut outstream,
    ))
}
