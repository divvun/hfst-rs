//! The walk on lang-kal's generator, against the prediction Oqaasileriffik
//! built into hfst-optimized-lookup on hfst's predict branch. When the
//! analysis typed so far runs out, that lookup reads each of 462 morpheme
//! names, then each of five Der tags, and offers every pair it can read: one
//! morpheme ahead. It never asks the generator about endings.
//!
//! The checks on lang-kal need its built transducers, which the repository
//! does not hold, so they are ignored. They take minutes in a release
//! build, longer than the nextest profile lets a test run, so run them with
//! `cargo test --release -p hfst --test prediction_kal -- --ignored --nocapture`
//! and these set to lang-kal's files:
//! HFST_KAL_GENERATOR to src/fst/generator-gt-norm.hfstol,
//! HFST_KAL_ANALYSER to src/fst/analyser-gt-norm.hfstol and
//! HFST_KAL_CORPUS to tools/analysers/test/corpus.txt.

mod prediction_kal_tino;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};

use hfst::prediction::model::generator_accepts;
use hfst::prediction::units::{
    AnalysisTokenizer, SymbolClass, Unit, UnitGrammar, UnitKind, classify_symbol,
};
use hfst::prediction::walk::{Acceptor, Candidate, Next, WalkLimits, Walker, Word};
use hfst::transducer::{OlMember, SymbolNumber, Transducer, TransducerTablesInterface};
use prediction_kal_tino::{
    Pair, TINO_DERS, TINO_MORPHEMES, Tino, after_stem, input_symbols, pairs, small,
};

const KAL: &[u8] = include_bytes!("fixtures/prediction/kal.toml");

/// How many distinct words of the corpus are analysed.
const WORDS: usize = 2000;

/// The most words in progress the sample keeps.
const MAX_PREFIXES: usize = 1000;

/// How many times the default configuration bound a walk gets when it
/// looks again at a word whose walk the bound cut.
const ROOM: usize = 20;

/// The lang-kal files the checks read, when the environment names them.
struct KalFiles {
    generator: String,
    analyser: String,
    corpus: String,
}

impl KalFiles {
    fn from_env() -> Option<KalFiles> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        match (
            var("HFST_KAL_GENERATOR"),
            var("HFST_KAL_ANALYSER"),
            var("HFST_KAL_CORPUS"),
        ) {
            (Some(generator), Some(analyser), Some(corpus)) => Some(KalFiles {
                generator,
                analyser,
                corpus,
            }),
            _ => {
                eprintln!(
                    "skipped: set HFST_KAL_GENERATOR, HFST_KAL_ANALYSER and HFST_KAL_CORPUS \
                     to lang-kal's src/fst/generator-gt-norm.hfstol, \
                     src/fst/analyser-gt-norm.hfstol and tools/analysers/test/corpus.txt"
                );
                None
            }
        }
    }
}

/// An optimized-lookup file, with or without the HFST header lang-kal's
/// build writes before it.
fn read_ol(path: &str) -> OlMember {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let payload = match bytes.strip_prefix(b"HFST\0") {
        Some(rest) => &rest[3 + usize::from(u16::from_le_bytes([rest[0], rest[1]]))..],
        None => &bytes[..],
    };
    OlMember::read_from(&mut std::io::Cursor::new(payload), None)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The analyses of `word`, flags and HFST's own symbols left out.
fn readings(analyser: &OlMember, word: &str) -> BTreeSet<String> {
    let paths = match analyser {
        OlMember::Weighted(t) => t.lookup_fd_cstr(word, -1, 0.0),
        OlMember::Unweighted(t) => t.lookup_fd_cstr(word, -1, 0.0),
    };
    paths
        .into_iter()
        .map(|path| {
            path.second
                .iter()
                .filter(|s| matches!(classify_symbol(s), SymbolClass::Letter | SymbolClass::Tag))
                .map(|s| s.as_str())
                .collect()
        })
        .collect()
}

/// One unit of a word in progress, as the walk is asked for it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Step {
    kind: UnitKind,
    key: String,
}

