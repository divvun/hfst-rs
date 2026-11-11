//! `hfst pair-test`: reading the pair strings or the test cases of a twolc
//! source, testing each against a compiled grammar, and the report the tool
//! prints on each.

use std::io::{BufRead, Write};

use crate::error::ErrorKind;
use crate::hfst_data_types::{StringPairVector, Symbol};
use crate::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use crate::pair_test_driver::{
    PairTestGrammar, Verdict, add_word_boundaries, backslash_escape, unescape,
};

/// The marker a twolc source puts before a positive test case.
const POSITIVE_MARKER: &str = "!!\u{20ac}";
/// The marker a twolc source puts before a negative test case.
const NEGATIVE_MARKER: &str = "!!$";

/// Why a test could not run. Each `Display`s as the message the tool prints
/// for it.
#[derive(Clone, Debug)]
pub enum PairTestError {
    /// A pair string with a `:` that is not a pair separator.
    UnquotedColons(String),
    /// A test case with a `:` that is not escaped.
    UnescapedCaseColons {
        input: String,
        output: String,
        positive: bool,
    },
    /// An input string without its output string.
    OddCases {
        positive: bool,
    },
    Operation(crate::error::Error),
}

impl std::fmt::Display for PairTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PairTestError::UnquotedColons(line) => writeln!(
                f,
                "The correspondence {} contains unquoted colon-symbols. If you want to input pairs where either symbol is epsilon, use 0 e.g. \"m a s s 0:e s\".",
                line
            ),
            PairTestError::UnescapedCaseColons {
                input,
                output,
                positive: true,
            } => write!(
                f,
                "The correspondence {} {} contains unescaped colon-symbols. Escape them using %.",
                input, output
            ),
            PairTestError::UnescapedCaseColons {
                input,
                output,
                positive: false,
            } => write!(
                f,
                "The correspondence {} {} contains unquoted colon-symbols. Quote them using %.",
                input, output
            ),
            PairTestError::OddCases { positive } => writeln!(
                f,
                "Got an odd number of {} test cases. Every input string\nhas to have an output string.",
                if *positive { "positive" } else { "negative" }
            ),
            PairTestError::Operation(e) => write!(f, "{}", e),
        }
    }
}

impl From<crate::error::Error> for PairTestError {
    fn from(e: crate::error::Error) -> Self {
        PairTestError::Operation(e)
    }
}

// [spec:hfst:def:hfst-pair-test.is-empty-or-comment-fn]
// [spec:hfst:sem:hfst-pair-test.is-empty-or-comment-fn]
// [spec:hfst:req:embed.tools-in-library]
/// Whether a line of pair strings is blank or a `!` comment.
pub fn is_empty_or_comment(line: &str) -> bool {
    line.trim_start_matches([' ', '\t'])
        .chars()
        .next()
        .is_none_or(|c| c == '!')
}

// [spec:hfst:def:hfst-pair-test.is-positive-test-line-fn]
// [spec:hfst:sem:hfst-pair-test.is-positive-test-line-fn]
// [spec:hfst:def:hfst-pair-test.is-negative-test-line-fn]
// [spec:hfst:sem:hfst-pair-test.is-negative-test-line-fn]
// [spec:hfst:def:hfst-pair-test.strip-space-fn]
// [spec:hfst:sem:hfst-pair-test.strip-space-fn]
fn test_case_after<'l>(line: &'l str, marker: &str) -> Option<&'l str> {
    let blanks = [' ', '\t'];
    line.trim_matches(blanks)
        .strip_prefix(marker)
        .map(|case| case.trim_matches(blanks))
}

/// The test cases a twolc source holds: each an input string and an output
/// string, from consecutive marked lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestCases {
    pub positive: Vec<(String, String)>,
    pub negative: Vec<(String, String)>,
}

fn pair_up(cases: Vec<String>, positive: bool) -> Result<Vec<(String, String)>, PairTestError> {
    if !cases.len().is_multiple_of(2) {
        return Err(PairTestError::OddCases { positive });
    }
    Ok(cases
        .chunks(2)
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect())
}

// [spec:hfst:req:embed.tools-in-library]
/// The test cases in the twolc source `input`, telling `progress` of each.
pub fn read_test_cases(
    input: &mut dyn BufRead,
    progress: &mut dyn FnMut(&str),
) -> Result<TestCases, PairTestError> {
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    let mut raw = String::new();
    loop {
        raw.clear();
        if input.read_line(&mut raw).unwrap_or(0) == 0 {
            break;
        }
        let line = raw.split('\n').next().unwrap_or_default();
        if let Some(case) = test_case_after(line, POSITIVE_MARKER) {
            progress(&format!("Positive test case: {}...\n", case));
            positive.push(case.to_string());
        } else if let Some(case) = test_case_after(line, NEGATIVE_MARKER) {
            progress(&format!("Negative test case: {} {}...\n", line, case));
            negative.push(case.to_string());
        }
    }
    Ok(TestCases {
        positive: pair_up(positive, true)?,
        negative: pair_up(negative, false)?,
    })
}

/// What the report on a test includes.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReportOptions {
    /// Report nothing.
    pub silent: bool,
    /// Report the tests that pass too.
    pub verbose: bool,
}

/// Tests strings against a grammar's rules and reports on each.
pub struct PairTester<'g> {
    grammar: &'g PairTestGrammar,
    tokenizer: HfstStrings2FstTokenizer,
    report: ReportOptions,
}

