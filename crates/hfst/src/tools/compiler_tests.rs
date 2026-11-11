//! The compilers the tools are made of: strings, regular expressions split
//! by line or semicolon, several lexc sources, and the format names and
//! weights they read.

use super::format::{format_description, parse_format_name, parse_weight};
use super::regexp2fst::{RegexpError, RegexpEvent, Separation, compile_regexps};
use super::strings2fst::{
    LogBase, StringsCompiler, StringsError, StringsOptions, compile_strings, multichar_symbols,
};
use super::test_support::{Tropical, paths_of, regex};
use crate::hfst_data_types::ImplementationType;
use crate::lexc::LexcCompiler;
use crate::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

fn same(t: &Tropical, src: &str) -> bool {
    t.compare(&regex(src), true).expect("tropical")
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn format_names_read_as_the_tools_read_them() {
    let parsed = |name| parse_format_name(name).map(|f| (f.implementation, f.warning(name)));
    assert_eq!(
        parsed("OFST-Tropical"),
        Some((ImplementationType::TROPICAL_OPENFST_TYPE, None))
    );
    assert_eq!(
        parsed("ol"),
        Some((
            ImplementationType::HFST_OLW_TYPE,
            Some("Ambiguous format name ol, guessing optimized-lookup-weighted".to_string())
        ))
    );
    assert_eq!(parsed("bogus"), None);
    assert_eq!(format_description(ImplementationType::FOMA_TYPE), "foma");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn weights_read_as_strtod_reads_them() {
    assert_eq!(parse_weight(""), Ok(0.0));
    assert_eq!(parse_weight(" \t2.5"), Ok(2.5));
    assert_eq!(parse_weight("2.5 "), Err("2.5 "));
    assert_eq!(parse_weight("  x"), Err("x"));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn strings_compile_one_transducer_per_line() {
    let compiled = compile_strings::<StdVectorFst>("cat\ndog\t1.5\n", StringsOptions::default())
        .expect("the lines compile");
    assert_eq!(compiled.transducers.len(), 2);
    assert_eq!(paths_of(&compiled.transducers[0]), ["cat:cat 0"]);
    assert_eq!(paths_of(&compiled.transducers[1]), ["dog:dog 1.5"]);
    assert_eq!(compiled.transducers[0].get_name(), "string()");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn disjuncted_strings_normalize_their_weights() {
    let options = StringsOptions {
        disjunct: true,
        normalize: true,
        ..StringsOptions::default()
    };
    let compiled =
        compile_strings::<StdVectorFst>("a\t1\nb\t3\n", options).expect("the lines compile");
    assert_eq!(compiled.transducers.len(), 1);
    assert_eq!(paths_of(&compiled.transducers[0]), ["a:a 0.25", "b:b 0.75"]);
    assert_eq!(compiled.transducers[0].get_name(), "strings(?)");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn pair_strings_and_logarithms_compile() {
    let options = StringsOptions {
        pairstrings: true,
        log: Some(LogBase::Ten),
        ..StringsOptions::default()
    };
    let compiled =
        compile_strings::<StdVectorFst>("c:d a\t10\n", options).expect("the line compiles");
    assert_eq!(paths_of(&compiled.transducers[0]), ["c a:d a -1"]);
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn negative_weight_is_reported_before_failing() {
    let mut compiler =
        StringsCompiler::<StdVectorFst>::new(StringsOptions::default()).expect("a compiler");
    let mut reported = Vec::new();
    let failed = compiler.compile_line("a:b:c\t-2\n", &mut |w| reported.push(w));
    assert_eq!(reported, [-2.0]);
    let Err(e @ StringsError::UnescapedColons { .. }) = failed else {
        panic!("a stray colon compiled");
    };
    assert!(
        e.to_string()
            .contains("If you are compiling pair strings, use option -p.")
    );

    let options = StringsOptions {
        warnings_are_errors: true,
        ..StringsOptions::default()
    };
    let compiled = compile_strings::<StdVectorFst>("a\n b\t-1\n", options);
    let Err(error) = compiled else {
        panic!("a negative weight passed as an error");
    };
    assert_eq!(error.line, 2);
    assert!(
        error
            .error
            .to_string()
            .starts_with("Found negative weight -1.000000;")
    );
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn bad_weights_and_logarithms_fail() {
    let compiled = compile_strings::<StdVectorFst>("a\tx\n", StringsOptions::default());
    assert_eq!(
        compiled.err().map(|e| e.error.to_string()),
        Some("x not a weight".to_string())
    );
    let options = StringsOptions {
        log: Some(LogBase::E),
        ..StringsOptions::default()
    };
    let compiled = compile_strings::<StdVectorFst>("a\t-1\n", options);
    assert!(matches!(compiled, Err(e) if matches!(e.error, StringsError::Logarithm)));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn multichar_symbols_are_tokenized_whole() {
    let symbols = multichar_symbols("+N\n\n+Pl\n");
    assert_eq!(symbols, ["+N", "+Pl"]);
    let options = StringsOptions {
        multichar_symbols: symbols,
        ..StringsOptions::default()
    };
    let compiled = compile_strings::<StdVectorFst>("cat+N\n", options).expect("compiles");
    assert!(same(&compiled.transducers[0], "{cat} %+N"));
}

/// The transducers and progress messages compiling `text` gives.
fn regexps(
    text: &str,
    separation: Separation,
    disjunct: bool,
) -> (Result<(), RegexpError>, Vec<Tropical>, Vec<String>) {
    let mut xre = XreCompiler::<StdVectorFst>::new();
    xre.set_source_name("in.xre");
    let mut compiled = Vec::new();
    let mut progress = Vec::new();
    let mut input = text.as_bytes();
    let result = compile_regexps(
        &mut xre,
        &mut input,
        separation,
        (disjunct, true),
        &mut |event| {
            match event {
                RegexpEvent::Progress(message) => progress.push(message),
                RegexpEvent::Compiled(t) => compiled.push(t),
            }
            Ok(())
        },
    );
    (result, compiled, progress)
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn regexps_split_by_line() {
    let (result, compiled, progress) = regexps("a b\n\n! comment\nc*\n", Separation::Lines, false);
    assert!(result.is_ok());
    assert_eq!(compiled.len(), 2);
    assert!(same(&compiled[0], "a b"));
    assert!(same(&compiled[1], "c*"));
    assert_eq!(compiled[0].get_name(), "xre(?)");
    assert_eq!(
        progress,
        [
            "Compiling expression 1\n",
            "Skipping whitespace expression #2",
            "Compiling expression 3\n",
            "Compiling expression 4\n"
        ]
    );
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn regexps_split_by_semicolon_and_disjunct() {
    let (result, compiled, _) = regexps("a ; b c ;", Separation::Semicolons, true);
    assert!(result.is_ok());
    assert_eq!(compiled.len(), 1);
    assert!(same(&compiled[0], "a | b c"));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn regexp_failures_say_where() {
    let (result, _, _) = regexps("a\n[b\n", Separation::Lines, false);
    assert!(matches!(result, Err(RegexpError::LineFailed { line: 2 })));
    let (result, _, _) = regexps("! only a comment\n", Separation::Lines, false);
    assert_eq!(
        result.err().map(|e| e.to_string()),
        Some("in.xre: XRE parsing failed: input contains only whitespace or comments".into())
    );
    let (result, _, _) = regexps("a ; [b ; c", Separation::Semicolons, false);
    assert_eq!(
        result.err().map(|e| e.to_string()),
        Some("in.xre: XRE parsing failed in expression #1 separated by semicolons".into())
    );
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn lexc_sources_compile_together() {
    let sources = [
        (
            "root.lexc".to_string(),
            "LEXICON Root\nNouns ;\n".to_string(),
        ),
        (
            "nouns.lexc".to_string(),
            "LEXICON Nouns\ncat # ;\n".to_string(),
        ),
    ];
    let mut lexc = LexcCompiler::<StdVectorFst>::new();
    let compiled = lexc
        .compile_named_sources(&sources)
        .expect("the sources compile");
    assert!(same(&compiled, "{cat}"));
    assert_eq!(compiled.get_name(), "lexc(root.lexc)");
    assert_eq!(compiled.get_property("formulaic-definition"), "L r");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn lexc_sources_that_fail_are_named() {
    let failed = |count: usize| {
        let sources: Vec<(String, String)> = (0..count)
            .map(|i| (format!("{i}.lexc"), "LEXICON Root\nabc # \n".to_string()))
            .collect();
        let mut lexc = LexcCompiler::<StdVectorFst>::new();
        lexc.compile_named_sources(&sources)
            .err()
            .map(|e| e.to_string())
    };
    assert_eq!(
        failed(1),
        Some("The file 0.lexc did not compile cleanly.".into())
    );
    assert_eq!(
        failed(2),
        Some("The files 0.lexc... did not compile cleanly.".into())
    );
}
