//! The queries the tools report on: summarize, check-alpha, grep and
//! pair-test, as data and as the text the tools print.

use super::check_alpha::AlphabetComparison;
use super::grep::{Dialect, GrepOptions, LineMatch, Matcher, PatternError};
use super::pair_test::{
    PairTestError, PairTester, ReportOptions, is_empty_or_comment, read_test_cases,
};
use super::summarize::{Summary, SummaryOptions, write_total};
use super::test_support::{read_all, regex, stream_bytes};
use crate::hfst_data_types::ImplementationType;
use crate::hfst_transducer::AnyTransducer;
use crate::pair_test_driver::PairTestGrammar;
use crate::twolc::TwolcCompiler;
use crate::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

fn named(src: &str) -> AnyTransducer {
    let mut t = regex(src);
    t.set_name("xre(?)");
    AnyTransducer::Tropical(t)
}

fn text(write: impl FnOnce(&mut Vec<u8>)) -> String {
    let mut out = Vec::new();
    write(&mut out);
    String::from_utf8(out).expect("utf8")
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn summary_reports_what_the_tool_prints() {
    let summary = Summary::of(&named("a:b c"), false).expect("a summary");
    assert_eq!(summary.stats.states, 3);
    assert_eq!(summary.stats.arcs, 2);
    let options = SummaryOptions {
        verbose: false,
        symbol_pairs: Some(-1),
    };
    let report = text(|out| {
        summary.write_report(out, 1, &options).expect("written");
        write_total(out, 1).expect("written");
    });
    let expected_start = "name: \"xre(?)\"\nfst type: OpenFST\narc type: tropical\n\
        input symbol table: yes\noutput symbol table: yes\n# of states: 3\n# of arcs: 2\n\
        initial state: 0\n# of final states: 1\n";
    assert!(report.starts_with(expected_start), "{report}");
    assert!(
        report.contains("acceptor: no\ninput deterministic: yes\n"),
        "{report}"
    );
    assert!(
        report.ends_with("symbol pairs:\na:b\t1\nc:c\t1\n\n\nRead 1 transducers in total.\n"),
        "{report}"
    );
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn detailed_summary_reports_its_statistics() {
    let summary = Summary::of(&named("a:b c"), true).expect("a summary");
    let options = SummaryOptions {
        verbose: true,
        symbol_pairs: None,
    };
    let report = text(|out| summary.write_report(out, 2, &options).expect("written"));
    assert!(report.starts_with("-- \nTransducer #2:\n"), "{report}");
    let expected = "number of arcs in sparsest state: 0\n\
        number of arcs in densest state: 1\n\
        average arcs per state: 0.666667\n\
        average input epsilons per state: 0.000000\n\
        most ambiguous input: a 1\n\
        most ambiguous output: b 1\n\
        average input ambiguity: 1.000000\n\
        average output ambiguity: 1.000000\n\
        expected arcs per symbol: 0.222222\n\
        infinitely ambiguous: no\n\
        sigma set:\n\
        @_EPSILON_SYMBOL_@, @_IDENTITY_SYMBOL_@, @_UNKNOWN_SYMBOL_@, a, b, c\n\
        arc symbols actually seen in transducer:\n\
        a, b, c\n\
        sigma symbols missing from transducer:\n\
        @_EPSILON_SYMBOL_@, @_IDENTITY_SYMBOL_@, @_UNKNOWN_SYMBOL_@\n\
        first input symbols:\n\
        a, c\n";
    assert!(report.ends_with(expected), "{report}");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn alphabet_comparison_reports_what_the_tool_prints() {
    let comparison = AlphabetComparison::of(&named("a:b c"), &named("a:b")).expect("compared");
    assert!(comparison.mismatch());
    let report = text(|out| comparison.write_report(out, true).expect("written"));
    let expected = "Actual alphabet differences:\n\
        In first xre(?) but not in second xre(?):c\n\
        Second xre(?) alpha is superset of second xre(?).\n\
        xre(?) alphabet:a, b, c\n\
        xre(?) alphabet:a, b\n\
        sigma set difference:\n\
        First xre(?) has but second xre(?) does not: c\n\
        Second xre(?) alpha is superset of first xre(?).\n\
        First (xre(?)):@_EPSILON_SYMBOL_@, @_IDENTITY_SYMBOL_@, @_UNKNOWN_SYMBOL_@, a, b, c\n\
        Second (xre(?)):@_EPSILON_SYMBOL_@, @_IDENTITY_SYMBOL_@, @_UNKNOWN_SYMBOL_@, a, b\n";
    assert_eq!(report, expected);
    let same = AlphabetComparison::of(&named("a"), &named("a")).expect("compared");
    assert!(!same.mismatch());
}

fn grep_options() -> GrepOptions {
    GrepOptions {
        dialect: Dialect::Xerox,
        colour: false,
        match_word: false,
        match_full_line: false,
        invert: false,
        max_count: 0,
        print_filenames: false,
        filename_null: false,
        print_line_numbers: false,
        only_filenames: false,
        flush_lines: false,
    }
}

fn matcher(pattern: &str, options: &GrepOptions) -> Result<Matcher, PatternError> {
    let mut xre = XreCompiler::<StdVectorFst>::new();
    let mut matcher = Matcher::compile(pattern, options.dialect, &mut xre, &mut |_| {})?;
    matcher.extend(options, &mut |_| {})?;
    Ok(matcher)
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn grep_marks_the_matches_in_a_line() {
    let options = grep_options();
    let m = matcher("c a t", &options).expect("the pattern compiles");
    assert_eq!(
        m.match_line("a cat sat").expect("matched"),
        LineMatch::Matched("a {{{cat}}} sat".to_string())
    );
    assert_eq!(
        m.match_line("a dog").expect("matched"),
        LineMatch::Unmatched("a dog".to_string())
    );
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn grep_prints_lines_as_options_say() {
    let options = GrepOptions {
        print_filenames: true,
        print_line_numbers: true,
        max_count: 1,
        ..grep_options()
    };
    let m = matcher("c a t", &options).expect("the pattern compiles");
    let mut out = Vec::new();
    let mut input = "dog\ncat\ncatcat\n".as_bytes();
    let matched = m
        .match_lines(&mut input, "f.txt", &options, &mut out, &mut |_| {})
        .expect("matched");
    assert!(matched);
    assert_eq!(
        String::from_utf8(out).expect("utf8"),
        "f.txt: 2: {{{cat}}}\n"
    );

    let inverted = GrepOptions {
        invert: true,
        ..grep_options()
    };
    let mut out = Vec::new();
    let mut input = "dog\ncat\n".as_bytes();
    let matched = m
        .match_lines(&mut input, "f.txt", &inverted, &mut out, &mut |_| {})
        .expect("matched");
    assert!(!matched);
    assert_eq!(String::from_utf8(out).expect("utf8"), "dog\n");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn grep_patterns_that_fail_say_why() {
    let options = grep_options();
    let messages: Vec<String> = ["", "[a"]
        .iter()
        .map(|p| {
            matcher(p, &options)
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(
        messages,
        [
            "empty pattern: XRE parsing failed (an empty Xerox regular expression is not valid)",
            "XRE parsing failed: [a"
        ]
    );
    let fixed = GrepOptions {
        dialect: Dialect::FixedStrings,
        ..grep_options()
    };
    let m = matcher("at", &fixed).expect("a fixed string compiles");
    assert!(matches!(m.match_line("cat"), Ok(LineMatch::Matched(_))));
}

/// The grammar of a twolc source, its rules as the twolc tool writes them.
fn grammar(source: &str) -> PairTestGrammar {
    let bytes = stream_bytes(ImplementationType::TROPICAL_OPENFST_TYPE, |out| {
        TwolcCompiler::<StdVectorFst>::new()
            .compile_and_store(source, out)
            .expect("the grammar compiles");
    });
    let mut grammar = PairTestGrammar::default();
    for rule in read_all(&bytes) {
        grammar.push_rule(rule.to_basic().expect("a basic rule"), rule.get_name());
    }
    grammar.define_known_symbols();
    grammar
}

const RULES: &str = "Alphabet a b c ;\nRules\n\"r1\" a:b <=> _ c ;\n";

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn pair_strings_are_tested_and_reported() {
    let grammar = grammar(RULES);
    let report = ReportOptions {
        silent: false,
        verbose: true,
    };
    let tester = PairTester::for_pair_strings(&grammar, report).expect("a tester");
    let mut out = Vec::new();
    let verdicts: Vec<Option<i32>> = ["! comment", "a:b c", "a c"]
        .iter()
        .map(|line| {
            tester
                .test_pair_string(line, true, &mut out, &mut |_| {})
                .expect("tested")
        })
        .collect();
    assert_eq!(verdicts, [None, Some(0), Some(1)]);
    let printed = String::from_utf8(out).expect("utf8");
    assert!(
        printed.starts_with("a:b c PASSED\n\nRule \"r1\" fails:\n"),
        "{printed}"
    );
    assert!(printed.contains("HERE ---> a c #:0 \n\n"), "{printed}");
    assert!(printed.ends_with("FAIL: a c REJECTED\n\n"), "{printed}");
    let Err(e) = tester.test_pair_string("a:b:c", true, &mut Vec::new(), &mut |_| {}) else {
        panic!("a stray colon was tested");
    };
    assert!(matches!(e, PairTestError::UnquotedColons(_)));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn twolc_test_cases_pair_up() {
    assert!(is_empty_or_comment("  \t! note"));
    assert!(!is_empty_or_comment(" a"));
    let source = "!!€ ac\n!!€  bc \nx\n!!$ ac\n!!$ ac\n";
    let cases = read_test_cases(&mut source.as_bytes(), &mut |_| {}).expect("cases");
    assert_eq!(cases.positive, [("ac".to_string(), "bc".to_string())]);
    assert_eq!(cases.negative, [("ac".to_string(), "ac".to_string())]);
    let grammar = grammar(RULES);
    let tester = PairTester::for_test_cases(&grammar, ReportOptions::default()).expect("a tester");
    let mut out = Vec::new();
    let verdict = tester
        .test_case(&cases.positive[0], true, &mut out)
        .expect("tested");
    assert_eq!(verdict, 0);
    let odd = read_test_cases(&mut "!!€ a\n".as_bytes(), &mut |_| {});
    assert!(matches!(
        odd,
        Err(PairTestError::OddCases { positive: true })
    ));
}
