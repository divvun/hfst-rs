//! Faithful 1:1 port of tools/src/hfst-fst2strings.cc — the transducer path
//! printing command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, parse_u64, verbose_print, warning,
};
use crate::hfst_getopt::{self as getopt, Getopt};
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

/// hfst-fst2strings's own options (the former tool-specific `static mut`s).
struct Options {
    /// the maximum number of strings printed for each transducer
    max_strings: i32,
    cycles: i32,
    nbest_strings: i32,
    max_random_strings: i32,
    /// weight of the best path, filled at runtime when `--beam` is used.
    max_weight: f32,
    beam: f32,
    display_weights: bool,
    eval_fd: bool,
    filter_fd: bool,
    quote_special: bool,
    print_spaces: bool,
    max_input_length: u32,
    max_output_length: u32,
    input_prefix: String,
    output_prefix: String,
    input_exclude: String,
    output_exclude: String,
    print_in_pairstring_format: bool,
    epsilon_format: String,
    print_separator_after_each_transducer: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            max_strings: 0,
            cycles: -1,
            nbest_strings: -1,
            max_random_strings: -1,
            max_weight: -1.0,
            beam: -1.0,
            display_weights: false,
            eval_fd: false,
            filter_fd: true,
            quote_special: false,
            print_spaces: false,
            max_input_length: 0,
            max_output_length: 0,
            input_prefix: String::new(),
            output_prefix: String::new(),
            input_exclude: String::new(),
            output_exclude: String::new(),
            print_in_pairstring_format: false,
            epsilon_format: String::new(),
            print_separator_after_each_transducer: false,
        }
    }
}

