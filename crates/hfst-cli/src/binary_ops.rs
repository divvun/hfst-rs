//! Shared driver for the two-input-stream (BINARY) command-line tools. The
//! main/<op>_streams scaffolding of tools/src/hfst-{subtract,concatenate,
//! disjunct,conjunct,compose,priority-disjunct,shuffle}.cc is a verbatim copy
//! per tool differing only in the operation applied, the operation's names in
//! messages, and a few structural quirks; it is lifted here once and
//! parameterized by an op descriptor. Each tool keeps its own print_usage,
//! parse_options and option statics, and passes the descriptor plus closures
//! in from its thin main. hfst-compare (no output stream) and
//! hfst-compose-intersect (rules-then-lexicons loop) keep their own loops and
//! reuse only the stream-opening/type-resolution helpers.

use hfst::backend::AlgebraBackend;
use hfst::error::ErrorKind;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use std::io::Write;

use crate::IntoAny;
use crate::globals;
use crate::hfst_commandline::{
    conversion_type, convert_transducers, error, hfst_strformat, is_input_stream_in_ol_format,
    verbose_print, warning,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_binary, hfst_set_name_binary};

/// How the n-ary read loop keeps or drops the pair between rounds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoopStyle {
    /// The disjunct-family loop: a new first transducer is read every round;
    /// the second transducer is kept only while the second input holds
    /// exactly one transducer; only the first input may hold the residue.
    Standard,
    /// The compose loop: either input may hold exactly one transducer that is
    /// reused against every transducer of the other input, and the
    /// fewer-transducers error for the second input carries the extra
    /// exactly-one clause.
    Compose,
}

/// What the per-pair apply does when the first attempt fails.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicy {
    /// Convert and retry on any failure (the C caught every exception here).
    AnyError,
    /// Convert and retry only on a transducer type mismatch; any other
    /// failure is fatal.
    TypeMismatchOnly,
    /// The shuffle variant of TypeMismatchOnly: a not-an-automaton failure
    /// additionally gets its own error text.
    ShuffleAutomata,
}

/// The per-tool constants of the shared scaffolding: the operation's names in
/// the message texts, the metadata op string and formula symbol, and the
/// structural knobs.
pub struct BinaryOpSpec {
    /// Program name for the OL-format rejection and the conversion_type
    /// panic text, e.g. "hfst-disjunct".
    pub tool_name: &'static str,
    /// Noun in the up-front type-mismatch error, e.g. "disjunction".
    pub mismatch_noun: &'static str,
    /// Verb in the per-pair could-not error, e.g. "disjunct".
    pub could_not_verb: &'static str,
    /// Noun in the per-pair could-not error, e.g. "disjunction" (shuffle
    /// alone uses a different noun here than in the up-front error).
    pub could_not_noun: &'static str,
    /// Op string for hfst_set_name_binary, e.g. "union".
    pub name_op: &'static str,
    /// Symbol for hfst_set_formula_binary, e.g. "\u{222a}".
    pub formula: &'static str,
    /// Start of the per-pair verbose line, e.g. "Disjuncting A and B"; the
    /// driver appends "...\n" or "... N\n".
    pub verbose_begin: fn(firstname: &str, secondname: &str) -> String,
    pub loop_style: LoopStyle,
    pub retry: RetryPolicy,
    /// Flush the output stream at the end of every round.
    pub flush_each_round: bool,
    /// Flush the output stream once after the loop, before closing.
    pub flush_at_end: bool,
}

/// Per-pair context handed to the pre-apply hook (and used for the could-not
/// error texts).
pub struct PairContext<'a> {
    pub firstname: &'a str,
    pub secondname: &'a str,
    pub transducer_n_first: usize,
    pub first_type: ImplementationType,
    pub second_type: ImplementationType,
}

/// The tool's operation, generic over the algebra backend: the driver
/// dispatches ONCE per stream-read pair ([dec:hfst:monomorphic-backends]) and
/// runs 'pre_apply' (the flag-diacritics gates; an Err(code) aborts the
/// streams function with that exit code) then 'apply' (dest op= src) inside
/// the monomorphic body.
pub trait BinaryToolOp {
    fn pre_apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &mut HfstTransducer<B>,
        ctx: &PairContext,
    ) -> Result<(), i32> {
        let _ = (first, second, ctx);
        Ok(())
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()>;
}

