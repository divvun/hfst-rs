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
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    convert_transducers, error, is_input_stream_in_ol_format, verbose_print, warning,
};
use hfst::tools::conversion::conversion_needed;
use hfst::tools::format::format_description;
use hfst::tools::metadata::hfst_get_name;
use hfst::tools::operations::{self, BinaryOptions, BinaryWarning, Operand};

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

/// Per-pair context (used for the could-not error texts).
pub struct PairContext<'a> {
    pub firstname: &'a str,
    pub secondname: &'a str,
    pub transducer_n_first: usize,
    pub first_type: ImplementationType,
    pub second_type: ImplementationType,
}

/// Why a tool's operation did not give a result.
pub enum ApplyError {
    /// The tool has reported the failure itself; exit with this code.
    Reported(i32),
    /// The library's operation failed.
    Operation(hfst::error::Error),
}

impl From<hfst::error::Error> for ApplyError {
    fn from(e: hfst::error::Error) -> Self {
        ApplyError::Operation(e)
    }
}

/// The tool's operation, generic over the algebra backend: the driver
/// dispatches ONCE per stream-read pair ([dec:hfst:monomorphic-backends]) and
/// runs 'apply' (dest op= src) inside the monomorphic body. 'apply' calls the
/// library's operation (`hfst::tools::operations`), which harmonizes flags
/// when asked, changing the second operand too, and names the result.
pub trait BinaryToolOp {
    fn apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        first: &mut HfstTransducer<B>,
        second: &mut Operand<'_, B>,
    ) -> Result<(), ApplyError>;
}

/// Which of the binary operations that take `--harmonize-flags` a tool runs.
#[derive(Clone, Copy)]
pub enum FlagHarmonizing {
    Concatenate,
    Conjunct,
    Subtract,
}

/// The op of `hfst concatenate`, `hfst conjunct` and `hfst subtract`, which
/// differ only in the library operation they run.
pub struct FlagHarmonizingOp {
    pub kind: FlagHarmonizing,
    pub options: BinaryOptions,
}

impl BinaryToolOp for FlagHarmonizingOp {
    fn apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        first: &mut HfstTransducer<B>,
        second: &mut Operand<'_, B>,
    ) -> Result<(), ApplyError> {
        let run = match self.kind {
            FlagHarmonizing::Concatenate => operations::concatenate,
            FlagHarmonizing::Conjunct => operations::conjunct,
            FlagHarmonizing::Subtract => operations::subtract,
        };
        run(first, second, &self.options, &mut |w| {
            print_binary_warning(common, w)
        })?;
        Ok(())
    }
}

/// Print what a binary operation warns of, as the tools print it.
pub fn print_binary_warning(common: &CommonOptions, warning_given: BinaryWarning) {
    match warning_given {
        BinaryWarning::FlagsNotHarmonized(text) => {
            if !common.silent {
                warning(common, 0, 0, text);
            }
        }
        BinaryWarning::SubtrahendHasFlags => warning(
            common,
            0,
            0,
            &format!(
                "Warning: {} contains flag diacritics. The result of subtraction may be incorrect.",
                common.second_filename
            ),
        ),
    }
}

