//! Every compiler hands its diagnostics to the caller as values: a failure's
//! diagnostics come back with the right severity, message and byte span, and
//! a success keeps its warnings.

use std::collections::HashMap;

use hfst::diag::{Diagnostic, Severity};
use hfst::lexc::LexcCompiler;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::twolc::TwolcCompiler;
use hfst::xfst_compiler::XfstCompiler;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

fn span_of(src: &str, piece: &str) -> std::ops::Range<usize> {
    let start = src.find(piece).expect("the piece is in the source");
    start..start + piece.len()
}

fn only(ds: &[Diagnostic], severity: Severity) -> &Diagnostic {
    let of: Vec<&Diagnostic> = ds.iter().filter(|d| d.severity() == severity).collect();
    assert_eq!(of.len(), 1, "expected one {severity:?} in {ds:#?}");
    of[0]
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn regex_failure_is_returned_with_its_span() {
    let src = "a -> b || c:d _";
    let mut c = XreCompiler::<StdVectorFst>::new();
    assert!(c.compile(src).is_none());
    let d = only(c.diagnostics(), Severity::Error);
    assert_eq!(d.message(), "Contexts need to be automata");
    assert_eq!(d.primary_span(), Some(span_of(src, "c:d")));
    assert_eq!(d.source_name(), "<regex>");
    assert_eq!(d.source(), src);
    assert_eq!(c.take_diagnostics().len(), 1);
    assert!(c.diagnostics().is_empty());

    let src = "[a b";
    assert!(c.compile(src).is_none());
    let d = only(c.diagnostics(), Severity::Error);
    assert_eq!(d.message(), "expected RightBracket");
    assert_eq!(d.primary_span(), Some(src.len()..src.len()));
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn regex_success_keeps_its_warning() {
    let src = "all b";
    let mut c = XreCompiler::<StdVectorFst>::new();
    c.set_verbosity(true);
    assert!(c.compile(src).is_some());
    let d = only(c.diagnostics(), Severity::Warning);
    assert_eq!(d.message(), "symbol 'all' has no special meaning in hfst");
    assert_eq!(d.primary_span(), Some(span_of(src, "all")));
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn lexc_failure_is_returned_with_its_span() {
    let src = "LEXICON Root\nabc # \n";
    let mut c = LexcCompiler::<StdVectorFst>::new();
    c.set_source_name("root.lexc");
    c.parse(src)
        .expect("a syntax error is reported, not raised");
    let d = only(c.diagnostics(), Severity::Error);
    assert_eq!(d.message(), "unexpected token None inside lexicon entry");
    let at = span_of(src, "#").end;
    assert_eq!(d.primary_span(), Some(at..at));
    assert_eq!(d.source_name(), "root.lexc");
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn lexc_success_keeps_its_warning() {
    let src = "LEXICON Root\nLexicon Foo\nab Foo ;\nLEXICON Foo\nc # ;\n";
    let mut c = LexcCompiler::<StdVectorFst>::new();
    assert!(c.compile(src).is_some());
    let d = only(c.diagnostics(), Severity::Warning);
    assert_eq!(d.message(), "Titlecase Lexicon parsed as LEXICON");
    let start = src.find("Lexicon Foo").expect("the titlecase keyword");
    assert_eq!(d.primary_span().map(|s| s.start), Some(start));
}

// A regex entry's failure comes from the embedded regex compiler and joins the
// lexc compiler's own diagnostics, before the lexc error it causes.
// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn lexc_gathers_its_regex_diagnostics() {
    let src = "LEXICON Root\n< ~[a:b] > # ;\n";
    let mut c = LexcCompiler::<StdVectorFst>::new();
    c.parse(src).expect("the source parses");
    let messages: Vec<&str> = c.diagnostics().iter().map(Diagnostic::message).collect();
    assert_eq!(
        messages,
        [
            "Complement operator ~ is defined only for automata",
            "Unable to parse regular expression"
        ]
    );
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn twolc_failure_is_returned_with_its_span() {
    let src = "Alphabet a b ;\nRules\n\"r\" a:b => _ [ ;\n";
    let mut c = TwolcCompiler::<StdVectorFst>::new();
    assert!(c.compile(src).is_none());
    let d = only(c.diagnostics(), Severity::Error);
    assert_eq!(d.message(), "expected `]`, got `;`");
    let start = src.rfind(';').expect("the stray semicolon");
    assert_eq!(d.primary_span(), Some(start..start + 1));
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn twolc_success_keeps_its_warning() {
    let src = "Alphabet a b ;\nRules\n\"r\" a:b => _ c ;\n";
    let mut c = TwolcCompiler::<StdVectorFst>::new();
    assert!(c.compile(src).is_some());
    let d = only(c.diagnostics(), Severity::Warning);
    assert_eq!(
        d.message(),
        "Symbol 'c' is not declared in the Alphabet; treating it as a literal symbol."
    );
    let start = span_of(src, "c ;").start;
    assert_eq!(d.primary_span(), Some(start..start + 1));
    assert_eq!(
        d.help_text(),
        Some("If 'c' is meant, declare it in the Alphabet.")
    );
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn pmatch_failure_is_returned_with_its_span() {
    let src = "Define TOP {a} L( ;\n";
    let start = src.find(';').expect("the semicolon");
    let mut c = PmatchCompiler::<StdVectorFst>::new();
    assert!(c.compile(src).is_err());
    let d = only(c.diagnostics(), Severity::Error);
    assert!(
        d.message().starts_with("pmatch: syntax error"),
        "{}",
        d.message()
    );
    assert_eq!(d.primary_span().map(|s| s.start), Some(start));

    let mut diagnostics = Vec::new();
    let compiled = hfst::pmatch_compiler::compile::<StdVectorFst>(
        src,
        &HashMap::new(),
        false,
        false,
        false,
        String::new(),
        &mut diagnostics,
    );
    assert!(compiled.is_err());
    assert_eq!(only(&diagnostics, Severity::Error).message(), d.message());
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn xfst_failure_is_returned_with_its_span() {
    let src = "regex a b ;\ndetrminize\n";
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_source_name("s.xfst");
    let err = c
        .parse(src)
        .expect_err("an unknown command fails the script");
    let d = only(c.diagnostics(), Severity::Error);
    assert_eq!(d.message(), "unknown command 'detrminize'");
    assert_eq!(d.primary_span(), Some(span_of(src, "detrminize")));
    assert_eq!(d.source_name(), "s.xfst");
    assert!(d.notes().iter().any(|n| n.contains("determinize")));
    assert_eq!(err.diagnostics, c.take_diagnostics());
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn xfst_success_keeps_its_warning() {
    let src = "set compose-flag-as-special ON\n";
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(src).expect("the script runs");
    let d = only(c.diagnostics(), Severity::Warning);
    assert!(
        d.message().contains("no compose-flag-as-special variable"),
        "{}",
        d.message()
    );
    assert_eq!(d.primary_span(), Some(0..src.trim_end().len()));
}
