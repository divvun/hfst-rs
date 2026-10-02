use super::*;

type T = HfstTransducer<StdVectorFst>;

fn pair(input: &str, output: &str) -> T {
    T::new_tokenized_pair(input, output, &HfstTokenizer::new()).expect("pair")
}

fn word(s: &str) -> T {
    T::new_tokenized(s, &HfstTokenizer::new()).expect("word")
}

fn union(parts: &[T]) -> T {
    let mut all = T::new();
    for p in parts {
        all.disjunct(p, true).expect("disjunct");
    }
    all
}

// [spec:hfst:sem:xfst-cmd.pmatch-quotient-subtract/test]
#[test]
fn left_quotient_removes_a_prefix() {
    let mut q = word("a");
    q.left_quotient(&union(&[word("abc"), word("ad"), word("e")]))
        .expect("quotient");
    assert!(
        q.compare(&union(&[word("bc"), word("d")]), true)
            .expect("compare")
    );
}

// [spec:hfst:sem:xfst-cmd.pmatch-quotient-subtract/test]
#[test]
fn quotient_by_nothing_is_empty() {
    let mut q = T::new();
    q.left_quotient(&word("abc")).expect("quotient");
    assert!(q.compare(&T::new(), true).expect("compare"));
}

// [spec:hfst:sem:xfst-cmd.pmatch-quotient-subtract/test]
#[test]
fn side_subtractions_filter_by_one_side() {
    let net = union(&[pair("a", "x"), pair("b", "y")]);
    let mut upper = net.clone();
    upper.upper_subtract(&word("a")).expect("upper");
    assert!(upper.compare(&pair("b", "y"), true).expect("compare"));
    let mut lower = net;
    lower.lower_subtract(&word("y")).expect("lower");
    assert!(lower.compare(&pair("a", "x"), true).expect("compare"));
}

// [spec:hfst:sem:xfst-cmd.pmatch-quotient-subtract/test]
#[test]
fn subtracting_an_unrelated_side_keeps_everything() {
    let net = union(&[pair("a", "x"), pair("b", "y")]);
    let mut upper = net.clone();
    upper.upper_subtract(&word("q")).expect("upper");
    assert!(upper.compare(&net, true).expect("compare"));
}
