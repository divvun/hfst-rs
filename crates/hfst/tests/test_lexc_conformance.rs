// Regression locks for four upstream hfst-lexc (nfst-lexc parser) conformance
// issues. Every one of these was found to ALREADY behave correctly in the Rust
// port (the AST-walk `LexcCompiler` over the fresh `nfst-lexc` parser); these
// tests pin the correct Xerox-conformant behaviour so it cannot regress.
//
//   hfst#281  hfst-lexc must accept a linebreak before the `;` that terminates
//             an entry (Xerox accepts it). The `nfst-lexc` lexer skips `\n` as
//             whitespace, so `dog #\n;` parses like `dog # ;`.
//
//   hfst#274  A bare `0` is EPSILON; only an escaped `%0` is a literal digit
//             zero. The lexer maps `%0` to the `@ZERO@` marker and leaves a
//             bare `0` as `"0"`; `LexcCompiler` then rewrites bare `"0"` to the
//             `@0@` epsilon marker and `@ZERO@` back to a literal `"0"`.
//
//   hfst#211  A multichar symbol whose name CONTAINS a zero (e.g. `A0B`,
//             `+Pl0`) is one atomic symbol whose `0` stays literal — longest
//             -match tokenization consumes the declared multichar before the
//             standalone `0`->epsilon rewrite can touch it. Companion to #274.
//
//   hfst#255  An `@`-delimited multichar symbol that is NOT a valid flag
//             diacritic (e.g. `@foo@`, `@X@`) is an ordinary literal symbol and
//             must survive tokenization intact; only genuine flag diacritics
//             (`@U.FOO.BAR@`) are treated as flags (epsilon-like at the
//             surface).
//
// The tests drive the `LexcCompiler` API directly (mirroring `test_lexc.rs`)
// and read the compiled transducer's paths the same way `hfst fst2strings`
// does: concatenate each pair's input/output symbols, skipping the internal
// epsilon. All tests share the symbol-table lock used by `test_lexc.rs`
// because the tropical symbol coding lives behind process-global statics.

use hfst::hfst_data_types::HfstTwoLevelPaths;
use hfst::lexc::LexcCompiler;
use hfst_openfst::StdVectorFst;

const EPSILON: &str = "@_EPSILON_SYMBOL_@";

// The tropical transition-data symbol coding lives in process-global statics
// behind Mutexes; cargo runs every #[test] as a parallel thread in ONE process,
// so concurrent symbol-table mutation can race. Serialize as `test_lexc.rs`
// does. into_inner() recovers from a poisoned lock so one failing test does not
// cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Compile `source` and return the set of `input:output` surface strings the
/// resulting transducer accepts, exactly as `hfst fst2strings` would render
/// them (epsilons dropped, flag diacritics filtered, upper and lower joined by
/// `:`). When upper == lower the pair collapses to a single string, matching
/// fst2strings' output. `extract_paths_fd(filter_fd=true)` mirrors the
/// fst2strings default so genuine flag diacritics are epsilon-like here too.
fn compile_to_strings(source: &str) -> std::collections::BTreeSet<String> {
    let mut compiler = LexcCompiler::<StdVectorFst>::new();
    let compiled = compiler
        .compile(source)
        .expect("lexc source must compile to a transducer");

    let mut paths: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
    compiled
        .extract_paths_fd(&mut paths, -1, -1, true)
        .expect("acyclic lexicon must extract");

    let mut out = std::collections::BTreeSet::new();
    for path in paths.iter() {
        let mut istring = String::new();
        let mut ostring = String::new();
        for pair in path.second.iter() {
            if pair.0 != EPSILON {
                istring.push_str(&pair.0);
            }
            if pair.1 != EPSILON {
                ostring.push_str(&pair.1);
            }
        }
        if istring == ostring {
            out.insert(istring);
        } else {
            out.insert(format!("{istring}:{ostring}"));
        }
    }
    out
}

fn expect_strings(source: &str, expected: &[&str]) {
    let got = compile_to_strings(source);
    let want: std::collections::BTreeSet<String> =
        expected.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(
        got, want,
        "\n--- lexc source ---\n{source}\n--- got {got:?}\n--- want {want:?}\n"
    );
}

// ===================================================================
// hfst#281 — a linebreak before the terminating `;` is accepted.
// ===================================================================

#[test]
fn issue_281_linebreak_before_semicolon() {
    let _g = serialized();
    // The `;` sits on its own line after the continuation.
    expect_strings("LEXICON Root\ncat # \n;\n", &["cat"]);
}

#[test]
fn issue_281_linebreak_no_trailing_space() {
    let _g = serialized();
    // Linebreak immediately after the continuation, no space before it.
    expect_strings("LEXICON Root\ncat #\n;\n", &["cat"]);
}

#[test]
fn issue_281_blank_lines_before_semicolon() {
    let _g = serialized();
    // Several blank lines between the entry body and its `;`.
    expect_strings("LEXICON Root\ncat:dog #\n\n\n;\n", &["cat:dog"]);
}

