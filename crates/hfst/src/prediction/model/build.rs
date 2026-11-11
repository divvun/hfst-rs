use std::collections::{BTreeMap, HashMap};

use super::input::{Token, tokens};
use super::{Ambiguity, ModelSettings};
use crate::error::Result;
use crate::prediction::units::{AnalysisTokenizer, UnitGrammar, UnitKey};

/// What one call of [`ModelBuilder::add_text`] read and counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildReport {
    pub tokens: usize,
    /// Every reading read, the `+?` of a token with no analysis left out.
    /// With [`Ambiguity::FirstReading`] the readings after the first one
    /// counted are passed over unchecked: they are the readings neither
    /// counted nor refused.
    pub readings: usize,
    /// Readings whose units were counted. With
    /// [`Ambiguity::FirstReading`] this is at most one for each token.
    pub counted: usize,
    /// Readings the generator does not accept, left out.
    pub refused_by_generator: usize,
    /// Readings the generator accepts that could not be cut into units,
    /// left out.
    pub refused_by_grammar: usize,
    /// Tokens with no reading left to count. Such a token breaks the text:
    /// no counted run spans it.
    pub tokens_without_reading: usize,
}

impl std::ops::AddAssign for BuildReport {
    fn add_assign(&mut self, other: BuildReport) {
        self.tokens += other.tokens;
        self.readings += other.readings;
        self.counted += other.counted;
        self.refused_by_generator += other.refused_by_generator;
        self.refused_by_grammar += other.refused_by_grammar;
        self.tokens_without_reading += other.tokens_without_reading;
    }
}

/// Counts units and runs of units in analysed text, then writes a model
/// file with [`ModelBuilder::write`].
pub struct ModelBuilder<'g> {
    pub(super) grammar: &'g UnitGrammar,
    tokenizer: &'g AnalysisTokenizer,
    accepts: Box<dyn Fn(&str) -> bool + 'g>,
    pub(super) settings: ModelSettings,
    pub(super) counts: Counts,
}

/// Everything counted so far. Units are numbered in the order they were
/// first seen.
#[derive(Default)]
pub(super) struct Counts {
    pub keys: Vec<UnitKey>,
    ids: HashMap<UnitKey, u32>,
    pub units: Vec<f64>,
    /// For each unit, how much of its count fell under each class, by the
    /// class's number in `class_names`. A key can have more than one class
    /// when its units differ in hidden tags.
    pub unit_classes: Vec<Vec<(u32, f64)>>,
    pub class_names: Vec<String>,
    /// The runs of each length from 2 to the order, at index length - 2.
    pub runs: Vec<HashMap<Vec<u32>, f64>>,
}

/// One reading the model can count: its units' keys and classes.
type Reading<'g> = Vec<(UnitKey, Option<&'g str>)>;

impl<'g> ModelBuilder<'g> {
    /// `accepts` says whether the generator generates an analysis;
    /// [`super::generator_accepts`] makes one from an optimized-lookup
    /// generator. The tokenizer must be built from the same generator's
    /// analysis alphabet. The order must be at least 1.
    pub fn new(
        grammar: &'g UnitGrammar,
        tokenizer: &'g AnalysisTokenizer,
        accepts: Box<dyn Fn(&str) -> bool + 'g>,
        settings: ModelSettings,
    ) -> Result<Self> {
        if settings.order == 0 {
            crate::bail!(Hfst, "prediction model: the order must be at least 1");
        }
        let counts = Counts {
            runs: vec![HashMap::new(); settings.order - 1],
            ..Counts::default()
        };
        Ok(ModelBuilder {
            grammar,
            tokenizer,
            accepts,
            settings,
            counts,
        })
    }

    /// Count the readings of hfst-lookup output. Each call is a text of its
    /// own: no run spans two calls.
    // [spec:hfst:req:prediction-model.input]
    pub fn add_text(&mut self, text: &str) -> Result<BuildReport> {
        let mut report = BuildReport::default();
        let mut histories = start_of_text();
        for token in tokens(text)? {
            report.tokens += 1;
            report.readings += token.analyses.len();
            let readings = self.readings(&token, &mut report);
            if readings.is_empty() {
                report.tokens_without_reading += 1;
                histories = start_of_text();
                continue;
            }
            report.counted += readings.len();
            histories = self.count_token(&histories, &readings);
        }
        Ok(report)
    }

