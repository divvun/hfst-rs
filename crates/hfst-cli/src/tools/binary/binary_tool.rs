//! Faithful 1:1 port of tools/src/hfst-binary-tool.cc — the GENERIC BINARY
//! TOOL TEMPLATE command-line tool. Option handling is clap 4 derive through
//! [`crate::cli`]; the rest drives the hfst-cli foundation (globals,
//! commandline, tool-metadata).

use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{hfst_set_program_name, hfst_strformat, verbose_print, warning};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;

/// hfst-binary-tool's command line. The skeleton tool adds nothing to the
/// shared common + binary option groups.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Do things with two transducers",
    after_help = "The operation is applied pairwise for INFILE1 and INFILE2, which must hold the \
same number of transducers; if INFILE2 holds only one, it is kept constant \
across INFILE1.

Examples:
  hfst-binary-tool -o catdog.hfst cat.hfst dog.hfst  does things"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-binary-tool.binaryoperate-streams-fn]
// [spec:hfst:sem:hfst-binary-tool.binaryoperate-streams-fn]
fn binaryoperate_streams(
    common: &CommonOptions,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    // (the C opens each stream here; the Rust streams are opened by their
    // constructors, so the explicit open() calls are not reproduced.)
    // should be is_good?
    let mut both_inputs = firststream.is_good() && secondstream.is_good();
    if firststream.get_type() != secondstream.get_type() {
        warning(
            common,
            0,
            0,
            &format!(
                "Tranducer type mismatch in {} and {}; using former type as output\n",
                common.first_filename, common.second_filename
            ),
        );
    }
    let mut transducer_n: usize = 0;
    while both_inputs {
        transducer_n += 1;
        if transducer_n == 1 {
            verbose_print(
                common,
                &format!(
                    "Doing things with {} and {}...\n",
                    common.first_filename, common.second_filename
                ),
            );
        } else {
            verbose_print(
                common,
                &format!(
                    "Doing things with {} and {}... {}\n",
                    common.first_filename, common.second_filename, transducer_n
                ),
            );
        }
        let first = match firststream.read() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
        };
        let second = match secondstream.read() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
        };
        // one dispatch per pair ([dec:hfst:monomorphic-backends]); the
        // C++ concatenate threw TransducerTypeMismatch for mixed operands
        // at runtime, which is now the boundary's mismatch arm. The output
        // stream was opened in the first stream's type, so every algebra
        // backend needs an arm of its own or the write cannot match it.
        use hfst::hfst_transducer::AnyTransducer;
        let code = match (first, second) {
            (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                concatenate_pair(f, s, outstream)
            }
            #[cfg(feature = "foma")]
            (AnyTransducer::Foma(f), AnyTransducer::Foma(s)) => concatenate_pair(f, s, outstream),
            (f, s) => {
                eprintln!(
                    "hfst-binary-tool: the formats {} and {} are not compatible",
                    hfst_strformat(f.get_type()),
                    hfst_strformat(s.get_type())
                );
                return 1;
            }
        };
        if code != 0 {
            return code;
        }
        both_inputs = firststream.is_good() && secondstream.is_good();
    }

    if firststream.is_good() {
        warning(
            common,
            0,
            0,
            &format!(
                "Warning: {} contains more transducers than {}; residue skipped\n",
                common.first_filename, common.second_filename
            ),
        );
    } else if secondstream.is_good() {
        warning(
            common,
            0,
            0,
            &format!(
                "Warning: {} contains fewer transducers than {}; residue skipped\n",
                common.first_filename, common.second_filename
            ),
        );
    }
    firststream.close();
    secondstream.close();
    outstream.close();
    0
}

// The monomorphic pair body of the skeleton tool.
fn concatenate_pair<B: hfst::backend::AlgebraBackend>(
    mut first: HfstTransducer<B>,
    second: HfstTransducer<B>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    if let Err(e) = first.concatenate(&second, true) {
        eprintln!("hfst-binary-tool: {e}");
        return 1;
    }
    if let Err(e) = outstream.write(&mut first) {
        eprintln!("hfst-binary-tool: {e}");
        return 1;
    }
    0
}

// [spec:hfst:def:hfst-binary-tool.main-fn]
// [spec:hfst:sem:hfst-binary-tool.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstGenericBinaryTool");
    let (common, _args) = cli::parse::<Args>(common, args)?;

    // close buffers, we use streams
    let first_opened = common.first_filename != "<stdin>";
    let second_opened = common.second_filename != "<stdin>";
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {} and {}, writing to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps each ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch
    // arms are not reproduced here.)
    let firststream_res = if first_opened {
        HfstInputStream::new_filename(&common.first_filename)
    } else {
        HfstInputStream::new()
    };
    let mut firststream = match firststream_res {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-binary-tool: {e}");
            return Err(1);
        }
    };
    let secondstream_res = if second_opened {
        HfstInputStream::new_filename(&common.second_filename)
    } else {
        HfstInputStream::new()
    };
    let mut secondstream = match secondstream_res {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-binary-tool: {e}");
            return Err(1);
        }
    };
    let ty = firststream.get_type();
    let outstream_res = if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    };
    let mut outstream = match outstream_res {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-binary-tool: {e}");
            return Err(1);
        }
    };

    // (the C main calls concatenate_streams; the defined function is
    // binaryoperate_streams — the same routine — which is invoked here.)
    cli::from_code(binaryoperate_streams(
        &common,
        &mut firststream,
        &mut secondstream,
        &mut outstream,
    ))
}
