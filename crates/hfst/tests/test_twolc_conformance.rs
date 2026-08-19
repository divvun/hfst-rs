// Regression locks for three upstream hfst-twolc (nfst-twolc parser) conformance
// issues. The port is a SUCCESSOR project: where upstream/Xerox silently
// produces garbage, the port MUST error cleanly instead.
//
//   hfst#189 — a symbol pair whose side is (or contains) a space. Upstream
//              mis-handled the PAIR tokenization; the port tokenizes the
//              percent-escaped space `% ` correctly and carries the literal
//              space symbol through into the compiled rule's alphabet.
//   hfst#570 — an UNESCAPED quotation mark `"` in a rule. Upstream silently
//              produced a bad FST with no error; the port's lexer treats every
//              `"` as a rule-name opener, so a stray/unbalanced `"` is a hard
//              lex error and `compile` fails (returns None) rather than
//              mis-compiling.
//   hfst#334 — a NON-EXISTENT multichar symbol used in a rule. Upstream's
//              `complete_alphabet` auto-declared every grammar symbol, so a typo
//              became nondeterministic garbage with no error. The port validates
//              every rule/context/definition pair side against the declared
//              Alphabet vocabulary and errors on undeclared symbols instead of
//              auto-declaring them.
//
// The compiler builds TROPICAL_OPENFST transducers, whose transition-data symbol
// coding lives in process-global statics behind Mutexes; cargo runs every #[test]
// as a parallel thread in ONE process. These tests serialize through one lock
// (the same pattern as test_xerox_rules.rs / foma_backend.rs) so the shared
// symbol tables are touched one-at-a-time. into_inner() recovers from a poisoned
// lock so one failing test does not cascade.

