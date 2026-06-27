//! Faithful 1:1 port of tools/src/hfst-summarize.cc — the transducer
//! information / properties command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options, inc fragments).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_exception_defs::FunctionNotImplementedException;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::{StringSet, is_epsilon};
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
use libc::{c_char, c_int};
use std::collections::BTreeMap;
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

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

// [spec:hfst:def:hfst-summarize.print-usage-fn]
// [spec:hfst:sem:hfst-summarize.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nCalculate the properties of a transducer\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        // (tool-specific options and short descriptions)
        fput(globals::message_out(), "Summarize options:\n");
        fput(
            globals::message_out(),
            "  -S, --print-symbol-pair-statistics=N  Print info about symbol pairs that occur\n",
        );
        fput(
            globals::message_out(),
            "                                        at most N times (default is infinity)\n",
        );
        fput(globals::message_out(), "\n");
        print_common_unary_program_parameter_instructions(globals::message_out());
        fput(globals::message_out(), "\n");
        fput(
            globals::message_out(),
            "The parameter --verbose gives more extensive information on\nthe properties of a transducer.\n",
        );
        fput(globals::message_out(), "\n");
        print_report_bugs();
        fput(globals::message_out(), "\n");
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
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "{} is not a valid argument for option --print-symbol-pair-statistics\n",
                                SYMBOL_PAIR_THRESHOLD as u32
                            ),
                        );
                    }
                    if SYMBOL_PAIR_THRESHOLD == 0 {
                        error(
                            libc::EXIT_FAILURE,
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
        let outfile = globals::outfile();
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
            let mut states: usize = 0;
            let mut final_states: usize = 0;
            //let paths: usize = 0;
            let mut arcs: usize = 0;
            //let sccs: usize = 0;
            let mut io_epsilons: usize = 0;
            let mut input_epsilons: usize = 0;
            let mut output_epsilons: usize = 0;
            // others
            let mut densest_arcs: usize = 0;
            let mut sparsest_arcs: usize = 1 << 31;
            let mut uniq_input_arcs: usize = 0;
            let mut uniq_output_arcs: usize = 0;
            let mut most_ambiguous_input: (String, u32) = (String::new(), 0);
            let mut most_ambiguous_output: (String, u32) = (String::new(), 0);
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
            let mut found_alphabet: StringSet = StringSet::new();
            //let expanded = true;
            #[allow(unused_assignments)]
            let mut is_mutable = true;
            let mut acceptor = true;
            let mut input_deterministic = true;
            let mut output_deterministic = true;
            //let input_label_sorted = false;
            //let output_label_sorted = false;
            #[allow(unused_assignments)]
            let mut weighted = true;
            let mut cyclic = false;
            let mut cyclic_at_initial_state = false;
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

            let mut symbol_pairs: BTreeMap<(String, String), u32> = BTreeMap::new();
            // iterate states in random order
            let mut source_state: u32 = 0;
            let is_begin_state = |s: u32| s == 0;
            for transitions in mutt.states_and_transitions() {
                let s = source_state;
                states += 1;
                if mutt.is_final_state(s) {
                    final_states += 1;
                }
                let mut arcs_here: usize = 0;
                let mut input_ambiguity: BTreeMap<String, u32> = BTreeMap::new();
                let mut output_ambiguity: BTreeMap<String, u32> = BTreeMap::new();

                for tr_it in transitions {
                    arcs += 1;
                    arcs_here += 1;
                    found_alphabet.insert(tr_it.get_input_symbol());
                    found_alphabet.insert(tr_it.get_output_symbol());

                    // ADDED
                    if PRINT_SYMBOL_PAIR_STATISTICS {
                        *symbol_pairs
                            .entry((tr_it.get_input_symbol(), tr_it.get_output_symbol()))
                            .or_insert(0) += 1;
                    }

                    if tr_it.get_input_symbol() != tr_it.get_output_symbol() {
                        acceptor = false;
                    }
                    if is_epsilon(&tr_it.get_input_symbol())
                        && is_epsilon(&tr_it.get_output_symbol())
                    {
                        io_epsilons += 1;
                        input_epsilons += 1;
                        output_epsilons += 1;
                        input_deterministic = false;
                        output_deterministic = false;
                    } else if is_epsilon(&tr_it.get_input_symbol()) {
                        input_epsilons += 1;
                        input_deterministic = false;
                    } else if is_epsilon(&tr_it.get_output_symbol()) {
                        output_epsilons += 1;
                        output_deterministic = false;
                    }
                    input_ambiguity.entry(tr_it.get_input_symbol()).or_insert(0);
                    output_ambiguity
                        .entry(tr_it.get_output_symbol())
                        .or_insert(0);
                    let in_amb = input_ambiguity.get_mut(&tr_it.get_input_symbol()).unwrap();
                    *in_amb += 1;
                    if *in_amb > 1 {
                        input_deterministic = false;
                    }
                    let out_amb = output_ambiguity
                        .get_mut(&tr_it.get_output_symbol())
                        .unwrap();
                    *out_amb += 1;
                    if *out_amb > 1 {
                        output_deterministic = false;
                    }
                    if is_begin_state(source_state) && (tr_it.get_target_state() == 0) {
                        cyclic = true;
                        cyclic_at_initial_state = true;
                    }
                    if source_state == tr_it.get_target_state() {
                        cyclic = true;
                    }
                }
                if arcs_here > densest_arcs {
                    densest_arcs = arcs_here;
                }
                if arcs_here < sparsest_arcs {
                    sparsest_arcs = arcs_here;
                }
                for (key, value) in input_ambiguity.iter() {
                    if *value > most_ambiguous_input.1 {
                        most_ambiguous_input.0 = key.clone();
                        most_ambiguous_input.1 = *value;
                    }
                    uniq_input_arcs += 1;
                }
                for (key, value) in output_ambiguity.iter() {
                    if *value > most_ambiguous_output.1 {
                        most_ambiguous_output.0 = key.clone();
                        most_ambiguous_output.1 = *value;
                    }
                    uniq_output_arcs += 1;
                }
                source_state += 1;
            }
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
                fput(outfile, &format!("-- \nTransducer #{}:\n", transducer_n));
            }
            fput(outfile, &format!("name: \"{}\"\n", trans.get_name()));
            // next is printed as in OpenFST's fstinfo
            // do not modify for compatibility
            match trans.get_type() {
                ImplementationType::SFST_TYPE => {
                    fput(outfile, "fst type: SFST\narc type: SFST\n");
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    fput(outfile, "fst type: OpenFST\narc type: tropical\n");
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    fput(outfile, "fst type: OpenFST\narc type: log\n");
                }
                ImplementationType::FOMA_TYPE => {
                    fput(outfile, "fst type: foma\narc type: foma\n");
                }
                ImplementationType::HFST_OL_TYPE => {
                    fput(
                        outfile,
                        "fst type: HFST optimized lookup\narc type: unweighted\n",
                    );
                }
                ImplementationType::HFST_OLW_TYPE => {
                    fput(
                        outfile,
                        "fst type: HFST optimized lookup\narc type: weighted\n",
                    );
                }
                _ => {
                    fput(outfile, "fst type: ???\narc type: ???\n");
                }
            }
            fput(
                outfile,
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
                outfile,
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
                    outfile,
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
                fput(outfile, "sigma set:\n");
                if transducer_knows_alphabet {
                    let mut first = true;
                    for s in transducer_alphabet.iter() {
                        if !first {
                            fput(outfile, ", ");
                        }
                        fput(outfile, s);
                        first = false;
                    }
                    fput(outfile, "\n");
                } else {
                    fput(outfile, "<Unknown in used transducer format>\n");
                }
                fput(outfile, "arc symbols actually seen in transducer:\n");
                let mut first = true;
                for s in found_alphabet.iter() {
                    if !first {
                        fput(outfile, ", ");
                    }
                    fput(outfile, s);
                    first = false;
                }
                fput(outfile, "\n");
                fput(outfile, "sigma symbols missing from transducer:\n");
                if transducer_knows_alphabet {
                    let transducer_minus_set: StringSet = transducer_alphabet
                        .difference(&found_alphabet)
                        .cloned()
                        .collect();

                    first = true;
                    for s in transducer_minus_set.iter() {
                        if !first {
                            fput(outfile, ", ");
                        }
                        fput(outfile, s);
                        first = false;
                    }
                    fput(outfile, "\n");
                } else {
                    fput(outfile, "<Unknown in used transducer format>\n");
                }
                // ADDED
                if trans.get_type() == ImplementationType::TROPICAL_OPENFST_TYPE {
                    let ss = trans.get_first_input_symbols();
                    fput(outfile, "first input symbols:\n");
                    first = true;
                    for s in ss.iter() {
                        if !first {
                            fput(outfile, ", ");
                        }
                        fput(outfile, s);
                        first = false;
                    }
                    fput(outfile, "\n");
                }
            } // if verbose

            // ADDED
            if PRINT_SYMBOL_PAIR_STATISTICS {
                if SYMBOL_PAIR_THRESHOLD > -1 {
                    fput(
                        outfile,
                        &format!(
                            "symbol pairs that occur at most {} times:\n",
                            SYMBOL_PAIR_THRESHOLD as u32
                        ),
                    );
                } else {
                    fput(outfile, "symbol pairs:\n");
                }
                for (key, value) in symbol_pairs.iter() {
                    // C: 'it->second <= symbol_pair_threshold' compares unsigned
                    // against int, promoting the int to unsigned; a -1 threshold
                    // wraps to UINT_MAX so every pair passes. Mirror with the same
                    // unsigned comparison.
                    if *value <= (SYMBOL_PAIR_THRESHOLD as u32) {
                        fput(outfile, &format!("{}:{}\t{}\n", key.0, key.1, value));
                    }
                }
                fput(outfile, "\n");
            }
        }

        fput(
            outfile,
            &format!("\nRead {} transducers in total.\n", transducer_n),
        );

        libc::EXIT_SUCCESS
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
        let input_opened = !globals::INPUTFILE.is_null();
        if input_opened {
            libc::fclose(globals::INPUTFILE);
        }
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

        let output_opened = !globals::OUTFILE.is_null();
        if output_opened {
            libc::fclose(globals::OUTFILE);
        }
        if !globals::INPUTFILENAME.is_null() {
            libc::free(globals::INPUTFILENAME as *mut libc::c_void);
        }
        if !globals::OUTFILENAME.is_null() {
            libc::free(globals::OUTFILENAME as *mut libc::c_void);
        }
        let _ = retval;
        libc::EXIT_SUCCESS
    }
}
