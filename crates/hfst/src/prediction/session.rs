//! A session: the word in progress, the letters typed after its last unit,
//! and the units of the words before it (`prediction.session`).

use super::entry::Relaxation;
use super::realise::{Script, realise};
use super::units::{AnalysisTokenizer, SymbolClass, Unit, UnitKey, UnitKind};
use super::walk::{CandidateOrder, Next, Walker, Word};
use crate::error::Result;
use crate::hfst_data_types::Symbol;
use crate::transducer::TransducerTablesInterface;

/// Builds words one unit at a time over one generator, unit grammar and
/// script.
// [spec:hfst:sem:prediction.session]
pub struct Session<'a, T: TransducerTablesInterface> {
    walker: Walker<'a, T>,
    script: Script<'a>,
    relaxation: Option<&'a dyn Relaxation>,
    context_len: usize,
    context: Vec<UnitKey>,
    /// The word after each unit picked, the empty word first, so backspace
    /// steps back without walking again.
    words: Vec<Word>,
    typed: String,
    offered: Option<Next>,
}

impl<'a, T: TransducerTablesInterface> Session<'a, T> {
    /// `context_len` is how many of the latest units the context keeps: as
    /// many as the model's order uses.
    pub fn new(walker: Walker<'a, T>, context_len: usize) -> Session<'a, T> {
        let empty = walker.start();
        Session {
            walker,
            script: Script::default(),
            relaxation: None,
            context_len,
            context: Vec::new(),
            words: vec![empty],
            typed: String::new(),
            offered: None,
        }
    }

    pub fn with_script(mut self, script: Script<'a>) -> Session<'a, T> {
        self.script = script;
        self.offered = None;
        self
    }

    pub fn with_relaxation(mut self, relaxation: &'a dyn Relaxation) -> Session<'a, T> {
        self.relaxation = Some(relaxation);
        self.offered = None;
        self
    }

    pub fn walker(&self) -> &Walker<'a, T> {
        &self.walker
    }

    pub fn word(&self) -> &Word {
        self.words
            .last()
            .expect("a session always holds at least the empty word")
    }

    /// The letters typed after the word's last unit, as typed.
    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// The keys of the latest units of earlier words, oldest first.
    pub fn context(&self) -> &[UnitKey] {
        &self.context
    }

    /// What a ranker ranks the next unit after: the context, then the keys
    /// of the word's own units.
    pub fn ranking_context(&self) -> Vec<UnitKey> {
        let grammar = self.walker.grammar();
        let mut keys = self.context.clone();
        keys.extend(self.word().units().iter().map(|unit| grammar.key(unit)));
        keys
    }

    /// The units on offer: those the generator accepts next, or, once
    /// letters are typed, those whose labels the letters match. Typed
    /// letters pass through the input transliterator first; letters it
    /// cannot convert match nothing.
    // [spec:hfst:req:prediction.script-edges]
    // [spec:hfst:sem:prediction.entry]
    pub fn candidates(&mut self) -> Result<&Next> {
        let offered = match self.offered.take() {
            Some(offered) => offered,
            None => self.offer()?,
        };
        Ok(self.offered.insert(offered))
    }

    fn offer(&self) -> Result<Next> {
        let word = self.word();
        if self.typed.is_empty() {
            return Ok(self.walker.next(word));
        }
        match self.script.typed(&self.typed) {
            Some(spelled) => self.walker.entry(word, &spelled, self.relaxation),
            None => Ok(Next {
                candidates: Vec::new(),
                forms: self.walker.forms(word),
                stem_next: true,
                truncated: false,
            }),
        }
    }

    /// Letters are refused where no stem can start.
    pub fn type_letters(&mut self, letters: &str) -> Result<()> {
        if !self.walker.stem_may_start(self.word()) {
            crate::bail!(
                FaultyStringInput,
                format!("letters {letters:?} are typed where no stem can start")
            );
        }
        self.typed.push_str(letters);
        self.offered = None;
        Ok(())
    }

    /// Append the unit offered with `key` to the word. The letters typed
    /// for it are spent.
    pub fn pick(&mut self, key: &UnitKey) -> Result<()> {
        let candidate = self
            .candidates()?
            .candidates
            .iter()
            .find(|candidate| &candidate.key == key)
            .cloned()
            .ok_or_else(|| {
                crate::err!(
                    FaultyStringInput,
                    format!("unit {:?} is not offered here", key.as_str())
                )
            })?;
        let word = self.word().pick(&candidate);
        self.words.push(word);
        self.typed.clear();
        self.offered = None;
        Ok(())
    }

    /// Remove the last letter typed, or the last unit picked when no letter
    /// is typed after it. False when there is nothing to remove.
    pub fn backspace(&mut self) -> bool {
        let removed =
            self.typed.pop().is_some() || (self.words.len() > 1 && self.words.pop().is_some());
        if removed {
            self.offered = None;
        }
        removed
    }

    /// Write the word, keep its units as context and start an empty word.
    /// Only a complete word with no letters typed after it commits.
    pub fn commit(&mut self) -> Result<String> {
        if !self.typed.is_empty() {
            crate::bail!(
                FaultyStringInput,
                format!(
                    "letters {:?} are typed but no unit is picked for them",
                    self.typed
                )
            );
        }
        let written = realise(&self.walker, self.word(), &self.script)?;
        let grammar = self.walker.grammar();
        let keys: Vec<UnitKey> = self
            .word()
            .units()
            .iter()
            .map(|unit| grammar.key(unit))
            .collect();
        self.context.extend(keys);
        let excess = self.context.len().saturating_sub(self.context_len);
        self.context.drain(..excess);
        self.words.truncate(1);
        self.offered = None;
        Ok(written)
    }

    /// The word's preview, ranked after [`Session::ranking_context`].
    // [spec:hfst:sem:prediction.preview]
    pub fn preview(&self, order: &dyn CandidateOrder) -> Option<String> {
        self.walker
            .preview(self.word(), &self.ranking_context(), order)
    }
}

impl<T: TransducerTablesInterface> Walker<'_, T> {
    /// The word in progress that an analysis, or the start of one, spells.
    /// The analysis is cut into units as the model builder cuts a reading,
    /// except that a start of only prefix tags is those prefixes. The stem
    /// is entered by its letters and every other unit is picked by its key
    /// from what the walk offers there. The tokenizer must be built from the
    /// generator's analysis alphabet. The error names the first unit the
    /// generator does not accept where it comes.
    pub fn word_from_analysis(
        &self,
        analysis: &str,
        tokenizer: &AnalysisTokenizer,
    ) -> Result<Word> {
        let grammar = self.grammar();
        let symbols = tokenizer.tokenize(analysis)?;
        let only_prefixes = symbols.iter().all(|symbol| {
            grammar.classify(symbol) == SymbolClass::Tag && grammar.is_prefix(symbol)
        });
        let units = if only_prefixes {
            symbols
                .into_iter()
                .map(|symbol| Unit {
                    kind: UnitKind::Prefix,
                    symbols: vec![symbol],
                })
                .collect()
        } else {
            grammar.segment(&symbols)?
        };
        let mut word = self.start();
        let mut spelled = String::new();
        for unit in &units {
            let next = match unit.kind {
                UnitKind::Stem => {
                    let letters: String = unit.symbols.iter().map(Symbol::as_str).collect();
                    self.entry(&word, &letters, None)?
                }
                UnitKind::Prefix
                | UnitKind::Boundary
                | UnitKind::Ending
                | UnitKind::Enclitic
                | UnitKind::Filter => self.next(&word),
            };
            let key = grammar.key(unit);
            let Some(candidate) = next.candidates.iter().find(|offered| offered.key == key) else {
                crate::bail!(FaultyStringInput, refusal(analysis, &key, &spelled, &next));
            };
            word = word.pick(candidate);
            spelled.extend(unit.symbols.iter().map(Symbol::as_str));
        }
        Ok(word)
    }
}

fn refusal(analysis: &str, key: &UnitKey, spelled: &str, next: &Next) -> String {
    let place = if spelled.is_empty() {
        "at the start of the word".to_owned()
    } else {
        format!("after {spelled:?}")
    };
    let cut = if next.truncated {
        "; a walk limit cut the search short"
    } else {
        ""
    };
    format!(
        "analysis {analysis:?}: the generator does not accept the unit {:?} {place}{cut}",
        key.as_str()
    )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::error::ErrorKind;
    use crate::prediction::units::UnitKind;
    use crate::prediction::walk::GeneratorWeightOrder;
    use crate::prediction::walk::fixtures_test::*;
    use crate::transducer::Weight;

    fn keys_of(session: &mut Session<'_, crate::transducer::WeightedTables>) -> Vec<String> {
        session
            .candidates()
            .unwrap()
            .candidates
            .iter()
            .map(|candidate| candidate.key.as_str().to_owned())
            .collect()
    }

    fn context<'s>(session: &'s Session<'_, crate::transducer::WeightedTables>) -> Vec<&'s str> {
        session.context().iter().map(UnitKey::as_str).collect()
    }

    // [spec:hfst:sem:prediction.session/test]
    #[test]
    fn a_session_builds_a_word_and_commits_it() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let mut session = Session::new(walker(&generator, &grammar), 2);
        session.type_letters("gu").unwrap();
        assert_eq!(keys_of(&mut session), ["guolli"]);
        session.pick(&UnitKey::from("guolli")).unwrap();
        assert_eq!(session.typed(), "");
        assert_eq!(keys_of(&mut session)[0], "+N+Cmp#");
        session.pick(&UnitKey::from("+N+Pl+Nom")).unwrap();
        assert_eq!(session.commit().unwrap(), "guolat");
        assert_eq!(context(&session), ["guolli", "+N+Pl+Nom"]);
        assert!(session.word().units().is_empty());
    }

