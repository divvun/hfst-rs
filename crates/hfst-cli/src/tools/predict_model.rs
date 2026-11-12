//! `hfst predict-model`: count the units and word forms of analysed text,
//! as hfst-lookup writes it, into a prediction model. The counting and the
//! file are [`hfst::prediction::model`]'s; this tool reads the texts and
//! writes the model.

use std::io::{Read, Write};

use super::predict::{AmbiguityName, BackoffName, Generator, Sources, fail, open_inputs, reason};
use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use hfst::prediction::model::{BuildReport, ModelBuilder, ModelSettings, generator_accepts};
use hfst::prediction::units::{AnalysisTokenizer, UnitGrammar};
use hfst::prediction::walk::{WalkLimits, Walker};
use hfst::transducer::{Transducer, TransducerTablesInterface};

/// hfst-predict-model's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Build a prediction model from analysed text")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    #[command(flatten)]
    sources: Sources,

    /// The length of the longest run of units counted
    #[arg(long = "order", value_name = "N", default_value_t = 3)]
    order: usize,

    /// How scores back off to shorter contexts
    #[arg(long = "backoff", value_enum, default_value_t = BackoffName::StupidBackoff)]
    backoff: BackoffName,

    /// How a token with more than one reading is counted
    #[arg(long = "ambiguity", value_enum, default_value_t = AmbiguityName::FirstReading)]
    ambiguity: AmbiguityName,

    /// How many unit keys each class's keyboard face holds
    #[arg(long = "face-size", value_name = "N", default_value_t = 0)]
    face_size: usize,

    /// The most bytes the model may take; the least frequent longest runs are
    /// dropped to fit
    #[arg(long = "size-limit", value_name = "BYTES")]
    size_limit: Option<usize>,

    /// Files of analysed text as hfst-lookup writes it; none or - reads the
    /// standard input. No run of units spans two files.
    #[arg(value_name = "INFILE", num_args = 0..)]
    inputs: Vec<String>,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, _opts: &mut CommonOptions) {}
}

impl Args {
    fn settings(&self) -> ModelSettings {
        ModelSettings {
            order: self.order,
            backoff: self.backoff.backoff(),
            ambiguity: self.ambiguity.ambiguity(),
            face_size: self.face_size,
            size_limit: self.size_limit.unwrap_or(usize::MAX),
        }
    }
}

/// The counts a build reports, one to a line.
fn report_lines(report: &BuildReport, word_forms: usize, model_bytes: usize) -> String {
    let lines = [
        ("tokens", report.tokens),
        ("readings", report.readings),
        ("counted", report.counted),
        ("refused by the generator", report.refused_by_generator),
        ("refused by the grammar", report.refused_by_grammar),
        ("tokens without a reading", report.tokens_without_reading),
        ("word forms", word_forms),
        ("model bytes", model_bytes),
    ];
    lines
        .iter()
        .map(|(what, count)| format!("{what:<25} {count:>10}\n"))
        .collect()
}

/// Count every input, each a text of its own, and give what was counted,
/// how many word forms, and the model's bytes.
fn build<T: TransducerTablesInterface>(
    args: &Args,
    generator: &Transducer<T>,
    grammar: &UnitGrammar,
) -> Result<(BuildReport, usize, Vec<u8>), String> {
    // Making a walker checks the grammar against the generator's alphabet.
    Walker::new(generator, grammar, WalkLimits::default()).map_err(|e| reason(&e))?;
    let tokenizer = AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table());
    let mut builder = ModelBuilder::new(
        grammar,
        &tokenizer,
        generator_accepts(generator),
        args.settings(),
    )
    .map_err(|e| reason(&e))?;
    let mut report = BuildReport::default();
    for (name, mut reader) in open_inputs(&args.inputs)? {
        let mut text = String::new();
        reader
            .read_to_string(&mut text)
            .map_err(|e| format!("cannot read {name}: {e}"))?;
        report += builder
            .add_text(&text)
            .map_err(|e| format!("{name}: {}", reason(&e)))?;
    }
    let model = builder.write().map_err(|e| reason(&e))?;
    Ok((report, builder.word_forms(), model))
}

fn run(common: &CommonOptions, args: &Args) -> Result<(), String> {
    let (generator, grammar) = args.sources.read()?;
    let (report, word_forms, model) = match &generator {
        Generator::Weighted(generator) => build(args, generator, &grammar)?,
        Generator::Unweighted(generator) => build(args, generator, &grammar)?,
    };
    let written = |e: std::io::Error| format!("cannot write {}: {e}", common.output_filename);
    let mut out = common.output_writer().map_err(written)?;
    out.write_all(&model).map_err(written)?;
    out.flush().map_err(written)?;
    if !common.silent {
        eprint!("{}", report_lines(&report, word_forms, model.len()));
    }
    Ok(())
}

pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();
    let common = hfst_set_program_name(&argv0, "0.1", "HfstPredictModel");
    let (common, args) = cli::parse::<Args>(common, args)?;
    run(&common, &args).map_err(|message| fail(&common, &message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_gives_each_count_a_line() {
        let report = BuildReport {
            tokens: 9,
            readings: 11,
            counted: 7,
            refused_by_generator: 2,
            refused_by_grammar: 1,
            tokens_without_reading: 1,
        };
        let lines = report_lines(&report, 5, 1234);
        assert_eq!(lines.lines().count(), 8);
        assert!(lines.contains("word forms                         5\n"));
        assert!(lines.starts_with("tokens                             9\n"));
        assert!(lines.contains("refused by the generator           2\n"));
        assert!(lines.ends_with("model bytes                     1234\n"));
    }

    #[test]
    fn no_size_limit_is_the_largest_size() {
        let args = <Args as clap::Parser>::parse_from([
            "hfst predict-model",
            "-g",
            "g.hfstol",
            "-G",
            "g.toml",
            "--backoff",
            "drop-oldest",
            "--ambiguity",
            "equal-shares",
        ]);
        let settings = args.settings();
        assert_eq!(settings.order, 3);
        assert_eq!(settings.size_limit, usize::MAX);
        assert_eq!(settings.face_size, 0);
        assert_eq!(
            settings.backoff,
            hfst::prediction::model::Backoff::DropOldest
        );
        assert_eq!(
            settings.ambiguity,
            hfst::prediction::model::Ambiguity::EqualShares
        );
    }
}
