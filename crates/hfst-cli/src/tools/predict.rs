//! `hfst predict`: for each analysis read, or start of one, the units the
//! generator accepts next, ranked by a prediction model or by the
//! generator's weight, and the forms of the word when it is complete. The
//! walk, the replay of an analysis and the ranking are
//! [`hfst::prediction`]'s; this tool reads the files and writes the answers.
//!
//! The generator and grammar options, the walk limits and the reading of an
//! optimized-lookup generator are shared with `hfst predict-model` and
//! `hfst predict-eval`.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name};
use hfst::pmatch::PmatchContainer;
use hfst::prediction::model::{Ambiguity, Backoff, Model};
use hfst::prediction::units::{AnalysisTokenizer, Label, UnitGrammar, UnitKey, UnitKind};
use hfst::prediction::walk::{
    Candidate, CandidateOrder, Form, GeneratorWeightOrder, WalkLimits, Walker, Word,
};
use hfst::transducer::{
    OlMember, Transducer, TransducerTablesInterface, UnweightedTables, Weight, WeightedTables,
};

/// The generator and the unit grammar every prediction tool reads.
#[derive(clap::Args, Debug, Clone)]
pub(super) struct Sources {
    /// The generator: an optimized-lookup transducer (.hfstol) from analyses
    /// to words
    #[arg(short = 'g', long = "generator", value_name = "FILE")]
    generator: String,

    /// The unit grammar (TOML) that cuts the generator's analyses into units
    #[arg(short = 'G', long = "grammar", value_name = "FILE")]
    grammar: String,
}

impl Sources {
    pub(super) fn read(&self) -> Result<(Generator, UnitGrammar), String> {
        let generator = read_generator(&self.generator)?;
        let bytes = std::fs::read(&self.grammar)
            .map_err(|e| format!("cannot read unit grammar {}: {e}", self.grammar))?;
        let grammar = UnitGrammar::from_toml(&bytes).map_err(|e| reason(&e))?;
        Ok((generator, grammar))
    }
}

/// The bounds on each walk.
#[derive(clap::Args, Debug, Clone)]
pub(super) struct Limits {
    /// The most symbols one unit may have
    #[arg(
        long = "max-unit-symbols",
        value_name = "N",
        default_value_t = WalkLimits::default().max_unit_symbols
    )]
    max_unit_symbols: usize,

    /// The most configurations one walk may visit
    #[arg(
        long = "max-configurations",
        value_name = "N",
        default_value_t = WalkLimits::default().max_configurations
    )]
    max_configurations: usize,
}

impl Limits {
    pub(super) fn walk(&self) -> WalkLimits {
        WalkLimits {
            max_unit_symbols: self.max_unit_symbols,
            max_configurations: self.max_configurations,
        }
    }
}

/// How a model backs off to shorter contexts.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BackoffName {
    DropOldest,
    StupidBackoff,
}

impl BackoffName {
    pub(super) fn backoff(self) -> Backoff {
        match self {
            BackoffName::DropOldest => Backoff::DropOldest,
            BackoffName::StupidBackoff => Backoff::StupidBackoff,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            BackoffName::DropOldest => "drop-oldest",
            BackoffName::StupidBackoff => "stupid-backoff",
        }
    }
}

/// How a model counts a token with more than one reading.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AmbiguityName {
    FirstReading,
    EqualShares,
}

impl AmbiguityName {
    pub(super) fn ambiguity(self) -> Ambiguity {
        match self {
            AmbiguityName::FirstReading => Ambiguity::FirstReading,
            AmbiguityName::EqualShares => Ambiguity::EqualShares,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            AmbiguityName::FirstReading => "first-reading",
            AmbiguityName::EqualShares => "equal-shares",
        }
    }
}

/// An optimized-lookup generator, in the table layout its header names.
pub(super) enum Generator {
    Weighted(Transducer<WeightedTables>),
    Unweighted(Transducer<UnweightedTables>),
}

/// Read the one optimized-lookup transducer in `path`, behind an HFST
/// header or bare. A transducer of another type is refused: the walk reads
/// optimized-lookup tables where they are loaded.
fn read_generator(path: &str) -> Result<Generator, String> {
    let file = File::open(path).map_err(|e| format!("cannot open generator {path}: {e}"))?;
    let mut reader = BufReader::new(file);
    // A reader that does not start with a header is left untouched.
    if let Ok(properties) = PmatchContainer::parse_hfst3_header(&mut reader) {
        let kind = properties.get("type").map_or("", String::as_str);
        if kind != "HFST_OL" && kind != "HFST_OLW" {
            return Err(format!(
                "generator {path} is a {kind} transducer, not optimized lookup; convert it with hfst fst2fst -w"
            ));
        }
    }
    match OlMember::read_from(&mut reader, None) {
        Ok(OlMember::Weighted(generator)) => Ok(Generator::Weighted(generator)),
        Ok(OlMember::Unweighted(generator)) => Ok(Generator::Unweighted(generator)),
        Err(e) => Err(format!("cannot read generator {path}: {}", reason(&e))),
    }
}

