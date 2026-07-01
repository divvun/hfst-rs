//! Faithful 1:1 port of tools/src/hfst-pair-test.cc — the twolc rule-file
//! pair-test command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, inc fragments).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::implementations::HfstState;
use hfst::hfst_data_types::{ImplementationType, StringPairVector};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_strings2_fst_tokenizer::{HfstStrings2FstTokenizer, UnescapedColsFound};
use hfst::hfst_symbol_defs::{internal_epsilon, is_epsilon};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::collections::BTreeSet;
use std::io::{BufRead, Write};

// [spec:hfst:def:hfst-pair-test.basic-transducer-vector]
type BasicTransducerVector = Vec<HfstBasicTransducer>;
// [spec:hfst:def:hfst-pair-test.string-vector]
type StringVector = Vec<String>;
// [spec:hfst:def:hfst-pair-test.symbol-set]
type SymbolSet = BTreeSet<String>;

// Tool-specific static state (file-scope statics in the C++ source). The C++
// PAIR_TEST_FILE FILE* is replaced by opening PAIR_TEST_FILE_NAME as a std
// BufRead in process_stream (the "<stdin>" sentinel selects stdin).
static mut PAIR_TEST_FILE_NAME: String = String::new();
static mut PAIR_TEST_GIVEN: bool = false;
static mut POSITIVE_TEST: bool = true;
static mut XEROX_MODE: bool = false;

fn pair_test_file_name() -> String {
    unsafe { (*std::ptr::addr_of!(PAIR_TEST_FILE_NAME)).clone() }
}

// Open the pair-test strings file (PAIR_TEST_FILE_NAME) as a buffered reader;
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
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\npair test for a twolc rule file.\n\n",
        globals::program_name()
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
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "If SFILE is missing, the test pair strings are read from STDIN.\n\
         If OUTFILE is missing, test output is written to STDOUT.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "The rule file is tested using correspondences given as\n\
         pair strings, e.g. \"e a r l y:i e r\". Every pair string is\n\
         tested using every rule and the program prints information\n\
         about correspondences that are incorrectly allowed or\n\
         disallowed.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "The test pair string files contain one pair string/line. Lines\n\
         where the first non-white-space character is \"!\" are\n\
         considered comment lines and skipped.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "There are three test modes positive, negative and Xerox mode. In\n\
         positive mode, all of the pair strings should be allowed and in\n\
         negative mode they should be disallowed. In Xerox mode the cases\n\
         are read from a twolc source file and both positive and negative\n\
         cases can occur.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "Ordinarily, positive test mode is in use. Option -N switches to\n\
         negative test mode. The exit code for a successful test is 0. \n\
         The exit code is 1 otherwise. A successful test will print\n\
         \"Test passed\". A failing test prints \"Test failed\" and\n\
         information about pair strings that are handled incorrectly.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "In positive test mode (i.e. without option -N), if a pair\n\
         string is not accepted, the names of the rules that reject\n\
         it are printed as well as the positions in the string where the\n\
         rules run out of possible transitions. In negative mode, only\n\
         the strings that are allowed are printed.\n"
    );
    let _ = write!(msg, "\n");
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
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "In silent mode (-s), the program won't print anything. Only the\n\
         exit code tells whether the test was successful or not.\n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-pair-test.parse-options-fn]
// [spec:hfst:sem:hfst-pair-test.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            // add tool-specific cases here
            match c as u8 as char {
                'I' => {
                    *std::ptr::addr_of_mut!(PAIR_TEST_FILE_NAME) = getopt::optarg();
                    PAIR_TEST_GIVEN = true;
                    continue;
                }
                'N' => {
                    POSITIVE_TEST = false;
                    continue;
                }
                'X' => {
                    XEROX_MODE = true;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        if !PAIR_TEST_GIVEN {
            *std::ptr::addr_of_mut!(PAIR_TEST_FILE_NAME) = String::from("<stdin>");
        }
        check_common_params();
        check_unary_params(args);

        if globals::input_filename() == "<stdin>" {
            error(
                1,
                0,
                "The rule transducer file needs to be given using option -i.",
            );
        }
        EXIT_CONTINUE
    }
}

// replace every occurrence of substr in str with repl, in place.
fn replace_all_substr(substr: &str, repl: &str, str: &mut String) {
    let mut pos = 0;
    while let Some(found) = str[pos..].find(substr) {
        let at = pos + found;
        str.replace_range(at..at + substr.len(), repl);
        pos = at + repl.len();
    }
}

