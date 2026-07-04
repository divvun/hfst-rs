#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-fst2strings.cc — the transducer path
//! printing command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name, parse_u64, verbose_print,
    warning,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_data_types::{HfstTwoLevelPath, HfstTwoLevelPaths};
use hfst::hfst_extract_strings::{ExtractStringsCb, RetVal};
use hfst::hfst_flag_diacritics::FdOperation;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::is_epsilon;
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

// Tool-specific globals. These mirror the file-scope statics of the C++ tool.
// the maximum number of strings printed for each transducer
static mut MAX_STRINGS: i32 = 0;
static mut CYCLES: i32 = -1;
static mut NBEST_STRINGS: i32 = -1;
static mut MAX_RANDOM_STRINGS: i32 = -1;
static mut MAX_WEIGHT: f32 = -1.0;
static mut BEAM: f32 = -1.0;
static mut DISPLAY_WEIGHTS: bool = false;
static mut EVAL_FD: bool = false;
static mut FILTER_FD: bool = true;
static mut QUOTE_SPECIAL: bool = false;
static mut PRINT_SPACES: bool = false;
static mut MAX_INPUT_LENGTH: u32 = 0;
static mut MAX_OUTPUT_LENGTH: u32 = 0;
static mut INPUT_PREFIX: String = String::new();
static mut OUTPUT_PREFIX: String = String::new();
static mut INPUT_EXCLUDE: String = String::new();
static mut OUTPUT_EXCLUDE: String = String::new();

static mut PRINT_IN_PAIRSTRING_FORMAT: bool = false;
static mut EPSILON_FORMAT: String = String::new();

static mut PRINT_SEPARATOR_AFTER_EACH_TRANSDUCER: bool = false;

// [spec:hfst:def:hfst-fst2strings.print-usage-fn]
// [spec:hfst:sem:hfst-fst2strings.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let program_name = globals::program_name();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nDisplay the strings recognized by a transducer\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Fst2strings options:\n\
         \x20 -n, --max-strings=NSTR     print at most NSTR strings\n\
         \x20 -N, --nbest=NBEST          print at most NBEST best strings\n\
         \x20 -r, --random=NRAND         print at most NRAND random strings\n\
         \x20 -c, --cycles=NCYC          follow cycles at most NCYC times\n\
         \x20 -w, --print-weights        display the weight for each string\n\
         \x20 -S, --print-separator      print separator \"--\" after each transducer\n\
         \x20 -e, --epsilon-format=EPS   print epsilon as EPS\n\
         \x20 -X, --xfst=VARIABLE        toggle xfst compatibility option VARIABLE\n"
    );
    let _ = write!(
        msg,
        "Path filters:\n\
         \x20 -b, --beam=B               reject output string with weight more than B away from\n\
         \x20                            the weight of the best output string\n\
         \x20 -l, --max-in-length=MIL    reject input string longer than MIL\n\
         \x20 -L, --max-out-length=MOL   reject output string longer than MOL\n\
         \x20 -p, --in-prefix=OPREFIX    input string must begin with IPREFIX\n\
         \x20 -P, --out-prefix=OPREFIX   output string must begin with OPREFIX\n\
         \x20 -u, --in-exclude=IXSTR     input string must not contain IXSTR\n\
         \x20 -U, --out-exclude=OXST     output string must not contain OXSTR\n"
    );

    let _ = write!(msg, "\n");

    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "If all NSTR, NBEST and NCYC are omitted, \
         all possible paths are printed:\n\
         NSTR, NBEST and NCYC default to infinity.\n\
         NBEST overrides NSTR and NCYC\n\
         NRAND overrides NBEST, NSTR and NCYC\n\
         B must be a non-negative float\n\
         If EPS is not given, default is empty string.\n\
         Numeric options are parsed with strtod(3).\n\
         Xfst variables supported are {{ obey-flags, print-flags,\n\
         print-pairs, print-space, quote-special }}.\n"
    );
    let _ = write!(
        msg,
        "\nExamples:\n\
         \x20 {} lexical.hfst    generates all forms of lexical.hfst\n\
         \x20 {} -P \"cat<n>\" -c 0 lexical.hfst\n\
         \x20                    generates paradigm for cat<n> without following cycles\n\n",
        program_name, program_name
    );

    let _ = write!(
        msg,
        "Known bugs:\n\
         \x20 Does not work correctly for hfst optimized lookup format.\n\n"
    );
}

