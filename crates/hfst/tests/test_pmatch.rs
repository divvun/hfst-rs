// Behavioral coverage for the PMATCH compiler and its PmatchObject AST /
// global-definition-table model. pmatch_compiler.rs is otherwise untested
// (only examples/pmatch_smoke.rs and pmatch_container_smoke.rs exercise it);
// these tests lock the compile + match + definition-reference (DAG) paths
// across the major node types so the static-mut / raw-pointer -> safe
// conversion (idiom1.pmatch) is validated rather than blind. The grammar
// constructs (predefined acceptors, repetition, references, EndTag) mirror
// scripts/windows_tests/test.pmatch.
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

// States of the named definition produced by compiling `src`.
fn def_states(src: &str, name: &str) -> Result<u32, hfst::error::Error> {
    let mut c = PmatchCompiler::<StdVectorFst>::new();
    let defs = c.compile(src)?;
    let d = defs
        .get(name)
        .unwrap_or_else(|| panic!("no {name} in pmatch result"));
    Ok(d.number_of_states())
}

fn top_states(src: &str) -> Result<u32, hfst::error::Error> {
    def_states(src, "TOP")
}

// Compile `src` to TOP, build a runtime container, and return the match output.
fn compile_and_match(src: &str, input: &str) -> Result<String, hfst::error::Error> {
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let defs = compiler.compile(src)?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    // The pmatch runtime pins the weighted optimized-lookup backend; convert the
    // compiled tropical TOP through the basic transducer (the typed replacement
    // for the old runtime convert()).
    let top_owned = HfstTransducer::<Transducer<WeightedTables>>::new_from_basic(&top.to_basic()?)?;
    let mut container = PmatchContainer::new_from_hfst_transducers(vec![top_owned])?;
    Ok(container.do_match(input, 0.0, 0.0))
}

#[test]
fn compile_simple_union() -> Result<(), hfst::error::Error> {
    assert!(top_states("Define TOP [{cat} | {dog}] ;\n")? >= 1);
    Ok(())
}

#[test]
fn compile_concatenation() -> Result<(), hfst::error::Error> {
    // juxtaposition concatenates.
    assert!(top_states("Define TOP {ab} {cd} ;\n")? >= 1);
    Ok(())
}

#[test]
fn compile_optional_and_union() -> Result<(), hfst::error::Error> {
    assert!(top_states("Define TOP [{a} | {b}] ({c}) ;\n")? >= 1);
    Ok(())
}

#[test]
fn compile_predefined_acceptors_and_repetition() -> Result<(), hfst::error::Error> {
    // UppercaseAlpha / Alpha predefined sets with * and +, behind a reference.
    let src = "Define CapWord UppercaseAlpha Alpha* ;\nDefine TOP CapWord+ EndTag(word) ;\n";
    assert!(top_states(src)? >= 1);
    Ok(())
}

#[test]
fn compile_definition_reference() -> Result<(), hfst::error::Error> {
    // A definition referenced by name from TOP exercises the DEFINITIONS table
    // and AST cross-references (the DAG path the conversion touches).
    let src = "Define Animal [{cat} | {dog}] ;\nDefine TOP Animal ;\n";
    assert!(top_states(src)? >= 1);
    Ok(())
}

#[test]
fn compile_deep_dag_french_streets() -> Result<(), hfst::error::Error> {
    // The test.pmatch grammar: a multi-level DAG (TOP -> StreetFr -> StreetWordFr
    // / DeFr / CapWord), predefined acceptors, repetition, optional, EndTag.
    let src = "\
Define CapWord UppercaseAlpha Alpha* ;
Define StreetWordFr [{avenue} | {boulevard} | {rue}] ;
Define DeFr [ [{de} | {du} | {des}] Whitespace ] | [{d'} | {l'}] ;
Define StreetFr StreetWordFr (Whitespace DeFr) CapWord+ ;
Define TOP StreetFr EndTag(PseudoFrenchStreetName) ;
";
    assert!(top_states(src)? >= 1);
    Ok(())
}

#[test]
fn match_endtag_runtime() -> Result<(), hfst::error::Error> {
    let out = compile_and_match("Define TOP [{cat} | {dog}] EndTag(animal) ;\n", "cat")?;
    assert!(
        out.contains("cat"),
        "expected 'cat' in match output, got {out:?}"
    );
    let out2 = compile_and_match("Define TOP [{cat} | {dog}] EndTag(animal) ;\n", "dog")?;
    assert!(
        out2.contains("dog"),
        "expected 'dog' in match output, got {out2:?}"
    );
    Ok(())
}

#[test]
fn match_numerals_runtime() -> Result<(), hfst::error::Error> {
    // Numeral+ predefined acceptor with a tag, matched against digits.
    let out = compile_and_match("Define TOP Numeral+ EndTag(num) ;\n", "123")?;
    assert!(
        out.contains("123"),
        "expected '123' in match output, got {out:?}"
    );
    Ok(())
}