/// What went wrong, in the library's words: its message when it gave one.
pub(super) fn reason(error: &hfst::error::Error) -> String {
    error.message.clone().unwrap_or_else(|| error.to_string())
}

/// Report `message` as the tool's error and give the failing status.
pub(super) fn fail(common: &CommonOptions, message: &str) -> i32 {
    error(common, 0, 0, message);
    1
}

/// An input's name, for messages, and its reader.
pub(super) type Input = (String, Box<dyn BufRead>);

/// Each named input in turn, the standard input for none or "-".
pub(super) fn open_inputs(inputs: &[String]) -> Result<Vec<Input>, String> {
    if inputs.is_empty() {
        return Ok(vec![("<stdin>".to_owned(), stdin())]);
    }
    inputs
        .iter()
        .map(|name| {
            if name == "-" {
                return Ok(("<stdin>".to_owned(), stdin()));
            }
            let file = File::open(name).map_err(|e| format!("cannot open {name}: {e}"))?;
            let reader: Box<dyn BufRead> = Box::new(BufReader::new(file));
            Ok((name.clone(), reader))
        })
        .collect()
}

fn stdin() -> Box<dyn BufRead> {
    Box::new(BufReader::new(std::io::stdin()))
}

/// hfst-predict's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Write the units a generator accepts after each analysis, ranked",
    after_help = OUTPUT_HELP
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    #[command(flatten)]
    sources: Sources,

    /// A prediction model built with the same grammar; it ranks the units,
    /// which otherwise go by the generator's weight, lowest first
    #[arg(short = 'm', long = "model", value_name = "FILE")]
    model: Option<String>,

    /// Write at most N units for each analysis; all when not given
    #[arg(short = 'n', long = "shown", value_name = "N")]
    shown: Option<usize>,

    /// Write each answer as one JSON object on a line of its own
    #[arg(short = 'j', long = "json")]
    json: bool,

    #[command(flatten)]
    limits: Limits,

    /// Files of analyses or starts of analyses, one per line; none or - reads
    /// the standard input
    #[arg(value_name = "INFILE", num_args = 0..)]
    inputs: Vec<String>,
}

const OUTPUT_HELP: &str = "\
Each line read is the start of an analysis in the generator's notation, such
as oqar, oqar+NIQ+Der/vn or oqar+NIQ+Der/vn+N+Abs+Sg. An empty line is the
start of a word. For each, the tab-separated output is a block ended by an
empty line:

  analysis  ANALYSIS  complete=yes|no  stem-next=yes|no  truncated=yes|no  units=TOTAL
  unit      KEY  KIND  LABEL  WEIGHT      one line for each unit, best first
  form      SURFACE  WEIGHT               one line for each form of a complete word

or, when the generator does not accept the analysis, the one line

  error     ANALYSIS  REASON

stem-next=yes says a stem can come next; stems are not listed. truncated=yes
says a walk limit cut the search short, so units may be missing. TOTAL counts
the units before -n cuts them. With --json each analysis is one JSON object
with the same fields, or the analysis and an error.";

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, _opts: &mut CommonOptions) {}
}

/// A unit offered after an analysis.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    key: String,
    kind: UnitKind,
    label: String,
    weight: Weight,
}

/// What the walk found after an analysis.
#[derive(Clone, Debug, PartialEq)]
struct Offer {
    complete: bool,
    stem_next: bool,
    truncated: bool,
    total: usize,
    units: Vec<Shown>,
    forms: Vec<Form>,
}

/// The answer for one line read.
#[derive(Clone, Debug, PartialEq)]
struct Answer {
    analysis: String,
    offer: Result<Offer, String>,
}

/// The name both hfst-embed's packages and this tool give a kind of unit.
fn kind_name(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Stem => "stem",
        UnitKind::Prefix => "prefix",
        UnitKind::Boundary => "boundary",
        UnitKind::Ending => "ending",
        UnitKind::Enclitic => "enclitic",
        UnitKind::Filter => "filter",
    }
}

