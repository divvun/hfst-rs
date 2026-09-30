// Regression locks for divvun/hfst-rs#4 (lang-esu's phonology).
//
//   `A / B` — the ignoring operator, A with B freely inserted anywhere. It
//             panicked as an unsupported binary operator.
//   `except` — the except contexts are subtracted from the positive
//             contexts. They were negated and OR'd in with them, so an
//             except clause had no effect.
//
// The tropical backend keeps its symbol coding in process-global statics, so
// these tests serialize through one lock (see test_twolc_conformance.rs).

use hfst::hfst_transducer::HfstTransducer;
use hfst::twolc::TwolcCompiler;
use hfst_openfst::StdVectorFst;

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn compile(src: &str) -> HfstTransducer<StdVectorFst> {
    let mut c = TwolcCompiler::<StdVectorFst>::new_with_options(true, false, false, true);
    c.compile(src).expect("grammar compiles")
}

/// Whether the rule accepts the pair string, given as space-separated
/// 'input:output' pairs (a bare symbol is an identity pair).
fn accepts(rule: &HfstTransducer<StdVectorFst>, pairs: &str) -> bool {
    let spv: Vec<_> = pairs
        .split_whitespace()
        .map(|p| {
            let (i, o) = p.split_once(':').unwrap_or((p, p));
            (i.into(), o.into())
        })
        .collect();
    let path = HfstTransducer::<StdVectorFst>::new_string_pair_vector(&spv).expect("path");
    let mut both = HfstTransducer::new_copy(rule).expect("copy");
    both.intersect(&path, true).expect("intersect");
    let mut paths = Default::default();
    both.extract_paths(&mut paths, 1, 0).expect("extract");
    !paths.is_empty()
}

#[test]
fn except_context_lifts_the_obligation() {
    let _g = serialized();
    let rule =
        compile("Alphabet a b c x a:x ;\nRules\n\"R\"\na:x <=> _ c ;\n    except\n        b _ ;\n");
    assert!(accepts(&rule, "x a:x c"));
    assert!(
        !accepts(&rule, "x a c"),
        "outside the except context a:x is obligatory"
    );
    assert!(
        accepts(&rule, "b a c"),
        "the except context lifts the <= obligation"
    );
    assert!(
        !accepts(&rule, "b a:x c"),
        "the except context does not license a:x for =>"
    );
}

#[test]
fn ignoring_operator_skips_the_ignored_symbols() {
    let _g = serialized();
    let rule = compile(
        "Alphabet a b c %< %> ;\nSets\nSep = %< %> ;\nRules\n\"R\"\na:b <=> [ c / Sep ] _ ;\n",
    );
    assert!(accepts(&rule, "c a:b"));
    assert!(
        accepts(&rule, "c < a:b"),
        "a Sep symbol inside the context is ignored"
    );
    assert!(
        !accepts(&rule, "c < a"),
        "a:b stays obligatory across ignored symbols"
    );
    assert!(!accepts(&rule, "b a:b"));
}
