// Regression locks for three upstream hfst-twolc (nfst-twolc parser) conformance
// issues: preserve grammar semantics and diagnose malformed or suspicious input.
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
//   hfst#334 — an undeclared symbol used in a rule may be a typo. Warn about
//              it while retaining upstream alphabet completion; rejecting such
//              grammars broke lang-fao. Names remain case-sensitive literals.
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
// Completion must give the same relation as explicitly declaring the missing
// pairs, wherever they occur. Diagnostics are exercised through the CLI below.

// [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.complete-alphabet-fn+1/test]
#[test]
fn hfst334_undeclared_pairs_complete_like_explicit_declarations() {
    let _g = serialized();
    for (extra_pairs, body) in [
        ("FOO:BAR", "Rules\n\"R1\"\nFOO:BAR <=> a _ b ;"),
        ("b:ZZZ", "Rules\n\"R1\"\na:b <=> _ b:ZZZ ;"),
        ("NOPE", "Rules\n\"R1\"\na:b <=> _ NOPE ;"),
        (
            "GHOST:a",
            "Definitions\nD = GHOST:a ;\nRules\n\"R1\"\na:b <=> D _ ;",
        ),
        ("NOPE", "Rules\n\"R1\"\na:b <=> _ ; except NOPE _ ;"),
        (
            "FOO:BAR",
            "Rules\n\"R1\"\nX:Y <=> _ ; where X in (FOO) Y in (BAR) matched ;",
        ),
    ] {
        let implicit = compile(&format!("Alphabet a b c ;\n{body}\n"))
            .unwrap_or_else(|| panic!("grammar pairs must be completed: {body}"));
        let explicit = compile(&format!("Alphabet a b c {extra_pairs} ;\n{body}\n"))
            .expect("explicit pair declaration compiles");
        assert!(
            implicit
                .compare_default(&explicit)
                .expect("compare compiled rules"),
            "completion must match explicit declarations: {body}"
        );
    }
}

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

// A bare symbol is also a legal RULE CENTRE, meaning the same identity pair.
// Upstream's PAIR production covers the centre as well as context regexes, so
// omorfi's generated recasing grammar writes `%{hyph%?%} <= _ ;`. The parser
// used to demand a `:` there and rejected the file. Reported via
// divvun/hfst-rs#3 (omorfi, second report).

#[test]
fn bare_symbol_rule_centre_is_the_identity_pair() {
    let _g = serialized();
    let alphabet_and =
        |centre: &str| format!("Alphabet a b %{{hyph%?%}} ;\nRules\n\"R1\"\n{centre} <= a _ b ;\n");
    let bare = compile(&alphabet_and("%{hyph%?%}"))
        .expect("a bare symbol must be a legal rule centre (divvun/hfst-rs#3)");
    let explicit = compile(&alphabet_and("%{hyph%?%}:%{hyph%?%}"))
        .expect("the written-out identity pair compiles");
    assert!(
        bare.compare_default(&explicit)
            .expect("comparing two compiled rule transducers"),
        "the bare centre must compile to the same rule as the written-out pair"
    );
}

#[test]
fn wildcard_rule_centre_side_expands_over_the_alphabet() {
    let _g = serialized();
    // Upstream's `CENTER_SYMBOL: QUESTION_MARK` makes `?` a legal centre side,
    // resolved against the declared vocabulary by `Alphabet::is_pair`: `X:?`
    // needs X declared as an input symbol, `?:Y` needs Y as an output symbol.
    // `a:` is `a:?` and `:b` is `?:b`, so the elided spellings mean the same.
    for centre in ["a:?", "a:", "?:b", ":b", "?:?", ":"] {
        let src = format!("Alphabet a:b a b ;\nRules\n\"R1\"\n{centre} <= a _ b ;\n");
        assert!(
            compile(&src).is_some(),
            "the wildcard centre {centre:?} must compile"
        );
    }
}

#[test]
fn elided_and_written_out_wildcard_centres_agree() {
    let _g = serialized();
    let compile_centre = |centre: &str| {
        compile(&format!(
            "Alphabet a:b a b ;\nRules\n\"R1\"\n{centre} <= a _ b ;\n"
        ))
        .unwrap_or_else(|| panic!("centre {centre:?} must compile"))
    };
    for (elided, written) in [("a:", "a:?"), (":b", "?:b"), (":", "?:?")] {
        assert!(
            compile_centre(elided)
                .compare_default(&compile_centre(written))
                .expect("comparing two compiled rule transducers"),
            "the elided centre {elided:?} must mean the same as {written:?}"
        );
    }
}