/// Words in progress, each by its units' keys, with the analysis prefixes
/// that spell it.
type Prefixes = BTreeMap<Vec<Step>, BTreeSet<Vec<String>>>;

/// The words in progress of one reading: after its stem and after each
/// boundary unit.
fn add_prefixes(prefixes: &mut Prefixes, grammar: &UnitGrammar, units: &[Unit]) {
    for end in 0..units.len() {
        if !matches!(units[end].kind, UnitKind::Stem | UnitKind::Boundary) {
            continue;
        }
        let steps = units[..=end]
            .iter()
            .map(|unit| Step {
                kind: unit.kind,
                key: grammar.key(unit).as_str().to_owned(),
            })
            .collect();
        let spelled = units[..=end]
            .iter()
            .flat_map(|unit| unit.symbols.iter().map(|s| s.to_string()))
            .collect();
        prefixes.entry(steps).or_default().insert(spelled);
    }
}

/// Words in progress drawn from analysed text.
#[derive(Default)]
struct Sample {
    prefixes: Prefixes,
    words: usize,
    readings: usize,
    accepted: usize,
    segmented: usize,
    /// The words in progress of all the words analysed.
    drawn: usize,
    /// Those of every this many words are kept.
    stride: usize,
}

impl Sample {
    /// The words in progress of the first `WORDS` distinct words of the
    /// corpus, from the readings the generator accepts and the grammar cuts
    /// into units. When they are more than `MAX_PREFIXES`, those of every
    /// n-th word are kept, n the least that keeps no more, so the sample
    /// spreads over the text.
    fn draw<T: TransducerTablesInterface>(
        files: &KalFiles,
        generator: &Transducer<T>,
        grammar: &UnitGrammar,
    ) -> Sample {
        let analyser = read_ol(&files.analyser);
        let corpus = std::fs::read_to_string(&files.corpus)
            .unwrap_or_else(|e| panic!("{}: {e}", files.corpus));
        let accepts = generator_accepts(generator);
        let tokenizer = AnalysisTokenizer::new(generator.get_symbol_table());
        let mut seen = HashSet::new();
        let mut sample = Sample::default();
        let mut by_word: Vec<Prefixes> = Vec::new();
        let words = corpus
            .split_whitespace()
            .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|word| !word.is_empty());
        for word in words {
            if by_word.len() == WORDS {
                break;
            }
            if !seen.insert(word) {
                continue;
            }
            let mut own = Prefixes::new();
            for reading in readings(&analyser, word) {
                sample.readings += 1;
                if !accepts(&reading) {
                    continue;
                }
                sample.accepted += 1;
                let Ok(symbols) = tokenizer.tokenize(&reading) else {
                    continue;
                };
                let Ok(units) = grammar.segment(&symbols) else {
                    continue;
                };
                sample.segmented += 1;
                add_prefixes(&mut own, grammar, &units);
            }
            by_word.push(own);
        }
        sample.words = by_word.len();
        let kept = |stride: usize| {
            by_word
                .iter()
                .step_by(stride)
                .flat_map(BTreeMap::keys)
                .collect::<HashSet<_>>()
                .len()
        };
        let most = by_word.len().max(1);
        sample.drawn = kept(1);
        sample.stride = (1..most)
            .find(|&stride| kept(stride) <= MAX_PREFIXES)
            .unwrap_or(most);
        for own in by_word.into_iter().step_by(sample.stride) {
            for (steps, spellings) in own {
                sample.prefixes.entry(steps).or_default().extend(spellings);
            }
        }
        sample
    }

    fn describe(&self) -> String {
        format!(
            "{} words, {} readings, {} the generator accepts, {} segmented, \
             {} words in progress, of which those of every {} words keep {}",
            self.words,
            self.readings,
            self.accepted,
            self.segmented,
            self.drawn,
            self.stride,
            self.prefixes.len()
        )
    }
}

/// A word in progress the walk reached, and what it offered there.
struct Visit<'v> {
    steps: &'v [Step],
    spellings: &'v BTreeSet<Vec<String>>,
    word: &'v Word,
    next: &'v Next,
}