const PTPP: &str = "PAIR_TEST_PERC_PERC";
const PTPC: &str = "PAIR_TEST_PERC_COL";

// perc_escaped is a string where special symols are escaped using
// %. Transform it into a string where specail symbols are escaped
// using \.
// [spec:hfst:def:hfst-pair-test.backslash-escape-fn]
// [spec:hfst:sem:hfst-pair-test.backslash-escape-fn]
fn backslash_escape(mut perc_escaped: String) -> String {
    replace_all_substr("%%", PTPP, &mut perc_escaped);
    replace_all_substr("%:", PTPC, &mut perc_escaped);
    replace_all_substr("%", "", &mut perc_escaped);
    replace_all_substr(PTPC, "\\:", &mut perc_escaped);
    replace_all_substr(PTPP, "%", &mut perc_escaped);
    perc_escaped
}

// [spec:hfst:def:hfst-pair-test.get-target-fn]
// [spec:hfst:sem:hfst-pair-test.get-target-fn]
fn get_target(
    isymbol: &str,
    osymbol: &str,
    s: HfstState,
    t: &HfstBasicTransducer,
    known_symbols: &SymbolSet,
) -> HfstState {
    t.pair_target_state(s, isymbol, osymbol, known_symbols)
        .unwrap_or(u32::MAX)
}

// [spec:hfst:def:hfst-pair-test.is-final-state-fn]
// [spec:hfst:sem:hfst-pair-test.is-final-state-fn]
fn is_final_state(s: HfstState, t: &HfstBasicTransducer) -> bool {
    t.is_final_state(s)
}

// One-rule test (overload of test). Mirrors the C++ 5-argument test().
fn test_rule(
    tokenized_pair_string: &StringPairVector,
    t: &HfstBasicTransducer,
    positive: bool,
    _outfile: &mut dyn std::io::Write,
    known_symbols: &SymbolSet,
) -> i32 {
    let mut s: HfstState = 0;
    for it in tokenized_pair_string.iter() {
        s = get_target(&it.0, &it.1, s, t, known_symbols);
        if s == u32::MAX {
            if positive {
                return 1;
            } else {
                return 0;
            }
        }
    }

    if is_final_state(s, t) && positive {
        0
    } else if positive {
        1
    } else if !is_final_state(s, t) {
        0
    } else {
        1
    }
}

// [spec:hfst:def:hfst-pair-test.get-transducer-fn]
// [spec:hfst:sem:hfst-pair-test.get-transducer-fn]
fn get_transducer(tokenized_pair_string: &StringPairVector) -> HfstTransducer {
    let mut t = HfstBasicTransducer::new();
    let mut s: HfstState = 0;
    for it in tokenized_pair_string.iter() {
        let target = t.add_state_new();
        let tr = HfstBasicTransition::new_symbols(
            target,
            it.0.clone(),
            it.1.clone(),
            0.0,
            t.coder_mut(),
        );
        t.add_transition(s, &tr, true);
        s = target;
    }
    t.set_final_weight(s, &0.0);
    match HfstTransducer::new_from_basic(&t, ImplementationType::TROPICAL_OPENFST_TYPE) {
        Ok(v) => v,
        Err(e) => {
            error(1, 0, &format!("{e}"));
            unreachable!()
        }
    }
}

// [spec:hfst:def:hfst-pair-test.unescape-fn]
// [spec:hfst:sem:hfst-pair-test.unescape-fn]
fn unescape(symbol: String) -> String {
    if is_epsilon(&symbol) {
        return "0".to_string();
    }
    if symbol == "@#@" {
        return "#".to_string();
    }
    symbol
}