/// Opens the two input streams (named file vs stdin), reporting failures the
/// way every binary tool does.
pub fn open_two_input_streams<'a, 'b>(
    common: &CommonOptions,
) -> Result<(HfstInputStream<'a>, HfstInputStream<'b>), i32> {
    let first_opened = common.first_filename != "<stdin>";
    let second_opened = common.second_filename != "<stdin>";
    // here starts the buffer handling part
    // (the C wraps each ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch
    // arms are not reproduced here.)
    let firststream = match if first_opened {
        HfstInputStream::new_filename(&common.first_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    let secondstream = match if second_opened {
        HfstInputStream::new_filename(&common.second_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    Ok((firststream, secondstream))
}

/// Resolves the output transducer type from the two input types, emitting the
/// shared type-mismatch warning/error texts (op-name parameterized).
pub fn resolve_output_type(
    common: &CommonOptions,
    mismatch_noun: &str,
    type1: ImplementationType,
    type2: ImplementationType,
) -> ImplementationType {
    let Some(note) = conversion_needed(type1, type2) else {
        return type1;
    };
    if !common.allow_transducer_conversion {
        error(
            common,
            1,
            0,
            &format!(
                "Transducer type mismatch in {} and {}; formats {} and {} are not compatible for {} (--do-not-convert was requested)",
                common.first_filename,
                common.second_filename,
                format_description(type1),
                format_description(type2),
                mismatch_noun
            ),
        );
        return ImplementationType::UNSPECIFIED_TYPE;
    }
    warning(
        common,
        0,
        0,
        &format!(
            "Transducer type mismatch in {} and {}; {}",
            common.first_filename,
            common.second_filename,
            note.output_choice()
        ),
    );
    note.target()
}

/// Creates the output stream (named file vs stdout) with the shared error
/// reporting.
pub fn open_output_stream(
    common: &CommonOptions,
    output_type: ImplementationType,
) -> Result<HfstOutputStream, i32> {
    let output_named = common.output_filename != "<stdout>";
    match if output_named {
        HfstOutputStream::new_filename(&common.output_filename, output_type, true)
    } else {
        HfstOutputStream::new(output_type, true)
    } {
        Ok(v) => Ok(v),
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            Err(1)
        }
    }
}

/// Emits the per-pair "--do-not-convert was requested" error (exits, as
/// error() with a non-zero status does).
pub fn print_do_not_convert_error(
    common: &CommonOptions,
    spec: &BinaryOpSpec,
    ctx: &PairContext<'_>,
) {
    error(
        common,
        1,
        0,
        &format!(
            "Could not {} {} and {} [{}]:\nformats {} and {} are not compatible for {} (--do-not-convert was requested)",
            spec.could_not_verb,
            ctx.firstname,
            ctx.secondname,
            ctx.transducer_n_first,
            format_description(ctx.first_type),
            format_description(ctx.second_type),
            spec.could_not_noun
        ),
    );
}

/// Everything a binary tool's real_main does after parse_options: the
/// reading/writing verbose line, opening both input streams, the OL-format
/// rejection, and the shared <op>_streams loop.
// [spec:hfst:req:cli.main]
pub fn run_binary_streams_tool(
    common: &CommonOptions,
    spec: &BinaryOpSpec,
    op: &mut impl BinaryToolOp,
) -> i32 {
    // close buffers, we use streams
    verbose_print(
        common,
        &format!(
            "Reading from {} and {}, writing to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    let (mut firststream, mut secondstream) = match open_two_input_streams(common) {
        Ok(v) => v,
        Err(code) => return code,
    };

    if is_input_stream_in_ol_format(&firststream, spec.tool_name)
        || is_input_stream_in_ol_format(&secondstream, spec.tool_name)
    {
        return 1;
    }

    binary_op_streams(common, spec, &mut firststream, &mut secondstream, op)
}

/// The monomorphic per-pair body: the operation with its error policy (the
/// library names the result) and the stream write. Returns both operands
/// (the loop may reuse either, per its LoopStyle) or the exit code.
#[allow(clippy::type_complexity)]
fn process_pair<B: AlgebraBackend>(
    common: &CommonOptions,
    spec: &BinaryOpSpec,
    op: &mut impl BinaryToolOp,
    mut first: HfstTransducer<B>,
    second: HfstTransducer<B>,
    ctx: &PairContext<'_>,
    outstream: &mut HfstOutputStream,
) -> Result<(HfstTransducer<B>, HfstTransducer<B>), i32> {
    let mut operand = Operand::Owned(second);
    // The C caught the op's TransducerTypeMismatch here and converted-and-
    // retried; same-backend operands are now a compile-time property (the
    // driver converted at the boundary), so only the ops' real failures
    // remain.
    match op.apply(common, &mut first, &mut operand) {
        Ok(()) => {}
        Err(ApplyError::Reported(code)) => return Err(code),
        Err(ApplyError::Operation(e)) => {
            if spec.retry == RetryPolicy::ShuffleAutomata
                && matches!(e.kind, ErrorKind::TransducersAreNotAutomata)
            {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "Could not {} {} and {} [{}]\nat least one of the input arguments is not an automaton",
                        spec.could_not_verb, ctx.firstname, ctx.secondname, ctx.transducer_n_first
                    ),
                );
            } else {
                error(common, 1, 0, &format!("{e}"));
            }
            return Err(1);
        }
    }
    let second = match operand.into_owned() {
        Ok(second) => second,
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    if let Err(e) = outstream.write(&first) {
        error(common, 1, 0, &format!("{e}"));
        return Err(1);
    }
    Ok((first, second))
}

/// The shared <op>_streams body: type resolution, output stream creation, the
/// n-ary read loop with the per-pair apply and its convert-and-retry
/// fallback, the fewer-transducers error texts, and the closing sequence.
fn binary_op_streams(
    common: &CommonOptions,
    spec: &BinaryOpSpec,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
    op: &mut impl BinaryToolOp,
) -> i32 {
    // there must be at least one transducer in both input streams
    let mut continue_reading = firststream.is_good() && secondstream.is_good();

    let type1 = firststream.get_type();
    let type2 = secondstream.get_type();
    let output_type = resolve_output_type(common, spec.mismatch_noun, type1, type2);

    let mut outstream = match open_output_stream(common, output_type) {
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
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            });
            transducer_n_first += 1;
        }
        if secondstream.is_good() {
            second = Some(match secondstream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            });
            transducer_n_second += 1;
        }
        let firstname = hfst_get_name(
            first
                .as_ref()
                .expect("first transducer present (just read)"),
            &common.first_filename,
        );
        if second.is_none() {
            // make scan-build happy, this should not happen
            std::panic::panic_any(String::from("Error: second stream has a NULL value."));
        }
        let secondname = hfst_get_name(
            second
                .as_ref()
                .expect("second transducer present (just read)"),
            &common.second_filename,
        );
        if transducer_n_first == 1 {
            verbose_print(
                common,
                &format!("{}...\n", (spec.verbose_begin)(&firstname, &secondname)),
            );
        } else {
            verbose_print(
                common,
                &format!(
                    "{}... {}\n",
                    (spec.verbose_begin)(&firstname, &secondname),
                    transducer_n_first
                ),
            );
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
            if common.allow_transducer_conversion {
                match convert_transducers(common, any_first, any_second) {
                    Ok(pair) => pair,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                }
            } else {
                print_do_not_convert_error(common, spec, &ctx);
                return 1;
            }
        } else {
            (any_first, any_second)
        };

        // the one runtime dispatch per pair ([dec:hfst:monomorphic-backends])
        let processed = match (any_first, any_second) {
            (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                process_pair(common, spec, op, f, s, &ctx, &mut outstream)
                    .map(|(f, s)| (f.into_any(), s.into_any()))
            }
            #[cfg(feature = "foma")]
            (AnyTransducer::Foma(f), AnyTransducer::Foma(s)) => {
                process_pair(common, spec, op, f, s, &ctx, &mut outstream)
                    .map(|(f, s)| (f.into_any(), s.into_any()))
            }
            _ => {
                // Unreachable: OL streams were rejected before the loop
                // and the mismatch arm above unified the algebra types;
                // keep the OL rejection text for safety.
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: {} cannot process transducers that are in optimized lookup format.",
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
                continue_reading =
                    firststream.is_good() && (secondstream.is_good() || transducer_n_second == 1);

                first = None;
                // delete the transducer of second stream, unless we
                // continue reading the first stream and there is only one
                // transducer in the second stream
                if secondstream.is_good() || !continue_reading {
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

        if spec.flush_each_round
            && let Err(e) = outstream.flush()
        {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }

    if firststream.is_good() {
        error(
            common,
            1,
            0,
            &format!(
                "second input '{}' contains fewer transducers than first input '{}'; this is only possible if the second input contains exactly one transducer",
                common.second_filename, common.first_filename
            ),
        );
    }

    if secondstream.is_good() {
        match spec.loop_style {
            LoopStyle::Standard => {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "first input '{}' contains fewer transducers than second input '{}'",
                        common.first_filename, common.second_filename
                    ),
                );
            }
            LoopStyle::Compose => {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "first input '{}' contains fewer transducers than second input '{}'; this is only possible if the first input contains exactly one transducer",
                        common.first_filename, common.second_filename
                    ),
                );
            }
        }
    }

    firststream.close();
    secondstream.close();
    if spec.flush_at_end
        && let Err(e) = outstream.flush()
    {
        error(common, 1, 0, &format!("{e}"));
        return 1;
    }
    outstream.close();
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::xre::XreCompiler;
    use hfst_openfst::StdVectorFst;

    fn regex(src: &str) -> HfstTransducer<StdVectorFst> {
        XreCompiler::<StdVectorFst>::new()
            .compile(src)
            .unwrap_or_else(|| panic!("{src} compiles"))
    }

    /// The input sides of the paths of `t`, sorted.
    fn words(t: &HfstTransducer<StdVectorFst>) -> Vec<String> {
        let mut paths = hfst::hfst_data_types::HfstTwoLevelPaths::new();
        t.extract_paths(&mut paths, -1, -1).expect("paths");
        let mut words: Vec<String> = paths
            .iter()
            .map(|p| {
                p.second
                    .iter()
                    .filter(|(i, _)| !hfst::hfst_symbol_defs::is_epsilon(i))
                    .map(|(i, _)| i.as_str())
                    .collect()
            })
            .collect();
        words.sort();
        words
    }

    // [spec:hfst:req:embed.tools-in-library/test]
    // Each flag-harmonizing tool runs its own library operation, which names
    // the result, and leaves an operand without flags as it was.
    #[test]
    fn each_kind_runs_its_operation() {
        let common = CommonOptions::default();
        for (kind, expected, name) in [
            (FlagHarmonizing::Concatenate, vec!["ab"], "concatenate"),
            (FlagHarmonizing::Conjunct, vec![], "intersect"),
            (FlagHarmonizing::Subtract, vec!["a"], "subtract"),
        ] {
            let mut op = FlagHarmonizingOp {
                kind,
                options: BinaryOptions::default(),
            };
            let mut first = regex("a");
            let second = regex("b");
            let mut operand = Operand::Borrowed(&second);
            assert!(op.apply(&common, &mut first, &mut operand).is_ok());
            assert_eq!(words(&first), expected);
            assert_eq!(first.get_name(), format!("{name}(UNNAMED, UNNAMED)"));
            assert!(matches!(operand, Operand::Borrowed(_)));
        }
    }

    // [spec:hfst:req:embed.tools-in-library/test]
    // A failing operation comes back as the library's error for the driver
    // to report, not as an exit code the op printed itself.
    #[test]
    fn a_failure_is_the_library_error() {
        let common = CommonOptions::default();
        let mut op = FlagHarmonizingOp {
            kind: FlagHarmonizing::Concatenate,
            options: BinaryOptions::default(),
        };
        let mut first = regex("a");
        let second = regex("b");
        let mut operand = Operand::Borrowed(&second);
        let applied = op.apply(&common, &mut first, &mut operand);
        assert!(!matches!(applied, Err(ApplyError::Reported(_))));
        let error = ApplyError::from(hfst::error::Error::new(hfst::error::ErrorKind::Fatal));
        assert!(matches!(error, ApplyError::Operation(_)));
    }
}