fn yes_no(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

impl Answer {
    fn tsv(&self) -> String {
        let offer = match &self.offer {
            Ok(offer) => offer,
            Err(why) => return format!("error\t{}\t{why}\n\n", self.analysis),
        };
        let mut text = format!(
            "analysis\t{}\tcomplete={}\tstem-next={}\ttruncated={}\tunits={}\n",
            self.analysis,
            yes_no(offer.complete),
            yes_no(offer.stem_next),
            yes_no(offer.truncated),
            offer.total
        );
        for unit in &offer.units {
            text.push_str(&format!(
                "unit\t{}\t{}\t{}\t{}\n",
                unit.key,
                kind_name(unit.kind),
                unit.label,
                unit.weight
            ));
        }
        for form in &offer.forms {
            text.push_str(&format!("form\t{}\t{}\n", form.surface, form.weight));
        }
        text.push('\n');
        text
    }

    fn json(&self) -> String {
        let value = match &self.offer {
            Err(why) => serde_json::json!({ "analysis": self.analysis, "error": why }),
            Ok(offer) => serde_json::json!({
                "analysis": self.analysis,
                "complete": offer.complete,
                "stem_next": offer.stem_next,
                "truncated": offer.truncated,
                "total": offer.total,
                "units": offer.units.iter().map(|unit| serde_json::json!({
                    "key": unit.key,
                    "kind": kind_name(unit.kind),
                    "label": unit.label,
                    "weight": unit.weight,
                })).collect::<Vec<_>>(),
                "forms": offer.forms.iter().map(|form| serde_json::json!({
                    "surface": form.surface,
                    "weight": form.weight,
                })).collect::<Vec<_>>(),
            }),
        };
        format!("{value}\n")
    }
}

/// One generator's walker with what ranks and shows its units.
struct Predictor<'a, T: TransducerTablesInterface> {
    walker: Walker<'a, T>,
    tokenizer: AnalysisTokenizer,
    order: &'a dyn CandidateOrder,
    shown: usize,
}

impl<T: TransducerTablesInterface> Predictor<'_, T> {
    fn answer(&self, analysis: &str) -> Answer {
        Answer {
            analysis: analysis.to_owned(),
            offer: self.offer(analysis).map_err(|e| reason(&e)),
        }
    }

    fn offer(&self, analysis: &str) -> hfst::error::Result<Offer> {
        let word = self.walker.word_from_analysis(analysis, &self.tokenizer)?;
        let next = self.walker.next(&word);
        let grammar = self.walker.grammar();
        let context: Vec<UnitKey> = word.units().iter().map(|unit| grammar.key(unit)).collect();
        let ranked: Vec<(UnitKey, Weight)> = next
            .candidates
            .iter()
            .map(|candidate| (candidate.key.clone(), candidate.weight))
            .collect();
        let units = self
            .order
            .order(&context, &ranked)
            .into_iter()
            .take(self.shown)
            .filter_map(|index| next.candidates.get(index))
            .map(|candidate| Shown {
                key: candidate.key.as_str().to_owned(),
                kind: candidate.unit.kind,
                label: self.label(&word, candidate),
                weight: candidate.weight,
            })
            .collect();
        Ok(Offer {
            complete: word.is_complete(),
            stem_next: next.stem_next,
            truncated: next.truncated,
            total: next.candidates.len(),
            units,
            forms: next.forms,
        })
    }

    /// The grammar's label for a unit. An ending without one is shown as the
    /// lowest-weight word it completes.
    fn label(&self, word: &Word, candidate: &Candidate) -> String {
        match self.walker.grammar().label(&candidate.unit) {
            Label::Text(text) => text,
            Label::CompleteWord => self
                .walker
                .forms(&word.pick(candidate))
                .into_iter()
                .next()
                .map_or_else(|| candidate.key.as_str().to_owned(), |form| form.surface),
        }
    }
}