#[test]
fn issue_281_gloss_then_linebreak_then_semicolon() {
    let _g = serialized();
    // A quoted gloss, then a linebreak, then the `;`.
    expect_strings("LEXICON Root\ncat # \"a cat\"\n;\n", &["cat"]);
}

// ===================================================================
// hfst#274 — bare `0` is epsilon; `%0` is a literal digit zero.
// ===================================================================

#[test]
fn issue_274_bare_zero_is_epsilon() {
    let _g = serialized();
    // `a0b` -> `ab` (bare 0 is epsilon); `c%0d` -> `c0d` (%0 is literal zero).
    expect_strings("LEXICON Root\na0b # ;\nc%0d # ;\n", &["ab", "c0d"]);
}

#[test]
fn issue_274_lone_zero_vs_escaped_zero() {
    let _g = serialized();
    // A lone bare `0` entry is the empty string; a lone `%0` entry is "0".
    expect_strings("LEXICON Root\n0 # ;\n%0 # ;\n", &["", "0"]);
}

#[test]
fn issue_274_pair_bare_zeros_are_epsilon() {
    let _g = serialized();
    // Bare zeros vanish on both sides: `c0t:d0g` -> `ct:dg`.
    expect_strings("LEXICON Root\nc0t:d0g # ;\n", &["ct:dg"]);
}

#[test]
fn issue_274_pair_escaped_zeros_are_literal() {
    let _g = serialized();
    // Escaped zeros stay literal on both sides: `c%0t:d%0g` -> `c0t:d0g`.
    expect_strings("LEXICON Root\nc%0t:d%0g # ;\n", &["c0t:d0g"]);
}

#[test]
fn issue_274_declared_zero_satisfies_alphabet_check() {
    let _g = serialized();
    let mut compiler = LexcCompiler::<StdVectorFst>::new();
    compiler.set_align_strings(true);
    compiler.set_warning("-Wmissing-alphabets", true);
    compiler.set_treat_warnings_as_errors(true);

    let source = "Multichar_Symbols a b %0\nLEXICON Root\na%0:b # ;\n";
    assert!(
        compiler.compile(source).is_some(),
        "a declared literal zero must not be reported as a missing alphabet"
    );
}

#[test]
fn issue_274_implicit_literal_zero_is_informational() {
    let _g = serialized();
    let mut compiler = LexcCompiler::<StdVectorFst>::new();
    compiler.set_align_strings(true);
    compiler.set_warning("-Wmissing-alphabets", true);
    compiler.set_treat_warnings_as_errors(true);

    let source = "Multichar_Symbols a b\nLEXICON Root\na%0:b # ;\n";
    assert!(
        compiler.compile(source).is_some(),
        "an implicit literal zero notice must not become an error under -Werror"
    );
}

// ===================================================================
// hfst#211 — multichar symbols containing zeros are atomic.
// ===================================================================

#[test]
fn issue_211_multichar_containing_zero_is_atomic() {
    let _g = serialized();
    // `+Pl0` and `A0B` are declared multichars: their embedded `0` stays
    // literal because longest-match consumes the whole symbol first.
    expect_strings(
        "Multichar_Symbols +Pl0 A0B\nLEXICON Root\ncat+Pl0 # ;\nfooA0B # ;\n",
        &["cat+Pl0", "fooA0B"],
    );
}

#[test]
fn issue_211_undeclared_zero_still_epsilon() {
    let _g = serialized();
    // Without a multichar declaration the same bare `0` inside `fooA0B` is
    // epsilon: `fooA0B` -> `fooAB`. This is the #274 rule and the contrast
    // that makes #211 meaningful.
    expect_strings("LEXICON Root\nfooA0B # ;\n", &["fooAB"]);
}

// ===================================================================
// hfst#255 — `@`-delimited NON-flag multichars survive; real flags are flags.
// ===================================================================

#[test]
fn issue_255_at_delimited_nonflag_is_literal() {
    let _g = serialized();
    // `@foo@` is not a valid flag diacritic (byte 2 is not `.`); it must
    // survive as a literal multichar symbol.
    expect_strings(
        "Multichar_Symbols @foo@\nLEXICON Root\ncat@foo@ # ;\n",
        &["cat@foo@"],
    );
}

#[test]
fn issue_255_short_at_symbol_is_literal() {
    let _g = serialized();
    // `@X@` is too short to be a flag diacritic; it must survive mid-string.
    expect_strings(
        "Multichar_Symbols @X@\nLEXICON Root\nb@X@d # ;\n",
        &["b@X@d"],
    );
}

#[test]
fn issue_255_real_flag_diacritic_is_a_flag() {
    let _g = serialized();
    // A genuine flag diacritic `@U.FOO.BAR@` is treated as a flag: it is
    // epsilon-like at the surface, so `cat@U.FOO.BAR@` -> `cat`.
    expect_strings(
        "Multichar_Symbols @U.FOO.BAR@\nLEXICON Root\ncat@U.FOO.BAR@ # ;\n",
        &["cat"],
    );
}