// [spec:hfst:def:hfst-fst2strings.parse-options-fn]
// [spec:hfst:sem:hfst-fst2strings.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let tool_long: [(&str, i32, i32); 15] = [
                ("beam", 1, b'b' as i32),
                ("cycles", 1, b'c' as i32),
                ("epsilon-format", 1, b'e' as i32),
                ("in-exclude", 1, b'u' as i32),
                ("in-prefix", 1, b'p' as i32),
                ("max-in-length", 1, b'l' as i32),
                ("max-out-length", 1, b'L' as i32),
                ("max-strings", 1, b'n' as i32),
                ("nbest", 1, b'N' as i32),
                ("random", 1, b'r' as i32),
                ("print-separator", 0, b'S' as i32),
                ("out-exclude", 1, b'U' as i32),
                ("out-prefix", 1, b'P' as i32),
                ("print-weights", 0, b'w' as i32),
                ("xfst", 1, b'X' as i32),
            ];
            for (name, has_arg, val) in tool_long.iter() {
                long_options.push(getopt::GetOpt {
                    name,
                    has_arg: *has_arg,
                    val: *val,
                });
            }
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
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

            let optarg = getopt::optarg();
            match c as u8 as char {
                'n' => {
                    MAX_STRINGS = parse_u64(&optarg, 10) as i32;
                }
                'N' => {
                    NBEST_STRINGS = parse_u64(&optarg, 10) as i32;
                }
                'r' => {
                    MAX_RANDOM_STRINGS = parse_u64(&optarg, 10) as i32;
                }
                'b' => {
                    BEAM = optarg.trim().parse::<f32>().unwrap_or(0.0);
                    if BEAM < 0.0 {
                        eprint!("Invalid argument for --beam\n");
                        return 1;
                    }
                }
                'c' => {
                    CYCLES = parse_u64(&optarg, 10) as i32;
                }
                'w' => {
                    DISPLAY_WEIGHTS = true;
                }
                'X' => {
                    if optarg == "obey-flags" {
                        EVAL_FD = true;
                    } else if optarg == "print-flags" {
                        FILTER_FD = false;
                    } else if optarg == "quote-special" {
                        QUOTE_SPECIAL = true;
                    } else if optarg == "print-pairs" {
                        PRINT_IN_PAIRSTRING_FORMAT = true;
                    } else if optarg == "print-space" {
                        PRINT_SPACES = true;
                    } else {
                        error(
                            0,
                            1,
                            "Unrecognised xfst option. available options are obey-flags, print-flags\n",
                        );
                    }
                }
                'l' => {
                    MAX_INPUT_LENGTH = parse_u64(&optarg, 10) as u32;
                }
                'L' => {
                    MAX_OUTPUT_LENGTH = parse_u64(&optarg, 10) as u32;
                }
                'p' => {
                    INPUT_PREFIX = optarg;
                }
                'P' => {
                    OUTPUT_PREFIX = optarg;
                }
                'u' => {
                    INPUT_EXCLUDE = optarg;
                }
                'U' => {
                    OUTPUT_EXCLUDE = optarg;
                }
                'S' => {
                    PRINT_SEPARATOR_AFTER_EACH_TRANSDUCER = true;
                }
                'e' => {
                    EPSILON_FORMAT = optarg;
                }
                _ => {
                    return handle_error_case(c);
                }
            }
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