/// A word on the way down the sample, with what the walk offers after it
/// once that is asked.
struct Frame {
    steps: Vec<Step>,
    word: Word,
    next: Option<Next>,
}

/// How the walk went over a sample.
#[derive(Default)]
struct Walked {
    next_calls: Vec<Duration>,
    entry_calls: Vec<Duration>,
    /// Words in progress the walk did not reach, with the unit it did not
    /// offer and whether a bound cut the walk that did not offer it.
    unreached: Vec<(Vec<Step>, Step, bool)>,
    /// Words in progress whose walk a bound cut short.
    truncated: Vec<Vec<Step>>,
}

impl Walked {
    fn ensure_next<T: TransducerTablesInterface>(
        &mut self,
        walker: &Walker<'_, T>,
        frame: &mut Frame,
    ) {
        if frame.next.is_none() {
            let started = Instant::now();
            frame.next = Some(walker.next(&frame.word));
            self.next_calls.push(started.elapsed());
        }
    }

    /// The word after `frame` with `step` picked, as the walk offers it,
    /// or, when it is not offered, whether a bound cut the walk.
    fn extend<T: TransducerTablesInterface>(
        &mut self,
        walker: &Walker<'_, T>,
        frame: &mut Frame,
        step: &Step,
    ) -> Result<Word, bool> {
        let entered;
        let offered = if step.kind == UnitKind::Stem {
            let started = Instant::now();
            entered = walker
                .entry(&frame.word, &step.key, None)
                .map_err(|_| false)?;
            self.entry_calls.push(started.elapsed());
            &entered
        } else {
            self.ensure_next(walker, frame);
            frame.next.as_ref().expect("asked above")
        };
        pick_step(&frame.word, offered, step).ok_or(offered.truncated)
    }

    /// Walk every word in progress of `sample`, unit by unit as a user
    /// picks them, and show `visit` what the walk offers after each.
    fn sample<T: TransducerTablesInterface>(
        walker: &Walker<'_, T>,
        sample: &Sample,
        visit: &mut dyn FnMut(Visit<'_>),
    ) -> Walked {
        let mut walked = Walked::default();
        let mut stack = vec![Frame {
            steps: Vec::new(),
            word: walker.start(),
            next: None,
        }];
        'prefixes: for (steps, spellings) in &sample.prefixes {
            while !steps.starts_with(&stack.last().expect("the empty word stays").steps) {
                stack.pop();
            }
            loop {
                let frame = stack.last_mut().expect("the empty word stays");
                let Some(step) = steps.get(frame.steps.len()).cloned() else {
                    break;
                };
                let word = match walked.extend(walker, frame, &step) {
                    Ok(word) => word,
                    Err(truncated) => {
                        walked.unreached.push((steps.clone(), step, truncated));
                        continue 'prefixes;
                    }
                };
                let mut grown = frame.steps.clone();
                grown.push(step);
                stack.push(Frame {
                    steps: grown,
                    word,
                    next: None,
                });
            }
            let frame = stack.last_mut().expect("the empty word stays");
            walked.ensure_next(walker, frame);
            let next = frame.next.as_ref().expect("asked above");
            if next.truncated {
                walked.truncated.push(steps.clone());
            }
            visit(Visit {
                steps,
                spellings,
                word: &frame.word,
                next,
            });
        }
        walked
    }

    fn describe(&self) -> String {
        let mut described = format!(
            "walk: {} next calls ({}), {} entry calls ({}), {} truncated, {} unreached",
            self.next_calls.len(),
            timing(&self.next_calls),
            self.entry_calls.len(),
            timing(&self.entry_calls),
            self.truncated.len(),
            self.unreached.len()
        );
        for steps in self.truncated.iter().take(20) {
            described.push_str(&format!("\n  truncated after {}", spell(steps)));
        }
        for (steps, step, truncated) in self.unreached.iter().take(20) {
            described.push_str(&format!(
                "\n  unreached: {} at {}, where a bound cut the walk: {truncated}",
                spell(steps),
                step.key
            ));
        }
        described
    }