/// Opens the two input streams (named file vs stdin), reporting failures the
/// way every binary tool does.
pub unsafe fn open_two_input_streams<'a, 'b>()
-> Result<(HfstInputStream<'a>, HfstInputStream<'b>), i32> {
    let first_opened = globals::first_filename() != "<stdin>";
    let second_opened = globals::second_filename() != "<stdin>";
    // here starts the buffer handling part
    // (the C wraps each ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch
    // arms are not reproduced here.)
    let firststream = match if first_opened {
        HfstInputStream::new_filename(&globals::first_filename())
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            error(1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    let secondstream = match if second_opened {
        HfstInputStream::new_filename(&globals::second_filename())
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            error(1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    Ok((firststream, secondstream))
}

/// Resolves the output transducer type from the two input types, emitting the
/// shared type-mismatch warning/error texts (op-name parameterized).
pub unsafe fn resolve_output_type(
    tool_name: &str,
    mismatch_noun: &str,
    type1: ImplementationType,
    type2: ImplementationType,
) -> ImplementationType {
    unsafe {
        let mut output_type = ImplementationType::UNSPECIFIED_TYPE;
        if type1 != type2 {
            if globals::ALLOW_TRANSDUCER_CONVERSION {
                let ct = conversion_type(type1, type2);
                let mut warnstr = format!(
                    "Transducer type mismatch in {} and {}; ",
                    globals::first_filename(),
                    globals::second_filename()
                );
                if ct == 1 {
                    warnstr.push_str("using former type as output");
                    output_type = type1;
                } else if ct == 2 {
                    warnstr.push_str("using latter type as output");
                    output_type = type2;
                } else if ct == -1 {
                    warnstr
                        .push_str("using former type as output, loss of information is possible");
                    output_type = type1;
                } else {
                    /* should not happen */
                    std::panic::panic_any(format!(
                        "Error: {}: conversion_type returned an invalid integer",
                        tool_name
                    ));
                }
                warning(0, 0, &warnstr);
            } else {
                error(
                    1,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; formats {} and {} are not compatible for {} (--do-not-convert was requested)",
                        globals::first_filename(),
                        globals::second_filename(),
                        hfst_strformat(type1),
                        hfst_strformat(type2),
                        mismatch_noun
                    ),
                );
            }
        } else {
            output_type = type1;
        }
        output_type
    }
}

/// Creates the output stream (named file vs stdout) with the shared error
/// reporting.
pub unsafe fn open_output_stream(output_type: ImplementationType) -> Result<HfstOutputStream, i32> {
    let output_named = globals::output_filename() != "<stdout>";
    match if output_named {
        HfstOutputStream::new_filename(&globals::output_filename(), output_type, true)
    } else {
        HfstOutputStream::new(output_type, true)
    } {
        Ok(v) => Ok(v),
        Err(e) => {
            error(1, 0, &format!("{e}"));
            Err(1)
        }
    }
}

/// Emits the per-pair "--do-not-convert was requested" error (exits, as
/// error() with a non-zero status does).
pub fn print_do_not_convert_error(spec: &BinaryOpSpec, ctx: &PairContext) {
    error(
        1,
        0,
        &format!(
            "Could not {} {} and {} [{}]:\nformats {} and {} are not compatible for {} (--do-not-convert was requested)",
            spec.could_not_verb,
            ctx.firstname,
            ctx.secondname,
            ctx.transducer_n_first,
            hfst_strformat(ctx.first_type),
            hfst_strformat(ctx.second_type),
            spec.could_not_noun
        ),
    );
}

/// Everything a binary tool's real_main does after parse_options: the
/// reading/writing verbose line, opening both input streams, the OL-format
/// rejection, and the shared <op>_streams loop.
pub unsafe fn run_binary_streams_tool(spec: &BinaryOpSpec, op: &mut impl BinaryToolOp) -> i32 {
    unsafe {
        // close buffers, we use streams
        verbose_print(&format!(
            "Reading from {} and {}, writing to {}\n",
            globals::first_filename(),
            globals::second_filename(),
            globals::output_filename()
        ));
        let (mut firststream, mut secondstream) = match open_two_input_streams() {
            Ok(v) => v,
            Err(code) => return code,
        };

        if is_input_stream_in_ol_format(&firststream, spec.tool_name)
            || is_input_stream_in_ol_format(&secondstream, spec.tool_name)
        {
            return 1;
        }

        binary_op_streams(spec, &mut firststream, &mut secondstream, op)
    }
}

/// The monomorphic per-pair body: pre-apply hook, the operation with its
/// error policy, tool metadata, and the stream write. Returns both operands
/// (the loop may reuse either, per its LoopStyle) or the exit code.
#[allow(clippy::type_complexity)]
unsafe fn process_pair<B: AlgebraBackend>(
    spec: &BinaryOpSpec,
    op: &mut impl BinaryToolOp,
    mut first: HfstTransducer<B>,
    mut second: HfstTransducer<B>,
    ctx: &PairContext,
    outstream: &mut HfstOutputStream,
) -> Result<(HfstTransducer<B>, HfstTransducer<B>), i32> {
    if let Err(code) = op.pre_apply(&mut first, &mut second, ctx) {
        return Err(code);
    }

    // The C caught the op's TransducerTypeMismatch here and converted-and-
    // retried; same-backend operands are now a compile-time property (the
    // driver converted at the boundary), so only the ops' real failures
    // remain.
    if let Err(e) = op.apply(&mut first, &second) {
        if spec.retry == RetryPolicy::ShuffleAutomata
            && matches!(e.kind, ErrorKind::TransducersAreNotAutomata)
        {
            error(
                1,
                0,
                &format!(
                    "Could not {} {} and {} [{}]\nat least one of the input arguments is not an automaton",
                    spec.could_not_verb, ctx.firstname, ctx.secondname, ctx.transducer_n_first
                ),
            );
        } else {
            error(1, 0, &format!("{e}"));
        }
        return Err(1);
    }

    // C: hfst_set_name(*first, *first, *second, op); the dest and
    // first src are the same object, which Rust cannot alias
    // mut+const, so the read side is taken from a copy (name/formula
    // are unchanged by the copy).
    let first_src = first.clone();
    hfst_set_name_binary(&mut first, &first_src, &second, spec.name_op);
    hfst_set_formula_binary(&mut first, &first_src, &second, spec.formula);
    if let Err(e) = outstream.redirect(&mut first) {
        error(1, 0, &format!("{e}"));
        return Err(1);
    }
    Ok((first, second))
}

/// The shared <op>_streams body: type resolution, output stream creation, the
/// n-ary read loop with the per-pair apply and its convert-and-retry
/// fallback, the fewer-transducers error texts, and the closing sequence.
unsafe fn binary_op_streams(
    spec: &BinaryOpSpec,
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
    op: &mut impl BinaryToolOp,
) -> i32 {
    unsafe {
        // there must be at least one transducer in both input streams
        let mut continue_reading = firststream.is_good() && secondstream.is_good();

        let type1 = firststream.get_type();
        let type2 = secondstream.get_type();
        let output_type = resolve_output_type(spec.tool_name, spec.mismatch_noun, type1, type2);

        let mut outstream = match open_output_stream(output_type) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut first: Option<AnyTransducer> = None;
        let mut second: Option<AnyTransducer> = None;
        let mut transducer_n_first: usize = 0; // transducers read from first stream
        let mut transducer_n_second: usize = 0; // transducers read from second stream
        while continue_reading {
            let read_first = match spec.loop_style {
                LoopStyle::Standard => true,
                LoopStyle::Compose => firststream.is_good(),
            };
            if read_first {
                first = Some(match firststream.read() {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                });
                transducer_n_first += 1;
            }
            if secondstream.is_good() {
                second = Some(match secondstream.read() {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                });
                transducer_n_second += 1;
            }
            let firstname = hfst_get_name(
                first
                    .as_ref()
                    .expect("first transducer present (just read)"),
                &globals::first_filename(),
            );
            if second.is_none() {
                // make scan-build happy, this should not happen
                std::panic::panic_any(String::from("Error: second stream has a NULL value."));
            }
            let secondname = hfst_get_name(
                second
                    .as_ref()
                    .expect("second transducer present (just read)"),
                &globals::second_filename(),
            );
            if transducer_n_first == 1 {
                verbose_print(&format!(
                    "{}...\n",
                    (spec.verbose_begin)(&firstname, &secondname)
                ));
            } else {
                verbose_print(&format!(
                    "{}... {}\n",
                    (spec.verbose_begin)(&firstname, &secondname),
                    transducer_n_first
                ));
            }

            let ctx = PairContext {
                firstname: &firstname,
                secondname: &secondname,
                transducer_n_first,
                first_type: firststream.get_type(),
                second_type: secondstream.get_type(),
            };

            // The C applied the op and caught TransducerTypeMismatch, then
            // converted and retried (per the tool's RetryPolicy) or emitted
            // the --do-not-convert error. Same-backend operands are now the
            // compile-time property of the generic body, so the type check
            // moves to this boundary: mismatched operands convert (with the
            // exact convert_transducers warning texts) before the ONE
            // dispatch below. (For mixed-type inputs whose pre-apply hook
            // also prints, the conversion warning now precedes the hook's
            // messages instead of following them.)
            let any_first = first.take().expect("first transducer present (just read)");
            let any_second = second
                .take()
                .expect("second transducer present (just read)");
            let (any_first, any_second) = if any_first.get_type() != any_second.get_type() {
                if globals::ALLOW_TRANSDUCER_CONVERSION {
                    match convert_transducers(any_first, any_second) {
                        Ok(pair) => pair,
                        Err(e) => {
                            error(1, 0, &format!("{e}"));
                            return 1;
                        }
                    }
                } else {
                    print_do_not_convert_error(spec, &ctx);
                    return 1;
                }
            } else {
                (any_first, any_second)
            };

            // the one runtime dispatch per pair ([dec:hfst:monomorphic-backends])
            let processed = match (any_first, any_second) {
                (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                    process_pair(spec, op, f, s, &ctx, &mut outstream)
                        .map(|(f, s)| (f.into_any(), s.into_any()))
                }
                (AnyTransducer::Log(f), AnyTransducer::Log(s)) => {
                    process_pair(spec, op, f, s, &ctx, &mut outstream)
                        .map(|(f, s)| (f.into_any(), s.into_any()))
                }
                #[cfg(feature = "foma")]
                (AnyTransducer::Foma(f), AnyTransducer::Foma(s)) => {
                    process_pair(spec, op, f, s, &ctx, &mut outstream)
                        .map(|(f, s)| (f.into_any(), s.into_any()))
                }
                _ => {
                    // Unreachable: OL streams were rejected before the loop
                    // and the mismatch arm above unified the algebra types;
                    // keep the OL rejection text for safety.
                    let _ = write!(
                        std::io::stderr(),
                        "Error: {} cannot process transducers that are in optimized lookup format.\n",
                        spec.tool_name
                    );
                    return 1;
                }
            };
            match processed {
                Ok((f, s)) => {
                    first = Some(f);
                    second = Some(s);
                }
                Err(code) => return code,
            }

            match spec.loop_style {
                LoopStyle::Standard => {
                    continue_reading = firststream.is_good()
                        && (secondstream.is_good() || transducer_n_second == 1);

                    first = None;
                    // delete the transducer of second stream, unless we
                    // continue reading the first stream and there is only one
                    // transducer in the second stream
                    if (continue_reading && secondstream.is_good()) || !continue_reading {
                        second = None;
                    }
                }
                LoopStyle::Compose => {
                    continue_reading = (firststream.is_good() && secondstream.is_good())
                        || (firststream.is_good() && (transducer_n_second == 1))
                        || ((transducer_n_first == 1) && secondstream.is_good());

                    if !continue_reading {
                        first = None;
                        second = None;
                    } else {
                        if firststream.is_good() {
                            first = None;
                        }
                        if secondstream.is_good() {
                            second = None;
                        }
                    }
                }
            }

            if spec.flush_each_round {
                if let Err(e) = outstream.flush() {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            }
        }

        if firststream.is_good() {
            error(
                1,
                0,
                &format!(
                    "second input '{}' contains fewer transducers than first input '{}'; this is only possible if the second input contains exactly one transducer",
                    globals::second_filename(),
                    globals::first_filename()
                ),
            );
        }

        if secondstream.is_good() {
            match spec.loop_style {
                LoopStyle::Standard => {
                    error(
                        1,
                        0,
                        &format!(
                            "first input '{}' contains fewer transducers than second input '{}'",
                            globals::first_filename(),
                            globals::second_filename()
                        ),
                    );
                }
                LoopStyle::Compose => {
                    error(
                        1,
                        0,
                        &format!(
                            "first input '{}' contains fewer transducers than second input '{}'; this is only possible if the first input contains exactly one transducer",
                            globals::first_filename(),
                            globals::second_filename()
                        ),
                    );
                }
            }
        }

        firststream.close();
        secondstream.close();
        if spec.flush_at_end {
            if let Err(e) = outstream.flush() {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        outstream.close();
        0
    }
}
