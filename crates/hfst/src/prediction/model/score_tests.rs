use super::fixture::{STORY, build, grammar, keys, settings};
use super::{Backoff, Model};
use crate::prediction::units::UnitKey;

fn score(backoff: Backoff, context: &[&str], key: &str) -> Option<f64> {
    let (bytes, _) = build(settings(3, backoff), STORY);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    model.score(&keys(context), &UnitKey::from(key))
}

fn close(found: Option<f64>, expected: f64) {
    let found = found.expect("the key is scored");
    assert!((found - expected).abs() < 1e-9, "{found} is not {expected}");
}

// [spec:hfst:sem:prediction-model.order/test]
#[test]
fn stupid_backoff_divides_by_the_context_count() {
    close(
        score(Backoff::StupidBackoff, &["+V+Ind+3Sg", "nuna"], "+N+Abs+Sg"),
        0.5,
    );
    close(
        score(Backoff::StupidBackoff, &["+V+Ind+3Sg", "nuna"], "+N+Rel+Sg"),
        0.5,
    );
    close(
        score(Backoff::StupidBackoff, &["sini", "+V+Ind+3Sg"], "nuna"),
        1.0,
    );
}

// [spec:hfst:sem:prediction-model.order/test]
#[test]
fn stupid_backoff_takes_four_tenths_per_unit() {
    // oqar never follows nuna: 0.4 × 0.4 × oqar's 1 of 10 units.
    close(
        score(Backoff::StupidBackoff, &["+V+Ind+3Sg", "nuna"], "oqar"),
        0.016,
    );
    // +N+Abs+Sg never follows +V+Ind+3Sg: 0.4 × 0.4 × its 2 of 10.
    close(
        score(Backoff::StupidBackoff, &["oqar", "+V+Ind+3Sg"], "+N+Abs+Sg"),
        0.032,
    );
    // An unknown oldest unit is dropped like any other: 0.4 × 2 / 3.
    close(
        score(Backoff::StupidBackoff, &["zzz", "nuna"], "+N+Abs+Sg"),
        0.4 * 2.0 / 3.0,
    );
}

// [spec:hfst:sem:prediction-model.order/test]
#[test]
fn context_is_cut_to_the_order() {
    close(
        score(
            Backoff::StupidBackoff,
            &["oqar", "sini", "+V+Ind+3Sg", "nuna"],
            "+N+Abs+Sg",
        ),
        0.5,
    );
    close(score(Backoff::StupidBackoff, &[], "nuna"), 0.3);
}

// [spec:hfst:sem:prediction-model.order/test]
#[test]
fn drop_oldest_scores_by_the_longest_context() {
    close(
        score(Backoff::DropOldest, &["+V+Ind+3Sg", "nuna"], "+N+Abs+Sg"),
        1.0,
    );
    close(
        score(Backoff::DropOldest, &["oqar", "+V+Ind+3Sg"], "nuna"),
        1.0,
    );
    close(
        score(Backoff::DropOldest, &["sini", "nuna"], "+N+Abs+Sg"),
        2.0,
    );
    close(
        score(Backoff::DropOldest, &["+V+Ind+3Sg", "nuna"], "oqar"),
        1.0,
    );
}

// [spec:hfst:sem:prediction-model.order/test]
#[test]
fn an_unseen_key_has_no_score() {
    assert_eq!(score(Backoff::StupidBackoff, &["nuna"], "zzz"), None);
    assert_eq!(score(Backoff::DropOldest, &["nuna"], "zzz"), None);
}