/* Replace all strings str1 in symbol with str2. */
// [spec:hfst:def:hfst-fst2strings.replace-all-fn]
// [spec:hfst:sem:hfst-fst2strings.replace-all-fn]
fn replace_all(symbol: String, str1: &str, str2: &str) -> String {
    let mut symbol = symbol;
    let mut pos = symbol.find(str1);
    while let Some(p) = pos {
        // erase str1
        symbol.replace_range(p..p + str1.len(), "");
        // insert str2 instead
        symbol.insert_str(p, str2);
        // find next str1
        pos = symbol[p + str2.len()..]
            .find(str1)
            .map(|rel| rel + p + str2.len());
    }
    symbol
}

// [spec:hfst:def:hfst-fst2strings.get-print-format-fn]
// [spec:hfst:sem:hfst-fst2strings.get-print-format-fn]
unsafe fn get_print_format(s: &str) -> String {
    unsafe {
        // print epsilon as defined by the user or use the default
        if is_epsilon(s) {
            return EPSILON_FORMAT.clone();
        }

        if !QUOTE_SPECIAL {
            return s.to_string();
        }

        // escape spaces and colons as they have a special meaning
        replace_all(
            replace_all(
                replace_all(s.to_string(), " ", "@_SPACE_@"),
                ":",
                "@_COLON_@",
            ),
            "\t",
            "@_TAB_@",
        )
    }
}

// Print results as they come
// [spec:hfst:def:hfst-fst2strings.callback]
struct Callback<'a> {
    count: i32,
    max_num: i32,
    out: &'a mut dyn std::io::Write,
}

impl<'a> Callback<'a> {
    // [spec:hfst:def:hfst-fst2strings.callback.callback-fn]
    // [spec:hfst:sem:hfst-fst2strings.callback.callback-fn]
    fn new(max: i32, out: &'a mut dyn std::io::Write) -> Self {
        Callback {
            count: 0,
            max_num: max,
            out: out,
        }
    }
}

