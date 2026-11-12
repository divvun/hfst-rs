//! `hfst predict-eval`: score variants of unit prediction, and suggestions
//! recorded from other systems, by the keystrokes a simulated user saves on
//! held-out analysed text. The harness is
//! [`hfst::prediction::evaluate`]'s; this tool reads the texts and the
//! recordings, names the variants and writes the report.

use std::io::{Read, Write};

use super::predict::{
    AmbiguityName, BackoffName, Generator, Limits, Sources, Switch, fail, open_inputs,
    parse_unit_weight, reason,
};
use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use hfst::prediction::evaluate::{Faces, Harness, Recording, RunSettings, Variant};
use hfst::prediction::model::ModelSettings;
use hfst::prediction::units::UnitGrammar;
use hfst::transducer::{Transducer, TransducerTablesInterface};

/// hfst-predict-eval's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Measure the keystrokes unit prediction saves on analysed text",
    after_help = "Every combination of the listed orders, backoffs, ambiguity rules, \
                  numbers shown, faces, complete words, forms, units and unit weights \
                  is scored as a variant, side by side with each recording. A variant \
                  with neither forms nor units is left out; unit weights vary only \
                  where forms and units are both on, and complete words only where \
                  units are."
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    #[command(flatten)]
    sources: Sources,

    /// The number of parts the text is split into; each part is scored with a
    /// model built from the others
    #[arg(long = "folds", value_name = "K", default_value_t = 10)]
    folds: usize,

    /// The model orders to score, separated by commas
    #[arg(
        long = "order",
        value_name = "N,...",
        value_delimiter = ',',
        default_value = "3"
    )]
    orders: Vec<usize>,

    /// The backoffs to score, separated by commas
    #[arg(
        long = "backoff",
        value_enum,
        value_delimiter = ',',
        default_value = "stupid-backoff"
    )]
    backoffs: Vec<BackoffName>,

    /// The ways of counting ambiguous tokens to score, separated by commas
    #[arg(
        long = "ambiguity",
        value_enum,
        value_delimiter = ',',
        default_value = "first-reading"
    )]
    ambiguities: Vec<AmbiguityName>,

    /// The numbers of suggestions shown to score, separated by commas
    #[arg(
        short = 'n',
        long = "shown",
        value_name = "N,...",
        value_delimiter = ',',
        default_value = "3"
    )]
    shown: Vec<usize>,

    /// Keyboard faces to score, separated by commas: KEYS+LONG is a face
    /// whose first KEYS units take one tap and next LONG units a long press.
    /// Without faces only suggestions are shown.
    #[arg(
        long = "faces",
        value_name = "KEYS+LONG,...",
        value_delimiter = ',',
        value_parser = parse_faces
    )]
    faces: Vec<Faces>,

    /// How many endings complete each stem on offer as a complete word, to
    /// score, separated by commas; 0 offers none
    #[arg(
        long = "whole-words",
        value_name = "K,...",
        value_delimiter = ',',
        default_value = "0"
    )]
    whole_words: Vec<usize>,

    /// Whether the word forms the model counted are offered, to score,
    /// separated by commas
    #[arg(
        long = "forms",
        value_enum,
        value_delimiter = ',',
        default_value = "off"
    )]
    forms: Vec<Switch>,

    /// Whether units and complete words built from them are offered, to
    /// score, separated by commas; off leaves an ordinary keyboard that
    /// completes counted forms
    #[arg(
        long = "units",
        value_enum,
        value_delimiter = ',',
        default_value = "on"
    )]
    units: Vec<Switch>,

    /// What the scores of units and complete words are multiplied by beside
    /// counted forms, from 0 to 1, to score, separated by commas
    #[arg(
        long = "unit-weight",
        value_name = "W,...",
        value_delimiter = ',',
        value_parser = parse_unit_weight,
        default_value = "1"
    )]
    unit_weights: Vec<f64>,

    /// Suggestions recorded from another system, scored beside the variants:
    /// NAME=FILE, the file holding a word, a prefix and the suggestions
    /// shown for it on each tab-separated line. May be given more than once.
    #[arg(long = "recording", value_name = "NAME=FILE", value_parser = parse_recording)]
    recordings: Vec<(String, String)>,

    /// How many threads score at once; as many as the machine runs when not
    /// given
    #[arg(long = "threads", value_name = "N")]
    threads: Option<usize>,

    #[command(flatten)]
    limits: Limits,

    /// Files of analysed text as hfst-lookup writes it, read as one text;
    /// none or - reads the standard input
    #[arg(value_name = "INFILE", num_args = 0..)]
    inputs: Vec<String>,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, _opts: &mut CommonOptions) {}
}

fn parse_faces(text: &str) -> Result<Faces, String> {
    let (keys, long_press) = text
        .split_once('+')
        .ok_or_else(|| format!("{text:?} is not KEYS+LONG"))?;
    let count = |part: &str| {
        part.parse::<usize>()
            .map_err(|_| format!("{part:?} in {text:?} is not a number"))
    };
    Ok(Faces {
        keys: count(keys)?,
        long_press: count(long_press)?,
    })
}

