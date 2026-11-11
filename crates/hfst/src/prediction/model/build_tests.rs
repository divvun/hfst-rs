use super::fixture::{STORY, build, build_accepting, builder, grammar, keys, settings};
use super::{Ambiguity, Backoff, BuildReport, Model};

fn count(bytes: &[u8], run: &[&str]) -> f64 {
    Model::from_bytes(bytes, &grammar())
        .unwrap()
        .count(&keys(run))
}

const AMBIGUOUS: &[&[&str]] = &[
    &["nuna+N+Abs+Sg"],
    &["oqar+V+Ind+3Sg", "oqar+V+Ind+3Pl"],
    &["nuna+N+Abs+Sg"],
];

// [spec:hfst:req:prediction-model.input/test]
// [spec:hfst:def:prediction-model.file/test]
#[test]
fn runs_up_to_the_order_span_tokens() {
    let (bytes, report) = build(settings(3, Backoff::StupidBackoff), STORY);
    assert_eq!(
        report,
        BuildReport {
            tokens: 5,
            readings: 5,
            counted: 5,
            ..BuildReport::default()
        }
    );
    assert_eq!(count(&bytes, &["nuna"]), 3.0);
    assert_eq!(count(&bytes, &["+N+Abs+Sg"]), 2.0);
    assert_eq!(count(&bytes, &["+V+Ind+3Sg"]), 2.0);
    assert_eq!(count(&bytes, &["nuna", "+N+Abs+Sg"]), 2.0);
    assert_eq!(count(&bytes, &["+V+Ind+3Sg", "nuna"]), 2.0);
    assert_eq!(count(&bytes, &["+N+Abs+Sg", "oqar"]), 1.0);
    assert_eq!(count(&bytes, &["+N+Abs+Sg", "oqar", "+V+Ind+3Sg"]), 1.0);
    assert_eq!(count(&bytes, &["+V+Ind+3Sg", "nuna", "+N+Rel+Sg"]), 1.0);
    assert_eq!(count(&bytes, &["oqar", "nuna"]), 0.0);
    assert_eq!(
        count(&bytes, &["nuna", "+N+Abs+Sg", "oqar", "+V+Ind+3Sg"]),
        0.0
    );
    assert_eq!(count(&bytes, &[]), 10.0);
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn derivations_and_hidden_tags_follow_the_grammar() {
    let (bytes, _) = build(
        settings(2, Backoff::StupidBackoff),
        &[&["oqar+NIQ+Der/vn+N+Abs+Sg"], &["oqar+V+Gram/IV+Ind+3Sg"]],
    );
    assert_eq!(count(&bytes, &["oqar", "+NIQ+Der/vn"]), 1.0);
    assert_eq!(count(&bytes, &["+NIQ+Der/vn", "+N+Abs+Sg"]), 1.0);
    assert_eq!(count(&bytes, &["oqar", "+V+Ind+3Sg"]), 1.0);
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn first_reading_counts_only_the_first() {
    let (bytes, report) = build(settings(3, Backoff::StupidBackoff), AMBIGUOUS);
    assert_eq!((report.readings, report.counted), (4, 3));
    assert_eq!(count(&bytes, &["oqar"]), 1.0);
    assert_eq!(count(&bytes, &["oqar", "+V+Ind+3Sg"]), 1.0);
    assert_eq!(count(&bytes, &["+V+Ind+3Pl"]), 0.0);
    assert_eq!(count(&bytes, &["+N+Abs+Sg", "oqar", "+V+Ind+3Sg"]), 1.0);
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn first_reading_passes_over_later_readings() {
    let refuse_plural = |a: &str| !a.ends_with("+3Pl");
    let (_, first) = build_accepting(
        settings(3, Backoff::StupidBackoff),
        AMBIGUOUS,
        refuse_plural,
    );
    assert_eq!(
        (first.readings, first.counted, first.refused_by_generator),
        (4, 3, 0)
    );
    let mut equal = settings(3, Backoff::StupidBackoff);
    equal.ambiguity = Ambiguity::EqualShares;
    let (_, all) = build_accepting(equal, AMBIGUOUS, refuse_plural);
    assert_eq!(
        (all.readings, all.counted, all.refused_by_generator),
        (4, 3, 1)
    );
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn equal_shares_split_one_token_over_its_readings() {
    let mut equal = settings(3, Backoff::StupidBackoff);
    equal.ambiguity = Ambiguity::EqualShares;
    let (bytes, report) = build(equal, AMBIGUOUS);
    assert_eq!((report.readings, report.counted), (4, 4));
    assert_eq!(count(&bytes, &["oqar"]), 1.0);
    assert_eq!(count(&bytes, &["+V+Ind+3Sg"]), 0.5);
    assert_eq!(count(&bytes, &["+V+Ind+3Pl"]), 0.5);
    assert_eq!(count(&bytes, &["+N+Abs+Sg", "oqar"]), 1.0);
    assert_eq!(count(&bytes, &["oqar", "+V+Ind+3Pl"]), 0.5);
    assert_eq!(count(&bytes, &["+V+Ind+3Sg", "nuna", "+N+Abs+Sg"]), 0.5);
    assert_eq!(count(&bytes, &["nuna", "+N+Abs+Sg"]), 2.0);
    assert_eq!(count(&bytes, &[]), 6.0);
}

#[test]
fn shares_multiply_along_a_run_of_ambiguous_tokens() {
    let mut equal = settings(3, Backoff::StupidBackoff);
    equal.ambiguity = Ambiguity::EqualShares;
    let (bytes, _) = build(
        equal,
        &[
            &["nuna+N+Abs+Sg", "nuna+N+Rel+Sg"],
            &["oqar+V+Ind+3Sg", "oqar+V+Ind+3Pl"],
        ],
    );
    assert_eq!(count(&bytes, &["+N+Rel+Sg", "oqar"]), 0.5);
    assert_eq!(count(&bytes, &["+N+Rel+Sg", "oqar", "+V+Ind+3Pl"]), 0.25);
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn generator_refusals_are_reported_and_left_out() {
    let (bytes, report) = build_accepting(settings(3, Backoff::StupidBackoff), AMBIGUOUS, |a| {
        a != "oqar+V+Ind+3Sg"
    });
    assert_eq!(
        report,
        BuildReport {
            tokens: 3,
            readings: 4,
            counted: 3,
            refused_by_generator: 1,
            ..BuildReport::default()
        }
    );
    assert_eq!(count(&bytes, &["+V+Ind+3Sg"]), 0.0);
    assert_eq!(count(&bytes, &["oqar", "+V+Ind+3Pl"]), 1.0);
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn grammar_refusals_are_skipped_and_counted() {
    let (bytes, report) = build(
        settings(2, Backoff::StupidBackoff),
        &[
            &["nuna+N+Abs+Sg"],
            &["+N+Abs+Sg"],
            &["oqar+Q+V", "oqar+V+Ind+3Sg"],
        ],
    );
    assert_eq!(
        report,
        BuildReport {
            tokens: 3,
            readings: 4,
            counted: 2,
            refused_by_grammar: 2,
            tokens_without_reading: 1,
            ..BuildReport::default()
        }
    );
    assert_eq!(count(&bytes, &["oqar", "+V+Ind+3Sg"]), 1.0);
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn a_token_with_no_reading_breaks_the_runs() {
    let (bytes, report) = build_accepting(
        settings(3, Backoff::StupidBackoff),
        &[
            &["nuna+N+Abs+Sg"],
            &[],
            &["oqar+V+Ind+3Sg"],
            &["sini+V+Ind+3Sg"],
            &["nuna+N+Abs+Sg"],
        ],
        |a| !a.starts_with("sini"),
    );
    assert_eq!(report.tokens, 5);
    assert_eq!(report.tokens_without_reading, 2);
    assert_eq!(count(&bytes, &["+N+Abs+Sg", "oqar"]), 0.0);
    assert_eq!(count(&bytes, &["+V+Ind+3Sg", "nuna"]), 0.0);
    assert_eq!(count(&bytes, &["oqar", "+V+Ind+3Sg"]), 1.0);
    assert_eq!(count(&bytes, &["nuna", "+N+Abs+Sg"]), 2.0);
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn no_run_spans_two_texts() {
    let mut builder = builder(settings(3, Backoff::StupidBackoff), |_| true).unwrap();
    let mut report = builder.add_text("nuna\tnuna+N+Abs+Sg\t0\n\n").unwrap();
    report += builder.add_text("oqar\toqar+V+Ind+3Sg\t0\n\n").unwrap();
    assert_eq!(report.tokens, 2);
    let bytes = builder.write().unwrap();
    assert_eq!(count(&bytes, &["+N+Abs+Sg", "oqar"]), 0.0);
    assert_eq!(count(&bytes, &["oqar"]), 1.0);
}

#[test]
fn bad_text_fails_the_call() {
    let mut builder = builder(settings(3, Backoff::StupidBackoff), |_| true).unwrap();
    assert!(builder.add_text("nuna+N+Abs+Sg\n").is_err());
}

#[test]
fn order_zero_is_refused() {
    assert!(builder(settings(0, Backoff::DropOldest), |_| true).is_err());
}

// [spec:hfst:sem:prediction-model.order/test]
#[test]
fn order_one_counts_units_alone() {
    let (bytes, _) = build(settings(1, Backoff::StupidBackoff), STORY);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    assert_eq!(model.order(), 1);
    assert_eq!(model.count(&keys(&["nuna"])), 3.0);
    assert_eq!(model.count(&keys(&["nuna", "+N+Abs+Sg"])), 0.0);
}

#[test]
fn the_same_text_builds_the_same_bytes() {
    let mut equal = settings(3, Backoff::StupidBackoff);
    equal.ambiguity = Ambiguity::EqualShares;
    let (first, _) = build(equal, AMBIGUOUS);
    let (second, _) = build(equal, AMBIGUOUS);
    assert_eq!(first, second);
}
