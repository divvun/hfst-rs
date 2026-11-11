//! Faithful 1:1 port of tools/src/hfst-preprocess-for-optimized-lookup-format.cc
//! — the transducer preprocessing tool (the C++ source is the epsilon-removal /
//! rebuild tool). Option handling is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print};

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::tools::metadata::hfst_get_name;
use hfst::tools::operations;
use std::io::Write;

/// hfst-preprocess-for-optimized-lookup-format's command line: the common
/// and unary options only.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Remove epsilons from a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_algebra!(any, trans => {
        let mut trans = trans;
        let inputname = hfst_get_name(&trans, &common.input_filename);
        if transducer_n == 1 {
            verbose_print(common, &format!("Removing epsilons {}...\n", inputname));
        } else {
            verbose_print(common, &format!(
                "Removing epsilons {}...{}\n",
                inputname, transducer_n
            ));
        }
        let rebuilt = operations::preprocess_for_optimized_lookup_format(&mut trans, &mut || {
            if transducer_n == 1 {
                verbose_print(common, &format!("Rebuilding and fixing {}...\n", inputname));
            } else {
                verbose_print(common, &format!(
                    "Rebuilding and fisting {}...{}\n",
                    inputname, transducer_n
                ));
            }
        });
        if let Err(e) = rebuilt {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if let Err(e) = outstream.write(&trans) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        }, else => {
            // The C++ ran its algebra on whatever type arrived and threw
            // FunctionNotImplemented (uncaught) on optimized-lookup input;
            // report the standard OL rejection instead.
            let _ = writeln!(
                std::io::stderr(),
                "Error: hfst-preprocess-for-optimized-lookup-format cannot process transducers that are in optimized lookup format."
            );
            return 1;
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.main-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPreprocessForOptimizedLookupFormat");
    let (common, _args) = cli::parse::<Args>(common, args)?;
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
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    let ty = instream.get_type();
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };

    cli::from_code(process_stream(&common, &mut instream, &mut outstream))
}