/// Answer every line of every input, each answer written as soon as it is
/// found, so a caller can pipe analyses in one at a time.
fn predict_all<T: TransducerTablesInterface>(
    common: &CommonOptions,
    args: &Args,
    generator: &Transducer<T>,
    grammar: &UnitGrammar,
    model: Option<&Model<'_>>,
) -> Result<(), String> {
    let order: &dyn CandidateOrder = match model {
        Some(model) => model,
        None => &GeneratorWeightOrder,
    };
    let predictor = Predictor {
        walker: Walker::new(generator, grammar, args.limits.walk()).map_err(|e| reason(&e))?,
        tokenizer: AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table()),
        order,
        shown: args.shown.unwrap_or(usize::MAX),
    };
    let mut out = common
        .output_writer()
        .map_err(|e| format!("cannot open {}: {e}", common.output_filename))?;
    let written = |e: std::io::Error| format!("cannot write {}: {e}", common.output_filename);
    for (name, reader) in open_inputs(&args.inputs)? {
        for line in reader.lines() {
            let line = line.map_err(|e| format!("cannot read {name}: {e}"))?;
            let analysis = line.strip_suffix('\r').unwrap_or(&line);
            let answer = predictor.answer(analysis);
            let text = if args.json {
                answer.json()
            } else {
                answer.tsv()
            };
            out.write_all(text.as_bytes()).map_err(written)?;
            out.flush().map_err(written)?;
        }
    }
    Ok(())
}

fn run(common: &CommonOptions, args: &Args) -> Result<(), String> {
    let (generator, grammar) = args.sources.read()?;
    let model_bytes = match &args.model {
        Some(path) => {
            Some(std::fs::read(path).map_err(|e| format!("cannot read model {path}: {e}"))?)
        }
        None => None,
    };
    let model = match &model_bytes {
        Some(bytes) => Some(Model::from_bytes(bytes, &grammar).map_err(|e| reason(&e))?),
        None => None,
    };
    match &generator {
        Generator::Weighted(generator) => {
            predict_all(common, args, generator, &grammar, model.as_ref())
        }
        Generator::Unweighted(generator) => {
            predict_all(common, args, generator, &grammar, model.as_ref())
        }
    }
}

pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();
    let common = hfst_set_program_name(&argv0, "0.1", "HfstPredict");
    let (common, args) = cli::parse::<Args>(common, args)?;
    run(&common, &args).map_err(|message| fail(&common, &message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer() -> Answer {
        Answer {
            analysis: "oqar+NIQ+Der/vn".to_owned(),
            offer: Ok(Offer {
                complete: false,
                stem_next: false,
                truncated: true,
                total: 3,
                units: vec![
                    Shown {
                        key: "+N+Abs+Sg".to_owned(),
                        kind: UnitKind::Ending,
                        label: "oqarneq".to_owned(),
                        weight: 0.5,
                    },
                    Shown {
                        key: "+KIP+Der/nv".to_owned(),
                        kind: UnitKind::Boundary,
                        label: "+KIP+Der/nv".to_owned(),
                        weight: 0.0,
                    },
                ],
                forms: vec![Form {
                    surface: "oqar\"neq".to_owned(),
                    weight: 1.0,
                }],
            }),
        }
    }

    #[test]
    fn a_block_holds_header_units_and_forms() {
        assert_eq!(
            answer().tsv(),
            "analysis\toqar+NIQ+Der/vn\tcomplete=no\tstem-next=no\ttruncated=yes\tunits=3\n\
             unit\t+N+Abs+Sg\tending\toqarneq\t0.5\n\
             unit\t+KIP+Der/nv\tboundary\t+KIP+Der/nv\t0\n\
             form\toqar\"neq\t1\n\n"
        );
    }

    #[test]
    fn json_is_one_object_on_one_line() {
        let json = answer().json();
        assert_eq!(json.lines().count(), 1);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["analysis"], "oqar+NIQ+Der/vn");
        assert_eq!(value["truncated"], true);
        assert_eq!(value["total"], 3);
        assert_eq!(value["units"][0]["kind"], "ending");
        assert_eq!(value["units"][1]["label"], "+KIP+Der/nv");
        assert_eq!(value["forms"][0]["surface"], "oqar\"neq");
    }

    #[test]
    fn a_refused_analysis_is_one_error_record() {
        let refused = Answer {
            analysis: "nuna+V".to_owned(),
            offer: Err("the generator does not accept the unit".to_owned()),
        };
        assert_eq!(
            refused.tsv(),
            "error\tnuna+V\tthe generator does not accept the unit\n\n"
        );
        let value: serde_json::Value = serde_json::from_str(&refused.json()).unwrap();
        assert_eq!(value["error"], "the generator does not accept the unit");
        assert!(value.get("units").is_none());
    }

    #[test]
    fn every_kind_has_its_own_name() {
        let kinds = [
            UnitKind::Stem,
            UnitKind::Prefix,
            UnitKind::Boundary,
            UnitKind::Ending,
            UnitKind::Enclitic,
            UnitKind::Filter,
        ];
        let names: std::collections::BTreeSet<&str> = kinds.into_iter().map(kind_name).collect();
        assert_eq!(names.len(), kinds.len());
    }
}