// [spec:hfst:def:hfst-pair-test.print-recognized-prefix-fn]
// [spec:hfst:sem:hfst-pair-test.print-recognized-prefix-fn]
unsafe fn print_recognized_prefix(
    tokenized_pair_string: &StringPairVector,
    str_transducer: &HfstBasicTransducer,
    name: &str,
    outfile: &mut dyn std::io::Write,
    known_symbols: &SymbolSet,
) {
    unsafe {
        if globals::SILENT {
            return;
        }

        let _ = write!(outfile, "Rule {} fails:\n", name);

        let mut s: HfstState = 0;
        let mut idx = 0;
        while idx < tokenized_pair_string.len() {
            let it = &tokenized_pair_string[idx];
            s = get_target(&it.0, &it.1, s, str_transducer, known_symbols);

            if s == u32::MAX {
                break;
            }

            if it.0 == it.1 {
                let _ = write!(outfile, "{} ", unescape(it.0.clone()));
            } else {
                let _ = write!(
                    outfile,
                    "{}:{} ",
                    unescape(it.0.clone()),
                    unescape(it.1.clone())
                );
            }
            idx += 1;
        }

        let _ = write!(outfile, "HERE ---> ");

        while idx < tokenized_pair_string.len() {
            let it = &tokenized_pair_string[idx];
            if it.0 == it.1 {
                let _ = write!(outfile, "{} ", unescape(it.0.clone()));
            } else {
                let _ = write!(
                    outfile,
                    "{}:{} ",
                    unescape(it.0.clone()),
                    unescape(it.1.clone())
                );
            }
            idx += 1;
        }
        let _ = write!(outfile, "\n\n");
    }
}

// [spec:hfst:def:hfst-pair-test.print-failure-info-fn]
// [spec:hfst:sem:hfst-pair-test.print-failure-info-fn]
unsafe fn print_failure_info(
    tokenized_pair_string: &StringPairVector,
    t: &HfstBasicTransducer,
    name: &str,
    outfile: &mut dyn std::io::Write,
    known_symbols: &SymbolSet,
) {
    unsafe {
        let mut str_transducer = get_transducer(tokenized_pair_string);
        let tt = match HfstTransducer::new_from_basic(t, ImplementationType::TROPICAL_OPENFST_TYPE)
        {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return;
            }
        };
        if let Err(e) = str_transducer.input_project() {
            error(1, 0, &format!("{e}"));
            return;
        }
        if let Err(e) = str_transducer.compose(&tt, true) {
            error(1, 0, &format!("{e}"));
            return;
        }
        if let Err(e) = str_transducer.minimize() {
            error(1, 0, &format!("{e}"));
            return;
        }
        let basic = HfstBasicTransducer::new_from_transducer(&str_transducer);
        print_recognized_prefix(tokenized_pair_string, &basic, name, outfile, known_symbols);
    }
}

// [spec:hfst:def:hfst-pair-test.test-fn]
// [spec:hfst:sem:hfst-pair-test.test-fn]
unsafe fn test(
    tokenized_pair_string: &StringPairVector,
    pair_string: &str,
    grammar: &BasicTransducerVector,
    names: &StringVector,
    positive: bool,
    outfile: &mut dyn std::io::Write,
    known_symbols: &SymbolSet,
) -> i32 {
    unsafe {
        let mut positive_exit_code: i32 = 0;
        let mut negative_exit_code: i32 = 1;

        let mut ind: usize = 0;

        for it in grammar.iter() {
            let new_exit_code = test_rule(
                tokenized_pair_string,
                it,
                positive,
                &mut *outfile,
                known_symbols,
            );

            if positive && new_exit_code == 1 {
                print_failure_info(
                    tokenized_pair_string,
                    it,
                    &names[ind],
                    &mut *outfile,
                    known_symbols,
                );
            }

            if positive && positive_exit_code == 0 {
                positive_exit_code = new_exit_code;
            }

            if !positive && negative_exit_code == 1 {
                negative_exit_code = new_exit_code;
            }

            ind += 1;
        }

        if positive {
            if positive_exit_code == 1 && !globals::SILENT {
                let _ = write!(outfile, "FAIL: {} REJECTED\n\n", pair_string);
            }
            if positive_exit_code == 0 && globals::VERBOSE {
                let _ = write!(outfile, "{} PASSED\n\n", pair_string);
            }
            return positive_exit_code;
        } else {
            if negative_exit_code == 1 && !globals::SILENT {
                let _ = write!(outfile, "FAIL: {} PASSED\n\n", pair_string);
            }
            if negative_exit_code == 0 && globals::VERBOSE {
                let _ = write!(outfile, "{} REJECTED\n\n", pair_string);
            }
            return negative_exit_code;
        }
    }
}