    /// The token's readings to count, in their order: the first the model
    /// can count, or all of them, as the build's ambiguity rule says.
    fn readings(&self, token: &Token<'_>, report: &mut BuildReport) -> Vec<Reading<'g>> {
        let mut counted = Vec::new();
        for analysis in &token.analyses {
            if self.settings.ambiguity == Ambiguity::FirstReading && !counted.is_empty() {
                break;
            }
            if !(self.accepts)(analysis) {
                report.refused_by_generator += 1;
                continue;
            }
            let units = self
                .tokenizer
                .tokenize(analysis)
                .and_then(|symbols| self.grammar.segment(&symbols));
            let Ok(units) = units else {
                report.refused_by_grammar += 1;
                continue;
            };
            let grammar = self.grammar;
            counted.push(
                units
                    .iter()
                    .map(|unit| (grammar.key(unit), grammar.class_of(unit)))
                    .collect(),
            );
        }
        counted
    }

    /// Count the runs that end in this token's units. `histories` holds the
    /// last units before the token, at most order - 1 of them, each with the
    /// share of the text's readings that ends with them; the shares sum to
    /// one. The histories after the token are returned.
    // [spec:hfst:req:prediction-model.input]
    // [spec:hfst:sem:prediction-model.order]
    fn count_token(&mut self, histories: &Histories, readings: &[Reading<'g>]) -> Histories {
        let share = 1.0 / readings.len() as f64;
        let mut runs = Vec::with_capacity(readings.len());
        for reading in readings {
            let mut ids = Vec::with_capacity(reading.len());
            for (key, class) in reading {
                let id = self.counts.id(key);
                let class = class.map(|name| self.counts.class_id(name));
                self.counts.add_unit(id, class, share);
                ids.push(id);
            }
            runs.push(ids);
        }
        let keep = self.settings.order - 1;
        let mut after = Histories::new();
        for (history, &weight) in histories {
            for ids in &runs {
                let weight = weight * share;
                let mut window = history.clone();
                for &id in ids {
                    window.push(id);
                    for length in 2..=window.len().min(self.settings.order) {
                        let run = &window[window.len() - length..];
                        self.counts.add_run(run, weight);
                    }
                    if window.len() > keep {
                        window.drain(..window.len() - keep);
                    }
                }
                *after.entry(window).or_default() += weight;
            }
        }
        after
    }
}

/// The last units before a token, with the share of readings that end with
/// them. Ordered, so that counts are summed in the same order on every run
/// and a model is the same bytes each time it is built.
type Histories = BTreeMap<Vec<u32>, f64>;

fn start_of_text() -> Histories {
    BTreeMap::from([(Vec::new(), 1.0)])
}

impl Counts {
    fn id(&mut self, key: &UnitKey) -> u32 {
        if let Some(&id) = self.ids.get(key) {
            return id;
        }
        let id = u32::try_from(self.keys.len()).expect("fewer than 2^32 unit keys");
        self.keys.push(key.clone());
        self.ids.insert(key.clone(), id);
        self.units.push(0.0);
        self.unit_classes.push(Vec::new());
        id
    }

    fn class_id(&mut self, name: &str) -> u32 {
        let at = match self.class_names.iter().position(|known| known == name) {
            Some(at) => at,
            None => {
                self.class_names.push(name.to_owned());
                self.class_names.len() - 1
            }
        };
        u32::try_from(at).expect("a grammar has fewer than 2^32 classes")
    }

    fn add_unit(&mut self, id: u32, class: Option<u32>, share: f64) {
        let id = id as usize;
        self.units[id] += share;
        let Some(class) = class else {
            return;
        };
        let classes = &mut self.unit_classes[id];
        match classes.iter_mut().find(|(known, _)| *known == class) {
            Some((_, count)) => *count += share,
            None => classes.push((class, share)),
        }
    }

    fn add_run(&mut self, run: &[u32], weight: f64) {
        let table = &mut self.runs[run.len() - 2];
        match table.get_mut(run) {
            Some(count) => *count += weight,
            None => {
                table.insert(run.to_vec(), weight);
            }
        }
    }
}
