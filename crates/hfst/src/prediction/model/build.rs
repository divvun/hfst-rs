use std::collections::{BTreeMap, HashMap};

use super::input::{Token, tokens};
use super::{Ambiguity, ModelSettings, fold};
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
    /// no counted run spans it, and its form is not counted.
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
    /// The word forms, capitals folded, numbered in the order they were
    /// first seen.
    pub forms: Vec<String>,
    form_ids: HashMap<String, u32>,
    pub form_counts: Vec<f64>,
    /// For each form, the units of each reading it was counted with, by
    /// unit id, and how much of its count went to that reading.
    pub form_readings: Vec<Vec<(Vec<u32>, f64)>>,
    /// The runs of forms of each length from 2 to the order, at index
    /// length - 2.
    pub form_runs: Vec<HashMap<Vec<u32>, f64>>,
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
            form_runs: vec![HashMap::new(); settings.order - 1],
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

    /// Count the readings of hfst-lookup output, and the surface forms of
    /// the tokens counted. Each call is a text of its own: no run spans two
    /// calls.
    // [spec:hfst:req:prediction-model.input]
    // [spec:hfst:req:prediction-model.word-forms]
    pub fn add_text(&mut self, text: &str) -> Result<BuildReport> {
        let mut report = BuildReport::default();
        let mut histories = start_of_text();
        let mut forms_before = Vec::new();
        for token in tokens(text)? {
            report.tokens += 1;
            report.readings += token.analyses.len();
            let readings = self.readings(&token, &mut report);
            if readings.is_empty() {
                report.tokens_without_reading += 1;
                histories = start_of_text();
                forms_before.clear();
                continue;
            }
            report.counted += readings.len();
            let ids = self.count_units(&readings);
            histories = self.count_runs(&histories, &ids);
            self.count_form(&mut forms_before, token.surface, &ids);
        }
        Ok(report)
    }

    /// How many different word forms, capitals folded, have been counted.
    // [spec:hfst:req:prediction-model.word-forms]
    pub fn word_forms(&self) -> usize {
        self.counts.forms.len()
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

    /// Count each reading's units, at its share of the token, and give
    /// each reading's unit ids.
    // [spec:hfst:req:prediction-model.input]
    fn count_units(&mut self, readings: &[Reading<'g>]) -> Vec<Vec<u32>> {
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
        runs
    }

    /// Count the runs that end in this token's units, given each reading's
    /// unit ids. `histories` holds the last units before the token, at most
    /// order - 1 of them, each with the share of the text's readings that
    /// ends with them; the shares sum to one. The histories after the token
    /// are returned.
    // [spec:hfst:req:prediction-model.input]
    // [spec:hfst:sem:prediction-model.order]
    fn count_runs(&mut self, histories: &Histories, runs: &[Vec<u32>]) -> Histories {
        let share = 1.0 / runs.len() as f64;
        let keep = self.settings.order - 1;
        let mut after = Histories::new();
        for (history, &weight) in histories {
            for ids in runs {
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

    /// Count the token's surface form, capitals folded, once, with each
    /// reading counted for it at the reading's share, and the runs of forms
    /// that end with it. `before` holds the forms of the tokens before it
    /// since the last break, at most order - 1 of them, and is moved on.
    // [spec:hfst:req:prediction-model.word-forms]
    fn count_form(&mut self, before: &mut Vec<u32>, surface: &str, readings: &[Vec<u32>]) {
        let form = self.counts.form_id(&fold(surface));
        let share = 1.0 / readings.len() as f64;
        self.counts.form_counts[form as usize] += 1.0;
        let tally = &mut self.counts.form_readings[form as usize];
        for ids in readings {
            match tally.iter_mut().find(|(known, _)| known == ids) {
                Some((_, count)) => *count += share,
                None => tally.push((ids.clone(), share)),
            }
        }
        before.push(form);
        for length in 2..=before.len().min(self.settings.order) {
            let run = &before[before.len() - length..];
            let table = &mut self.counts.form_runs[length - 2];
            *table.entry(run.to_vec()).or_default() += 1.0;
        }
        let keep = self.settings.order - 1;
        if before.len() > keep {
            before.drain(..before.len() - keep);
        }
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

    fn form_id(&mut self, form: &str) -> u32 {
        if let Some(&id) = self.form_ids.get(form) {
            return id;
        }
        let id = u32::try_from(self.forms.len()).expect("fewer than 2^32 word forms");
        self.forms.push(form.to_owned());
        self.form_ids.insert(form.to_owned(), id);
        self.form_counts.push(0.0);
        self.form_readings.push(Vec::new());
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
