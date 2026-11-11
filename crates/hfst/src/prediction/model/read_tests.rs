use super::fixture::{GRAMMAR, STORY, build, grammar, keys, settings};
use super::{Ambiguity, Backoff, Model};
use crate::error::ErrorKind;
use crate::prediction::units::{UnitGrammar, UnitKey};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

// [spec:hfst:syn:prediction-model.format/test]
// [spec:hfst:def:prediction-model.file/test]
#[test]
fn the_header_holds_hash_and_settings() {
    let mut equal = settings(3, Backoff::DropOldest);
    equal.ambiguity = Ambiguity::EqualShares;
    let (bytes, _) = build(equal, STORY);
    assert_eq!(&bytes[..8], b"HFSTPRED");
    assert_eq!(bytes[8..12], 1u32.to_le_bytes());
    assert_eq!(bytes[12..44], grammar().hash());
    assert_eq!(bytes[44..48], 3u32.to_le_bytes());
    assert_eq!(bytes[48..52], 0u32.to_le_bytes());
    assert_eq!(bytes[52..56], 1u32.to_le_bytes());
    assert_eq!(bytes[56..64], 10.0f64.to_le_bytes());
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    assert_eq!(model.order(), 3);
    assert_eq!(model.backoff(), Backoff::DropOldest);
    assert_eq!(model.ambiguity(), Ambiguity::EqualShares);
}

// [spec:hfst:syn:prediction-model.format/test]
#[test]
fn tables_are_sorted_and_found_by_the_directory() {
    let (bytes, _) = build(settings(2, Backoff::StupidBackoff), STORY);
    let entry = |index: usize| {
        let at = 64 + 8 * index;
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        (word(at), word(at + 4))
    };
    let (strings, _) = entry(0);
    let (units, unit_count) = entry(1);
    assert_eq!(unit_count, 6);
    let unit_keys: Vec<&str> = (0..unit_count)
        .map(|unit| {
            let record = &bytes[units + 16 * unit..units + 16 * unit + 16];
            let offset = u32::from_le_bytes(record[..4].try_into().unwrap()) as usize;
            let length = u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize;
            std::str::from_utf8(&bytes[strings + offset..strings + offset + length]).unwrap()
        })
        .collect();
    assert_eq!(
        unit_keys,
        [
            "+N+Abs+Sg",
            "+N+Rel+Sg",
            "+V+Ind+3Sg",
            "nuna",
            "oqar",
            "sini"
        ]
    );
    let (runs, run_count) = entry(4);
    assert_eq!(run_count, 7);
    let first_run = &bytes[runs..runs + 12];
    assert_eq!(first_run[..4], 0u32.to_le_bytes());
    assert_eq!(first_run[4..8], 4u32.to_le_bytes());
    assert_eq!(first_run[8..], 1.0f32.to_le_bytes());
}

// [spec:hfst:syn:prediction-model.format/test]
#[test]
fn a_model_reads_in_place_at_any_offset() {
    let (bytes, _) = build(settings(3, Backoff::StupidBackoff), STORY);
    let mut buffer = vec![0xAB; 3];
    buffer.extend_from_slice(&bytes);
    buffer.extend_from_slice(&[0xCD; 5]);
    let inner = &buffer[3..3 + bytes.len()];
    let model = Model::from_bytes(inner, &grammar()).unwrap();
    assert_eq!(model.count(&keys(&["nuna", "+N+Abs+Sg"])), 2.0);
    assert_eq!(
        model.count(&keys(&["+V+Ind+3Sg", "nuna", "+N+Rel+Sg"])),
        1.0
    );
    assert_eq!(model.count(&keys(&["zzz"])), 0.0);
}

// [spec:hfst:req:prediction-model.same-grammar/test]
#[test]
fn another_grammar_is_refused_with_both_hashes() {
    let (bytes, _) = build(settings(3, Backoff::StupidBackoff), STORY);
    let other_source = format!("{GRAMMAR}\n# one more line\n");
    let other = UnitGrammar::from_toml(other_source.as_bytes()).unwrap();
    let error = Model::from_bytes(&bytes, &other).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidModelFile);
    let message = error.to_string();
    assert!(message.contains(&hex(&grammar().hash())), "{message}");
    assert!(message.contains(&hex(&other.hash())), "{message}");
}

#[test]
fn a_file_not_a_model_is_refused() {
    let grammar = grammar();
    assert!(Model::from_bytes(b"", &grammar).is_err());
    assert!(Model::from_bytes(&[0; 200], &grammar).is_err());
    let (mut bytes, _) = build(settings(3, Backoff::StupidBackoff), STORY);
    bytes[8] = 2;
    let error = Model::from_bytes(&bytes, &grammar).unwrap_err();
    assert!(error.to_string().contains("format version 2"), "{error}");
}

fn exercise(model: &Model<'_>) {
    let context = keys(&["+V+Ind+3Sg", "nuna"]);
    for key in ["nuna", "+N+Abs+Sg", "+N+Rel+Sg", "oqar", "zzz"] {
        let key = UnitKey::from(key);
        model.score(&context, &key);
        model.count(&[context[1].clone(), key.clone()]);
        model.class_of(&key);
    }
    model.face("noun");
    model.face("verb");
    model.rank(
        &context,
        &[(UnitKey::from("+N+Abs+Sg"), 0.0), (UnitKey::from("x"), 1.0)],
    );
}

// [spec:hfst:syn:prediction-model.format/test]
#[test]
fn a_damaged_model_never_panics() {
    let grammar = grammar();
    let (bytes, _) = build(settings(3, Backoff::StupidBackoff), STORY);
    for length in 0..bytes.len() {
        if let Ok(model) = Model::from_bytes(&bytes[..length], &grammar) {
            exercise(&model);
        }
    }
    for at in 44..bytes.len() {
        for value in [0x00, 0x7F, 0xFF] {
            let mut damaged = bytes.clone();
            damaged[at] = value;
            if let Ok(model) = Model::from_bytes(&damaged, &grammar) {
                exercise(&model);
            }
        }
    }
}