    /// The words in progress a bound does not explain the walk missing:
    /// those where the walk was not cut, and those a walk with `ROOM` times
    /// the configurations does not reach either.
    fn unexplained<T: TransducerTablesInterface>(&self, roomy: &Walker<'_, T>) -> Vec<String> {
        self.unreached
            .iter()
            .filter(|(steps, _, truncated)| !truncated || walk_to(roomy, steps).is_none())
            .map(|(steps, step, _)| format!("{} at {}", spell(steps), step.key))
            .collect()
    }
}

fn pick_step(word: &Word, offered: &Next, step: &Step) -> Option<Word> {
    let candidate = offered
        .candidates
        .iter()
        .find(|c| c.unit.kind == step.kind && c.key.as_str() == step.key)?;
    Some(word.pick(candidate))
}

/// The word in progress `steps` name, walked from the empty word.
fn walk_to<T: TransducerTablesInterface>(walker: &Walker<'_, T>, steps: &[Step]) -> Option<Word> {
    let mut word = walker.start();
    for step in steps {
        let offered = match step.kind {
            UnitKind::Stem => walker.entry(&word, &step.key, None).ok()?,
            UnitKind::Prefix
            | UnitKind::Boundary
            | UnitKind::Ending
            | UnitKind::Enclitic
            | UnitKind::Filter => walker.next(&word),
        };
        word = pick_step(&word, &offered, step)?;
    }
    Some(word)
}

/// The (morpheme name, Der tag) pairs of the boundary units on offer.
fn offered_pairs<T: TransducerTablesInterface>(tino: &Tino<'_, T>, next: &Next) -> BTreeSet<Pair> {
    next.candidates
        .iter()
        .filter(|c| c.unit.kind == UnitKind::Boundary)
        .filter_map(|c| tino.pair_of(&c.unit))
        .collect()
}

/// How many `items` there are, and the first of them, one to a line.
fn listed(what: &str, items: &[String]) -> String {
    let first: String = items
        .iter()
        .take(20)
        .map(|item| format!("\n  {item}"))
        .collect();
    format!("{} {what}{first}", items.len())
}

fn timing(calls: &[Duration]) -> String {
    let mut sorted = calls.to_vec();
    sorted.sort();
    let Some(&max) = sorted.last() else {
        return "none".to_owned();
    };
    let total: Duration = sorted.iter().sum();
    let count = u32::try_from(sorted.len()).expect("fewer than 2^32 calls");
    format!(
        "mean {:.1?}, median {:.1?}, max {:.1?}, total {:.1?}",
        total / count,
        sorted[sorted.len() / 2],
        max,
        total
    )
}

fn spell(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|step| step.key.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The default bounds, or with `ROOM` times the configurations.
fn kal_walker<'a, T: TransducerTablesInterface>(
    generator: &'a Transducer<T>,
    grammar: &'a UnitGrammar,
    roomy: bool,
) -> Walker<'a, T> {
    let limits = WalkLimits::default();
    let configurations = if roomy { ROOM } else { 1 } * limits.max_configurations;
    let limits = WalkLimits {
        max_configurations: configurations,
        ..limits
    };
    Walker::new(generator, grammar, limits).expect("the kal grammar fits lang-kal")
}

/// What the walk offers that the patched lookup does not, by kind, as
/// candidates summed over the words in progress and as distinct keys.
#[derive(Default)]
struct Beyond {
    counts: BTreeMap<&'static str, usize>,
    keys: BTreeMap<&'static str, BTreeSet<String>>,
}

impl Beyond {
    fn add(&mut self, what: &'static str, candidate: &Candidate) {
        *self.counts.entry(what).or_default() += 1;
        self.keys
            .entry(what)
            .or_default()
            .insert(candidate.key.as_str().to_owned());
    }
}

