//! Faithful 1:1 port of tools/src/hfst-pair-test.cc — the twolc rule-file
//! pair-test command-line tool. Option handling is clap 4 derive through
//! [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::pair_test_driver::PairTestGrammar;
use hfst::tools::pair_test::{PairTestError, PairTester, ReportOptions, read_test_cases};
use std::io::{BufRead, Write};

/// hfst-pair-test's own options (the former tool-specific `static mut`s). The
/// C++ PAIR_TEST_FILE FILE* is replaced by opening `pair_test_file_name` as a
/// std BufRead in process_stream (the "<stdin>" sentinel selects stdin).
struct Options {
    pair_test_file_name: String,
    positive_test: bool,
    xerox_mode: bool,
}

// Open the pair-test strings file (pair_test_file_name) as a buffered reader;
// the "<stdin>"/"-"/unset name selects stdin. The std counterpart of the old
// PAIR_TEST_FILE FILE* opened with hfst_fopen.
fn pair_test_reader(name: &str) -> std::io::Result<Box<dyn BufRead>> {
    if name == "<stdin>" || name == "-" || name.is_empty() {
        Ok(Box::new(std::io::BufReader::new(std::io::stdin())))
    } else {
        Ok(Box::new(std::io::BufReader::new(std::fs::File::open(
            name,
        )?)))
    }
}

/// hfst-pair-test's command line.
// [spec:hfst:def:hfst-pair-test.parse-options-fn]
// [spec:hfst:sem:hfst-pair-test.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "pair test for a twolc rule file",
    after_help = "If SFILE is missing, the test pair strings are read from STDIN.
If OUTFILE is missing, test output is written to STDOUT.

The rule file is tested using correspondences given as pair strings,
e.g. \"e a r l y:i e r\". Lines whose first non-white-space character
is \"!\" are comment lines and skipped.

Ordinarily, positive test mode is in use. Option -N switches to
negative test mode; option -X reads both positive ('!!\u{20ac}') and
negative ('!!$') cases from a twolc source file. The exit code for a
successful test is 0 and 1 otherwise; in silent mode (-s) only the
exit code tells whether the test was successful."
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Read pair test strings from SFILE
    #[arg(
        short = 'I',
        long = "input-strings",
        value_name = "SFILE",
        allow_hyphen_values = true
    )]
    input_strings: Option<String>,

    /// Test fails if any of the pair strings is accepted
    #[arg(short = 'N', long = "negative-test")]
    negative_test: bool,

    /// In xerox mode, test cases are harvested from a twolc source file
    #[arg(short = 'X', long = "xerox-mode")]
    xerox_mode: bool,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

/// Print why a test could not run, and end the process.
fn fail(common: &CommonOptions, e: PairTestError) -> i32 {
    error(common, 1, 0, &e.to_string());
    1
}

fn read_grammar(
    common: &CommonOptions,
    inputstream: &mut HfstInputStream<'_>,
) -> Result<PairTestGrammar, i32> {
    let mut grammar = PairTestGrammar::default();
    let mut transducer_n: usize = 0;
    while inputstream.is_good() {
        transducer_n += 1;
        if transducer_n == 1 {
            verbose_print(common, &format!("Reading {}...\n", common.input_filename));
        } else {
            verbose_print(
                common,
                &format!("Reading {}...{}\n", common.input_filename, transducer_n),
            );
        }
        let rule = inputstream
            .read()
            .and_then(|t| Ok((t.to_basic()?, t.get_name())));
        match rule {
            Ok((basic, name)) => grammar.push_rule(basic, name),
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        }
    }
    inputstream.close();
    if !grammar.is_empty() {
        verbose_print(common, "Defining known symbols.\n");
        grammar.define_known_symbols();
        for it in grammar.known_symbols().iter() {
            verbose_print(common, &format!("Symbol {}\n", it));
        }
    }
    Ok(grammar)
}