    // [spec:hfst:sem:prediction.session/test]
    #[test]
    fn the_context_keeps_the_latest_units() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let mut session = Session::new(walker(&generator, &grammar), 3);
        for (stem, units) in [
            ("girji", ["+N+Sg+Nom", "+Qst"]),
            ("borra", ["+V+Der/NomAg", "+N+Sg+Gen"]),
        ] {
            session.type_letters(stem).unwrap();
            session.pick(&UnitKey::from(stem)).unwrap();
            for unit in units {
                session.pick(&UnitKey::from(unit)).unwrap();
            }
            session.commit().unwrap();
        }
        assert_eq!(context(&session), ["borra", "+V+Der/NomAg", "+N+Sg+Gen"]);
    }

    // [spec:hfst:sem:prediction.session/test]
    #[test]
    fn backspace_removes_a_letter_then_a_unit() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let mut session = Session::new(walker(&generator, &grammar), 2);
        assert!(!session.backspace());
        session.type_letters("gi").unwrap();
        session.pick(&UnitKey::from("girji")).unwrap();
        session.pick(&UnitKey::from("+N+Cmp#")).unwrap();
        session.type_letters("gu").unwrap();
        assert!(session.backspace());
        assert_eq!(session.typed(), "g");
        assert!(session.backspace());
        assert_eq!(session.typed(), "");
        assert_eq!(session.word().units().len(), 2);
        assert!(session.backspace());
        assert_eq!(session.word().units().len(), 1);
        assert_eq!(session.word().units()[0].kind, UnitKind::Stem);
        assert!(session.backspace());
        assert!(session.word().units().is_empty());
        assert!(!session.backspace());
    }

    // [spec:hfst:sem:prediction.session/test]
    #[test]
    fn only_a_complete_word_commits() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let mut session = Session::new(walker(&generator, &grammar), 2);
        session.type_letters("girji").unwrap();
        session.pick(&UnitKey::from("girji")).unwrap();
        let refused = session.commit().expect_err("a stem alone is not a word");
        assert_eq!(refused.kind, ErrorKind::StateIsNotFinal);
        session.pick(&UnitKey::from("+N+Cmp#")).unwrap();
        session.type_letters("g").unwrap();
        assert!(session.commit().is_err());
        assert_eq!(session.word().units().len(), 2);
        assert!(session.context().is_empty());
    }

    // [spec:hfst:sem:prediction.session/test]
    #[test]
    fn letters_and_units_must_be_on_offer() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let mut session = Session::new(walker(&generator, &grammar), 2);
        assert!(session.pick(&UnitKey::from("+N+Sg+Nom")).is_err());
        session.type_letters("girji").unwrap();
        session.pick(&UnitKey::from("girji")).unwrap();
        assert!(session.type_letters("x").is_err());
        assert_eq!(session.typed(), "");
    }

    /// Ranks by generator weight and keeps the context it was asked with.
    struct Recorder(RefCell<Vec<String>>);

    impl CandidateOrder for Recorder {
        fn order(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<usize> {
            *self.0.borrow_mut() = context.iter().map(|key| key.as_str().to_owned()).collect();
            GeneratorWeightOrder.order(context, candidates)
        }
    }

    // [spec:hfst:sem:prediction.preview/test]
    // [spec:hfst:sem:prediction.session/test]
    #[test]
    fn the_preview_ranks_after_context_and_word() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let mut session = Session::new(walker(&generator, &grammar), 4);
        session.type_letters("guo").unwrap();
        session.pick(&UnitKey::from("guolli")).unwrap();
        session.pick(&UnitKey::from("+N+Sg+Gen")).unwrap();
        session.commit().unwrap();
        session.type_letters("gi").unwrap();
        session.pick(&UnitKey::from("girji")).unwrap();
        let recorder = Recorder(RefCell::new(Vec::new()));
        assert_eq!(session.preview(&recorder).as_deref(), Some("girji"));
        assert_eq!(*recorder.0.borrow(), ["guolli", "+N+Sg+Gen", "girji"]);
    }

    // [spec:hfst:req:prediction.script-edges/test]
    // [spec:hfst:sem:prediction.entry/test]
    #[test]
    fn a_session_converts_script_only_at_its_edges() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let lower = regex("[ G:g | I:i | R:r | U:u | A:a ]*");
        let upper = regex("[ g:G | i:I | r:R | j:J | a:A | t:T ]*");
        let relaxation = regex("[ a | a:á | b | g | h | i | j | k | l | o | r | u ]*");
        let script = Script {
            input: Some(&lower),
            output: Some(&upper),
        };
        let mut session = Session::new(walker(&generator, &grammar), 2)
            .with_script(script)
            .with_relaxation(&relaxation);
        session.type_letters("GA").unwrap();
        assert_eq!(keys_of(&mut session), ["gáhkku"]);
        assert!(session.backspace());
        assert!(session.backspace());
        session.type_letters("GIR").unwrap();
        assert_eq!(keys_of(&mut session), ["girji"]);
        session.pick(&UnitKey::from("girji")).unwrap();
        session.pick(&UnitKey::from("+N+Pl+Nom")).unwrap();
        assert_eq!(
            session.ranking_context(),
            [UnitKey::from("girji"), UnitKey::from("+N+Pl+Nom")]
        );
        assert_eq!(session.commit().unwrap(), "GIRJAT");
        assert_eq!(context(&session), ["girji", "+N+Pl+Nom"]);
        session.type_letters("Q").unwrap();
        assert!(keys_of(&mut session).is_empty());
    }

    fn tokenizer(generator: &crate::transducer::Transducer) -> AnalysisTokenizer {
        AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table())
    }

    fn unit_keys(
        walker: &Walker<'_, crate::transducer::WeightedTables>,
        word: &Word,
    ) -> Vec<String> {
        word.units()
            .iter()
            .map(|unit| walker.grammar().key(unit).as_str().to_owned())
            .collect()
    }

    #[test]
    fn an_analysis_replays_into_the_word_it_spells() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let tokenizer = tokenizer(&generator);
        let word = walker
            .word_from_analysis("girji+N+Sem/Obj+Sg+Nom+Qst", &tokenizer)
            .unwrap();
        assert_eq!(unit_keys(&walker, &word), ["girji", "+N+Sg+Nom", "+Qst"]);
        assert_eq!(surfaces(&walker, &word)[0], "girjigo");
        let compound = walker
            .word_from_analysis("girji+N+Cmp#guolli+N+Pl+Nom", &tokenizer)
            .unwrap();
        assert_eq!(surfaces(&walker, &compound), ["girjguolat"]);
        let stem = walker.word_from_analysis("girji", &tokenizer).unwrap();
        assert!(!stem.is_complete());
        assert_eq!(keys(&walker.next(&stem))[0], "+N+Cmp#");
    }

    #[test]
    fn a_replayed_derivation_reaches_its_ending() {
        let generator = lexc(DERIVATIONS);
        let grammar = grammar(DERIVATIONS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let tokenizer = tokenizer(&generator);
        let derived = walker
            .word_from_analysis("oqar+Gram/IV+NIQ+Der/vn", &tokenizer)
            .unwrap();
        assert_eq!(unit_keys(&walker, &derived), ["oqar", "+NIQ+Der/vn"]);
        assert!(keys(&walker.next(&derived)).contains(&"+N+Abs+Sg"));
        let word = walker
            .word_from_analysis("oqar+NIQ+Der/vn+N+Abs+Sg", &tokenizer)
            .unwrap();
        assert_eq!(surfaces(&walker, &word), ["oqarniq"]);
    }

    #[test]
    fn prefixes_alone_and_nothing_are_words_in_progress() {
        let generator = lexc(PERSONS);
        let grammar = grammar(PERSONS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let tokenizer = tokenizer(&generator);
        assert!(
            walker
                .word_from_analysis("", &tokenizer)
                .unwrap()
                .units()
                .is_empty()
        );
        let prefixed = walker.word_from_analysis("PV/kii+", &tokenizer).unwrap();
        assert_eq!(prefixed.units()[0].kind, UnitKind::Prefix);
        assert!(walker.next(&prefixed).stem_next);
        let word = walker
            .word_from_analysis("PV/kii+nipaa+V+AI+Ind+1Sg", &tokenizer)
            .unwrap();
        assert_eq!(surfaces(&walker, &word), ["nikii-nipaan"]);
    }

    #[test]
    fn refusals_name_the_unit_and_its_place() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let tokenizer = tokenizer(&generator);
        let ending = walker
            .word_from_analysis("borra+N+Sg+Nom", &tokenizer)
            .expect_err("a verb takes no noun ending");
        assert_eq!(ending.kind, ErrorKind::FaultyStringInput);
        let message = ending.message.unwrap();
        assert!(
            message.contains(r#"unit "+N+Sg+Nom" after "borra""#),
            "{message}"
        );
        let stem = walker
            .word_from_analysis("gir+N+Sg+Nom", &tokenizer)
            .expect_err("gir is no stem");
        let message = stem.message.unwrap();
        assert!(
            message.contains(r#"unit "gir" at the start of the word"#),
            "{message}"
        );
        let unknown = walker
            .word_from_analysis("girji+X", &tokenizer)
            .expect_err("+X is not in the alphabet");
        assert_eq!(unknown.kind, ErrorKind::SymbolNotFound);
    }
}
