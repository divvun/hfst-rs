//! Source a compiler cannot make sense of comes back as an error, never as a
//! panic or a recursion that exhausts the stack.

use hfst::lexc::LexcCompiler;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::twolc::{OstConfig, TwolcCompiler};
use hfst::xfst_compiler::XfstCompiler;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;
use nfst_twolc::{BinaryOp, Span, Spanned, TwolcRegex};

fn pmatch_error(src: &str) -> String {
    let mut c = PmatchCompiler::<StdVectorFst>::new();
    match c.compile(src) {
        Ok(_) => panic!("{src:?} compiled"),
        Err(e) => e.to_string(),
    }
}

fn regex(src: &str) -> hfst::hfst_transducer::HfstTransducer<StdVectorFst> {
    XreCompiler::<StdVectorFst>::new()
        .compile(src)
        .unwrap_or_else(|| panic!("{src:?} does not compile"))
}

fn spanned(value: TwolcRegex) -> Box<Spanned<TwolcRegex>> {
    Box::new(Spanned::new(value, Span::anonymous(0..0)))
}

// The parser never hands one over, but the evaluator takes any regex as a
// pair side, so it has to refuse one that is not a single symbol.
// [spec:hfst:req:embed.no-panic/test]
#[test]
fn twolc_non_symbol_pair_side_is_an_error() {
    let upper = spanned(TwolcRegex::Binary(
        BinaryOp::Union,
        spanned(TwolcRegex::Symbol("a".into())),
        spanned(TwolcRegex::Symbol("b".into())),
    ));
    let lower = spanned(TwolcRegex::Symbol("c".into()));
    let pair = Spanned::new(TwolcRegex::Pair { upper, lower }, Span::anonymous(0..0));
    let mut c = TwolcCompiler::<StdVectorFst>::new();
    let Err(e) = c.eval_regex(&OstConfig::default(), &pair) else {
        panic!("a union compiled as a pair side");
    };
    assert!(e.to_string().contains("single symbol"), "{e}");
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn lexc_verbosity_reads_back_every_level() {
    let mut c = LexcCompiler::<StdVectorFst>::new();
    for (set, read) in [(0, 0), (1, 1), (2, 2), (7, 2)] {
        c.set_verbosity(set);
        assert_eq!(c.get_verbosity(), read, "set_verbosity({set})");
    }
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_malformed_range_is_an_error() {
    let e = pmatch_error("Define TOP \"z-a\" ;\n");
    assert!(
        e.contains("Range expression goes from higher to lower"),
        "{e}"
    );
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_wrong_argument_count_is_an_error() {
    let e = pmatch_error("Define F(x) x ;\nDefine TOP F(a, b) ;\n");
    assert!(e.contains("Function F expected 1 args, got 2"), "{e}");
    let e = pmatch_error("Define TOP Interpolate({a}, {b}) ;\n");
    assert!(e.contains("expects at least 3"), "{e}");
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_merge_of_transducers_is_an_error() {
    let e = pmatch_error("Define TOP [a:b] .m>. [c:d] ;\n");
    assert!(e.contains("must be automata in merge"), "{e}");
}

// A function argument that is an expression rather than a name is evaluated
// to its transducer, as the C++ did.
// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_function_argument_may_be_an_expression() {
    let mut c = PmatchCompiler::<StdVectorFst>::new();
    let called = c
        .compile("Define F(x) x ;\nDefine TOP F([a b]|c:d) ;\n")
        .expect("the call compiles");
    let mut c = PmatchCompiler::<StdVectorFst>::new();
    let direct = c
        .compile("Define TOP [a b]|c:d ;\n")
        .expect("the expression compiles");
    assert!(
        called["TOP"]
            .compare(&direct["TOP"], true)
            .expect("both are tropical")
    );
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_self_referring_definition_is_an_error() {
    let e = pmatch_error("Define X X a:b ;\nDefine TOP Ins(X) ;\n");
    assert!(e.contains("X is defined in terms of itself"), "{e}");
    let e = pmatch_error("Define F(x) A x ;\nDefine A F(a) ;\nDefine TOP A ;\n");
    assert!(e.contains("A is defined in terms of itself"), "{e}");
}

// Insertions with no TOP leave nothing to add separators to, so the compile
// comes back without a TOP.
// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_insertions_without_top_compile() {
    let mut c = PmatchCompiler::<StdVectorFst>::new();
    let compiled = c
        .compile("Define Root Ins(X) ;\nDefine X a:b ;\n")
        .expect("the script compiles");
    assert!(!compiled.contains_key("TOP"));
}

// No count is both at least 3 and at most 1, and none is below zero: the
// empty language, over the alphabet the expression names.
// [spec:hfst:req:embed.no-panic/test]
#[test]
fn unsatisfiable_repetition_is_the_empty_language() {
    for src in ["a^{3,1}", "a^<0"] {
        let t = regex(src);
        assert!(
            t.compare(&regex("~[?*]"), true).expect("both are tropical"),
            "{src} accepts something"
        );
        assert!(
            t.get_alphabet().expect("an alphabet").contains("a"),
            "{src} lost its alphabet"
        );
    }
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn recursive_xre_function_is_an_error() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_output_stream(std::io::sink());
    assert!(c.parse("define F(x) F(x) x ;\nregex F(a) ;\n").is_err());
    assert!(
        c.diagnostics()
            .iter()
            .any(|d| d.message().contains("function 'F' calls itself")),
        "{:#?}",
        c.diagnostics()
    );
}
