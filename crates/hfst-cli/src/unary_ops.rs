//! Shared driver for the single-input-stream (UNARY) transform tools. The
//! run()/process_stream scaffolding of tools/src/hfst-{minimize,invert,
//! reverse,determinize,...}.cc is a verbatim copy per tool differing only in
//! the operation applied, the operation's names in messages, and whether
//! optimized-lookup input is rejected; it is lifted here once and
//! parameterized by an op descriptor, the unary analogue of
//! [`crate::binary_ops`]. Each tool keeps its own parse_options and option
//! struct, and passes the descriptor plus its op in from a thin run().

use hfst::backend::AlgebraBackend;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;
use std::io::Write;

use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, is_input_stream_in_ol_format, verbose_print};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};

/// The per-tool constants of the shared scaffolding.
pub struct UnaryOpSpec {
    /// Program name for the OL-format rejection text, e.g. "hfst-minimize".
    pub tool_name: &'static str,
    /// Reject optimized-lookup input streams up front (most transform tools
    /// do; the C++ tools that operate on OL input leave this false).
    pub reject_ol: bool,
}

/// The tool's operation, generic over the algebra backend: the driver
/// dispatches ONCE per stream read ([dec:hfst:monomorphic-backends]) and runs
/// 'apply' inside the monomorphic body. The message/metadata methods have
/// access to the op's parsed-option state because several upstream tools
/// derive their verbose verb and name stamp from options (push-weights'
/// initial/final, repeat's bounds, ...).
pub trait UnaryToolOp {
    /// Start of the per-transducer verbose line, e.g. "Minimizing cat.hfst";
    /// the driver appends "...\n" for the first transducer and
    /// "...{sep}N\n" after.
    fn verbose_begin(&self, inputname: &str) -> String;

    /// Separator between the "..." and the transducer counter on the 2nd..Nth
    /// verbose line. Upstream drifts between "...2" and "... 2" per tool.
    fn verbose_sep(&self) -> &'static str {
        ""
    }

    /// Op string for hfst_set_name_unary; None skips the name stamp.
    fn name_op(&self) -> Option<Cow<'_, str>> {
        None
    }

    /// Symbol for hfst_set_formula_unary; None skips the formula stamp.
    fn formula(&self) -> Option<Cow<'_, str>> {
        None
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()>;
}

/// Opens the input stream (named file vs stdin), reporting failures the way
/// every unary tool does.
pub fn open_input_stream<'a>(common: &CommonOptions) -> Result<HfstInputStream<'a>, i32> {
    let input_opened = common.input_filename != "<stdin>";
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch
    // arms are not reproduced here.)
    match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => Ok(v),
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            Err(1)
        }
    }
}

/// Opens the output stream (named file vs stdout) in the input stream's
/// implementation type.
pub fn open_output_stream_like(
    common: &CommonOptions,
    instream: &HfstInputStream<'_>,
) -> Result<HfstOutputStream, i32> {
    let output_opened = common.output_filename != "<stdout>";
    let ty = instream.get_type();
    match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(v) => Ok(v),
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            Err(1)
        }
    }
}

/// The shared read-transform-write loop over one input stream.
pub fn unary_streams(
    common: &CommonOptions,
    spec: &UnaryOpSpec,
    op: &mut impl UnaryToolOp,
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
            let begin = op.verbose_begin(&inputname);
            if transducer_n == 1 {
                verbose_print(common, &format!("{begin}...\n"));
            } else {
                let sep = op.verbose_sep();
                verbose_print(common, &format!("{begin}...{sep}{transducer_n}\n"));
            }

            if let Err(e) = op.apply(common, &mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }

            // C: hfst_set_name(trans, trans, op); the dest and src are the
            // same object, which Rust cannot alias mut+const, so the read
            // side is taken from a copy (name/formula unchanged by the copy).
            let name_op = op.name_op().map(Cow::into_owned);
            let formula = op.formula().map(Cow::into_owned);
            if name_op.is_some() || formula.is_some() {
                let src = trans.clone();
                if let Some(op_name) = name_op {
                    hfst_set_name_unary(&mut trans, &src, &op_name);
                }
                if let Some(formula) = formula {
                    hfst_set_formula_unary(&mut trans, &src, &formula);
                }
            }
            if let Err(e) = outstream.redirect(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable when reject_ol is set: the optimized-lookup stream
            // rejection already returned before the loop; keep its text for
            // safety.
            let _ = writeln!(
                std::io::stderr(),
                "Error: {} cannot process transducers that are in optimized lookup format.",
                spec.tool_name
            );
            return 1;
        });
    }
    if let Err(e) = outstream.flush() {
        error(common, 1, 0, &format!("{e}"));
        return 1;
    }
    instream.close();
    outstream.close();
    0
}

/// The whole post-parse_options body of a standard unary tool: the
/// reading-from/writing-to verbose line, stream opening, the optional
/// optimized-lookup rejection, and the loop.
// [spec:hfst:req:cli.main]
pub fn run_unary_tool(
    common: &CommonOptions,
    spec: &UnaryOpSpec,
    op: &mut impl UnaryToolOp,
) -> i32 {
    verbose_print(
        common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    let mut instream = match open_input_stream(common) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let mut outstream = match open_output_stream_like(common, &instream) {
        Ok(v) => v,
        Err(code) => return code,
    };

    if spec.reject_ol && is_input_stream_in_ol_format(&instream, spec.tool_name) {
        return 1;
    }

    unary_streams(common, spec, op, &mut instream, &mut outstream)
}