/// The patched lookup's pairs against the walk's offers.
#[derive(Default)]
struct TinoTally {
    prefixes: usize,
    pairs: usize,
    offered: usize,
    /// Pairs read from an analysis prefix but not from the word's
    /// configurations.
    spelled_only: usize,
    /// The pairs the walk did not offer, by word in progress, with whether
    /// a bound cut the walk there.
    misses: BTreeMap<Vec<Step>, (bool, Vec<Pair>)>,
    matched: usize,
    beyond: Beyond,
}

impl TinoTally {
    fn add<T: TransducerTablesInterface>(
        &mut self,
        tino: &Tino<'_, T>,
        walker: &Walker<'_, T>,
        visit: Visit<'_>,
    ) {
        let from_word = tino.pairs(&tino.points(walker, visit.word));
        let from_spelling: BTreeSet<Pair> = visit
            .spellings
            .iter()
            .flat_map(|spelled| tino.pairs(&tino.spell(spelled)))
            .collect();
        self.spelled_only += from_spelling.difference(&from_word).count();
        let found: BTreeSet<Pair> = from_word.union(&from_spelling).cloned().collect();
        let offered = offered_pairs(tino, visit.next);
        self.prefixes += 1;
        self.pairs += found.len();
        self.offered += found.intersection(&offered).count();
        let missed: Vec<Pair> = found.difference(&offered).cloned().collect();
        if !missed.is_empty() {
            let truncated = visit.next.truncated;
            self.misses
                .insert(visit.steps.to_vec(), (truncated, missed));
        }
        for candidate in &visit.next.candidates {
            self.sort(tino, walker.grammar(), &found, candidate);
        }
    }

    /// Put one offer of the walk in its place against the pairs found.
    fn sort<T: TransducerTablesInterface>(
        &mut self,
        tino: &Tino<'_, T>,
        grammar: &UnitGrammar,
        found: &BTreeSet<Pair>,
        candidate: &Candidate,
    ) {
        let unit = &candidate.unit;
        let what = match unit.kind {
            UnitKind::Boundary => match tino.pair_of(unit) {
                _ if !unit.symbols.last().is_some_and(|der| tino.is_der(der)) => {
                    "boundary: ending in a Der tag the lookup does not try"
                }
                Some(pair) if candidate.key.as_str() == format!("{}{}", pair.0, pair.1) => {
                    if found.contains(&pair) {
                        self.matched += 1;
                        return;
                    }
                    "boundary: a listed name and a Der tag, a hidden tag in the way on every path"
                }
                Some(pair) if found.contains(&pair) => {
                    "boundary: a pair the lookup finds, with other tags in the unit"
                }
                Some(_) => "boundary: a listed name and a Der tag, with other tags in the unit",
                None if unit.symbols.iter().any(|s| grammar.is_derivation(s)) => {
                    "boundary: a morpheme name the list lacks"
                }
                None => "boundary: a Der tag with no morpheme name",
            },
            UnitKind::Ending => "ending",
            UnitKind::Enclitic => "enclitic",
            UnitKind::Stem | UnitKind::Prefix | UnitKind::Filter => "other",
        };
        self.beyond.add(what, candidate);
    }

    fn report(&self) -> String {
        let mut report = format!(
            "{} words in progress; the patched lookup finds {} pairs, the walk offers {} of them; \
             {} pairs only an analysis prefix reads; {} walk offers are such pairs\n\
             the walk offers beyond them (offers summed over words in progress, distinct keys):",
            self.prefixes, self.pairs, self.offered, self.spelled_only, self.matched
        );
        for (what, count) in &self.beyond.counts {
            let keys = self.beyond.keys.get(what).cloned().unwrap_or_default();
            let some: Vec<String> = keys.iter().take(8).cloned().collect();
            report.push_str(&format!(
                "\n  {what}: {count} ({} keys, such as {})",
                keys.len(),
                some.join(" ")
            ));
        }
        for (steps, (truncated, missed)) in &self.misses {
            report.push_str(&format!(
                "\n  {} pairs missed after {}, where a bound cut the walk: {truncated}",
                missed.len(),
                spell(steps)
            ));
        }
        report
    }