use hfst::twolc::TwolcCompiler;
use hfst_openfst::StdVectorFst;

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Compile a twolc grammar source at the tropical backend, silently (no
/// diagnostics on stderr). Returns None on any parse / semantic failure, exactly
/// as `hfst-twolc` reports a failed compile.
fn compile(src: &str) -> Option<hfst::hfst_transducer::HfstTransducer<StdVectorFst>> {
    // silent = true, verbose = false, resolve_left = false, resolve_right = true.
    let mut c = TwolcCompiler::<StdVectorFst>::new_with_options(true, false, false, true);
    c.compile(src)
}

/// The set of surface symbols in a compiled transducer's alphabet.
fn alphabet(t: &hfst::hfst_transducer::HfstTransducer<StdVectorFst>) -> Vec<String> {
    t.get_alphabet()
        .expect("compiled transducer exposes its alphabet")
        .iter()
        .map(|s| s.as_str().to_string())
        .collect()
}

// ───────────────────────────── hfst#189 ─────────────────────────────
// A pair whose side is a percent-escaped space (`% `) compiles cleanly and the
// literal space symbol is carried through into the rule transducer's alphabet.

#[test]
fn hfst189_escaped_space_pair_side_compiles_and_preserves_space() {
    let _g = serialized();
    // Alphabet declares the escaped-space symbol `% ` (percent then a space);
    // the rule maps that space symbol to `a`.
    let src = "Alphabet %  a b ;\nRules\n\"R1\"\n%  :a <=> _ ;\n";
    let t = compile(src).expect("a space-containing pair side must compile (hfst#189)");
    let alpha = alphabet(&t);
    assert!(
        alpha.iter().any(|s| s == " "),
        "the literal space symbol must survive into the compiled alphabet, got {alpha:?}"
    );
}

#[test]
fn hfst189_escaped_space_on_both_pair_sides_compiles() {
    let _g = serialized();
    // A space:space identity pair used as the rule center.
    let src = "Alphabet %  a ;\nRules\n\"R1\"\n%  :%  <=> a _ a ;\n";
    let t = compile(src).expect("a space:space identity pair must compile (hfst#189)");
    let alpha = alphabet(&t);
    assert!(
        alpha.iter().any(|s| s == " "),
        "the literal space symbol must survive into the compiled alphabet, got {alpha:?}"
    );
}

// ───────────────────────────── hfst#570 ─────────────────────────────
// An unescaped `"` in a rule is a hard lex error: `compile` fails (None) rather
// than silently building a bad FST. A properly percent-escaped `%"` is a normal
// symbol and compiles.

#[test]
fn hfst570_stray_unescaped_quote_errors_not_silent() {
    let _g = serialized();
    // Stray opening `"` before `c`: the lexer scans for a closing quote, hits
    // the newline, and reports "unterminated rule name".
    let src = "Alphabet a b c ;\nRules\n\"R1\"\na:b <=> _ \"c ;\n";
    assert!(
        compile(src).is_none(),
        "a stray unescaped quotation mark must fail compilation, not mis-compile (hfst#570)"
    );
}

#[test]
fn hfst570_trailing_stray_quote_errors() {
    let _g = serialized();
    // A `"` after `c`, again unbalanced before the newline.
    let src = "Alphabet a b c ;\nRules\n\"R1\"\na:b <=> _ c\" ;\n";
    assert!(
        compile(src).is_none(),
        "an unbalanced trailing quotation mark must fail compilation (hfst#570)"
    );
}

#[test]
fn hfst570_balanced_stray_quotes_still_error() {
    let _g = serialized();
    // Two `"` on one line lex as a spurious RuleName token mid-rule, which the
    // parser rejects (`expected ;`). It must NOT silently absorb the quoted run.
    let src = "Alphabet a b c ;\nRules\n\"R1\"\na:b <=> _ \"c\" d ;\n";
    assert!(
        compile(src).is_none(),
        "a spurious mid-rule quoted run must be a parse error, not silently absorbed (hfst#570)"
    );
}

#[test]
fn hfst570_percent_escaped_quote_is_an_ordinary_symbol() {
    let _g = serialized();
    // `%"` is the ordinary symbol `"`; declared in the alphabet and used as a
    // pair, it compiles and the literal double-quote is in the alphabet.
    let src = "Alphabet %\" a ;\nRules\n\"R1\"\n%\":a <=> _ ;\n";
    let t = compile(src).expect(
        "a properly percent-escaped quote is an ordinary symbol and must compile (hfst#570)",
    );
    let alpha = alphabet(&t);
    assert!(
        alpha.iter().any(|s| s == "\""),
        "the escaped quote symbol must survive into the compiled alphabet, got {alpha:?}"
    );
}

// ───────────────────────────── hfst#334 ─────────────────────────────
// A symbol used in a rule but never declared in the Alphabet must be an error,
// not silently auto-declared into nondeterministic garbage.

#[test]
fn hfst334_undeclared_pair_in_center_errors() {
    let _g = serialized();
    // FOO and BAR are never declared; upstream auto-declared FOO:BAR and
    // produced garbage. The port must reject it.
    let src = "Alphabet a b c ;\nRules\n\"R1\"\nFOO:BAR <=> a _ b ;\n";
    assert!(
        compile(src).is_none(),
        "an undeclared pair in a rule center must fail compilation (hfst#334)"
    );
}

#[test]
fn hfst334_undeclared_pair_in_context_errors() {
    let _g = serialized();
    // ZZZ is undeclared, used only as the lower side of a context pair.
    let src = "Alphabet a b ;\nRules\n\"R1\"\na:b <=> _ b:ZZZ ;\n";
    assert!(
        compile(src).is_none(),
        "an undeclared symbol in a context pair must fail compilation (hfst#334)"
    );
}

#[test]
fn hfst334_undeclared_symbol_in_context_errors() {
    let _g = serialized();
    // A bare undeclared symbol NOTDECLARED in a context (an implicit X:X pair).
    let src = "Alphabet a b c ;\nRules\n\"R1\"\na:b <=> _ NOTDECLARED ;\n";
    assert!(
        compile(src).is_none(),
        "an undeclared bare symbol in a context must fail compilation (hfst#334)"
    );
}

#[test]
fn hfst334_undeclared_symbol_in_definition_errors() {
    let _g = serialized();
    // GHOST is undeclared, hidden inside a definition body that a rule uses.
    let src = "Alphabet a b ;\nDefinitions\nD = GHOST:a ;\nRules\n\"R1\"\na:b <=> D _ ;\n";
    assert!(
        compile(src).is_none(),
        "an undeclared symbol in a definition body must fail compilation (hfst#334)"
    );
}

// A control: a grammar that uses ONLY declared symbols — plus the legitimate
// alphabet-completion cases the fix must NOT break — still compiles. This proves
// the #334 validation rejects undeclared symbols WITHOUT rejecting valid
// grammars (pairings such as `e:0` completed from the declared vocabulary, set
// members, and definition bodies).

#[test]
fn hfst334_fully_declared_grammar_still_compiles() {
    let _g = serialized();
    let src = "\
Alphabet a b c e ;
Sets
Vowel = a e ;
Definitions
AnyC = b:b ;
Rules
\"Deletion\"
e:0 <=> a _ b ;
\"WithSet\"
a:b <=> Vowel _ ;
";
    let t = compile(src).expect("a grammar using only declared symbols must compile (hfst#334)");
    assert!(
        t.number_of_states() >= 1,
        "the compiled grammar must be a non-empty rule transducer"
    );
}

// ─────────────────────── bare-symbol identity pairs ───────────────────────
// A symbol used bare in a rule context is an implicit X:X pair. Upstream's
// htwolcpre1 rewrote `X` into `X:X` before alphabet completion ran, so the
// pair reached the alphabet; the port collected only explicit `X:Y` nodes, so
// a grammar whose vocabulary is declared in a `Sets` section and then used
// bare lost every identity pair and failed at rule-compile time with
// `Unknown pair: a a`. Reported via divvun/hfst-rs#3 (omorfi).

#[test]
fn bare_context_symbol_contributes_its_identity_pair() {
    let _g = serialized();
    // `a` and `e` are declared only as Sets members, and the rule uses them
    // bare (via the where-variable) rather than as `a:a`.
    let src = "\
Alphabet %{h%}:0 %{h%}:%- %- ;
Sets
Vowels = a e ;
Rules
\"Disallow no hyphen between equal vowels\"
%{h%}:0 /<= VOWEL :0* _ :0* VOWEL ;
     where VOWEL in Vowels matched ;
";
    let t = compile(src).expect("a bare Sets-declared symbol in a context must compile");
    let alpha = alphabet(&t);
    for sym in ["a", "e"] {
        assert!(
            alpha.iter().any(|s| s == sym),
            "the bare context symbol {sym:?} must reach the compiled alphabet, got {alpha:?}"
        );
    }
}

#[test]
fn bare_symbol_identity_pair_still_checks_declaration() {
    let _g = serialized();
    // The counterpart: collecting bare symbols must not auto-declare them.
    // `NOPE` is in no section at all, so hfst#334 still rejects the grammar.
    let src = "Alphabet a b ;\nSets\nV = a ;\nRules\n\"R1\"\na:b <=> _ NOPE ;\n";
    assert!(
        compile(src).is_none(),
        "a bare symbol declared nowhere must still fail compilation (hfst#334)"
    );
}