// [spec:hfst:def:hfst-pair-test.demangle-fn]
// [spec:hfst:sem:hfst-pair-test.demangle-fn]
fn demangle(mut name: String) -> String {
    let space_subst = "__HFST_TWOLC_SPACE";
    let name_subst = "__HFST_TWOLC_RULE_NAME=";

    while let Some(pos) = name.find(name_subst) {
        name.replace_range(pos..pos + name_subst.len(), "");
    }

    while let Some(pos) = name.find(space_subst) {
        name.replace_range(pos..pos + space_subst.len(), " ");
    }

    name
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

// [spec:hfst:def:hfst-pair-test.get-symbols-fn]
// [spec:hfst:sem:hfst-pair-test.get-symbols-fn]
fn get_symbols(t: &HfstBasicTransducer, known_symbols: &mut SymbolSet) {
    known_symbols.extend(t.symbols_used());
}

// [spec:hfst:def:hfst-pair-test.strip-space-fn]
// [spec:hfst:sem:hfst-pair-test.strip-space-fn]
fn strip_space(line: &str) -> String {
    let first = line.find(|c: char| c != ' ' && c != '\t');
    let first_non_white_space_pos = match first {
        None => return String::new(),
        Some(p) => p,
    };
    let last_non_white_space_pos = line.rfind(|c: char| c != ' ' && c != '\t').unwrap();
    line[first_non_white_space_pos..=last_non_white_space_pos].to_string()
}

// [spec:hfst:def:hfst-pair-test.is-positive-test-line-fn]
// [spec:hfst:sem:hfst-pair-test.is-positive-test-line-fn]
fn is_positive_test_line(line: &str) -> bool {
    let stripped = strip_space(line);
    let marker = "!!\u{20ac}";
    stripped.as_bytes().len() >= marker.len()
        && &stripped.as_bytes()[..marker.len()] == marker.as_bytes()
}

// [spec:hfst:def:hfst-pair-test.is-negative-test-line-fn]
// [spec:hfst:sem:hfst-pair-test.is-negative-test-line-fn]
fn is_negative_test_line(line: &str) -> bool {
    let stripped = strip_space(line);
    let marker = "!!$";
    stripped.as_bytes().len() >= marker.len()
        && &stripped.as_bytes()[..marker.len()] == marker.as_bytes()
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
unsafe fn process_stream(
    inputstream: &mut HfstInputStream,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    unsafe {
        let mut grammar: BasicTransducerVector = Vec::new();
        let mut rule_names: StringVector = Vec::new();

        // Read transducers in rule file.
        let mut transducer_n: usize = 0;
        while inputstream.is_good() {
            transducer_n += 1;
            if transducer_n == 1 {
                verbose_printf(&format!("Reading {}...\n", globals::input_filename()));
            } else {
                verbose_printf(&format!(
                    "Reading {}...{}\n",
                    globals::input_filename(),
                    transducer_n
                ));
            }
            let trans = match HfstTransducer::new_from_stream(inputstream) {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let basic = HfstBasicTransducer::new_from_transducer(&trans);
            grammar.push(basic);
            rule_names.push(demangle(trans.get_name()));
        }

        inputstream.close();

        let mut known_symbols: SymbolSet = BTreeSet::new();
        if !grammar.is_empty() {
            verbose_printf("Defining known symbols.\n");
            get_symbols(&grammar[0], &mut known_symbols);
            for it in known_symbols.iter() {
                verbose_printf(&format!("Symbol {}\n", it));
            }
        }

        // Open the pair-test strings file (the std counterpart of the C++
        // PAIR_TEST_FILE FILE* read with hfst_getline). The "<stdin>" sentinel
        // selects stdin.
        let mut pair_reader = match pair_test_reader(&pair_test_file_name()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-pair-test: cannot open pair-test strings file: {e}");
                return 1;
            }
        };

        let mut exit_code: i32 = 0;

        if !XEROX_MODE {
            // Define tokenizer with no multi character symbols and an
            // empty epsilon representation.
            let empty_v: StringVector = Vec::new();
            let input_tokenizer = HfstStrings2FstTokenizer::new(&empty_v, "0");

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
                verbose_printf(&format!("Pair test on {}...\n", line_str));

                let line_for_panic = line_str.clone();
                let tok_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    input_tokenizer.tokenize_pair_string(&line_str, true)
                }));

                match tok_result {
                    Ok(mut tokenized_pair_string) => {
                        tokenized_pair_string
                            .insert(0, ("@#@".to_string(), internal_epsilon.to_string()));
                        tokenized_pair_string
                            .push(("@#@".to_string(), internal_epsilon.to_string()));

                        let new_exit_code = test(
                            &tokenized_pair_string,
                            &line_for_panic,
                            &grammar,
                            &rule_names,
                            POSITIVE_TEST,
                            &mut *outstream,
                            &known_symbols,
                        );

                        if exit_code == 0 {
                            exit_code = new_exit_code;
                        }
                    }
                    Err(e) => {
                        if e.downcast_ref::<UnescapedColsFound>().is_some() {
                            error(
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
                            std::panic::resume_unwind(e);
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

            let symbols: StringVector = known_symbols.iter().cloned().collect();

            let input_tokenizer = HfstStrings2FstTokenizer::new(&symbols, "0");

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

                    verbose_printf(&format!("Positive test case: {}...\n", test_case));
                    positive_test_cases.push(test_case);
                } else if is_negative_test_line(&line_str) {
                    // "!!$ xyz" -> "xyz"
                    let marker_len = "!!$".len();
                    let test_case =
                        strip_space(&substr_from_bytes(&strip_space(&line_str), marker_len));

                    verbose_printf(&format!(
                        "Negative test case: {} {}...\n",
                        line_str, test_case
                    ));
                    negative_test_cases.push(test_case);
                } else {
                    continue;
                }
            } // while lines in input
            if positive_test_cases.len() % 2 != 0 {
                error(
                    1,
                    0,
                    "Got an odd number of positive test cases. Every input string\n\
                     has to have an output string.\n",
                );
            }

            if negative_test_cases.len() % 2 != 0 {
                error(
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
                let tok_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // We need to convert the %-escaped input and output
                    // string to \-escpaed strings for input_toknizer.
                    input_tokenizer.tokenize_string_pair(&to_tokenize, false)
                }));

                let mut test_case = match tok_result {
                    Ok(tc) => tc,
                    Err(e) => {
                        if e.downcast_ref::<UnescapedColsFound>().is_some() {
                            error(
                                1,
                                0,
                                &format!(
                                    "The correspondence {} {} contains unescaped \
                                     colon-symbols. Escape them using %.",
                                    input_case, output_case
                                ),
                            );
                            unreachable!()
                        } else {
                            std::panic::resume_unwind(e);
                        }
                    }
                };
                test_case.insert(0, ("@#@".to_string(), internal_epsilon.to_string()));
                test_case.push(("@#@".to_string(), internal_epsilon.to_string()));

                let new_exit_code = test(
                    &test_case,
                    &format!("{} : {}", input_case, output_case),
                    &grammar,
                    &rule_names,
                    true,
                    &mut *outstream,
                    &known_symbols,
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
                let tok_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // We need to convert the %-escaped input and output
                    // string to \-escpaed strings for input_toknizer.
                    input_tokenizer.tokenize_string_pair(&to_tokenize, false)
                }));

                let mut test_case = match tok_result {
                    Ok(tc) => tc,
                    Err(e) => {
                        if e.downcast_ref::<UnescapedColsFound>().is_some() {
                            error(
                                1,
                                0,
                                &format!(
                                    "The correspondence {} {} contains unquoted \
                                     colon-symbols. Quote them using %.",
                                    input_case, output_case
                                ),
                            );
                            unreachable!()
                        } else {
                            std::panic::resume_unwind(e);
                        }
                    }
                };
                test_case.insert(0, ("@#@".to_string(), internal_epsilon.to_string()));
                test_case.push(("@#@".to_string(), internal_epsilon.to_string()));

                let new_exit_code = test(
                    &test_case,
                    &format!("{} : {}", input_case, output_case),
                    &grammar,
                    &rule_names,
                    false,
                    &mut *outstream,
                    &known_symbols,
                );

                if exit_code == 0 {
                    exit_code = new_exit_code;
                }
                i += 2;
            }
        }

        exit_code
    }
}

// [spec:hfst:def:hfst-pair-test.main-fn]
// [spec:hfst:sem:hfst-pair-test.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.6", "HfstPairTest");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let input_named = globals::input_filename() != "<stdin>";
        let mut instream = match if input_named {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-pair-test: cannot open output: {e}");
                return 1;
            }
        };

        let exit_code = process_stream(&mut instream, &mut *out);

        if !globals::SILENT {
            if exit_code == 0 {
                let _ = write!(out, "Test passed.\n");
            } else {
                let _ = write!(out, "Test failed.\n");
            }
        }

        exit_code
    }
}
