// Port of test/libhfst/test_lexc.cc
//
// Tests the LEXC (lexicon) compiler front-end: parsing a valid lexc file via
// LexcCompiler, reading the same file via HfstTransducer::read_lexc, and the
// failure behaviour for a malformed file and a missing file (both must yield a
// null transducer).
//
// The C++ main loops over the implementation types {SFST, TROPICAL, FOMA} (with
// LOG commented out). Per the Wave-2 port scope, only the in-scope OpenFST
// backend is exercised here: with the monomorphic backends the loop body
// becomes helpers generic over the backend type, instantiated for the
// formerly-exercised TROPICAL_OPENFST_TYPE -> StdVectorFst. The out-of-scope
// SFST_TYPE / FOMA_TYPE / XFSM_TYPE iterations are intentionally skipped.
//
// Each logical group from the C++ loop body becomes its own helper, run once
// per in-scope type:
//   (valid) parse + compileLexical, then compare against cat | dog | mouse;
//   (valid) read_lexc, then the same compare;
//   (invalid) malformed file -> compileLexical returns 0;
//   (missing) nonexistent file -> compileLexical returns 0.
//
// C++ compare(another) defaults to harmonize=true, mirrored here by
// compare_default. C++ disjunct(another) also defaults to harmonize=true,
// mirrored by disjunct(&other, true).
//
// The C++ LexcCompiler::parse(filename) opens the file through the Flex/Bison
// lexer; the ported LexcCompiler instead walks an AST built from source text via
// compile(&str). The parse_and_compile helper below reads the file and feeds
// compile, so a read failure (missing file) mirrors the C++ "could not open the
// file" path, where compileLexical returns 0.

use hfst::backend::AlgebraBackend;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::HfstTransducer;
use hfst::lexc::LexcCompiler;
use hfst_openfst::StdVectorFst;

// The tropical transition-data symbol coding lives in process-global
// statics behind Mutexes; cargo runs every #[test] as a parallel thread in ONE
// process, so concurrent symbol-table mutation can race and throw
// HfstFatalException. Each C++ test was its own process and never hit this.
// Serializing the tests through this lock restores the one-at-a-time model
// without touching the library or weakening any assertion. into_inner() recovers
// from a poisoned lock so one failing test does not cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

// C++: cat | dog | mouse, each built with a default tokenizer (no multichar
// symbols) and disjuncted into an initially empty transducer.
fn build_animals<B: AlgebraBackend>() -> Result<HfstTransducer<B>, hfst::error::Error> {
    let tok = HfstTokenizer::new();
    let cat = HfstTransducer::<B>::new_tokenized("cat", &tok)?;
    let dog = HfstTransducer::<B>::new_tokenized("dog", &tok)?;
    let mouse = HfstTransducer::<B>::new_tokenized("mouse", &tok)?;

    let mut animals = HfstTransducer::<B>::new();
    animals.disjunct(&cat, true)?;
    animals.disjunct(&dog, true)?;
    animals.disjunct(&mouse, true)?;
    Ok(animals)
}

// Mirrors C++ "LexcCompiler compiler(type); compiler.parse(filename);
// HfstTransducer * parsed = compiler.compileLexical();". Returns None when the
// C++ would have produced a null pointer (parse error or unopenable file).
fn parse_and_compile<B: AlgebraBackend>(filename: &str) -> Option<HfstTransducer<B>> {
    let mut compiler = LexcCompiler::<B>::new();
    let source = match std::fs::read_to_string(filename) {
        Ok(s) => s,
        // C++ parse() could not open the file -> parseErrors set ->
        // compileLexical() returns 0.
        Err(_) => return None,
    };
    compiler.compile(&source)
}

// (1) A file in valid lexc format: parse + compileLexical, then compare.
fn valid_file_parse<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let parsed = parse_and_compile::<B>(&fixture_path("test_lexc.lexc"));
    assert!(
        parsed.is_some(),
        "compileLexical() returned 0 for a valid file"
    );
    let parsed = parsed.expect("valid lexc file must compile to a transducer");

    let animals = build_animals::<B>()?;
    assert!(animals.compare_default(&parsed)?);
    Ok(())
}

// (1) The same valid file via HfstTransducer::read_lexc. C++ catches
// FunctionNotImplementedException and asserts false; for TROPICAL read_lexc
// does not throw it.
fn valid_file_read_lexc<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let animals = build_animals::<B>()?;
    let rlexc = HfstTransducer::<B>::read_lexc(&fixture_path("test_lexc.lexc"), false)?;
    assert!(animals.compare_default(&rlexc)?);
    Ok(())
}

// (2) A file that does not follow lexc format: compileLexical returns 0.
fn invalid_file_parse<B: AlgebraBackend>() {
    let parsed = parse_and_compile::<B>(&fixture_path("test_lexc_fail.lexc"));
    assert!(
        parsed.is_none(),
        "compileLexical() should return 0 for a malformed file"
    );
}

// (3) A file that does not exist: compileLexical returns 0.
fn missing_file_parse<B: AlgebraBackend>() {
    let parsed = parse_and_compile::<B>(&fixture_path("nonexistent.lexc"));
    assert!(
        parsed.is_none(),
        "compileLexical() should return 0 for a missing file"
    );
}

