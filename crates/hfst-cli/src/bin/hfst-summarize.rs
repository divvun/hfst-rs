//! Faithful 1:1 port of tools/src/hfst-summarize.cc — the transducer
//! information / properties command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options, inc fragments).

use core::ffi::{c_char, c_int};
use hfst::hfst_basic_transducer::{HfstBasicTransducer, SummaryStats};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_exception_defs::FunctionNotImplementedException;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, hfst_strtoul,
    print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::ffi::{CStr, CString};

// add tools-specific variables here
static mut PRINT_SYMBOL_PAIR_STATISTICS: bool = false;
static mut SYMBOL_PAIR_THRESHOLD: i32 = -1;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

fn fput(f: &mut dyn std::io::Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

// [spec:hfst:def:hfst-summarize.print-usage-fn]
// [spec:hfst:sem:hfst-summarize.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nCalculate the properties of a transducer\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        // (tool-specific options and short descriptions)
        fput(&mut *msg, "Summarize options:\n");
        fput(
            &mut *msg,
            "  -S, --print-symbol-pair-statistics=N  Print info about symbol pairs that occur\n",
        );
        fput(
            &mut *msg,
            "                                        at most N times (default is infinity)\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            "The parameter --verbose gives more extensive information on\nthe properties of a transducer.\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-summarize.parse-options-fn]
// [spec:hfst:sem:hfst-summarize.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let print_symbol_pair_statistics_name =
                CString::new("print-symbol-pair-statistics").unwrap();
            long_options.push(getopt::Option {
                name: print_symbol_pair_statistics_name.as_ptr(),
                has_arg: 2, // optional_argument
                flag: std::ptr::null_mut(),
                val: 'S' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}S::",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            // add tool-specific options here
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
            match handle_common_case(c, || print_usage()) {
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
            if c == 'S' as c_int {
                PRINT_SYMBOL_PAIR_STATISTICS = true;
                if !getopt::OPTARG.is_null() {
                    let mut optarg = getopt::OPTARG as *const c_char;
                    if *optarg == b'=' as c_char {
                        optarg = optarg.add(1);
                    }
                    SYMBOL_PAIR_THRESHOLD = hfst_strtoul(&cstr(optarg), 10) as i32;
                    if SYMBOL_PAIR_THRESHOLD < 0 {
                        error(
                            1,
                            0,
                            &format!(
                                "{} is not a valid argument for option --print-symbol-pair-statistics\n",
                                SYMBOL_PAIR_THRESHOLD as u32
                            ),
                        );
                    }
                    if SYMBOL_PAIR_THRESHOLD == 0 {
                        error(
                            1,
                            0,
                            "0 is not a valid argument for option --print-symbol-pair-statistics\n",
                        );
                    }
                }
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-summarize.process-stream-fn]
// [spec:hfst:sem:hfst-summarize.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> c_int {
    unsafe {
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-summarize: cannot open output: {e}");
                return 1;
            }
        };
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;

            if transducer_n < 2 {
                verbose_printf("Summarizing...\n");
            } else {
                verbose_printf(&format!("Summarizing... {}\n", transducer_n));
            }
            let trans = HfstTransducer::new_from_stream(instream);
            let mutt = HfstBasicTransducer::new_from_transducer(&trans);
            let initial_state: u32 = 0; // mutt.get_initial_state();
            let mut transducer_alphabet: StringSet = StringSet::new();
            #[allow(unused_assignments)]
            let mut transducer_knows_alphabet = false;
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| trans.get_alphabet())) {
                Ok(alpha) => {
                    transducer_alphabet = alpha;
                    transducer_knows_alphabet = true;
                }
                Err(e) => {
                    if e.downcast_ref::<FunctionNotImplementedException>()
                        .is_some()
                    {
                        transducer_knows_alphabet = false;
                    } else {
                        std::panic::resume_unwind(e);
                    }
                }
            }
            //let expanded = true;
            #[allow(unused_assignments)]
            let mut is_mutable = true;
            //let input_label_sorted = false;
            //let output_label_sorted = false;
            #[allow(unused_assignments)]
            let mut weighted = true;
            //let topologically_sorted = false;
            //let accessible = true;
            //let coaccessible = true;
            //let is_string = true;
            //let minimised = false;
            // assign data from knowledge about source type
            match trans.get_type() {
                ImplementationType::SFST_TYPE => {
                    is_mutable = true;
                    weighted = false;
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    is_mutable = true;
                    weighted = true;
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    is_mutable = true;
                    weighted = true;
                }
                ImplementationType::FOMA_TYPE => {
                    is_mutable = true;
                    weighted = false;
                }
                ImplementationType::HFST_OL_TYPE => {
                    is_mutable = false;
                    weighted = false;
                }
                ImplementationType::HFST_OLW_TYPE => {
                    is_mutable = false;
                    weighted = true;
                }
                _ => {
                    is_mutable = false;
                }
            }

            let SummaryStats {
                states,
                final_states,
                arcs,
                io_epsilons,
                input_epsilons,
                output_epsilons,
                densest_arcs,
                sparsest_arcs,
                uniq_input_arcs,
                uniq_output_arcs,
                most_ambiguous_input,
                most_ambiguous_output,
                found_alphabet,
                symbol_pairs,
                acceptor,
                input_deterministic,
                output_deterministic,
                cyclic,
                cyclic_at_initial_state,
            } = mutt.summarize();
            // traverse

            // count physical size

            // average calculations
            let average_arcs_per_state = (arcs as f64) / (states as f32) as f64;
            let average_input_epsilons = (input_epsilons as f64) / (states as f64);
            let average_input_ambiguity = (arcs as f64) / (uniq_input_arcs as f64);
            let average_output_ambiguity = (arcs as f64) / (uniq_output_arcs as f64);
            let expected_arcs_per_symbol =
                (average_arcs_per_state) / (found_alphabet.len() as f32) as f64;

            if transducer_n > 1 {
                fput(&mut *out, &format!("-- \nTransducer #{}:\n", transducer_n));
            }
            fput(&mut *out, &format!("name: \"{}\"\n", trans.get_name()));
            // next is printed as in OpenFST's fstinfo
            // do not modify for compatibility
            match trans.get_type() {
                ImplementationType::SFST_TYPE => {
                    fput(&mut *out, "fst type: SFST\narc type: SFST\n");
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    fput(&mut *out, "fst type: OpenFST\narc type: tropical\n");
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    fput(&mut *out, "fst type: OpenFST\narc type: log\n");
                }
                ImplementationType::FOMA_TYPE => {
                    fput(&mut *out, "fst type: foma\narc type: foma\n");
                }
                ImplementationType::HFST_OL_TYPE => {
                    fput(
                        &mut *out,
                        "fst type: HFST optimized lookup\narc type: unweighted\n",
                    );
                }
                ImplementationType::HFST_OLW_TYPE => {
                    fput(
                        &mut *out,
                        "fst type: HFST optimized lookup\narc type: weighted\n",
                    );
                }
                _ => {
                    fput(&mut *out, "fst type: ???\narc type: ???\n");
                }
            }
            fput(
                &mut *out,
                &format!(
                    "input symbol table: yes\n\
                     output symbol table: yes\n\
                     # of states: {}\n\
                     # of arcs: {}\n\
                     initial state: {}\n\
                     # of final states: {}\n\
                     # of input/output epsilons: {}\n\
                     # of input epsilons: {}\n\
                     # of output epsilons: {}\n\
                     # of ... accessible states: ???\n\
                     # of ... coaccessible states: ???\n\
                     # of ... connected states: ???\n\
                     # of ... strongly conn components: ???\n",
                    states,
                    arcs,
                    initial_state as i64,
                    final_states,
                    io_epsilons,
                    input_epsilons,
                    output_epsilons
                ),
            );
            // other names from properties...
            fput(
                &mut *out,
                &format!(
                    "expanded: ???\n\
                     mutable: {}\n\
                     acceptor: {}\n\
                     input deterministic: {}\n\
                     output deterministic: {}\n\
                     input label sorted: ???\n\
                     output label sorted: ???\n\
                     weighted: {}\n\
                     cyclic: {}\n\
                     cyclic at initial state: {}\n\
                     topologically sorted: ???\n\
                     accessible: ???\n\
                     coaccessible: ???\n\
                     string: ???\n\
                     minimised: ???\n",
                    if is_mutable { "yes" } else { "no" },
                    if acceptor { "yes" } else { "no" },
                    if input_deterministic { "yes" } else { "no" },
                    if output_deterministic { "yes" } else { "no" },
                    if weighted { "yes" } else { "no" },
                    if cyclic { "yes" } else { "no" },
                    if cyclic_at_initial_state { "yes" } else { "no" }
                ),
            );
            if globals::VERBOSE {
                // our extensions for nice statistics maybe
                fput(
                    &mut *out,
                    &format!(
                        "number of arcs in sparsest state: {}\n\
                         number of arcs in densest state: {}\n\
                         average arcs per state: {:.6}\n\
                         average input epsilons per state: {:.6}\n\
                         most ambiguous input: {} {}\n\
                         most ambiguous output: {} {}\n\
                         average input ambiguity: {:.6}\n\
                         average output ambiguity: {:.6}\n\
                         expected arcs per symbol: {:.6}\n\
                         infinitely ambiguous: {}\n",
                        sparsest_arcs,
                        densest_arcs,
                        average_arcs_per_state,
                        average_input_epsilons,
                        most_ambiguous_input.0,
                        most_ambiguous_input.1,
                        most_ambiguous_output.0,
                        most_ambiguous_output.1,
                        average_input_ambiguity,
                        average_output_ambiguity,
                        expected_arcs_per_symbol,
                        if mutt.is_infinitely_ambiguous() {
                            "yes"
                        } else {
                            "no"
                        }
                    ),
                );
                // alphabets
                fput(&mut *out, "sigma set:\n");
                if transducer_knows_alphabet {
                    let mut first = true;
                    for s in transducer_alphabet.iter() {
                        if !first {
                            fput(&mut *out, ", ");
                        }
                        fput(&mut *out, s);
                        first = false;
                    }
                    fput(&mut *out, "\n");
                } else {
                    fput(&mut *out, "<Unknown in used transducer format>\n");
                }
                fput(&mut *out, "arc symbols actually seen in transducer:\n");
                let mut first = true;
                for s in found_alphabet.iter() {
                    if !first {
                        fput(&mut *out, ", ");
                    }
                    fput(&mut *out, s);
                    first = false;
                }
                fput(&mut *out, "\n");
                fput(&mut *out, "sigma symbols missing from transducer:\n");
                if transducer_knows_alphabet {
                    let transducer_minus_set: StringSet = transducer_alphabet
                        .difference(&found_alphabet)
                        .cloned()
                        .collect();

                    first = true;
                    for s in transducer_minus_set.iter() {
                        if !first {
                            fput(&mut *out, ", ");
                        }
                        fput(&mut *out, s);
                        first = false;
                    }
                    fput(&mut *out, "\n");
                } else {
                    fput(&mut *out, "<Unknown in used transducer format>\n");
                }
                // ADDED
                if trans.get_type() == ImplementationType::TROPICAL_OPENFST_TYPE {
                    let ss = trans.get_first_input_symbols();
                    fput(&mut *out, "first input symbols:\n");
                    first = true;
                    for s in ss.iter() {
                        if !first {
                            fput(&mut *out, ", ");
                        }
                        fput(&mut *out, s);
                        first = false;
                    }
                    fput(&mut *out, "\n");
                }
            } // if verbose

            // ADDED
            if PRINT_SYMBOL_PAIR_STATISTICS {
                if SYMBOL_PAIR_THRESHOLD > -1 {
                    fput(
                        &mut *out,
                        &format!(
                            "symbol pairs that occur at most {} times:\n",
                            SYMBOL_PAIR_THRESHOLD as u32
                        ),
                    );
                } else {
                    fput(&mut *out, "symbol pairs:\n");
                }
                for (key, value) in symbol_pairs.iter() {
                    // C: 'it->second <= symbol_pair_threshold' compares unsigned
                    // against int, promoting the int to unsigned; a -1 threshold
                    // wraps to UINT_MAX so every pair passes. Mirror with the same
                    // unsigned comparison.
                    if *value <= (SYMBOL_PAIR_THRESHOLD as u32) {
                        fput(&mut *out, &format!("{}:{}\t{}\n", key.0, key.1, value));
                    }
                }
                fput(&mut *out, "\n");
            }
        }

        fput(
            &mut *out,
            &format!("\nRead {} transducers in total.\n", transducer_n),
        );

        0
    }
}

// [spec:hfst:def:hfst-summarize.main-fn]
// [spec:hfst:sem:hfst-summarize.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt and
        // extend_options_getenv reorder/replace it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = cstr(globals::INPUTFILENAME) != "<stdin>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; on a bad file the
        // Rust ctor currently panics rather than throwing, so the catch arm that
        // reports "%s is not a valid transducer file" is not reproduced here.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };
        let retval = process_stream(&mut instream);

        if !globals::INPUTFILENAME.is_null() {
            hfst_cli::hfst_commandline::hfst_free(globals::INPUTFILENAME as *mut c_char);
        }
        if !globals::OUTFILENAME.is_null() {
            hfst_cli::hfst_commandline::hfst_free(globals::OUTFILENAME as *mut c_char);
        }
        let _ = retval;
        0
    }
}
