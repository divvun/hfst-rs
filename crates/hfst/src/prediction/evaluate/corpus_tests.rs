use super::corpus::Corpus;
use super::fixture::*;
use super::{characters, is_character};
use crate::error::ErrorKind;
use crate::prediction::model::generator_accepts;
use crate::prediction::units::{AnalysisTokenizer, UnitGrammar};
use crate::transducer::Transducer;

fn read<'t>(generator: &Transducer, grammar: &UnitGrammar, text: &'t str) -> Corpus<'t> {
    let tokenizer = AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table());
    let accepts = generator_accepts(generator);
    Corpus::read(text, grammar, &tokenizer, &*accepts).unwrap()
}

#[test]
fn tokens_keep_their_readings_and_where_they_lie() {
    let generator = generator();
    let grammar = grammar();
    let text = "nunat\tnuna+N+Abs+Pl\t0.000000\n\n\
                ,\t,+?\tinf\n\n\
                nunap\tnuna+N+Rel+Sg\t0.000000\r\n\
                nunap\tnuna+N+Erg+Sg\t0.000000\r\n\r\n";
    let corpus = read(&generator, &grammar, text);
    let tokens = corpus.tokens();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].surface, "nunat");
    assert_eq!(tokens[0].readings, ["nuna+N+Abs+Pl"]);
    assert!(tokens[1].readings.is_empty());
    assert_eq!(tokens[2].readings, ["nuna+N+Rel+Sg", "nuna+N+Erg+Sg"]);
    assert_eq!(corpus.text_of(1..2), ",\t,+?\tinf\n\n");
    assert_eq!(corpus.text_of(0..3), text);
    assert_eq!(corpus.text_of(2..2), "");
}

#[test]
fn malformed_analysed_text_is_refused() {
    let generator = generator();
    let grammar = grammar();
    let tokenizer = AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table());
    let accepts = generator_accepts(&generator);
    let two_fields = "nuna\tnuna+N+Abs+Sg\n\n";
    let refused = Corpus::read(two_fields, &grammar, &tokenizer, &*accepts).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::FaultyStringInput);
    assert!(refused.to_string().contains("line 1"), "{refused}");
    let joined = "nuna\tnuna+N+Abs+Sg\t0\nillu\tillu+N+Abs+Sg\t0\n\n";
    let refused = Corpus::read(joined, &grammar, &tokenizer, &*accepts).unwrap_err();
    assert!(refused.to_string().contains("line 2"), "{refused}");
}

/// A token's target is its first reading the generator accepts and the
/// grammar cuts, as the model builder counts it.
#[test]
fn the_target_is_the_first_reading_counted() {
    let generator = generator();
    let grammar = grammar();
    let text = lookup(&[
        ("nuna", &["nuna+N+Abs+Pl+Sg", "nuna+N+Abs+Sg"]),
        ("nunap", &["nuna+N+Erg+Sg", "nuna+N+Rel+Sg"]),
        ("xyz", &[]),
        ("illu", &["illu+N+Abs+Du"]),
    ]);
    let corpus = read(&generator, &grammar, &text);
    let first = corpus.target(0).unwrap();
    assert_eq!(first.reading, "nuna+N+Abs+Sg");
    let keys: Vec<&str> = first.keys.iter().map(|key| key.as_str()).collect();
    assert_eq!(keys, ["nuna", "+N+Abs+Sg"]);
    assert_eq!(corpus.target(1).unwrap().reading, "nuna+N+Erg+Sg");
    assert!(corpus.target(2).is_none());
    assert!(corpus.target(3).is_none());
    assert!(corpus.target(4).is_none());
}

// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn characters_leave_out_spaces_and_punctuation() {
    assert_eq!(characters("Illu"), 4);
    assert_eq!(characters("ê-nipâyân"), 8);
    assert_eq!(characters("Nuuk,"), 4);
    assert_eq!(characters("«nuna»"), 4);
    assert_eq!(characters("1990"), 4);
    assert_eq!(characters(". . ."), 0);
    assert!(is_character('ŋ'));
    assert!(!is_character('\u{00A0}'));
    assert!(!is_character('—'));
}

// [spec:hfst:req:prediction-evaluation.held-out/test]
#[test]
fn folds_split_the_tokens_into_contiguous_parts() {
    let generator = generator();
    let grammar = grammar();
    let word: (&str, &[&str]) = ("nuna", &["nuna+N+Abs+Sg"]);
    let text = lookup(&[word; 7]);
    let corpus = read(&generator, &grammar, &text);
    let folds = corpus.folds(3).unwrap();
    let scored: Vec<_> = folds.iter().map(|fold| fold.scored.clone()).collect();
    assert_eq!(scored, [0..2, 2..4, 4..7]);
    let built_from: Vec<Vec<(usize, usize)>> = folds
        .iter()
        .map(|fold| {
            let ranges = fold.built_from.iter();
            ranges.map(|range| (range.start, range.end)).collect()
        })
        .collect();
    assert_eq!(
        built_from,
        [vec![(2, 7)], vec![(0, 2), (4, 7)], vec![(0, 4)]]
    );
    assert!(corpus.folds(1).is_err());
    assert!(corpus.folds(8).is_err());
    assert_eq!(corpus.folds(7).unwrap().len(), 7);
}