// =====================================================================
// TROPICAL_OPENFST_TYPE (StdVectorFst)
// =====================================================================

#[test]
fn valid_file_parse_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    valid_file_parse::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn valid_file_read_lexc_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    valid_file_read_lexc::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn invalid_file_parse_tropical() {
    let _g = serialized();
    invalid_file_parse::<StdVectorFst>();
}

#[test]
fn missing_file_parse_tropical() {
    let _g = serialized();
    missing_file_parse::<StdVectorFst>();
}

// =====================================================================
// Undeclared multi-code-point graphemes.
//
// lexc reads `a` + U+0301 as one symbol only because the two code points form
// one grapheme cluster; an author who meant that should say so in
// Multichar_Symbols, and one who did not meant to type the precomposed letter.
// The rendering goes to stderr, which a test cannot read; what is asserted here
// is the shaping that feeds it — where it points, what it says, and its advice.
// =====================================================================

// An entry using the decomposed form of 'á' without declaring it.
const DECOMPOSED: &str = "LEXICON Root\na\u{301}bc # ;\n";

fn grapheme_reports(src: &str) -> Vec<hfst::lexc::GraphemeDiagnostic> {
    let _g = serialized();
    let mut compiler = LexcCompiler::<StdVectorFst>::new();
    compiler.parse(src).expect("the source parses");
    compiler.grapheme_diagnostics().to_vec()
}

#[test]
fn decomposed_grapheme_is_reported() {
    let ds = grapheme_reports(DECOMPOSED);
    let first = ds.first().expect("the grapheme is reported");
    assert_eq!(ds.len(), 1, "reported more than once: {ds:?}");
    assert!(
        first.message.contains("a\u{301}"),
        "grapheme left unnamed in {:?}",
        first.message
    );
}

// Upstream anchors this at the entry's semicolon; the caret belongs under the
// grapheme the author has to change.
#[test]
fn report_points_at_the_grapheme() {
    let ds = grapheme_reports(DECOMPOSED);
    let first = ds.first().expect("the grapheme is reported");
    assert_eq!(&DECOMPOSED[first.span.clone()], "a\u{301}");
}

#[test]
fn report_advises_declaring_the_grapheme() {
    let ds = grapheme_reports(DECOMPOSED);
    let first = ds.first().expect("the grapheme is reported");
    assert!(
        first.notes.iter().any(|n| n.contains("Multichar_Symbols")),
        "no declaration advice in {:?}",
        first.notes
    );
    // Every code point, not upstream's truncated first two.
    assert!(
        first.notes.iter().any(|n| n.contains("U+0061 U+0301")),
        "code points left unspelled in {:?}",
        first.notes
    );
}

// The decomposition is usually an accident of the author's keyboard, so the
// single-code-point spelling is worth naming where one exists.
#[test]
fn precomposed_spelling_is_offered_when_available() {
    let ds = grapheme_reports(DECOMPOSED);
    let first = ds.first().expect("the grapheme is reported");
    assert!(
        first.notes.iter().any(|n| n.contains("U+00E1")),
        "no precomposed spelling in {:?}",
        first.notes
    );
}

// 'u' with combining dot above has no precomposed form — the case Giella
// declares in Multichar_Symbols. Advice to normalise would be a dead end.
#[test]
fn no_precomposed_spelling_means_no_such_advice() {
    let ds = grapheme_reports("LEXICON Root\nu\u{307}bc # ;\n");
    let first = ds.first().expect("the grapheme is reported");
    assert!(
        !first.notes.iter().any(|n| n.contains("NFC")),
        "unreachable advice in {:?}",
        first.notes
    );
}

#[test]
fn an_ascii_lexicon_reports_nothing() {
    assert!(grapheme_reports("LEXICON Root\nabc # ;\n").is_empty());
}

// The Giella convention: declare the grapheme and the warning goes away.
#[test]
fn a_declared_grapheme_stays_silent() {
    let src = "Multichar_Symbols a\u{301}\n\nLEXICON Root\na\u{301}bc # ;\n";
    assert!(grapheme_reports(src).is_empty());
}

// Once per distinct grapheme, not once per occurrence — including the two
// sides of a single pair entry, which are checked separately.
#[test]
fn a_repeated_grapheme_is_reported_once() {
    let src = "LEXICON Root\nxa\u{301}y:qa\u{301}z # ;\na\u{301}bc # ;\n";
    assert_eq!(grapheme_reports(src).len(), 1);
}

// A lower-side-only grapheme is still the author's to declare.
#[test]
fn a_lower_side_grapheme_is_reported() {
    assert_eq!(grapheme_reports("LEXICON Root\nx:a\u{301} # ;\n").len(), 1);
}

// Splitting characters is a request to read each code point on its own, so
// there is no grouping left to declare.
#[test]
fn split_characters_disables_the_check() {
    let _g = serialized();
    let mut compiler = LexcCompiler::<StdVectorFst>::new();
    compiler.set_split_characters(true);
    compiler.parse(DECOMPOSED).expect("the source parses");
    assert!(compiler.grapheme_diagnostics().is_empty());
}
