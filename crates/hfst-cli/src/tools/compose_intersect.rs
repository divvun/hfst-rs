//! Faithful 1:1 port of tools/src/hfst-compose-intersect.cc — the
//! compose-intersect command-line tool (compose a lexicon with one or more
//! rule transducers). This is a BINARY tool: it reads a first stream (the
//! lexicon) and a second stream (the rule file). Option handling is clap 4
//! derive through [`crate::cli`].

use crate::binary_ops::{open_output_stream, open_two_input_streams, resolve_output_type};
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_set_program_name, is_input_stream_in_ol_format, verbose_print, warning,
};
use crate::memory_limit::{self, LimitSource, ResolvedMemoryLimit};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::{HfstTransducer, HfstTransducerVector};
use hfst::tools::compose_intersect::{
    ComposeIntersectOptions, ComposeIntersectWarning, compose_intersect, prepare_rule,
};
use hfst::tools::metadata::hfst_get_name;
use std::io::Write;

// static bool insert_missing_flags=false;

/// hfst-compose-intersect's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Compose a lexicon with one or more rule transducers",
    after_help = "SIZE, in --memory-limit=SIZE, is an integer byte count with an optional binary K/KB/KiB through T/TB/TiB suffix; 0 forces a nonempty product to spill. HFST_COMPOSE_MEMORY_LIMIT supplies SIZE when the option is absent.

If OUTFILE, or either INFILE1 or INFILE2 is missing or -, standard
streams will be used. INFILE1, INFILE2, or both, must be specified
The format of INFILE1 and INFILE2 must be the same; the result will
have the same format as these.
INFILE1 (the lexicon) must contain exactly one transducer.
INFILE2 (rule file) may contain several transducers.

Examples:
  hfst-compose-intersect -o analyzer.hfst lexicon.hfst rules.hfst
compose rules with lexicon"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,

    /// Compose the intersection of the rules with the lexicon instead of
    /// composing the lexicon with the intersection of the rules
    #[arg(short = 'I', long = "invert")]
    invert: bool,

    /// Faster compose intersect using more memory
    #[arg(short = 'f', long = "fast")]
    fast: bool,

    /// Encode weights when minimizing (default is false)
    #[arg(short = 'e', long = "encode-weights")]
    encode_weights: bool,

    /// Harmonize symbols
    #[arg(short = 'a', long = "harmonize")]
    harmonize: bool,

    /// Working-memory allowance for one-rule, non-inverted OpenFst tropical
    /// compose-intersect, as --memory-limit=SIZE (default: 50% of available
    /// RAM; excess spills)
    #[arg(long = "memory-limit", value_name = "SIZE")]
    memory_limit: Option<String>,
}

impl Args {
    /// Case GETOPT_MEMORY_LIMIT: parse SIZE, or refuse before any input is
    /// opened.
    fn memory_limit_bytes(&self, common: &CommonOptions) -> Result<Option<u64>, i32> {
        let Some(argument) = self.memory_limit.as_deref() else {
            return Ok(None);
        };
        match memory_limit::parse_size(argument) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(detail) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "{}: invalid value for --memory-limit: {detail}",
                    common.program_name
                );
                Err(1)
            }
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
        // The SIZE parse ran inside the C getopt loop, before the parameter
        // checks; run it here for the same ordering.
        self.memory_limit_bytes(opts)?;
        Ok(())
    }
}

/// hfst-compose-intersect's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-I, --invert': if true, the intersection of the rules is composed with
    /// the lexicon; otherwise the lexicon is composed with the intersection of
    /// the rules.
    invert: bool,
    /// '-e, --encode-weights': encode weights when minimizing.
    encode_weights: bool,
    /// '-f, --fast': faster compose intersect using more memory.
    fast_ci: bool,
    /// '-a, --harmonize': harmonize symbols.
    harmonize: bool,
}

