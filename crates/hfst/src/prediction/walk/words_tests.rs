use std::cell::Cell;
use std::rc::Rc;

use super::fixtures_test::*;
use super::{
    CandidateOrder, GeneratorWeightOrder, Next, Ranking, Suggest, Suggestion, Walker, Word,
};
use crate::prediction::model::{
    Ambiguity, Backoff, Model, ModelBuilder, ModelSettings, generator_accepts,
};
use crate::prediction::units::{AnalysisTokenizer, UnitGrammar, UnitKey, UnitKind};
use crate::transducer::{Transducer, WeightedTables};

/// Six nouns and a verb that start with n, two of the nouns heavier than
/// the rest, three noun endings of different weights and a derivation to a
/// verb. Nothing changes where morphemes meet.
pub(super) const STEMS: &str = r#"
Multichar_Symbols
+N +V +Abs +Rel +Sg +Pl +Ind +3Sg +QAR +Der/nv

LEXICON Root
Nouns ;
Verbs ;

LEXICON Nouns
nuna:nuna N ;
nanoq:nanoq N ;
naja:naja N ;
niaqoq:niaqoq N ;
nipi:nipi N "weight: 1" ;
nuka:nuka N "weight: 2" ;

LEXICON Verbs
nalu:nalu V ;

LEXICON N
+N+Abs+Sg:0 # ;
+N+Abs+Pl:t # "weight: 1" ;
+N+Rel+Sg:p # "weight: 2" ;
+QAR+Der/nv:qar V ;

LEXICON V
+V+Ind+3Sg:poq # ;
"#;

pub(super) const STEMS_GRAMMAR: &str = r#"
boundary = ["+Der/*"]
"#;

/// nuna seven times, six of them +N+Abs+Sg; nanoq five times, three of
/// them +N+Abs+Pl; naja four times, always +N+Abs+Sg. 32 units in all.
const READINGS: &[(&str, &str, usize)] = &[
    ("nuna", "nuna+N+Abs+Sg", 6),
    ("nunap", "nuna+N+Rel+Sg", 1),
    ("nanoqt", "nanoq+N+Abs+Pl", 3),
    ("nanoq", "nanoq+N+Abs+Sg", 2),
    ("naja", "naja+N+Abs+Sg", 4),
];

fn model_bytes(generator: &Transducer, grammar: &UnitGrammar, backoff: Backoff) -> Vec<u8> {
    let tokenizer = AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table());
    let settings = ModelSettings {
        order: 2,
        backoff,
        ambiguity: Ambiguity::FirstReading,
        face_size: 0,
        size_limit: usize::MAX,
    };
    let mut builder =
        ModelBuilder::new(grammar, &tokenizer, generator_accepts(generator), settings).unwrap();
    let text: String = READINGS
        .iter()
        .flat_map(|&(surface, reading, times)| {
            std::iter::repeat_n(format!("{surface}\t{reading}\t0.000000\n\n"), times)
        })
        .collect();
    builder.add_text(&text).unwrap();
    builder.write().unwrap()
}