    /// The misses a bound does not explain: those where the walk was not
    /// cut, and those a walk with `ROOM` times the configurations does not
    /// offer either.
    fn unexplained<T: TransducerTablesInterface>(
        &self,
        tino: &Tino<'_, T>,
        roomy: &Walker<'_, T>,
    ) -> Vec<String> {
        let mut unexplained = Vec::new();
        for (steps, (truncated, missed)) in &self.misses {
            let offered = truncated
                .then(|| walk_to(roomy, steps))
                .flatten()
                .map(|word| roomy.next(&word))
                .filter(|next| !next.truncated)
                .map(|next| offered_pairs(tino, &next))
                .unwrap_or_default();
            for pair in missed.iter().filter(|pair| !offered.contains(*pair)) {
                unexplained.push(format!("after {}: {}{}", spell(steps), pair.0, pair.1));
            }
        }
        unexplained
    }
}

/// Every ending the walk offers, picked and checked against the generator.
#[derive(Default)]
struct EndingTally {
    checked: usize,
    generated: usize,
    accepted_after_walk: usize,
    accepted_after_reading: usize,
    accepted: usize,
    failures: Vec<String>,
}

impl EndingTally {
    fn add<T: TransducerTablesInterface>(
        &mut self,
        walker: &Walker<'_, T>,
        acceptor: &Acceptor<'_, T>,
        numbers: &HashMap<String, SymbolNumber>,
        visit: Visit<'_>,
    ) {
        let accepts = |prefix: Vec<String>, ending: &Unit| {
            let symbols: Option<Vec<SymbolNumber>> = prefix
                .iter()
                .map(String::as_str)
                .chain(ending.symbols.iter().map(|s| s.as_str()))
                .map(|name| numbers.get(name).copied())
                .collect();
            symbols.is_some_and(|symbols| acceptor.accepts(&symbols))
        };
        let walked: Vec<String> = visit
            .word
            .units()
            .iter()
            .flat_map(|unit| unit.symbols.iter().map(|s| s.to_string()))
            .collect();
        for candidate in &visit.next.candidates {
            if candidate.unit.kind != UnitKind::Ending {
                continue;
            }
            self.checked += 1;
            let picked = visit.word.pick(candidate);
            let generated = picked.is_complete() && !walker.forms(&picked).is_empty();
            let after_walk = accepts(walked.clone(), &candidate.unit);
            let after_reading = visit
                .spellings
                .iter()
                .any(|spelled| accepts(spelled.clone(), &candidate.unit));
            self.generated += usize::from(generated);
            self.accepted_after_walk += usize::from(after_walk);
            self.accepted_after_reading += usize::from(after_reading);
            self.accepted += usize::from(after_walk || after_reading);
            if !generated || !(after_walk || after_reading) {
                self.failures.push(format!(
                    "{} {}: generated {generated}, accepted {}",
                    spell(visit.steps),
                    candidate.key.as_str(),
                    after_walk || after_reading
                ));
            }
        }
    }

    fn report(&self) -> String {
        let mut report = format!(
            "{} endings offered; {} make a complete word with a form; the generator accepts \
             {} after the walk's units, {} after a reading's own prefix, {} after either",
            self.checked,
            self.generated,
            self.accepted_after_walk,
            self.accepted_after_reading,
            self.accepted
        );
        for failure in self.failures.iter().take(20) {
            report.push_str(&format!("\n  failed: {failure}"));
        }
        report
    }
}