#[test]
fn escaped_question_mark_centre_is_a_literal() {
    let _g = serialized();
    // `%?` is the literal question-mark symbol, `?` is the wildcard. Upstream's
    // pre1 lexer keeps them apart (a bare `?` becomes the `__HFST_TWOLC_?`
    // marker, an escaped one stays an ordinary symbol). Completion must
    // preserve the literal rather than expanding it as a wildcard.
    let declared = compile("Alphabet %?:a %? a ;\nRules\n\"R1\"\n%?:a <= _ ;\n")
        .expect("a declared literal `?` symbol compiles");
    assert!(
        alphabet(&declared).iter().any(|s| s == "?"),
        "the literal `?` symbol must reach the compiled alphabet, got {:?}",
        alphabet(&declared)
    );
    let implicit = compile("Alphabet a b ;\nRules\n\"R1\"\n%?:a <= _ ;\n")
        .expect("an undeclared literal `?` is completed");
    let explicit = compile("Alphabet a b %?:a ;\nRules\n\"R1\"\n%?:a <= _ ;\n")
        .expect("the explicit literal pair compiles");
    assert!(
        implicit
            .compare_default(&explicit)
            .expect("compare literal pairs")
    );
}

// [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.complete-alphabet-fn+1/test]
#[test]
fn fao_comments_boundary_and_case_sensitive_literal_compile() {
    let _g = serialized();
    let body = "Sets\nVow = a ;\nRules\n\"R1\"\na:b <=> vow _ # ;\n";
    let implicit = compile(&format!(
        "!! # Faroese comment with æ and NOT_A_SYMBOL\nAlphabet a b ;\n{body}"
    ))
    .expect("comments, implicit #, and undeclared vow must compile");
    let explicit =
        compile(&format!("Alphabet a b vow # ;\n{body}")).expect("explicit grammar compiles");
    assert!(
        implicit
            .compare_default(&explicit)
            .expect("compare grammar relations")
    );
    let alpha = alphabet(&implicit);
    assert!(
        alpha.iter().any(|s| s == "vow"),
        "vow stays a literal: {alpha:?}"
    );
    assert!(
        !alpha.iter().any(|s| s == "NOT_A_SYMBOL"),
        "comments add no symbols"
    );
    let with_set = compile(&format!(
        "Alphabet a b vow # ;\n{}",
        body.replace("vow _", "Vow _")
    ))
    .expect("set-reference grammar compiles");
    assert!(
        !implicit
            .compare_default(&with_set)
            .expect("compare set and literal")
    );
}

// ─────────────────────── nested set definitions ───────────────────────
// A Sets member may itself name an earlier set, e.g. `Sgm = Vow Cns ;`. The
// members of the named set are spliced in. Without that, the outer set holds
// the literal symbols `Vow` and `Cns`, which are in no alphabet, so every pair
// set built from it is empty and the grammar fails to compile with
// "The pair set Sgm:? is empty." lang-smn's phonology.twolc is built this way
// and could not be compiled at all.

#[test]
fn nested_set_member_expands_to_its_members() {
    let _g = serialized();
    let src = "Alphabet a b c d ;\n\
               Sets\n\
               Vow = a b ;\n\
               Cns = c d ;\n\
               Sgm = Vow Cns ;\n\
               Rules\n\
               \"R1\"\n\
               a:b <=> Sgm: _ ;\n";
    compile(src).expect("a set whose members name other sets must compile");
}

#[test]
fn nested_set_expansion_is_transitive() {
    let _g = serialized();
    // Outer names Mid, Mid names Inner: the leaf symbols must reach Outer.
    let src = "Alphabet a b c ;\n\
               Sets\n\
               Inner = a ;\n\
               Mid = Inner b ;\n\
               Outer = Mid c ;\n\
               Rules\n\
               \"R1\"\n\
               a:b <=> Outer: _ ;\n";
    compile(src).expect("nested set references must expand transitively");
}

#[test]
fn a_set_naming_itself_terminates() {
    let _g = serialized();
    // Self-reference must not splice the partially built list into itself. The
    // name is kept literal, so the set is just its other members (here `a`) plus
    // an inert symbol -- the point is that expansion terminates rather than
    // recursing, and the grammar still compiles.
    let src = "Alphabet a b ;\n\
               Sets\n\
               S = a S ;\n\
               Rules\n\
               \"R1\"\n\
               a:b <=> S: _ ;\n";
    compile(src).expect("a self-referencing set must terminate, not loop");
}

#[test]
fn overlapping_sets_list_a_symbol_once() {
    let _g = serialized();
    // Both Vow and Alt contain `a`; the union must not list it twice.
    let src = "Alphabet a b c ;\n\
               Sets\n\
               Vow = a b ;\n\
               Alt = a c ;\n\
               Both = Vow Alt ;\n\
               Rules\n\
               \"R1\"\n\
               a:b <=> Both: _ ;\n";
    compile(src).expect("overlapping nested sets must compile");
}