/// Units by their keys, complete words by their surface in brackets.
fn shown(suggestions: &[Suggestion<'_>]) -> Vec<String> {
    suggestions
        .iter()
        .map(|suggestion| match suggestion {
            Suggestion::Unit(candidate) => candidate.key.as_str().to_owned(),
            Suggestion::Word(word) => format!("[{}]", word.surface()),
            Suggestion::Form(seen) => format!("<{}>", seen.form),
        })
        .collect()
}

fn whole_words(whole_words: usize, limit: Option<usize>) -> Suggest {
    Suggest {
        whole_words,
        limit,
        ..Suggest::default()
    }
}

fn suggested(
    walker: &Walker<'_, WeightedTables>,
    typed: &str,
    order: &dyn CandidateOrder,
    settings: Suggest,
) -> Vec<String> {
    let start = walker.start();
    let next = walker.entry(&start, typed, None).unwrap();
    let ranking = Ranking::after_units(order, &[]);
    shown(&walker.suggest(&start, &next.candidates, &ranking, settings))
}

// [spec:hfst:sem:prediction.whole-words/test]
// [spec:hfst:sem:prediction-model.word-score/test]
#[test]
fn stems_on_offer_bring_their_complete_words() {
    let generator = lexc(STEMS);
    let grammar = grammar(STEMS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let bytes = model_bytes(&generator, &grammar, Backoff::StupidBackoff);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    assert_eq!(
        suggested(&walker, "n", &model, whole_words(0, Some(3))),
        ["nuna", "nanoq", "naja"]
    );
    // [nuna] scores 7/32 × 6/7, [naja] 4/32 × 1 and [nanoqt] 5/32 × 3/5.
    // nanoq keeps 2/32, nuna 1/32 and naja nothing.
    assert_eq!(
        suggested(&walker, "n", &model, whole_words(1, Some(6))),
        ["[nuna]", "[naja]", "[nanoqt]", "nanoq", "nuna", "naja"]
    );
    assert_eq!(
        suggested(&walker, "n", &model, whole_words(2, Some(5))),
        ["[nuna]", "[naja]", "[nanoqt]", "[nanoq]", "[nunap]"]
    );
}

// [spec:hfst:sem:prediction-model.word-score/test]
#[test]
fn drop_oldest_scores_words_by_shares_of_counts() {
    let generator = lexc(STEMS);
    let grammar = grammar(STEMS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let bytes = model_bytes(&generator, &grammar, Backoff::DropOldest);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    // The counts 7, 5 and 4 times the shares 6/7, 3/5 and 1.
    assert_eq!(
        suggested(&walker, "n", &model, whole_words(1, Some(6))),
        ["[nuna]", "[naja]", "[nanoqt]", "nanoq", "nuna", "naja"]
    );
}

// [spec:hfst:sem:prediction.whole-words/test]
// [spec:hfst:req:prediction-model.rank-only/test]
#[test]
fn unseen_words_follow_by_weight() {
    let generator = lexc(STEMS);
    let grammar = grammar(STEMS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let all = suggested(&walker, "ni", &GeneratorWeightOrder, whole_words(1, None));
    assert_eq!(all, ["niaqoq", "[niaqoq]", "nipi", "[nipi]"]);
    assert_eq!(
        suggested(
            &walker,
            "nu",
            &GeneratorWeightOrder,
            whole_words(2, Some(4))
        ),
        ["nuna", "[nuna]", "[nunat]", "nuka"]
    );
}

/// What a lazy ranking found, and how many stems it completed.
fn lazily(
    walker: &Walker<'_, WeightedTables>,
    word: &Word,
    next: &Next,
    order: &dyn CandidateOrder,
    settings: Suggest,
) -> (Vec<String>, usize) {
    let completed = Cell::new(0);
    let ranking = Ranking::after_units(order, &[]);
    let found = walker.suggest_with(word, &next.candidates, &ranking, settings, |_, picked| {
        completed.set(completed.get() + 1);
        Ok::<_, std::convert::Infallible>(Rc::new(walker.endings(picked)))
    });
    let Ok(found) = found;
    (shown(&found), completed.get())
}

// [spec:hfst:sem:prediction.whole-words/test]
// [spec:hfst:sem:prediction-model.word-score/test]
#[test]
fn the_lazy_ranking_is_the_exhaustive_one() {
    let generator = lexc(STEMS);
    let grammar = grammar(STEMS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let stupid = model_bytes(&generator, &grammar, Backoff::StupidBackoff);
    let stupid = Model::from_bytes(&stupid, &grammar).unwrap();
    let oldest = model_bytes(&generator, &grammar, Backoff::DropOldest);
    let oldest = Model::from_bytes(&oldest, &grammar).unwrap();
    let orders: [&dyn CandidateOrder; 3] = [&stupid, &oldest, &GeneratorWeightOrder];
    let start = walker.start();
    let (mut lazy_walks, mut all_walks) = (0, 0);
    for typed in ["n", "na", "nu", "ni", "nan", "x"] {
        let next = walker.entry(&start, typed, None).unwrap();
        for order in orders {
            for count in 0..=3 {
                let (all, every) = lazily(&walker, &start, &next, order, whole_words(count, None));
                for limit in 0..=all.len() + 1 {
                    let settings = whole_words(count, Some(limit));
                    let (found, completed) = lazily(&walker, &start, &next, order, settings);
                    assert_eq!(
                        found,
                        all[..limit.min(all.len())],
                        "{typed} {count} {limit}"
                    );
                    assert!(completed <= every);
                    lazy_walks += completed;
                    all_walks += every;
                }
            }
        }
    }
    assert!(lazy_walks < all_walks, "{lazy_walks} of {all_walks}");
    let next = walker.entry(&start, "n", None).unwrap();
    let (_, completed) = lazily(&walker, &start, &next, &stupid, whole_words(1, Some(3)));
    assert_eq!(completed, 3);
}

// [spec:hfst:sem:prediction.whole-words/test]
#[test]
fn a_complete_word_picks_stem_and_ending() {
    let generator = lexc(STEMS);
    let grammar = grammar(STEMS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let start = walker.start();
    let next = walker.entry(&start, "nan", None).unwrap();
    let suggestions = walker.suggest(
        &start,
        &next.candidates,
        &Ranking::after_units(&GeneratorWeightOrder, &[]),
        whole_words(1, None),
    );
    let Some(Suggestion::Word(word)) = suggestions.get(1) else {
        panic!("{:?}", shown(&suggestions));
    };
    assert_eq!(word.key(), "nanoq+N+Abs+Sg");
    assert_eq!(word.stem().key, UnitKey::from("nanoq"));
    assert_eq!(word.ending().unit.kind, UnitKind::Ending);
    let picked = start.pick_word(word);
    assert!(picked.is_complete());
    assert_eq!(picked.units().len(), 2);
    assert_eq!(surfaces(&walker, &picked)[0], word.surface());
    let found = walker
        .whole_word(&start, &next.candidates, "nanoq+N+Rel+Sg")
        .unwrap();
    assert_eq!(found.surface(), "nanoqp");
    for refused in ["nanoq+N+Rel", "nanoq+QAR+Der/nv", "nanoq", "nuna+N+Abs+Sg"] {
        assert!(
            walker
                .whole_word(&start, &next.candidates, refused)
                .is_none(),
            "{refused}"
        );
    }
}

/// After a compound join a stem can be picked again, and its complete
/// words are labelled with the whole compound.
// [spec:hfst:sem:prediction.whole-words/test]
#[test]
fn complete_words_follow_a_compound_join() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let joined = word_after(&walker, "girji", &["+N+Cmp#"]);
    let next = walker.entry(&joined, "guo", None).unwrap();
    let suggestions = walker.suggest(
        &joined,
        &next.candidates,
        &Ranking::after_units(&GeneratorWeightOrder, &[]),
        whole_words(1, None),
    );
    assert_eq!(shown(&suggestions), ["guolli", "[girjguoli]"]);
}

// [spec:hfst:sem:prediction.whole-words/test]
#[test]
fn the_endings_are_those_the_full_walk_offers() {
    let generator = lexc(STEMS);
    let grammar = grammar(STEMS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    for stem in ["nuna", "nalu"] {
        let word = word_after(&walker, stem, &[]);
        let endings = walker.endings(&word);
        let full = walker.next(&word);
        let of_full: Vec<(&str, f32)> = full
            .candidates
            .iter()
            .filter(|candidate| candidate.unit.kind == UnitKind::Ending)
            .map(|candidate| (candidate.key.as_str(), candidate.weight))
            .collect();
        let found: Vec<(&str, f32)> = endings
            .candidates
            .iter()
            .map(|candidate| (candidate.key.as_str(), candidate.weight))
            .collect();
        assert_eq!(found, of_full, "{stem}");
        assert!(!endings.stem_next && !endings.truncated);
    }
    assert!(keys(&walker.next(&word_after(&walker, "nuna", &[]))).contains(&"+QAR+Der/nv"));
    assert!(walker.endings(&walker.start()).candidates.is_empty());
}