impl Options {
    fn library(&self) -> ComposeIntersectOptions {
        ComposeIntersectOptions {
            invert: self.invert,
            fast: self.fast_ci,
            encode_weights: self.encode_weights,
            harmonize: self.harmonize,
        }
    }
}

// [spec:hfst:def:hfst-compose-intersect.string-set]
// (typedef std::set<std::string> StringSet → std::collections::BTreeSet<String>)

fn explicit_memory_limit_name(source: LimitSource) -> Option<&'static str> {
    match source {
        LimitSource::Cli => Some("--memory-limit"),
        LimitSource::Environment => Some("HFST_COMPOSE_MEMORY_LIMIT"),
        LimitSource::Automatic | LimitSource::ProbeFallback => None,
    }
}

fn report_memory_policy(common: &CommonOptions, memory_limit: ResolvedMemoryLimit) {
    if common.silent {
        return;
    }
    if memory_limit.source == LimitSource::ProbeFallback {
        warning(
            common,
            0,
            0,
            "Could not determine available RAM; using a 0-byte compose-intersect memory allowance and spilling immediately. Use --memory-limit to override.",
        );
    }
    if memory_limit.cgroup_clamped
        && let Some(requested) = memory_limit.requested_bytes
    {
        warning(
            common,
            0,
            0,
            &format!(
                "Requested compose-intersect memory allowance of {requested} bytes exceeds current cgroup headroom; using {} bytes.",
                memory_limit.allowance_bytes
            ),
        );
    }
}

// [spec:hfst:def:hfst-compose-intersect.compose-streams-fn]
// [spec:hfst:sem:hfst-compose-intersect.compose-streams-fn]
fn compose_streams(
    common: &CommonOptions,
    options: &Options,
    memory_limit: ResolvedMemoryLimit,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
) -> i32 {
    // there must be at least one transducer in both input streams
    let type1 = firststream.get_type();
    let type2 = secondstream.get_type();
    let output_type = resolve_output_type(common, "compose-intersect", type1, type2);

    let tropical = output_type == hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
    if !tropical {
        if let Some(name) = explicit_memory_limit_name(memory_limit.source) {
            error(
                common,
                1,
                0,
                &format!("{name} is supported only for OpenFst tropical compose-intersect"),
            );
            return 1;
        }
    } else {
        report_memory_policy(common, memory_limit);
    }

    let mut outstream = match open_output_stream(common, output_type) {
        Ok(s) => s,
        Err(code) => return code,
    };

    let _both_inputs = firststream.is_good() && secondstream.is_good();

    if is_input_stream_in_ol_format(firststream, "hfst-compose-intersect")
        || is_input_stream_in_ol_format(secondstream, "hfst-compose-intersect")
    {
        return 1;
    }

    // The resolved output type is matched ONCE into the backend type
    // parameter ([dec:hfst:monomorphic-backends]); every read converts to
    // it, exactly as the C++ convert(output_type) calls did.
    match output_type {
        #[cfg(feature = "foma")]
        hfst::hfst_data_types::ImplementationType::FOMA_TYPE => {
            compose_streams_typed::<hfst::backend_foma::FomaTransducer>(
                common,
                options,
                memory_limit,
                firststream,
                secondstream,
                &mut outstream,
            )
        }
        _ => compose_streams_typed::<hfst_openfst::StdVectorFst>(
            common,
            options,
            memory_limit,
            firststream,
            secondstream,
            &mut outstream,
        ),
    }
}

fn compose_streams_typed<
    B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer,
