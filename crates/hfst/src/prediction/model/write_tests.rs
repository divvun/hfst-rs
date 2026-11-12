use super::fixture::{STORY, build, builder, grammar, keys, lookup, settings};
use super::{Backoff, Model, ModelSettings};
use crate::error::Result;

const DERIVED: &[&[&str]] = &[
    &["nuna+N+Abs+Sg"],
    &["oqar+NIQ+Der/vn+N+Abs+Sg"],
    &["sini+V+Ind+3Sg"],
    &["nuna+N+Rel+Sg"],
    &["nuna+N+Abs+Pl"],
    &["oqar+V+Ind+3Pl"],
    &["oqar+V+Ind+3Pl"],
];

fn faces(face_size: usize) -> (Vec<u8>, ModelSettings) {
    let mut settings = settings(2, Backoff::StupidBackoff);
    settings.face_size = face_size;
    (build(settings, DERIVED).0, settings)
}

// [spec:hfst:req:prediction-model.faces/test]
#[test]
fn each_class_shows_its_most_frequent_keys_first() {
    let (bytes, _) = faces(10);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    assert_eq!(
        model.face("noun"),
        keys(&["+N+Abs+Sg", "+N+Abs+Pl", "+N+Rel+Sg", "+NIQ+Der/vn"])
    );
    assert_eq!(model.face("verb"), keys(&["+V+Ind+3Pl", "+V+Ind+3Sg"]));
    assert_eq!(model.face("adjective"), keys(&[]));
}

// [spec:hfst:req:prediction-model.faces/test]
#[test]
fn a_face_holds_the_keys_asked_for() {
    let (bytes, _) = faces(2);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    assert_eq!(model.face("noun"), keys(&["+N+Abs+Sg", "+N+Abs+Pl"]));
    assert_eq!(model.face("verb"), keys(&["+V+Ind+3Pl", "+V+Ind+3Sg"]));
    let (bytes, _) = faces(0);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    assert_eq!(model.face("noun"), keys(&[]));
}

#[test]
fn keys_keep_their_class() {
    let (bytes, _) = faces(10);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    let class = |key: &str| model.class_of(&key.into());
    assert_eq!(class("+NIQ+Der/vn"), Some("noun"));
    assert_eq!(class("+V+Ind+3Sg"), Some("verb"));
    assert_eq!(class("oqar"), None);
    assert_eq!(class("zzz"), None);
}

fn sized(size_limit: usize) -> Result<Vec<u8>> {
    let mut settings = settings(3, Backoff::StupidBackoff);
    settings.size_limit = size_limit;
    let mut builder = builder(settings, |_| true).unwrap();
    builder.add_text(&lookup(STORY)).unwrap();
    builder.write()
}

/// The size of a record of each run length, and how many runs STORY has of
/// each: runs of units, then runs of its forms, which are all different.
const RUN3: usize = 16;
const RUNS3: usize = 8 + 3;
const RUN2: usize = 12;
const RUNS2: usize = 7 + 4;

// [spec:hfst:req:prediction-model.budget/test]
#[test]
fn a_model_within_its_limit_is_written_whole() {
    let full = sized(usize::MAX).unwrap();
    assert_eq!(sized(full.len()).unwrap(), full);
}

// [spec:hfst:req:prediction-model.budget/test]
#[test]
fn the_longest_runs_go_first_least_frequent_first() {
    let full = sized(usize::MAX).unwrap().len();
    let one_less = sized(full - 1).unwrap();
    assert_eq!(one_less.len(), full - RUN3);

    let limit = full - RUNS3 * RUN3 - 1;
    let bytes = sized(limit).unwrap();
    assert!(bytes.len() <= limit);
    assert_eq!(bytes.len(), full - RUNS3 * RUN3 - RUN2);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    let count = |run: &[&str]| model.count(&keys(run));
    assert_eq!(count(&["+N+Abs+Sg", "oqar", "+V+Ind+3Sg"]), 0.0);
    // Of the runs of 2 counted once, units go before forms, and sini
    // +V+Ind+3Sg has the last key.
    assert_eq!(count(&["sini", "+V+Ind+3Sg"]), 0.0);
    assert_eq!(count(&["oqar", "+V+Ind+3Sg"]), 1.0);
    assert_eq!(count(&["nuna", "+N+Rel+Sg"]), 1.0);
    assert_eq!(count(&["nuna", "+N+Abs+Sg"]), 2.0);
    assert_eq!(count(&["sini"]), 1.0);
}

// [spec:hfst:req:prediction-model.budget/test]
#[test]
fn single_keys_are_never_dropped() {
    let full = sized(usize::MAX).unwrap().len();
    let floor = full - RUNS3 * RUN3 - RUNS2 * RUN2;
    let bytes = sized(floor).unwrap();
    assert_eq!(bytes.len(), floor);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    assert_eq!(model.count(&keys(&["nuna", "+N+Abs+Sg"])), 0.0);
    for (key, count) in [("nuna", 3.0), ("+N+Abs+Sg", 2.0), ("oqar", 1.0)] {
        assert_eq!(model.count(&keys(&[key])), count);
    }
    assert_eq!(model.face("noun"), keys(&["+N+Abs+Sg", "+N+Rel+Sg"]));
}

// [spec:hfst:req:prediction-model.budget/test]
#[test]
fn too_small_a_limit_fails_by_its_shortfall() {
    let full = sized(usize::MAX).unwrap().len();
    let floor = full - RUNS3 * RUN3 - RUNS2 * RUN2;
    let error = sized(floor - 7).unwrap_err();
    let message = error.to_string();
    assert!(message.contains(&format!("{floor} bytes")), "{message}");
    assert!(message.contains("7 bytes more"), "{message}");
    assert!(message.contains("unit keys and word forms"), "{message}");
}