// [spec:hfst:def:hfst-pair-test.process-stream-fn]
// [spec:hfst:sem:hfst-pair-test.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
// Reading the tests and testing them are the library's; the tool reads the
// files, writes the reports and turns the verdicts into its exit code.
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    inputstream: &mut HfstInputStream<'_>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    let grammar = match read_grammar(common, inputstream) {
        Ok(grammar) => grammar,
        Err(code) => return code,
    };
    let mut pair_reader = match pair_test_reader(&options.pair_test_file_name) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-pair-test: cannot open pair-test strings file: {e}");
            return 1;
        }
    };
    let report = ReportOptions {
        silent: common.silent,
        verbose: common.verbose,
    };
    let mut progress = |message: &str| verbose_print(common, message);
    let tested = if options.xerox_mode {
        test_cases(
            &grammar,
            report,
            &mut *pair_reader,
            outstream,
            &mut progress,
        )
    } else {
        test_pair_strings(
            &grammar,
            (report, options.positive_test),
            &mut *pair_reader,
            outstream,
            &mut progress,
        )
    };
    match tested {
        Ok(exit_code) => exit_code,
        Err(e) => fail(common, e),
    }
}

/// Test each pair string `input` holds; the first failing verdict, or 0.
fn test_pair_strings(
    grammar: &PairTestGrammar,
    (report, positive): (ReportOptions, bool),
    input: &mut dyn BufRead,
    outstream: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(&str),
) -> Result<i32, PairTestError> {
    let tester = PairTester::for_pair_strings(grammar, report)?;
    let mut exit_code = 0;
    let mut raw_line = String::new();
    loop {
        raw_line.clear();
        if input.read_line(&mut raw_line).unwrap_or(0) == 0 {
            break;
        }
        let line = raw_line.split('\n').next().unwrap_or_default();
        let verdict = tester.test_pair_string(line, positive, outstream, progress)?;
        if exit_code == 0 {
            exit_code = verdict.unwrap_or(0);
        }
    }
    Ok(exit_code)
}

/// Test the positive and then the negative test cases of the twolc source
/// `input`; the first failing verdict, or 0.
fn test_cases(
    grammar: &PairTestGrammar,
    report: ReportOptions,
    input: &mut dyn BufRead,
    outstream: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(&str),
) -> Result<i32, PairTestError> {
    let cases = read_test_cases(input, progress)?;
    let tester = PairTester::for_test_cases(grammar, report)?;
    let mut exit_code = 0;
    let all = cases
        .positive
        .iter()
        .map(|case| (case, true))
        .chain(cases.negative.iter().map(|case| (case, false)));
    for (case, positive) in all {
        let verdict = tester.test_case(case, positive, outstream)?;
        if exit_code == 0 {
            exit_code = verdict;
        }
    }
    Ok(exit_code)
}

// [spec:hfst:def:hfst-pair-test.main-fn]
// [spec:hfst:sem:hfst-pair-test.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.6", "HfstPairTest");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options {
        pair_test_file_name: args
            .input_strings
            .clone()
            .unwrap_or_else(|| String::from("<stdin>")),
        positive_test: !args.negative_test,
        xerox_mode: args.xerox_mode,
    };

    // The C ran this right after check-params-unary.h resolved the operand.
    if common.input_filename == "<stdin>" {
        error(
            &common,
            1,
            0,
            "The rule transducer file needs to be given using option -i.",
        );
        return Err(1);
    }

    // close buffers, we use streams
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    // here starts the buffer handling part
    let input_named = common.input_filename != "<stdin>";
    let mut instream = match if input_named {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-pair-test: cannot open output: {e}");
            return Err(1);
        }
    };

    let exit_code = process_stream(&common, &options, &mut instream, &mut *out);

    if !common.silent {
        if exit_code == 0 {
            let _ = writeln!(out, "Test passed.");
        } else {
            let _ = writeln!(out, "Test failed.");
        }
    }

    cli::from_code(exit_code)
}