impl ExtractStringsCb for Callback<'_> {
    // [spec:hfst:def:hfst-fst2strings.callback.operator-fn]
    // [spec:hfst:sem:hfst-fst2strings.callback.operator-fn]
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        unsafe {
            let mut istring = String::new();
            let mut ostring = String::new();
            for it in path.second.iter() {
                istring.push_str(&it.0);
                ostring.push_str(&it.1);
            }
            let weight = path.first;

            if (MAX_INPUT_LENGTH > 0) && (istring.len() as u32 > MAX_INPUT_LENGTH) {
                // continue searching, break off this path
                return RetVal::new(true, false);
            }
            if (MAX_OUTPUT_LENGTH > 0) && (ostring.len() as u32 > MAX_OUTPUT_LENGTH) {
                return RetVal::new(true, false);
                // continue searching, break off this path
            }
            if !INPUT_PREFIX.is_empty() {
                if istring.len() < INPUT_PREFIX.len() {
                    return RetVal::new(true, true);
                }
                if istring.as_bytes()[..INPUT_PREFIX.len()] != *INPUT_PREFIX.as_bytes() {
                    return RetVal::new(true, false);
                    // continue searching, break off this path
                }
            }
            if !OUTPUT_PREFIX.is_empty() {
                if ostring.len() < OUTPUT_PREFIX.len() {
                    return RetVal::new(true, true);
                }
                if ostring.as_bytes()[..OUTPUT_PREFIX.len()] != *OUTPUT_PREFIX.as_bytes() {
                    return RetVal::new(true, false);
                    // continue searching, break off this path
                }
            }
            if !INPUT_EXCLUDE.is_empty() && istring.contains(INPUT_EXCLUDE.as_str()) {
                return RetVal::new(true, false);
                // continue searching, break off this path
            }
            if !OUTPUT_EXCLUDE.is_empty() && ostring.contains(OUTPUT_EXCLUDE.as_str()) {
                return RetVal::new(true, false);
                // continue searching, break off this path
            }
            if MAX_WEIGHT >= 0.0 && weight > (MAX_WEIGHT + BEAM) {
                return RetVal::new(true, false);
                // continue searching, break off this path
            }
            // the path passed the checks. Print it if it is final
            if is_final {
                if PRINT_IN_PAIRSTRING_FORMAT {
                    let mut first_pair = true;
                    for it in path.second.iter() {
                        if (!FILTER_FD) || (!FdOperation::is_diacritic(&it.0)) {
                            if PRINT_SPACES && !first_pair {
                                let _ = self.out.write_all(b" ");
                            }

                            let _ = self.out.write_all(get_print_format(&it.0).as_bytes());
                            first_pair = false;
                        }

                        if it.0 != it.1 && ((!FILTER_FD) || (!FdOperation::is_diacritic(&it.1))) {
                            let _ = write!(self.out, ":{}", get_print_format(&it.1));
                        }
                    }
                    if DISPLAY_WEIGHTS {
                        let _ = write!(self.out, "\t{}", path.first);
                    }
                    let _ = self.out.write_all(b"\n");
                } else {
                    let mut is_automaton = true;

                    let mut first_symbol = true;
                    for it in path.second.iter() {
                        if (!FILTER_FD) || (!FdOperation::is_diacritic(&it.0)) {
                            if PRINT_SPACES && !first_symbol {
                                let _ = self.out.write_all(b" ");
                            }
                            if it.0 != it.1 {
                                is_automaton = false;
                            }

                            let _ = self.out.write_all(get_print_format(&it.0).as_bytes());
                        }
                        first_symbol = false;
                    }
                    if PRINT_SPACES {
                        let _ = self.out.write_all(b" ");
                    }

                    if !is_automaton {
                        let _ = self.out.write_all(b":");
                        for it in path.second.iter() {
                            if (!FILTER_FD) || (!FdOperation::is_diacritic(&it.1)) {
                                if PRINT_SPACES {
                                    let _ = self.out.write_all(b" ");
                                }
                                let _ = self.out.write_all(get_print_format(&it.1).as_bytes());
                            }
                        }
                    }

                    if DISPLAY_WEIGHTS {
                        let _ = write!(self.out, "\t{}", path.first);
                    }
                    let _ = self.out.write_all(b"\n");
                    // std::endl flushes
                    let _ = self.out.flush();
                }
                self.count += 1;
            }
            // continue until we've printed max_num strings
            RetVal::new((self.max_num < 1) || (self.count < self.max_num), true)
        }
    }
}

// [spec:hfst:def:hfst-fst2strings.process-stream-fn]
// [spec:hfst:sem:hfst-fst2strings.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    unsafe {
        let mut first_transducer = true;
        while instream.is_good() {
            if !first_transducer && PRINT_SEPARATOR_AFTER_EACH_TRANSDUCER {
                let _ = outstream.write_all(b"--\n");
            }
            first_transducer = false;

            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };

            /* Pairstring format is not supported on optimized lookup format. */
            if PRINT_IN_PAIRSTRING_FORMAT
                && (instream.get_type() == ImplementationType::HFST_OL_TYPE
                    || instream.get_type() == ImplementationType::HFST_OLW_TYPE)
            {
                eprint!(
                    "Error: option --print-in-pairstring-format not supported on \n       optimized lookup transducers, exiting program\n"
                );
                std::process::exit(1);
            }

            if !INPUT_PREFIX.is_empty() {
                verbose_print(&format!("input_prefix: '{}'\n", INPUT_PREFIX));
            }

            // the one runtime dispatch per stream read
            // ([dec:hfst:monomorphic-backends]): the algebra-only pruning
            // options (--beam, --nbest, --random without flag evaluation)
            // become the C++ catch-FunctionNotImplemented error messages on
            // the optimized-lookup variants.
            let code = match any {
                hfst::hfst_transducer::AnyTransducer::Tropical(t) => {
                    process_one_algebra(t, outstream)
                }
                hfst::hfst_transducer::AnyTransducer::Log(t) => process_one_algebra(t, outstream),
                hfst::hfst_transducer::AnyTransducer::OlW(t) => process_one_ol(t, outstream),
                hfst::hfst_transducer::AnyTransducer::OlU(t) => process_one_ol(t, outstream),
            };
            if code != 0 {
                return code;
            }
        }

        instream.close();
        0
    }
}