// Regression: the giellacg (--giella-cg) input driver 'process_input_0delim'
// accumulated raw input bytes into a Rust String via `ch as char`, which
// reinterprets each UTF-8 byte as a Latin-1 codepoint and corrupts multibyte
// characters BEFORE they reach the matcher. A known word containing 'å'
// (0xC3 0xA5) then failed to match and was split at the byte boundary
// ("<g>" ... "<etie>"). Drive the driver with a NUL-delimited multibyte input
// and assert the word tokenizes whole.
#[test]
fn giellacg_input_preserves_multibyte_utf8() -> Result<(), hfst::error::Error> {
    use hfst::pmatch_tokenize::{OutputFormat, TokenizeSettings, process_input_0delim};

    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let defs = compiler.compile("Define TOP {gåetie} EndTag(N) ;\n")?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    let top_owned = HfstTransducer::<Transducer<WeightedTables>>::new_from_basic(&top.to_basic()?)?;
    let mut container = PmatchContainer::new_from_hfst_transducers(vec![top_owned])?;
    container.set_single_codepoint_tokenization(true);

    let settings = TokenizeSettings {
        output_format: OutputFormat::giellacg,
        print_all: true,
        print_weights: true,
        ..TokenizeSettings::default()
    };

    // "gåetie\0" — å is the two bytes 0xC3 0xA5.
    let input: &[u8] = b"g\xc3\xa5etie\0";
    let mut reader = input;
    let mut out: Vec<u8> = Vec::new();
    process_input_0delim(
        &mut container,
        &mut reader,
        &mut out,
        false,
        &settings,
        false,
    );
    let out = String::from_utf8_lossy(&out);

    assert!(
        out.contains("gåetie"),
        "multibyte word should tokenize whole, got:\n{out}"
    );
    assert!(
        !out.contains("\"<g>\""),
        "word was split at the multibyte boundary:\n{out}"
    );
    Ok(())
}

// [hfst/hfst#483] hfst-tokenize hung / went O(n^2) on large single input
// sentences. Root cause: PmatchContainer::initialize_input segments the FIRST
// grapheme once per input position, but did so by validating the WHOLE
// remaining tail (`std::str::from_utf8(&buf[p..])`) on every position — O(n)
// work per position, O(n^2) over the line. nByte_grapheme_bytes must depend
// only on the leading grapheme, not on the tail.
//
// The tail-independence is asserted deterministically (no timing): the fix
// only ever inspects a bounded prefix, so a leading grapheme followed by
// arbitrary — even invalid-UTF-8 — bytes still yields that grapheme's length.
// The old whole-tail version passed the entire slice through
// `from_utf8(..).unwrap_or("")`, so a single invalid byte anywhere in the tail
// collapsed the answer to 0 (and it scanned the whole tail besides). That makes
// the invalid-tail cases below fail loudly on the quadratic code without relying
// on wall-clock, and they double as a correctness lock: the walk must not be
// derailed by later bytes.
#[test]
fn n_byte_grapheme_bytes_reads_only_leading_cluster() {
    use hfst::pmatch::nByte_grapheme_bytes;

    // Empty slice: no complete cluster.
    assert_eq!(nByte_grapheme_bytes(b""), 0);

    // Leading grapheme, then a long VALID tail that must not change the answer.
    let valid: &[(&str, i32)] = &[
        ("a", 1),
        ("é", 2),                  // NFC single codepoint (2 bytes)
        ("e\u{0301}", 3),          // e + combining acute = one 3-byte cluster
        ("\u{1F1F3}\u{1F1F4}", 8), // 🇳🇴 regional-indicator pair = one 8-byte cluster
        (" ", 1),
    ];
    for &(head, expected) in valid {
        let mut whole = String::from(head);
        whole.push_str(&"z".repeat(4096));
        assert_eq!(
            nByte_grapheme_bytes(whole.as_bytes()),
            expected,
            "valid leading {head:?} with a 4 KiB tail must stay {expected}"
        );
    }

    // Leading grapheme, then INVALID UTF-8 bytes. The bounded prefix is still
    // valid, so the leading grapheme is returned; the old whole-tail impl
    // returned 0 here (whole-slice from_utf8 failed).
    for &(head_bytes, expected) in &[
        (b"a" as &[u8], 1),
        (&[0xC3, 0xA5], 2), // 'é'
    ] {
        let mut buf = head_bytes.to_vec();
        buf.extend(std::iter::repeat_n(0xFFu8, 4096)); // invalid tail
        assert_eq!(
            nByte_grapheme_bytes(&buf),
            expected,
            "leading grapheme must be read past an invalid-UTF-8 tail"
        );
    }
}

// [hfst/hfst#483] End-to-end: a large single line (no newlines) must tokenize
// correctly — the whole line goes through initialize_input in one locate() call,
// the exact shape that used to hang. We assert the output is right (every
// matching token appears the expected number of times), which also exercises
// that the fixed bounded-prefix segmentation preserves matching behaviour at
// scale. Sized to stay quick with the O(n) walk.
#[test]
fn locate_large_single_line_tokenizes_correctly() -> Result<(), hfst::error::Error> {
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let defs = compiler.compile("Define TOP [{cat} | {dog}] EndTag(w) ;\n")?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    let top_owned = HfstTransducer::<Transducer<WeightedTables>>::new_from_basic(&top.to_basic()?)?;
    let mut container = PmatchContainer::new_from_hfst_transducers(vec![top_owned])?;

    let reps = 20_000usize;
    let line = "cat dog xyz ".repeat(reps); // ~240 KB, no newline

    let locations = container.locate(&line, 0.0, hfst::transducer::INFINITE_WEIGHT);

    let mut cats = 0usize;
    let mut dogs = 0usize;
    for lv in &locations {
        if let Some(loc) = lv.first() {
            match loc.output.as_str() {
                "cat" => cats += 1,
                "dog" => dogs += 1,
                _ => {}
            }
        }
    }
    assert_eq!(cats, reps, "every 'cat' token should be located");
    assert_eq!(dogs, reps, "every 'dog' token should be located");
    Ok(())
}
