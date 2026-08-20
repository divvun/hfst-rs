//! Faithful 1:1 port of tools/src/hfst-pair-test.cc — the twolc rule-file
//! pair-test command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{StringPairVector, Symbol};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use hfst::pair_test_driver::{
    PairTestGrammar, StringVector, Verdict, add_word_boundaries, backslash_escape, unescape,
};
use std::io::{BufRead, Write};

/// hfst-pair-test's own options (the former tool-specific `static mut`s). The
/// C++ PAIR_TEST_FILE FILE* is replaced by opening `pair_test_file_name` as a
/// std BufRead in process_stream (the "<stdin>" sentinel selects stdin).
struct Options {
    pair_test_file_name: String,
    pair_test_given: bool,
    positive_test: bool,
    xerox_mode: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            pair_test_file_name: String::new(),
            pair_test_given: false,
            positive_test: true,
            xerox_mode: false,
        }
    }
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

// [spec:hfst:def:hfst-pair-test.print-usage-fn]
// [spec:hfst:sem:hfst-pair-test.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\npair test for a twolc rule file.\n\n",
        common.program_name
    );

    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Input/Output options:\n\
         \x20 -i, --input=INFILE     Read input rule file from INFILE\n\
         \x20 -o, --output=OUTFILE   Write test output to OUTFILE\n\
         \x20 -N  --negative-test    Test fails if any of the pair strings is\n\
         \x20                        accepted.\n\
         \x20 -X  --xerox-mode       In xerox mode, test cases are harvested\n\
         \x20                        from a twolc source file.\n"
    );

    let _ = write!(
        msg,
        "Pair test options:\n\
         \x20 -I, --input-strings=SFILE        Read pair test strings from\n\
         \x20                                  SFILE\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "If SFILE is missing, the test pair strings are read from STDIN.\n\
         If OUTFILE is missing, test output is written to STDOUT.\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "The rule file is tested using correspondences given as\n\
         pair strings, e.g. \"e a r l y:i e r\". Every pair string is\n\
         tested using every rule and the program prints information\n\
         about correspondences that are incorrectly allowed or\n\
         disallowed.\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "The test pair string files contain one pair string/line. Lines\n\
         where the first non-white-space character is \"!\" are\n\
         considered comment lines and skipped.\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "There are three test modes positive, negative and Xerox mode. In\n\
         positive mode, all of the pair strings should be allowed and in\n\
         negative mode they should be disallowed. In Xerox mode the cases\n\
         are read from a twolc source file and both positive and negative\n\
         cases can occur.\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "Ordinarily, positive test mode is in use. Option -N switches to\n\
         negative test mode. The exit code for a successful test is 0. \n\
         The exit code is 1 otherwise. A successful test will print\n\
         \"Test passed\". A failing test prints \"Test failed\" and\n\
         information about pair strings that are handled incorrectly.\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "In positive test mode (i.e. without option -N), if a pair\n\
         string is not accepted, the names of the rules that reject\n\
         it are printed as well as the positions in the string where the\n\
         rules run out of possible transitions. In negative mode, only\n\
         the strings that are allowed are printed.\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "In Xerox mode, the input should be a twolc file. Tests consist of\n\
         two lines: an input form and an output form. The test cases are\n\
         specialized comments prefixed with either '!!\u{20ac}' or '!!$' depeding on\n\
         whether the pair should succeed or fail. An example of a positive\n\
         test:\n\n\
         !!\u{20ac} earlYer\n\
         !!\u{20ac} earlier\n\n\
         An example of a negative test:\n\n\
         !!$ earlYer\n\
         !!$ earlyer\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "In silent mode (-s), the program won't print anything. Only the\n\
         exit code tells whether the test was successful or not.\n"
    );
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-pair-test.parse-options-fn]
// [spec:hfst:sem:hfst-pair-test.parse-options-fn]
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_unary_long());
        // add tool-specific options here
        long_options.push(getopt::GetOpt {
            name: "input-strings",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'I' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "negative-test",
            has_arg: getopt::NO_ARGUMENT,
            val: 'N' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "xerox-mode",
            has_arg: getopt::NO_ARGUMENT,
            val: 'X' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        // add tool-specific cases here
        match c as u8 as char {
            'I' => {
                options.pair_test_file_name = opt.optarg();
                options.pair_test_given = true;
                continue;
            }
            'N' => {
                options.positive_test = false;
                continue;
            }
            'X' => {
                options.xerox_mode = true;
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    if !options.pair_test_given {
        options.pair_test_file_name = String::from("<stdin>");
    }
    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);

    if common.input_filename == "<stdin>" {
        error(
            &common,
            1,
            0,
            "The rule transducer file needs to be given using option -i.",
        );
    }
    Ok((common, options))
}

// [spec:hfst:def:hfst-pair-test.print-recognized-prefix-fn]
// [spec:hfst:sem:hfst-pair-test.print-recognized-prefix-fn]
//
// Renders one rule's failure: the pairs it recognized, the 'HERE --->' marker
// at 'prefix_len', then the rest of the pair string.
fn print_recognized_prefix(
    common: &CommonOptions,
    tokenized_pair_string: &StringPairVector,
    prefix_len: usize,
    name: &str,
    outfile: &mut dyn std::io::Write,
) {
    if common.silent {
        return;
    }

    let _ = writeln!(outfile, "Rule {} fails:", name);

    let mut idx = 0;
    while idx < prefix_len {
        let it = &tokenized_pair_string[idx];
        if it.0 == it.1 {
            let _ = write!(outfile, "{} ", unescape(&it.0));
        } else {
            let _ = write!(outfile, "{}:{} ", unescape(&it.0), unescape(&it.1));
        }
        idx += 1;
    }

    let _ = write!(outfile, "HERE ---> ");

    while idx < tokenized_pair_string.len() {
        let it = &tokenized_pair_string[idx];
        if it.0 == it.1 {
            let _ = write!(outfile, "{} ", unescape(&it.0));
        } else {
            let _ = write!(outfile, "{}:{} ", unescape(&it.0), unescape(&it.1));
        }
        idx += 1;
    }
    let _ = write!(outfile, "\n\n");
}

// [spec:hfst:def:hfst-pair-test.print-failure-info-fn]
// [spec:hfst:sem:hfst-pair-test.print-failure-info-fn]
fn print_failure_info(
    common: &CommonOptions,
    tokenized_pair_string: &StringPairVector,
    grammar: &PairTestGrammar,
    index: usize,
    outfile: &mut dyn std::io::Write,
) {
    // The prefix is computed even when the tool is silent: the C++ tool ran
    // the composition before consulting the silent flag, and a failure in it
    // is reported (and exits) either way.
    let prefix_len = match grammar.recognized_prefix_length(index, tokenized_pair_string) {
        Ok(n) => n,
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
    };
    print_recognized_prefix(
        common,
        tokenized_pair_string,
        prefix_len,
        grammar.name(index),
        outfile,
    );
}

// [spec:hfst:def:hfst-pair-test.test-fn]
// [spec:hfst:sem:hfst-pair-test.test-fn]
fn test(
    common: &CommonOptions,
    tokenized_pair_string: &StringPairVector,
    pair_string: &str,
    grammar: &PairTestGrammar,
    positive: bool,
    outfile: &mut dyn std::io::Write,
) -> Verdict {
    let mut positive_exit_code: Verdict = 0;
    let mut negative_exit_code: Verdict = 1;

    for ind in 0..grammar.len() {
        let new_exit_code = grammar.test_rule(ind, tokenized_pair_string, positive);

        if positive && new_exit_code == 1 {
            print_failure_info(common, tokenized_pair_string, grammar, ind, &mut *outfile);
        }

        if positive && positive_exit_code == 0 {
            positive_exit_code = new_exit_code;
        }

        if !positive && negative_exit_code == 1 {
            negative_exit_code = new_exit_code;
        }
    }

    if positive {
        if positive_exit_code == 1 && !common.silent {
            let _ = write!(outfile, "FAIL: {} REJECTED\n\n", pair_string);
        }
        if positive_exit_code == 0 && common.verbose {
            let _ = write!(outfile, "{} PASSED\n\n", pair_string);
        }
        positive_exit_code
    } else {
        if negative_exit_code == 1 && !common.silent {
            let _ = write!(outfile, "FAIL: {} PASSED\n\n", pair_string);
        }
        if negative_exit_code == 0 && common.verbose {
            let _ = write!(outfile, "{} REJECTED\n\n", pair_string);
        }
        negative_exit_code
    }
}

// [spec:hfst:def:hfst-pair-test.is-empty-or-comment-fn]
// [spec:hfst:sem:hfst-pair-test.is-empty-or-comment-fn]
fn is_empty_or_comment(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] == b'!' {
        return true;
    }
    false
}

// [spec:hfst:def:hfst-pair-test.strip-space-fn]
// [spec:hfst:sem:hfst-pair-test.strip-space-fn]
fn strip_space(line: &str) -> String {
    // C++ strips leading/trailing ' '/'\t' via find_first/last_not_of + an
    // inclusive substr. Ported literally that panics on UTF-8: rfind on a char
    // predicate yields the START byte of the last non-space char, so an
    // inclusive byte slice `..=last` cuts through a trailing multi-byte char
    // (e.g. 'ç') mid-codepoint. trim_matches is the char-boundary-safe
    // equivalent with identical semantics (empty in → empty out).
    line.trim_matches(|c: char| c == ' ' || c == '\t')
        .to_string()
}

// [spec:hfst:def:hfst-pair-test.is-positive-test-line-fn]
// [spec:hfst:sem:hfst-pair-test.is-positive-test-line-fn]
fn is_positive_test_line(line: &str) -> bool {
    let stripped = strip_space(line);
    let marker = "!!\u{20ac}";
    stripped.len() >= marker.len() && &stripped.as_bytes()[..marker.len()] == marker.as_bytes()
}

// [spec:hfst:def:hfst-pair-test.is-negative-test-line-fn]
// [spec:hfst:sem:hfst-pair-test.is-negative-test-line-fn]
fn is_negative_test_line(line: &str) -> bool {
    let stripped = strip_space(line);
    let marker = "!!$";
    stripped.len() >= marker.len() && &stripped.as_bytes()[..marker.len()] == marker.as_bytes()
}

// substr from a byte offset, matching C++ std::string::substr semantics.
fn substr_from_bytes(s: &str, byte_off: usize) -> String {
    if byte_off >= s.len() {
        String::new()
    } else {
        s[byte_off..].to_string()
    }
}

// [spec:hfst:def:hfst-pair-test.process-stream-fn]
// [spec:hfst:sem:hfst-pair-test.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    inputstream: &mut HfstInputStream<'_>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    let mut grammar = PairTestGrammar::new();

    // Read transducers in rule file.
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
        let trans = match inputstream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // one dispatch per read: the rules only feed the basic-transducer
        // grammar ([dec:hfst:monomorphic-backends]).
        let basic = crate::for_any!(&trans, t => HfstBasicTransducer::new_from_transducer(t));
        grammar.push_rule(basic, trans.get_name());
    }

    inputstream.close();

    if !grammar.is_empty() {
        verbose_print(common, "Defining known symbols.\n");
        grammar.define_known_symbols();
        for it in grammar.known_symbols().iter() {
            verbose_print(common, &format!("Symbol {}\n", it));
        }
    }

    // Open the pair-test strings file (the std counterpart of the C++
    // PAIR_TEST_FILE FILE* read with hfst_getline). The "<stdin>" sentinel
    // selects stdin.
    let mut pair_reader = match pair_test_reader(&options.pair_test_file_name) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-pair-test: cannot open pair-test strings file: {e}");
            return 1;
        }
    };

    let mut exit_code: i32 = 0;

    if !options.xerox_mode {
        // Define tokenizer with no multi character symbols and an
        // empty epsilon representation.
        let empty_v: Vec<Symbol> = Vec::new();
        let input_tokenizer = match HfstStrings2FstTokenizer::new(&empty_v, "0") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        let mut raw_line = String::new();
        loop {
            raw_line.clear();
            // getline returns -1 at EOF; read_line returns Ok(0) at EOF.
            if pair_reader.read_line(&mut raw_line).unwrap_or(0) == 0 {
                break;
            }
            // strip a trailing newline (the C truncated the buffer at the
            // first '\n').
            let line_str = match raw_line.find('\n') {
                Some(p) => raw_line[..p].to_string(),
                None => raw_line.clone(),
            };
            if is_empty_or_comment(&line_str) {
                continue;
            }
            verbose_print(common, &format!("Pair test on {}...\n", line_str));

            let line_for_panic = line_str.clone();
            let tok_result = input_tokenizer.tokenize_pair_string(&line_str, true);

            match tok_result {
                Ok(mut tokenized_pair_string) => {
                    add_word_boundaries(&mut tokenized_pair_string);

                    let new_exit_code = test(
                        common,
                        &tokenized_pair_string,
                        &line_for_panic,
                        &grammar,
                        options.positive_test,
                        &mut *outstream,
                    );

                    if exit_code == 0 {
                        exit_code = new_exit_code;
                    }
                }
                Err(e) => {
                    if e.kind == hfst::error::ErrorKind::UnescapedColsFound {
                        error(
                            common,
                            1,
                            0,
                            &format!(
                                "The correspondence {} contains unquoted colon-symbols. If \
                                 you want to input pairs where either symbol is epsilon, \
                                 use 0 e.g. \"m a s s 0:e s\".\n",
                                line_for_panic
                            ),
                        );
                    } else {
                        error(common, 1, 0, &format!("{e}"));
                    }
                }
            }
        } // while lines in input
    } else {
        // Read test cases from a twolc source file.
        //
        // Positive test cases are prefixed by "!!\u{20ac}" and negative test
        // cases by "!!$".
        //
        // Each test case spans two lines: the input and output cases.

        let mut positive_test_cases: StringVector = Vec::new();
        let mut negative_test_cases: StringVector = Vec::new();

        let symbols: Vec<Symbol> = grammar.known_symbols().iter().map(Symbol::new).collect();

        let input_tokenizer = match HfstStrings2FstTokenizer::new(&symbols, "0") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        let mut raw_line = String::new();
        loop {
            raw_line.clear();
            if pair_reader.read_line(&mut raw_line).unwrap_or(0) == 0 {
                break;
            }
            let line_str = match raw_line.find('\n') {
                Some(p) => raw_line[..p].to_string(),
                None => raw_line.clone(),
            };

            if is_positive_test_line(&line_str) {
                // "!!\u{20ac} xyz" -> "xyz"
                let marker_len = "!!\u{20ac}".len();
                let test_case =
                    strip_space(&substr_from_bytes(&strip_space(&line_str), marker_len));

                verbose_print(common, &format!("Positive test case: {}...\n", test_case));
                positive_test_cases.push(test_case);
            } else if is_negative_test_line(&line_str) {
                // "!!$ xyz" -> "xyz"
                let marker_len = "!!$".len();
                let test_case =
                    strip_space(&substr_from_bytes(&strip_space(&line_str), marker_len));

                verbose_print(
                    common,
                    &format!("Negative test case: {} {}...\n", line_str, test_case),
                );
                negative_test_cases.push(test_case);
            } else {
                continue;
            }
        } // while lines in input
        if !positive_test_cases.len().is_multiple_of(2) {
            error(
                common,
                1,
                0,
                "Got an odd number of positive test cases. Every input string\n\
                 has to have an output string.\n",
            );
        }

        if !negative_test_cases.len().is_multiple_of(2) {
            error(
                common,
                1,
                0,
                "Got an odd number of negative test cases. Every input string\n\
                 has to have an output string.\n",
            );
        }

        let mut i = 0;
        while i < positive_test_cases.len() {
            let input_case = positive_test_cases[i].clone();
            let output_case = positive_test_cases[i + 1].clone();

            let to_tokenize = format!(
                "{}:{}",
                backslash_escape(input_case.clone()),
                backslash_escape(output_case.clone())
            );
            // We need to convert the %-escaped input and output
            // string to \-escpaed strings for input_toknizer.
            let tok_result = input_tokenizer.tokenize_string_pair(&to_tokenize, false);

            let mut test_case = match tok_result {
                Ok(tc) => tc,
                Err(e) => {
                    if e.kind == hfst::error::ErrorKind::UnescapedColsFound {
                        error(
                            common,
                            1,
                            0,
                            &format!(
                                "The correspondence {} {} contains unescaped \
                                 colon-symbols. Escape them using %.",
                                input_case, output_case
                            ),
                        );
                    } else {
                        error(common, 1, 0, &format!("{e}"));
                    }
                    unreachable!("error(1, ...) exits the process")
                }
            };
            add_word_boundaries(&mut test_case);

            let new_exit_code = test(
                common,
                &test_case,
                &format!("{} : {}", input_case, output_case),
                &grammar,
                true,
                &mut *outstream,
            );

            if exit_code == 0 {
                exit_code = new_exit_code;
            }
            i += 2;
        }

        let mut i = 0;
        while i < negative_test_cases.len() {
            let input_case = negative_test_cases[i].clone();
            let output_case = negative_test_cases[i + 1].clone();

            let to_tokenize = format!(
                "{}:{}",
                backslash_escape(input_case.clone()),
                backslash_escape(output_case.clone())
            );
            // We need to convert the %-escaped input and output
            // string to \-escpaed strings for input_toknizer.
            let tok_result = input_tokenizer.tokenize_string_pair(&to_tokenize, false);

            let mut test_case = match tok_result {
                Ok(tc) => tc,
                Err(e) => {
                    if e.kind == hfst::error::ErrorKind::UnescapedColsFound {
                        error(
                            common,
                            1,
                            0,
                            &format!(
                                "The correspondence {} {} contains unquoted \
                                 colon-symbols. Quote them using %.",
                                input_case, output_case
                            ),
                        );
                    } else {
                        error(common, 1, 0, &format!("{e}"));
                    }
                    unreachable!("error(1, ...) exits the process")
                }
            };
            add_word_boundaries(&mut test_case);

            let new_exit_code = test(
                common,
                &test_case,
                &format!("{} : {}", input_case, output_case),
                &grammar,
                false,
                &mut *outstream,
            );

            if exit_code == 0 {
                exit_code = new_exit_code;
            }
            i += 2;
        }
    }

    exit_code
}

// [spec:hfst:def:hfst-pair-test.main-fn]
// [spec:hfst:sem:hfst-pair-test.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.6", "HfstPairTest");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

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
            return 1;
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-pair-test: cannot open output: {e}");
            return 1;
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

    exit_code
}