fn check_tino<T: TransducerTablesInterface>(files: &KalFiles, generator: &Transducer<T>) {
    let started = Instant::now();
    let grammar = UnitGrammar::from_toml(KAL).expect("the kal grammar reads");
    let walker = kal_walker(generator, &grammar, false);
    let sample = Sample::draw(files, generator, &grammar);
    let tino = Tino::new(generator);
    let mut tally = TinoTally::default();
    let walked = Walked::sample(&walker, &sample, &mut |visit| {
        tally.add(&tino, &walker, visit)
    });
    let roomy = kal_walker(generator, &grammar, true);
    let unexplained = tally.unexplained(&tino, &roomy);
    let unreached = walked.unexplained(&roomy);
    eprintln!(
        "{} of the list's {} morpheme names and {} of its {} Der tags are in the generator\n\
         {}\n{}\n{}\n{}\n{}\nin {:.1?}",
        tino.morphemes.len(),
        TINO_MORPHEMES.split_whitespace().count(),
        tino.ders.len(),
        TINO_DERS.len(),
        sample.describe(),
        walked.describe(),
        tally.report(),
        listed("misses a bound does not explain", &unexplained),
        listed(
            "units of readings a bound does not explain missing",
            &unreached
        ),
        started.elapsed()
    );
    assert!(tally.pairs > 0, "the sample finds pairs");
    assert!(unexplained.is_empty(), "the walk misses pairs");
    assert!(unreached.is_empty(), "the walk misses units of readings");
}

fn check_endings<T: TransducerTablesInterface>(files: &KalFiles, generator: &Transducer<T>) {
    let started = Instant::now();
    let grammar = UnitGrammar::from_toml(KAL).expect("the kal grammar reads");
    let walker = kal_walker(generator, &grammar, false);
    let sample = Sample::draw(files, generator, &grammar);
    let acceptor = Acceptor::new(generator);
    let numbers = input_symbols(generator);
    let mut tally = EndingTally::default();
    let walked = Walked::sample(&walker, &sample, &mut |visit| {
        tally.add(&walker, &acceptor, &numbers, visit)
    });
    let unreached = walked.unexplained(&kal_walker(generator, &grammar, true));
    eprintln!(
        "{}\n{}\n{}\n{}\nin {:.1?}",
        sample.describe(),
        walked.describe(),
        tally.report(),
        listed(
            "units of readings a bound does not explain missing",
            &unreached
        ),
        started.elapsed()
    );
    assert!(tally.checked > 0, "the sample offers endings");
    assert!(unreached.is_empty(), "the walk misses units of readings");
    assert!(tally.failures.is_empty(), "endings that do not generate");
}

#[test]
#[ignore = "needs lang-kal's generator, analyser and corpus; see the module docs"]
fn walk_offers_every_pair_the_lookup_finds() {
    let Some(files) = KalFiles::from_env() else {
        return;
    };
    match read_ol(&files.generator) {
        OlMember::Weighted(generator) => check_tino(&files, &generator),
        OlMember::Unweighted(generator) => check_tino(&files, &generator),
    }
}

#[test]
#[ignore = "needs lang-kal's generator, analyser and corpus; see the module docs"]
fn every_ending_offered_on_lang_kal_generates() {
    let Some(files) = KalFiles::from_env() else {
        return;
    };
    match read_ol(&files.generator) {
        OlMember::Weighted(generator) => check_endings(&files, &generator),
        OlMember::Unweighted(generator) => check_endings(&files, &generator),
    }
}

#[test]
fn the_tally_sorts_offers_beyond_the_lookup() {
    let (generator, grammar) = small();
    let walker = Walker::new(&generator, &grammar, WalkLimits::default()).expect("it fits");
    let tino = Tino::new(&generator);
    let word = after_stem(&walker);
    let next = walker.next(&word);
    assert_eq!(
        offered_pairs(&tino, &next),
        pairs(&[
            ("+KIP", "+Der/nv"),
            ("+LI", "+Der/vv"),
            ("+NIQ", "+Der/vn"),
            ("+TAQ", "+Der/vn"),
        ])
    );
    let spelled: Vec<String> = "oqar".chars().map(String::from).collect();
    let mut tally = TinoTally::default();
    tally.add(
        &tino,
        &walker,
        Visit {
            steps: &[],
            spellings: &BTreeSet::from([spelled]),
            word: &word,
            next: &next,
        },
    );
    assert_eq!((tally.pairs, tally.offered, tally.matched), (2, 2, 2));
    assert!(tally.misses.is_empty());
    let hidden = "boundary: a listed name and a Der tag, a hidden tag in the way on every path";
    assert_eq!(tally.beyond.counts.get(hidden), Some(&2));
    assert_eq!(tally.beyond.counts.get("ending"), Some(&1));
}
