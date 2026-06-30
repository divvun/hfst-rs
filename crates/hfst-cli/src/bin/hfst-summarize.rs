//! Faithful 1:1 port of tools/src/hfst-summarize.cc — the transducer
//! information / properties command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options, inc fragments).

use hfst::hfst_basic_transducer::{HfstBasicTransducer, SummaryStats};
use hfst::hfst_data_types::ImplementationType;
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
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::Write;

// add tools-specific variables here
static mut PRINT_SYMBOL_PAIR_STATISTICS: bool = false;
static mut SYMBOL_PAIR_THRESHOLD: i32 = -1;

// [spec:hfst:def:hfst-summarize.print-usage-fn]
// [spec:hfst:sem:hfst-summarize.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nCalculate the properties of a transducer\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    // (tool-specific options and short descriptions)
    let _ = write!(msg, "Summarize options:\n");
    let _ = write!(
        msg,
        "  -S, --print-symbol-pair-statistics=N  Print info about symbol pairs that occur\n",
    );
    let _ = write!(
        msg,
        "                                        at most N times (default is infinity)\n",
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "The parameter --verbose gives more extensive information on\nthe properties of a transducer.\n",
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-summarize.parse-options-fn]
// [spec:hfst:sem:hfst-summarize.parse-options-fn]
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
                name: "print-symbol-pair-statistics",
                has_arg: getopt::OPTIONAL_ARGUMENT,
                val: 'S' as i32,
            });
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
            // add tool-specific cases here
            if c == 'S' as i32 {
                PRINT_SYMBOL_PAIR_STATISTICS = true;
                if let Some(mut optarg) = getopt::optarg_opt() {
                    if let Some(rest) = optarg.strip_prefix('=') {
                        optarg = rest.to_string();
                    }
                    SYMBOL_PAIR_THRESHOLD = hfst_strtoul(&optarg, 10) as i32;
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
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-summarize.process-stream-fn]
// [spec:hfst:sem:hfst-summarize.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> i32 {
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
                    if e.downcast_ref::<hfst::error::Error>()
                        .filter(|__e| {
                            matches!(__e.kind, hfst::error::ErrorKind::FunctionNotImplemented)
                        })
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
                let _ = write!(out, "-- \nTransducer #{}:\n", transducer_n);
            }
            let _ = write!(out, "name: \"{}\"\n", trans.get_name());
            // next is printed as in OpenFST's fstinfo
            // do not modify for compatibility
            match trans.get_type() {
                ImplementationType::SFST_TYPE => {
                    let _ = write!(out, "fst type: SFST\narc type: SFST\n");
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    let _ = write!(out, "fst type: OpenFST\narc type: tropical\n");
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    let _ = write!(out, "fst type: OpenFST\narc type: log\n");
                }
                ImplementationType::FOMA_TYPE => {
                    let _ = write!(out, "fst type: foma\narc type: foma\n");
                }
                ImplementationType::HFST_OL_TYPE => {
                    let _ = write!(
                        out,
                        "fst type: HFST optimized lookup\narc type: unweighted\n"
                    );
                }
                ImplementationType::HFST_OLW_TYPE => {
                    let _ = write!(out, "fst type: HFST optimized lookup\narc type: weighted\n");
                }
                _ => {
                    let _ = write!(out, "fst type: ???\narc type: ???\n");
                }
            }
            let _ = write!(
                out,
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
            );
            // other names from properties...
            let _ = write!(
                out,
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
            );
            if globals::VERBOSE {
                // our extensions for nice statistics maybe
                let _ = write!(
                    out,
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
                );
                // alphabets
                let _ = write!(out, "sigma set:\n");
                if transducer_knows_alphabet {
                    let mut first = true;
                    for s in transducer_alphabet.iter() {
                        if !first {
                            let _ = write!(out, ", ");
                        }
                        let _ = write!(out, "{}", s);
                        first = false;
                    }
                    let _ = write!(out, "\n");
                } else {
                    let _ = write!(out, "<Unknown in used transducer format>\n");
                }
                let _ = write!(out, "arc symbols actually seen in transducer:\n");
                let mut first = true;
                for s in found_alphabet.iter() {
                    if !first {
                        let _ = write!(out, ", ");
                    }
                    let _ = write!(out, "{}", s);
                    first = false;
                }
                let _ = write!(out, "\n");
                let _ = write!(out, "sigma symbols missing from transducer:\n");
                if transducer_knows_alphabet {
                    let transducer_minus_set: StringSet = transducer_alphabet
                        .difference(&found_alphabet)
                        .cloned()
                        .collect();

                    first = true;
                    for s in transducer_minus_set.iter() {
                        if !first {
                            let _ = write!(out, ", ");
                        }
                        let _ = write!(out, "{}", s);
                        first = false;
                    }
                    let _ = write!(out, "\n");
                } else {
                    let _ = write!(out, "<Unknown in used transducer format>\n");
                }
                // ADDED
                if trans.get_type() == ImplementationType::TROPICAL_OPENFST_TYPE {
                    let ss = trans.get_first_input_symbols();
                    let _ = write!(out, "first input symbols:\n");
                    first = true;
                    for s in ss.iter() {
                        if !first {
                            let _ = write!(out, ", ");
                        }
                        let _ = write!(out, "{}", s);
                        first = false;
                    }
                    let _ = write!(out, "\n");
                }
            } // if verbose

            // ADDED
            if PRINT_SYMBOL_PAIR_STATISTICS {
                if SYMBOL_PAIR_THRESHOLD > -1 {
                    let _ = write!(
                        out,
                        "symbol pairs that occur at most {} times:\n",
                        SYMBOL_PAIR_THRESHOLD as u32
                    );
                } else {
                    let _ = write!(out, "symbol pairs:\n");
                }
                for (key, value) in symbol_pairs.iter() {
                    // C: 'it->second <= symbol_pair_threshold' compares unsigned
                    // against int, promoting the int to unsigned; a -1 threshold
                    // wraps to UINT_MAX so every pair passes. Mirror with the same
                    // unsigned comparison.
                    if *value <= (SYMBOL_PAIR_THRESHOLD as u32) {
                        let _ = write!(out, "{}:{}\t{}\n", key.0, key.1, value);
                    }
                }
                let _ = write!(out, "\n");
            }
        }

        let _ = write!(out, "\nRead {} transducers in total.\n", transducer_n);

        0
    }
}

// [spec:hfst:def:hfst-summarize.main-fn]
// [spec:hfst:sem:hfst-summarize.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; on a bad file the
        // Rust ctor currently panics rather than throwing, so the catch arm that
        // reports "%s is not a valid transducer file" is not reproduced here.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        let retval = process_stream(&mut instream);
        let _ = retval;
        0
    }
}