impl<'g> PairTester<'g> {
    // [spec:hfst:req:embed.tools-in-library]
    /// A tester of pair strings, which spell each symbol on its own.
    pub fn for_pair_strings(
        grammar: &'g PairTestGrammar,
        report: ReportOptions,
    ) -> crate::error::Result<Self> {
        Ok(Self {
            grammar,
            tokenizer: HfstStrings2FstTokenizer::new(&Vec::new(), "0")?,
            report,
        })
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// A tester of a twolc source's test cases, which spell the grammar's
    /// multicharacter symbols run together.
    pub fn for_test_cases(
        grammar: &'g PairTestGrammar,
        report: ReportOptions,
    ) -> crate::error::Result<Self> {
        let symbols: Vec<Symbol> = grammar.known_symbols().iter().map(Symbol::new).collect();
        Ok(Self {
            grammar,
            tokenizer: HfstStrings2FstTokenizer::new(&symbols, "0")?,
            report,
        })
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// Test the pair string `line`, writing the report to `out`; `None` for
    /// a blank or comment line, which is not a test.
    pub fn test_pair_string(
        &self,
        line: &str,
        positive: bool,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Option<Verdict>, PairTestError> {
        if is_empty_or_comment(line) {
            return Ok(None);
        }
        progress(&format!("Pair test on {}...\n", line));
        let mut pairs = self
            .tokenizer
            .tokenize_pair_string(line, true)
            .map_err(|e| {
                if e.kind == ErrorKind::UnescapedColsFound {
                    PairTestError::UnquotedColons(line.to_string())
                } else {
                    PairTestError::Operation(e)
                }
            })?;
        add_word_boundaries(&mut pairs);
        self.test(&pairs, line, positive, out).map(Some)
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// Test the test case of `input` and `output`, `%`-escaped, writing the
    /// report to `out`.
    pub fn test_case(
        &self,
        (input, output): &(String, String),
        positive: bool,
        out: &mut dyn Write,
    ) -> Result<Verdict, PairTestError> {
        let spelled = format!(
            "{}:{}",
            backslash_escape(input.clone()),
            backslash_escape(output.clone())
        );
        let mut pairs = self
            .tokenizer
            .tokenize_string_pair(&spelled, false)
            .map_err(|e| {
                if e.kind == ErrorKind::UnescapedColsFound {
                    PairTestError::UnescapedCaseColons {
                        input: input.clone(),
                        output: output.clone(),
                        positive,
                    }
                } else {
                    PairTestError::Operation(e)
                }
            })?;
        add_word_boundaries(&mut pairs);
        self.test(&pairs, &format!("{} : {}", input, output), positive, out)
    }

    // [spec:hfst:def:hfst-pair-test.test-fn]
    // [spec:hfst:sem:hfst-pair-test.test-fn]
    fn test(
        &self,
        pairs: &StringPairVector,
        shown: &str,
        positive: bool,
        out: &mut dyn Write,
    ) -> Result<Verdict, PairTestError> {
        let mut positive_verdict: Verdict = 0;
        let mut negative_verdict: Verdict = 1;
        for rule in 0..self.grammar.len() {
            let verdict = self.grammar.test_rule(rule, pairs, positive);
            if positive && verdict == 1 {
                self.write_failure(pairs, rule, out)?;
            }
            if positive && positive_verdict == 0 {
                positive_verdict = verdict;
            }
            if !positive && negative_verdict == 1 {
                negative_verdict = verdict;
            }
        }
        let (verdict, failed, passed) = if positive {
            (positive_verdict, "REJECTED", "PASSED")
        } else {
            (negative_verdict, "PASSED", "REJECTED")
        };
        // As in the C++ tool, a report that fails to write does not fail the
        // test.
        if verdict == 1 && !self.report.silent {
            let _ = write!(out, "FAIL: {} {}\n\n", shown, failed);
        }
        if verdict == 0 && self.report.verbose {
            let _ = write!(out, "{} {}\n\n", shown, passed);
        }
        Ok(verdict)
    }

    // [spec:hfst:def:hfst-pair-test.print-failure-info-fn]
    // [spec:hfst:sem:hfst-pair-test.print-failure-info-fn]
    // [spec:hfst:def:hfst-pair-test.print-recognized-prefix-fn]
    // [spec:hfst:sem:hfst-pair-test.print-recognized-prefix-fn]
    /// Report where `rule` stopped accepting `pairs`. The prefix is worked
    /// out even when silent, as the C++ tool did, so a failure working it
    /// out is an error either way.
    fn write_failure(
        &self,
        pairs: &StringPairVector,
        rule: usize,
        out: &mut dyn Write,
    ) -> Result<(), PairTestError> {
        let prefix_len = self.grammar.recognized_prefix_length(rule, pairs)?;
        if self.report.silent {
            return Ok(());
        }
        let mut text = format!("Rule {} fails:\n", self.grammar.name(rule));
        for (i, (input, output)) in pairs.iter().enumerate() {
            if i == prefix_len {
                text.push_str("HERE ---> ");
            }
            if input == output {
                text.push_str(&format!("{} ", unescape(input)));
            } else {
                text.push_str(&format!("{}:{} ", unescape(input), unescape(output)));
            }
        }
        if prefix_len >= pairs.len() {
            text.push_str("HERE ---> ");
        }
        text.push_str("\n\n");
        let _ = out.write_all(text.as_bytes());
        Ok(())
    }
}