// The full per-transducer body for the algebra backends.
unsafe fn process_one_algebra<B: hfst::backend::AlgebraBackend>(
    mut t: HfstTransducer<B>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    unsafe {
        {
            if BEAM >= 0.0 {
                verbose_print("Finding the weight of the best path...\n");
                // (the C wraps this in try/catch on FunctionNotImplementedException
                // and HfstFatalException; in Rust these surface as panics rather
                // than being caught here.)
                let mut tc = t.clone();
                if let Err(e) = tc.n_best(1) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                let mut best_paths: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
                if let Err(e) = tc.extract_paths(&mut best_paths, -1, -1) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                if best_paths.len() != 1 {
                    error(1, 0, "n_best(1) produced more than one path");
                }
                MAX_WEIGHT = best_paths.iter().next().unwrap().first;
            }

            if NBEST_STRINGS > 0 {
                verbose_print(&format!(
                    "Pruning transducer to {} best path(s)...\n",
                    NBEST_STRINGS
                ));
                // (the C wraps this in try/catch on FunctionNotImplementedException
                // and HfstFatalException; in Rust these surface as panics.)
                if let Err(e) = t.n_best(NBEST_STRINGS as u32) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            } else if MAX_RANDOM_STRINGS <= 0
                && MAX_STRINGS <= 0
                && MAX_INPUT_LENGTH == 0
                && MAX_OUTPUT_LENGTH == 0
                && CYCLES < 0
            {
                let is_cyclic = match t.is_cyclic() {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if is_cyclic {
                    error(
                        1,
                        0,
                        "Transducer is cyclic. Use one or more of these options: -n, -N, -r, -l, -L, -c",
                    );
                    return 1;
                }
            }

            if MAX_STRINGS > 0 {
                verbose_print(&format!("Finding at most {} path(s)...\n", MAX_STRINGS));
            } else if MAX_RANDOM_STRINGS > 0 {
                verbose_print(&format!(
                    "Finding at most {} random path(s)...\n",
                    MAX_RANDOM_STRINGS
                ));
            } else {
                verbose_print("Finding strings...\n");
            }

            /* not random strings */
            if MAX_RANDOM_STRINGS <= 0 {
                let mut cb = Callback::new(MAX_STRINGS, &mut *outstream);
                let extract_res = if EVAL_FD {
                    t.extract_paths_fd_cb(&mut cb, CYCLES, FILTER_FD)
                } else {
                    t.extract_paths_cb(&mut cb, CYCLES)
                };
                if let Err(e) = extract_res {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                verbose_print(&format!("Printed {} string(s)\n", cb.count));
            }
            /* random strings */
            else {
                let mut results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
                // (the C wraps this in try/catch on FunctionNotImplementedException;
                // in Rust the not-implemented case surfaces as a panic.)
                let random_res = if EVAL_FD {
                    t.extract_random_paths_fd(&mut results, MAX_RANDOM_STRINGS, FILTER_FD)
                } else {
                    t.extract_random_paths(&mut results, MAX_RANDOM_STRINGS)
                };
                if let Err(e) = random_res {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }

                let mut cb = Callback::new(MAX_RANDOM_STRINGS, &mut *outstream);
                for it in results.iter() {
                    let mut path: HfstTwoLevelPath = it.clone();
                    cb.operator_call(&mut path, true /*final*/);
                }
                verbose_print(&format!("Printed {} random string(s)\n", cb.count));
            }
        }
        0
    }
}

// The per-transducer body for the optimized-lookup backends: path extraction
// works; the algebra-only pruning options produce the C++ error texts (the
// former catch-FunctionNotImplemented arms of hfst-fst2strings.cc).
unsafe fn process_one_ol<B: hfst::backend::LookupBackend>(
    t: HfstTransducer<B>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    unsafe {
        if BEAM >= 0.0 {
            verbose_print("Finding the weight of the best path...\n");
            crate::hfst_commandline::hfst_error(
                1,
                0,
                "option --beam not implemented for optimized lookup format",
            );
            return 1;
        }

        if NBEST_STRINGS > 0 {
            verbose_print(&format!(
                "Pruning transducer to {} best path(s)...\n",
                NBEST_STRINGS
            ));
            crate::hfst_commandline::hfst_error(
                1,
                0,
                "option --nbest not implemented for optimized lookup format",
            );
            return 1;
        } else if MAX_RANDOM_STRINGS <= 0
            && MAX_STRINGS <= 0
            && MAX_INPUT_LENGTH == 0
            && MAX_OUTPUT_LENGTH == 0
            && CYCLES < 0
        {
            let is_cyclic = match t.is_cyclic() {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if is_cyclic {
                error(
                    1,
                    0,
                    "Transducer is cyclic. Use one or more of these options: -n, -N, -r, -l, -L, -c",
                );
                return 1;
            }
        }

        if MAX_STRINGS > 0 {
            verbose_print(&format!("Finding at most {} path(s)...\n", MAX_STRINGS));
        } else if MAX_RANDOM_STRINGS > 0 {
            verbose_print(&format!(
                "Finding at most {} random path(s)...\n",
                MAX_RANDOM_STRINGS
            ));
        } else {
            verbose_print("Finding strings...\n");
        }

        /* not random strings */
        if MAX_RANDOM_STRINGS <= 0 {
            let mut cb = Callback::new(MAX_STRINGS, &mut *outstream);
            let extract_res = if EVAL_FD {
                t.extract_paths_fd_cb(&mut cb, CYCLES, FILTER_FD)
            } else {
                t.extract_paths_cb(&mut cb, CYCLES)
            };
            if let Err(e) = extract_res {
                error(1, 0, &format!("{e}"));
                return 1;
            }
            verbose_print(&format!("Printed {} string(s)\n", cb.count));
        }
        /* random strings */
        else {
            if !EVAL_FD {
                // C++: HfstTransducer::extract_random_paths threw
                // FunctionNotImplemented for the OL backends.
                crate::hfst_commandline::hfst_error(
                    1,
                    0,
                    "option --random not implemented for optimized lookup format",
                );
                return 1;
            }
            let mut results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
            if let Err(e) = t.extract_random_paths_fd(&mut results, MAX_RANDOM_STRINGS, FILTER_FD) {
                error(1, 0, &format!("{e}"));
                return 1;
            }

            let mut cb = Callback::new(MAX_RANDOM_STRINGS, &mut *outstream);
            for it in results.iter() {
                let mut path: HfstTwoLevelPath = it.clone();
                cb.operator_call(&mut path, true /*final*/);
            }
            verbose_print(&format!("Printed {} random string(s)\n", cb.count));
        }
        0
    }
}

// [spec:hfst:def:hfst-fst2strings.main-fn]
// [spec:hfst:sem:hfst-fst2strings.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstFst2Strings");
        EPSILON_FORMAT = String::new();
        let mut retval = parse_options(&mut args);

        if MAX_STRINGS > 0 && MAX_RANDOM_STRINGS > 0 && !globals::SILENT {
            warning(0, 0, "option --max_strings ignored, --random used\n");
            MAX_STRINGS = -1;
        }

        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        // (C closes outfile here when it is not stdout and re-opens an ofstream
        // to outfilename inside; the foundation now models the output as a std
        // writer opened from OUTFILENAME, written to directly.)
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // printing "%s is not a valid transducer file" is not reproduced here.)
        let mut instream = match if input_opened {
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

        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-fst2strings: cannot open output: {e}");
                return 1;
            }
        };
        retval = process_stream(&mut instream, &mut *out);

        retval
    }
}