>(
    common: &CommonOptions,
    options: &Options,
    memory_limit: ResolvedMemoryLimit,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
    outstream: &mut hfst::hfst_output_stream::HfstOutputStream,
) -> i32 {
    let mut rules: HfstTransducerVector<B> = Vec::new();
    let mut rule_n: usize = 1;

    while secondstream.is_good() {
        let mut rule: HfstTransducer<B> = match secondstream.read().and_then(|any| any.into_typed())
        {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let rulename = rule.get_name();
        if !rulename.is_empty() {
            verbose_print(
                common,
                &format!("Reading and minimizing rule {}...\n", rulename),
            );
        } else {
            verbose_print(
                common,
                &format!("Reading and minimizing rule {}...\n", rule_n),
            );
        }
        if let Err(e) = prepare_rule(&mut rule, &options.library()) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }

        rules.push(rule);
        rule_n += 1;
    }

    if explicit_memory_limit_name(memory_limit.source).is_some()
        && (rules.len() != 1 || options.invert || options.fast_ci || !B::SUPPORTS_COMPOSE_LOOKAHEAD)
    {
        error(
            common,
            1,
            0,
            "an explicit compose-intersect memory limit requires exactly one rule, non-inverted composition without --fast, and the OpenFst tropical backend",
        );
        return 1;
    }

    let memory_limit = B::SUPPORTS_COMPOSE_LOOKAHEAD.then_some(memory_limit.allowance_bytes);

    while firststream.is_good() {
        verbose_print(common, "Reading lexicon...");
        let mut lexicon: HfstTransducer<B> =
            match firststream.read().and_then(|any| any.into_typed()) {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
        let lexiconname = hfst_get_name(&lexicon, &common.first_filename);
        verbose_print(common, &format!(" {} read\n", lexiconname));

        verbose_print(common, "Computing intersecting composition...\n");

        if let Err(e) = compose_intersect(
            &mut lexicon,
            &mut rules,
            &options.library(),
            (&lexiconname, &common.second_filename),
            memory_limit,
            &mut |w| match w {
                ComposeIntersectWarning::FilteredOutputSymbol(symbol) => warning(
                    common,
                    0,
                    0,
                    &format!(
                        "\nFound output symbols (e.g. \"{}\") in transducer in\n\
                         file {} which will be filtered out because they are\n\
                         not found on the input tapes of transducers in file\n\
                         {}.",
                        symbol, common.first_filename, common.second_filename
                    ),
                ),
                ComposeIntersectWarning::MultiCharOutputSymbol(symbol) => warning(
                    common,
                    0,
                    0,
                    &format!(
                        "\nFound output multi-char symbols (\"{}\") in \n\
                         transducer in file {} which are not found on the\n\
                         input tapes of transducers in file {}.",
                        symbol, common.first_filename, common.second_filename
                    ),
                ),
            },
        ) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }

        verbose_print(
            common,
            &format!("Storing result in {}...\n", common.output_filename),
        );
        if let Err(e) = outstream.write(&lexicon) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }

    firststream.close();
    secondstream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-compose-intersect.main-fn]
// [spec:hfst:sem:hfst-compose-intersect.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstComposeIntersect");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options {
        invert: args.invert,
        encode_weights: args.encode_weights,
        fast_ci: args.fast,
        harmonize: args.harmonize,
    };
    let memory_limit = match memory_limit::resolve(args.memory_limit_bytes(&common)?) {
        Ok(limit) => limit,
        Err(detail) => {
            let _ = writeln!(std::io::stderr(), "{}: {detail}", common.program_name);
            return Err(1);
        }
    };
    // close buffers, we use streams
    verbose_print(
        &common,
        &format!(
            "Reading from {} and {}, writing to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    let (mut firststream, mut secondstream) = open_two_input_streams(&common)?;

    cli::from_code(compose_streams(
        &common,
        &options,
        memory_limit,
        &mut firststream,
        &mut secondstream,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_limit_sources_are_named() {
        assert_eq!(
            explicit_memory_limit_name(LimitSource::Cli),
            Some("--memory-limit")
        );
        assert_eq!(
            explicit_memory_limit_name(LimitSource::Environment),
            Some("HFST_COMPOSE_MEMORY_LIMIT")
        );
    }

    #[test]
    fn automatic_limits_are_implicit() {
        assert_eq!(explicit_memory_limit_name(LimitSource::Automatic), None);
        assert_eq!(explicit_memory_limit_name(LimitSource::ProbeFallback), None);
    }
}
