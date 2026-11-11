use super::fixtures_test::*;
use super::*;
use crate::lookup_state::LookupState;

const EPSILON: &str = "@_EPSILON_SYMBOL_@";

/// Reads `a`, then writes `x` and goes round an epsilon cycle that writes
/// nothing more. `b` comes after a flag arc that sets `f`, `c` after one
/// that leaves the cycle setting `f` another way, and `d` after one that
/// requires a value of `f` no path sets before it.
fn cycling() -> Transducer {
    arcs(
        &[
            (0, 1, "a", "a"),
            (1, 2, EPSILON, "x"),
            (2, 1, EPSILON, EPSILON),
            (1, 3, "@P.f.A@", "@P.f.A@"),
            (3, 4, "b", "b"),
            (1, 5, "@R.f.B@", "@R.f.B@"),
            (5, 6, "d", "d"),
            (2, 7, "@P.f.B@", "@P.f.B@"),
            (7, 8, "c", "c"),
        ],
        &[4, 6, 8],
    )
}

fn agree<T: crate::transducer::TransducerTablesInterface>(
    transducer: &crate::transducer::Transducer<T>,
    analyses: &[&str],
) -> Vec<bool> {
    let acceptor = Acceptor::new(transducer);
    let mut lookup = LookupState::new(transducer);
    analyses
        .iter()
        .map(|analysis| {
            let symbols = acceptor
                .tokenize(analysis)
                .unwrap_or_else(|| panic!("{analysis} tokenizes"));
            let accepted = acceptor.accepts(&symbols);
            let looked_up = !lookup.lookup_fd(analysis, 1, 0.0).is_empty();
            assert_eq!(accepted, looked_up, "{analysis}");
            accepted
        })
        .collect()
}

#[test]
fn acceptance_agrees_with_lookup_under_flags() {
    let generator = lexc(PERSONS);
    let analyses = [
        "nipaa+V+AI+Ind+1Sg",
        "nipaa+V+AI+Ind+2Sg",
        "PV/kii+pimi+V+AI+Ind+3Sg",
        "nipaa+V+AI+Ind",
        "nipaa",
        "pimi+V+AI+Ind+1Sg+1Sg",
        "",
    ];
    assert_eq!(
        agree(&generator, &analyses),
        [true, true, true, false, false, false, false]
    );
}

#[test]
fn acceptance_agrees_with_lookup_through_epsilon_cycles() {
    let cycles = lexc(CYCLES);
    assert_eq!(
        agree(
            &cycles,
            &["bb+N+Sg", "bb", "aa+Rep+Rep+N+Sg", "aa+Rep", "aa+N+Sg+Rep"]
        ),
        [true, false, true, false, false]
    );
    let cycling = cycling();
    assert_eq!(
        agree(&cycling, &["ab", "ac", "ad", "a", "abc", "aab"]),
        [true, true, false, false, false, false]
    );
}

#[test]
fn acceptance_agrees_with_lookup_when_output_comes_first() {
    let generator = lexc(DERIVATIONS);
    let analyses = [
        "oqar+V+Ind+3Sg",
        "oqar+NIQ+N+Abs+Sg",
        "oqar+Gram/IV+NIQ+Der/vn+N+Abs+Sg+LI",
        "Akoq+Prop+U+Der/nv+V+Ind+3Sg",
        "oqar+NIQ+Der/vn",
        "Akoq+V+Ind+3Sg",
    ];
    assert_eq!(
        agree(&generator, &analyses),
        [true, true, true, true, false, false]
    );
}

#[test]
fn unknown_characters_do_not_tokenize() {
    let generator = lexc(NOUNS);
    let acceptor = Acceptor::new(&generator);
    assert!(acceptor.tokenize("girji+N+Sg+Nom").is_some());
    assert_eq!(acceptor.tokenize("girji+N+Sg+Nomz"), None);
    assert!(acceptor.reads_like_lookup());
}