// [spec:hfst:req:cli.help]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let program_name = &common.program_name;
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

    let _ = writeln!(msg);

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
// [spec:hfst:req:cli.arg-parse]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own, then the terminal
        // error arm.
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

        let optarg = opt.optarg();
        match c as u8 as char {
            'n' => {
                options.max_strings = parse_u64(&common, &optarg, 10) as i32;
            }
            'N' => {
                options.nbest_strings = parse_u64(&common, &optarg, 10) as i32;
            }
            'r' => {
                options.max_random_strings = parse_u64(&common, &optarg, 10) as i32;
            }
            'b' => {
                options.beam = optarg.trim().parse::<f32>().unwrap_or(0.0);
                if options.beam < 0.0 {
                    eprintln!("Invalid argument for --beam");
                    return Err(1);
                }
            }
            'c' => {
                options.cycles = parse_u64(&common, &optarg, 10) as i32;
            }
            'w' => {
                options.display_weights = true;
            }
            'X' => {
                if optarg == "obey-flags" {
                    options.eval_fd = true;
                } else if optarg == "print-flags" {
                    options.filter_fd = false;
                } else if optarg == "quote-special" {
                    options.quote_special = true;
                } else if optarg == "print-pairs" {
                    options.print_in_pairstring_format = true;
                } else if optarg == "print-space" {
                    options.print_spaces = true;
                } else {
                    error(
                        &common,
                        0,
                        1,
                        "Unrecognised xfst option. available options are obey-flags, print-flags\n",
                    );
                }
            }
            'l' => {
                options.max_input_length = parse_u64(&common, &optarg, 10) as u32;
            }
            'L' => {
                options.max_output_length = parse_u64(&common, &optarg, 10) as u32;
            }
            'p' => {
                options.input_prefix = optarg;
            }
            'P' => {
                options.output_prefix = optarg;
            }
            'u' => {
                options.input_exclude = optarg;
            }
            'U' => {
                options.output_exclude = optarg;
            }
            'S' => {
                options.print_separator_after_each_transducer = true;
            }
            'e' => {
                options.epsilon_format = optarg;
            }
            _ => {
                return Err(handle_error_case(&common, &opt, c));
            }
        }
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
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
fn get_print_format(options: &Options, s: &str) -> String {
    // print epsilon as defined by the user or use the default
    if is_epsilon(s) {
        return options.epsilon_format.clone();
    }

    if !options.quote_special {
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

// Print results as they come
// [spec:hfst:def:hfst-fst2strings.callback]
struct Callback<'a> {
    count: i32,
    max_num: i32,
    out: &'a mut dyn std::io::Write,
    options: &'a Options,
}

impl<'a> Callback<'a> {
    // [spec:hfst:def:hfst-fst2strings.callback.callback-fn]
    // [spec:hfst:sem:hfst-fst2strings.callback.callback-fn]
    fn new(max: i32, out: &'a mut dyn std::io::Write, options: &'a Options) -> Self {
        Callback {
            count: 0,
            max_num: max,
            out,
            options,
        }
    }
}

impl ExtractStringsCb for Callback<'_> {
    // [spec:hfst:def:hfst-fst2strings.callback.operator-fn]
    // [spec:hfst:sem:hfst-fst2strings.callback.operator-fn]
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        let options = self.options;
        let mut istring = String::new();
        let mut ostring = String::new();
        // Epsilon symbols carry the internal `@_EPSILON_SYMBOL_@` marker in the
        // path but render as the empty string on output. The length / prefix
        // (`-p`/`-P`) / exclude (`-u`/`-U`) filters below compare against these
        // accumulated strings, so epsilons must be skipped here too — otherwise a
        // leading (or interior) epsilon prepends the marker and an `-P "prefix"`
        // query never matches, even though the printed output does start with the
        // prefix. Including epsilons in the compare was hfst/hfst#587.
        // [upstream hfst/hfst#587]
        for it in path.second.iter() {
            if !is_epsilon(&it.0) {
                istring.push_str(&it.0);
            }
            if !is_epsilon(&it.1) {
                ostring.push_str(&it.1);
            }
        }
        let weight = path.first;

        if (options.max_input_length > 0) && (istring.len() as u32 > options.max_input_length) {
            // continue searching, break off this path
            return RetVal::new(true, false);
        }
        if (options.max_output_length > 0) && (ostring.len() as u32 > options.max_output_length) {
            return RetVal::new(true, false);
            // continue searching, break off this path
        }
        if !options.input_prefix.is_empty() {
            if istring.len() < options.input_prefix.len() {
                return RetVal::new(true, true);
            }
            if istring.as_bytes()[..options.input_prefix.len()] != *options.input_prefix.as_bytes()
            {
                return RetVal::new(true, false);
                // continue searching, break off this path
            }
        }
        if !options.output_prefix.is_empty() {
            if ostring.len() < options.output_prefix.len() {
                return RetVal::new(true, true);
            }
            if ostring.as_bytes()[..options.output_prefix.len()]
                != *options.output_prefix.as_bytes()
            {
                return RetVal::new(true, false);
                // continue searching, break off this path
            }
        }
        if !options.input_exclude.is_empty() && istring.contains(options.input_exclude.as_str()) {
            return RetVal::new(true, false);
            // continue searching, break off this path
        }
        if !options.output_exclude.is_empty() && ostring.contains(options.output_exclude.as_str()) {
            return RetVal::new(true, false);
            // continue searching, break off this path
        }
        if options.max_weight >= 0.0 && weight > (options.max_weight + options.beam) {
            return RetVal::new(true, false);
            // continue searching, break off this path
        }
        // the path passed the checks. Print it if it is final
        if is_final {
            if options.print_in_pairstring_format {
                let mut first_pair = true;
                for it in path.second.iter() {
                    if (!options.filter_fd) || (!FdOperation::is_diacritic(&it.0)) {
                        if options.print_spaces && !first_pair {
                            let _ = self.out.write_all(b" ");
                        }

                        let _ = self
                            .out
                            .write_all(get_print_format(options, &it.0).as_bytes());
                        first_pair = false;
                    }

                    if it.0 != it.1 && ((!options.filter_fd) || (!FdOperation::is_diacritic(&it.1)))
                    {
                        let _ = write!(self.out, ":{}", get_print_format(options, &it.1));
                    }
                }
                if options.display_weights {
                    let _ = write!(self.out, "\t{}", path.first);
                }
                let _ = self.out.write_all(b"\n");
            } else {
                let mut is_automaton = true;

                let mut first_symbol = true;
                for it in path.second.iter() {
                    if (!options.filter_fd) || (!FdOperation::is_diacritic(&it.0)) {
                        if options.print_spaces && !first_symbol {
                            let _ = self.out.write_all(b" ");
                        }
                        if it.0 != it.1 {
                            is_automaton = false;
                        }

                        let _ = self
                            .out
                            .write_all(get_print_format(options, &it.0).as_bytes());
                    }
                    first_symbol = false;
                }
                if options.print_spaces {
                    let _ = self.out.write_all(b" ");
                }

                if !is_automaton {
                    let _ = self.out.write_all(b":");
                    for it in path.second.iter() {
                        if (!options.filter_fd) || (!FdOperation::is_diacritic(&it.1)) {
                            if options.print_spaces {
                                let _ = self.out.write_all(b" ");
                            }
                            let _ = self
                                .out
                                .write_all(get_print_format(options, &it.1).as_bytes());
                        }
                    }
                }

                if options.display_weights {
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

// [spec:hfst:def:hfst-fst2strings.process-stream-fn]
// [spec:hfst:sem:hfst-fst2strings.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    let mut first_transducer = true;
    while instream.is_good() {
        if !first_transducer && options.print_separator_after_each_transducer {
            let _ = outstream.write_all(b"--\n");
        }
        first_transducer = false;

        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        /* Pairstring format is not supported on optimized lookup format. */
        if options.print_in_pairstring_format
            && (instream.get_type() == ImplementationType::HFST_OL_TYPE
                || instream.get_type() == ImplementationType::HFST_OLW_TYPE
                || instream.get_type() == ImplementationType::THFST_TYPE)
        {
            eprint!(
                "Error: option --print-in-pairstring-format not supported on \n       optimized lookup transducers, exiting program\n"
            );
            std::process::exit(1);
        }

        if !options.input_prefix.is_empty() {
            verbose_print(
                common,
                &format!("input_prefix: '{}'\n", options.input_prefix),
            );
        }

        // the one runtime dispatch per stream read
        // ([dec:hfst:monomorphic-backends]): the algebra-only pruning
        // options (--beam, --nbest, --random without flag evaluation)
        // become the C++ catch-FunctionNotImplemented error messages on
        // the optimized-lookup variants.
        let code = match any {
            hfst::hfst_transducer::AnyTransducer::Tropical(t) => {
                process_one_algebra(common, options, t, outstream)
            }
            hfst::hfst_transducer::AnyTransducer::OlW(t) => {
                process_one_ol(common, options, t, outstream)
            }
            hfst::hfst_transducer::AnyTransducer::OlU(t) => {
                process_one_ol(common, options, t, outstream)
            }
            hfst::hfst_transducer::AnyTransducer::Thfst(t) => {
                process_one_ol(common, options, t, outstream)
            }
            #[cfg(feature = "foma")]
            hfst::hfst_transducer::AnyTransducer::Foma(t) => {
                process_one_algebra(common, options, t, outstream)
            }
        };
        if code != 0 {
            return code;
        }
    }

    instream.close();
    0
}

// The full per-transducer body for the algebra backends.
fn process_one_algebra<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &mut Options,
    mut t: HfstTransducer<B>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    if options.beam >= 0.0 {
        verbose_print(common, "Finding the weight of the best path...\n");
        // (the C wraps this in try/catch on FunctionNotImplementedException
        // and HfstFatalException; in Rust these surface as panics rather
        // than being caught here.)
        let mut tc = t.clone();
        if let Err(e) = tc.n_best(1) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        let mut best_paths: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        if let Err(e) = tc.extract_paths(&mut best_paths, -1, -1) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if best_paths.len() != 1 {
            error(common, 1, 0, "n_best(1) produced more than one path");
        }
        options.max_weight = best_paths.iter().next().unwrap().first;
    }

    if options.nbest_strings > 0 {
        verbose_print(
            common,
            &format!(
                "Pruning transducer to {} best path(s)...\n",
                options.nbest_strings
            ),
        );
        // (the C wraps this in try/catch on FunctionNotImplementedException
        // and HfstFatalException; in Rust these surface as panics.)
        if let Err(e) = t.n_best(options.nbest_strings as u32) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    } else if options.max_random_strings <= 0
        && options.max_strings <= 0
        && options.max_input_length == 0
        && options.max_output_length == 0
        && options.cycles < 0
    {
        let is_cyclic = match t.is_cyclic() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        if is_cyclic {
            error(
                common,
                1,
                0,
                "Transducer is cyclic. Use one or more of these options: -n, -N, -r, -l, -L, -c",
            );
            return 1;
        }
    }

    if options.max_strings > 0 {
        verbose_print(
            common,
            &format!("Finding at most {} path(s)...\n", options.max_strings),
        );
    } else if options.max_random_strings > 0 {
        verbose_print(
            common,
            &format!(
                "Finding at most {} random path(s)...\n",
                options.max_random_strings
            ),
        );
    } else {
        verbose_print(common, "Finding strings...\n");
    }

    /* not random strings */
    if options.max_random_strings <= 0 {
        let mut cb = Callback::new(options.max_strings, &mut *outstream, options);
        let extract_res = if options.eval_fd {
            t.extract_paths_fd_cb(&mut cb, options.cycles, options.filter_fd)
        } else {
            t.extract_paths_cb(&mut cb, options.cycles)
        };
        let count = cb.count;
        if let Err(e) = extract_res {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        verbose_print(common, &format!("Printed {} string(s)\n", count));
    }
    /* random strings */
    else {
        let mut results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        // (the C wraps this in try/catch on FunctionNotImplementedException;
        // in Rust the not-implemented case surfaces as a panic.)
        let random_res = if options.eval_fd {
            t.extract_random_paths_fd(&mut results, options.max_random_strings, options.filter_fd)
        } else {
            t.extract_random_paths(&mut results, options.max_random_strings)
        };
        if let Err(e) = random_res {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }

        let mut cb = Callback::new(options.max_random_strings, &mut *outstream, options);
        for it in results.iter() {
            let mut path: HfstTwoLevelPath = it.clone();
            cb.operator_call(&mut path, true /*final*/);
        }
        verbose_print(common, &format!("Printed {} random string(s)\n", cb.count));
    }
    0
}

// The per-transducer body for the optimized-lookup backends: path extraction
// works; the algebra-only pruning options produce the C++ error texts (the
// former catch-FunctionNotImplemented arms of hfst-fst2strings.cc).
fn process_one_ol<B: hfst::backend::LookupBackend>(
    common: &CommonOptions,
    options: &mut Options,
    t: HfstTransducer<B>,
    outstream: &mut dyn std::io::Write,
) -> i32 {
    if options.beam >= 0.0 {
        verbose_print(common, "Finding the weight of the best path...\n");
        crate::hfst_commandline::hfst_error(
            common,
            1,
            0,
            "option --beam not implemented for optimized lookup format",
        );
        return 1;
    }

    if options.nbest_strings > 0 {
        verbose_print(
            common,
            &format!(
                "Pruning transducer to {} best path(s)...\n",
                options.nbest_strings
            ),
        );
        crate::hfst_commandline::hfst_error(
            common,
            1,
            0,
            "option --nbest not implemented for optimized lookup format",
        );
        return 1;
    } else if options.max_random_strings <= 0
        && options.max_strings <= 0
        && options.max_input_length == 0
        && options.max_output_length == 0
        && options.cycles < 0
    {
        let is_cyclic = match t.is_cyclic() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        if is_cyclic {
            error(
                common,
                1,
                0,
                "Transducer is cyclic. Use one or more of these options: -n, -N, -r, -l, -L, -c",
            );
            return 1;
        }
    }

    if options.max_strings > 0 {
        verbose_print(
            common,
            &format!("Finding at most {} path(s)...\n", options.max_strings),
        );
    } else if options.max_random_strings > 0 {
        verbose_print(
            common,
            &format!(
                "Finding at most {} random path(s)...\n",
                options.max_random_strings
            ),
        );
    } else {
        verbose_print(common, "Finding strings...\n");
    }

    /* not random strings */
    if options.max_random_strings <= 0 {
        let mut cb = Callback::new(options.max_strings, &mut *outstream, options);
        let extract_res = if options.eval_fd {
            t.extract_paths_fd_cb(&mut cb, options.cycles, options.filter_fd)
        } else {
            t.extract_paths_cb(&mut cb, options.cycles)
        };
        let count = cb.count;
        if let Err(e) = extract_res {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        verbose_print(common, &format!("Printed {} string(s)\n", count));
    }
    /* random strings */
    else {
        if !options.eval_fd {
            // C++: HfstTransducer::extract_random_paths threw
            // FunctionNotImplemented for the OL backends.
            crate::hfst_commandline::hfst_error(
                common,
                1,
                0,
                "option --random not implemented for optimized lookup format",
            );
            return 1;
        }
        let mut results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        if let Err(e) =
            t.extract_random_paths_fd(&mut results, options.max_random_strings, options.filter_fd)
        {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }

        let mut cb = Callback::new(options.max_random_strings, &mut *outstream, options);
        for it in results.iter() {
            let mut path: HfstTwoLevelPath = it.clone();
            cb.operator_call(&mut path, true /*final*/);
        }
        verbose_print(common, &format!("Printed {} random string(s)\n", cb.count));
    }
    0
}

// [spec:hfst:def:hfst-fst2strings.main-fn]
// [spec:hfst:sem:hfst-fst2strings.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstFst2Strings");
    let (common, mut options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    if options.max_strings > 0 && options.max_random_strings > 0 && !common.silent {
        warning(
            &common,
            0,
            0,
            "option --max_strings ignored, --random used\n",
        );
        options.max_strings = -1;
    }

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    // (C closes outfile here when it is not stdout and re-opens an ofstream
    // to outfilename inside; the foundation now models the output as a std
    // writer opened from OUTFILENAME, written to directly.)
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // printing "%s is not a valid transducer file" is not reproduced here.)
    let mut instream = match if input_opened {
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

    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-fst2strings: cannot open output: {e}");
            return 1;
        }
    };
    process_stream(&common, &mut options, &mut instream, &mut *out)
}