fn parse_recording(text: &str) -> Result<(String, String), String> {
    match text.split_once('=') {
        Some((name, path)) if !name.is_empty() && !path.is_empty() => {
            Ok((name.to_owned(), path.to_owned()))
        }
        Some(_) | None => Err(format!("{text:?} is not NAME=FILE")),
    }
}

impl Args {
    /// Every combination of the listed settings.
    fn variants(&self) -> Vec<Variant> {
        let faces: Vec<Option<Faces>> = if self.faces.is_empty() {
            vec![None]
        } else {
            self.faces.iter().copied().map(Some).collect()
        };
        let mut variants = Vec::new();
        for &order in &self.orders {
            for &backoff in &self.backoffs {
                for &ambiguity in &self.ambiguities {
                    for &shown in &self.shown {
                        for &faces in &faces {
                            let variant = self.variant(order, backoff, ambiguity, shown, faces);
                            variants.extend(self.offers(&variant));
                        }
                    }
                }
            }
        }
        variants
    }

    /// `variant` with each listed way of offering forms, units, unit
    /// weights and complete words that makes a difference.
    fn offers(&self, variant: &Variant) -> Vec<Variant> {
        let mut offers = Vec::new();
        for &forms in &self.forms {
            for &units in &self.units {
                let (forms, units) = (forms.on(), units.on());
                let weights = if forms && units {
                    self.unit_weights.as_slice()
                } else {
                    &[1.0]
                };
                let whole_words = if units {
                    self.whole_words.as_slice()
                } else {
                    &[0]
                };
                if !forms && !units {
                    continue;
                }
                for &whole_words in whole_words {
                    for &unit_weight in weights {
                        let mut name = variant.name.clone();
                        if whole_words > 0 {
                            name = format!("{name} words {whole_words}");
                        }
                        if forms && units {
                            name = format!("{name} forms w{unit_weight}");
                        } else if forms {
                            name = format!("{name} forms only");
                        }
                        offers.push(Variant {
                            name,
                            whole_words,
                            forms,
                            units,
                            unit_weight,
                            ..variant.clone()
                        });
                    }
                }
            }
        }
        offers
    }

    /// A variant named by its settings. The ambiguity rule is named only
    /// when more than one is scored.
    fn variant(
        &self,
        order: usize,
        backoff: BackoffName,
        ambiguity: AmbiguityName,
        shown: usize,
        faces: Option<Faces>,
    ) -> Variant {
        let mut name = format!("o{order} {} n{shown}", backoff.name());
        if self.ambiguities.len() > 1 {
            name = format!("{name} {}", ambiguity.name());
        }
        if let Some(faces) = faces {
            name = format!("{name} faces {}+{}", faces.keys, faces.long_press);
        }
        let model = ModelSettings {
            order,
            backoff: backoff.backoff(),
            ambiguity: ambiguity.ambiguity(),
            face_size: faces.map_or(0, |faces| faces.keys + faces.long_press),
            size_limit: usize::MAX,
        };
        Variant {
            faces,
            ..Variant::units(&name, model, shown)
        }
    }

    fn run_settings(&self) -> RunSettings {
        let mut settings = RunSettings {
            folds: self.folds,
            limits: self.limits.walk(),
            ..RunSettings::default()
        };
        if let Some(threads) = self.threads {
            settings.threads = threads;
        }
        settings
    }
}

/// The inputs as one text, each ended by a blank line so no token runs from
/// one into the next.
fn read_text(inputs: &[String]) -> Result<String, String> {
    let mut text = String::new();
    for (name, mut reader) in open_inputs(inputs)? {
        reader
            .read_to_string(&mut text)
            .map_err(|e| format!("cannot read {name}: {e}"))?;
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push('\n');
    }
    Ok(text)
}

fn read_recordings(named: &[(String, String)]) -> Result<Vec<Recording>, String> {
    named
        .iter()
        .map(|(name, path)| {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read recording {path}: {e}"))?;
            Recording::parse(name, &text).map_err(|e| reason(&e))
        })
        .collect()
}

fn evaluate<T: TransducerTablesInterface + Sync>(
    args: &Args,
    generator: &Transducer<T>,
    grammar: &UnitGrammar,
    text: &str,
    recordings: &[Recording],
) -> Result<String, String> {
    let harness = Harness::new(generator, grammar, args.run_settings()).map_err(|e| reason(&e))?;
    let corpus = harness.corpus(text).map_err(|e| reason(&e))?;
    let variants = args.variants();
    if variants.is_empty() {
        return Err("no variant offers forms or units".to_owned());
    }
    let report = harness
        .run(&corpus, &variants, recordings)
        .map_err(|e| reason(&e))?;
    Ok(report.table())
}

fn run(common: &CommonOptions, args: &Args) -> Result<(), String> {
    let (generator, grammar) = args.sources.read()?;
    let text = read_text(&args.inputs)?;
    let recordings = read_recordings(&args.recordings)?;
    let table = match &generator {
        Generator::Weighted(generator) => evaluate(args, generator, &grammar, &text, &recordings)?,
        Generator::Unweighted(generator) => {
            evaluate(args, generator, &grammar, &text, &recordings)?
        }
    };
    let written = |e: std::io::Error| format!("cannot write {}: {e}", common.output_filename);
    let mut out = common.output_writer().map_err(written)?;
    out.write_all(table.as_bytes()).map_err(written)?;
    out.flush().map_err(written)
}

pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();
    let common = hfst_set_program_name(&argv0, "0.1", "HfstPredictEval");
    let (common, args) = cli::parse::<Args>(common, args)?;
    run(&common, &args).map_err(|message| fail(&common, &message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Args {
        let mut argv = vec!["hfst predict-eval", "-g", "g.hfstol", "-G", "g.toml"];
        argv.extend_from_slice(extra);
        <Args as clap::Parser>::parse_from(argv)
    }

    fn names(args: &Args) -> Vec<String> {
        args.variants()
            .into_iter()
            .map(|variant| variant.name)
            .collect()
    }

    #[test]
    fn every_combination_of_settings_is_a_variant() {
        let args = args(&["--order", "2,3", "-n", "3,4", "--faces", "8+4"]);
        assert_eq!(
            names(&args),
            [
                "o2 stupid-backoff n3 faces 8+4",
                "o2 stupid-backoff n4 faces 8+4",
                "o3 stupid-backoff n3 faces 8+4",
                "o3 stupid-backoff n4 faces 8+4",
            ]
        );
        let variant = &args.variants()[3];
        assert_eq!(variant.model.order, 3);
        assert_eq!(variant.model.face_size, 12);
        assert_eq!(variant.shown, 4);
    }

    #[test]
    fn the_ambiguity_rule_is_named_when_it_varies() {
        let args = args(&[
            "--backoff",
            "drop-oldest",
            "--ambiguity",
            "first-reading,equal-shares",
        ]);
        assert_eq!(
            names(&args),
            [
                "o3 drop-oldest n3 first-reading",
                "o3 drop-oldest n3 equal-shares"
            ]
        );
        assert_eq!(args.variants()[0].faces, None);
        assert_eq!(args.variants()[0].model.face_size, 0);
    }

    // [spec:hfst:sem:prediction.whole-words/test]
    #[test]
    fn complete_words_are_a_setting_of_each_variant() {
        let args = args(&["-n", "3", "--whole-words", "0,2"]);
        assert_eq!(
            names(&args),
            ["o3 stupid-backoff n3", "o3 stupid-backoff n3 words 2"]
        );
        let counts: Vec<usize> = args
            .variants()
            .iter()
            .map(|variant| variant.whole_words)
            .collect();
        assert_eq!(counts, [0, 2]);
    }

    // [spec:hfst:sem:prediction.seen-forms/test]
    #[test]
    fn forms_units_and_weights_vary_by_variant() {
        let listed = args(&[
            "--forms",
            "off,on",
            "--units",
            "on,off",
            "--unit-weight",
            "1,0.5",
            "--whole-words",
            "0,1",
        ]);
        assert_eq!(
            names(&listed),
            [
                "o3 stupid-backoff n3",
                "o3 stupid-backoff n3 words 1",
                "o3 stupid-backoff n3 forms w1",
                "o3 stupid-backoff n3 forms w0.5",
                "o3 stupid-backoff n3 words 1 forms w1",
                "o3 stupid-backoff n3 words 1 forms w0.5",
                "o3 stupid-backoff n3 forms only",
            ]
        );
        let variants = listed.variants();
        let settings: Vec<(bool, bool, f64, usize)> = variants
            .iter()
            .map(|v| (v.forms, v.units, v.unit_weight, v.whole_words))
            .collect();
        assert_eq!(settings[3], (true, true, 0.5, 0));
        assert_eq!(settings[6], (true, false, 1.0, 0));
        assert!(variants[..2].iter().all(|v| !v.forms && v.units));
        assert!(args(&["--units", "off"]).variants().is_empty());
        assert!(
            <Args as clap::Parser>::try_parse_from([
                "hfst predict-eval",
                "-g",
                "g",
                "-G",
                "g",
                "--unit-weight",
                "1.5"
            ])
            .is_err()
        );
    }

    #[test]
    fn faces_and_recordings_are_parsed_or_refused() {
        assert_eq!(
            parse_faces("6+2"),
            Ok(Faces {
                keys: 6,
                long_press: 2
            })
        );
        assert!(parse_faces("6").is_err());
        assert!(parse_faces("six+2").is_err());
        assert_eq!(
            parse_recording("crk=out/recorded.tsv"),
            Ok(("crk".to_owned(), "out/recorded.tsv".to_owned()))
        );
        assert!(parse_recording("crk").is_err());
        assert!(parse_recording("=file").is_err());
    }

    #[test]
    fn threads_and_folds_come_from_the_command_line() {
        let settings = args(&["--folds", "4", "--threads", "2"]).run_settings();
        assert_eq!(settings.folds, 4);
        assert_eq!(settings.threads, 2);
        assert_eq!(
            args(&[]).run_settings().threads,
            RunSettings::default().threads
        );
    }
}
