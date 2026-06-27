#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-reweight.cc — the transducer reweighting
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! This is a unary tool (it #includes inc/globals-common.h and
//! inc/globals-unary.h and reads a single input stream).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_error, hfst_error_at_line, hfst_fopen, hfst_getline,
    hfst_set_program_name, hfst_strdup, hfst_strndup, hfst_strtoweight, hfst_warning,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::collections::BTreeMap;
use std::ffi::{CStr, CString};

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

// add tools-specific variables here
// [spec:hfst:def:hfst-reweight.id-fn]
// [spec:hfst:sem:hfst-reweight.id-fn]
fn id(w: f32) -> f32 {
    w
}
static mut ADDITION: f32 = 0.0;
static mut MULTIPLIER: f32 = 1.0;
static mut FUNCNAME: *mut c_char = std::ptr::null_mut();
// [spec:hfst:def:hfst-reweight.func-fn]
// [spec:hfst:sem:hfst-reweight.func-fn]
static mut FUNC: fn(f32) -> f32 = id;
static mut UPPER_BOUND: f32 = f32::MAX;
static mut LOWER_BOUND: f32 = 0.0;
static mut INPUT_SYMBOL: *mut c_char = std::ptr::null_mut();
static mut OUTPUT_SYMBOL: *mut c_char = std::ptr::null_mut();
static mut SYMBOL: *mut c_char = std::ptr::null_mut();
static mut ENDS_ONLY: bool = false;
static mut ARCS_ONLY: bool = false;
static mut TSV_FILE_NAME: *mut c_char = std::ptr::null_mut();
static mut TSV_FILE: *mut libc::FILE = std::ptr::null_mut();

// [spec:hfst:def:hfst-reweight.print-usage-fn]
// [spec:hfst:sem:hfst-reweight.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nReweight transducer weights simply\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Reweighting options:\n\
  -a, --addition=AVAL        add AVAL to matching weights\n\
  -b, --multiplier=BVAL      multiply matching weights by BVAL\n\
  -F, --function=FNAME       operate matching weights by FNAME\n\
  -l, --lower-bound=LVAL     match weights greater than LVAL\n\
  -u, --upper-bound=UVAL     match weights less than UVAL\n\
  -I, --input-symbol=ISYM    match arcs with input symbol ISYM\n\
  -O, --output-symbol=OSYM   match arcs with output symbol OSYM\n\
  -S, --symbol=SYM           match arcs with input or output symbol SYM or both\n\
  -e, --end-states-only      match end states only, no arcs\n\
  -A, --arcs-only            match arcs only, no end states\n\
  -T, --tsv-file=TFILE       read reweighting rules from TFILE\n\
\n",
        );
        fput(globals::message_out(), "\n");
        print_common_unary_program_parameter_instructions(globals::message_out());
        fput(
            globals::message_out(),
            "If AVAL, BVAL or FNAME are omitted, they default to neutral \
elements of addition, multiplication or identity function.\n\
If LVAL or UVAL are omitted, they default to minimum and maximum \
values of the weight structure.\n\
If ISYM, OSYM or SYM are omitted, they default to a value that \
matches all arcs.\nOnly one ISYM, OSYM and SYM can be given.\n\n\
Float values are parsed with strtod(3) and integers strtoul(3).\n\
The functions allowed for FNAME are <cmath> float functions with \
parameter count of 1 and a matching return value:\n\
abs, acos, asin, ... sqrt, tan, tanh\n\n\
The precedence of operands follows the formula \
BVAL * FNAME(w) + AVAL.\n\
The formula is applied iff:\n\
((LVAL <= w) && (w <= UVAL)),\n\
where w is weight of arc, and \n\
(ISYM == i) && (OSYM == o) && ((SYM == i) || (SYM == o)) ^^ \n\
(end state && -e).\n\n\
TFILE should contain lines with tab-separated pairs of SYM and \
AVAL or BVAL. AVAL values must be preceded by a + character, \
BVAL should be given as plain digits. \
Comment lines starting with # and empty lines are ignored.\n\n\
Weights are by default modified for all arcs and end states,\n\
unless option --end-states-only or --arcs-only is used.\n",
        );
        fput(globals::message_out(), "\n");
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-reweight.parse-options-fn]
// [spec:hfst:sem:hfst-reweight.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let opt_names = [
                "addition",
                "multiplier",
                "function",
                "lower-bound",
                "upper-bound",
                "input-symbol",
                "output-symbol",
                "symbol",
                "end-states-only",
                "arcs-only",
                "tsv",
            ];
            let opt_specs: [(i32, c_int); 11] = [
                (1, 'a' as c_int),
                (1, 'b' as c_int),
                (1, 'F' as c_int),
                (1, 'l' as c_int),
                (1, 'u' as c_int),
                (1, 'I' as c_int),
                (1, 'O' as c_int),
                (1, 'S' as c_int),
                (0, 'e' as c_int),
                (0, 'A' as c_int),
                (1, 'T' as c_int),
            ];
            // keep the CStrings alive for the duration of getopt_long
            let opt_cstrings: Vec<CString> = opt_names
                .iter()
                .map(|n| CString::new(*n).unwrap())
                .collect();
            for (i, cs) in opt_cstrings.iter().enumerate() {
                long_options.push(getopt::Option {
                    name: cs.as_ptr(),
                    has_arg: opt_specs[i].0,
                    flag: std::ptr::null_mut(),
                    val: opt_specs[i].1,
                });
            }
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}a:b:F:l:u:I:O:S:eT:A",
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
            // tool-specific cases
            let optarg = getopt::OPTARG;
            match c as u8 as char {
                'a' => {
                    ADDITION = hfst_strtoweight(&cstr(optarg));
                    continue;
                }
                'b' => {
                    MULTIPLIER = hfst_strtoweight(&cstr(optarg));
                    continue;
                }
                'F' => {
                    FUNCNAME = hfst_strdup(optarg);
                    let name = cstr(optarg);
                    match name.as_str() {
                        "cos" => FUNC = f32::cos,
                        "sin" => FUNC = f32::sin,
                        "tan" => FUNC = f32::tan,
                        "acos" => FUNC = f32::acos,
                        "asin" => FUNC = f32::asin,
                        "atan" => FUNC = f32::atan,
                        "cosh" => FUNC = f32::cosh,
                        "sinh" => FUNC = f32::sinh,
                        "tanh" => FUNC = f32::tanh,
                        "exp" => FUNC = f32::exp,
                        "log" => FUNC = f32::ln,
                        "log10" => FUNC = f32::log10,
                        "sqrt" => FUNC = f32::sqrt,
                        "floor" => FUNC = f32::floor,
                        "ceil" => FUNC = f32::ceil,
                        _ => {
                            hfst_error(
                                libc::EXIT_FAILURE,
                                0,
                                &format!("Cannot parse {} as function name", name),
                            );
                            return libc::EXIT_FAILURE;
                        }
                    }
                    continue;
                }
                'l' => {
                    LOWER_BOUND = hfst_strtoweight(&cstr(optarg));
                    continue;
                }
                'u' => {
                    UPPER_BOUND = hfst_strtoweight(&cstr(optarg));
                    continue;
                }
                'I' => {
                    INPUT_SYMBOL = hfst_strdup(optarg);
                    continue;
                }
                'O' => {
                    OUTPUT_SYMBOL = hfst_strdup(optarg);
                    continue;
                }
                'S' => {
                    SYMBOL = hfst_strdup(optarg);
                    continue;
                }
                'e' => {
                    ENDS_ONLY = true;
                    continue;
                }
                'A' => {
                    ARCS_ONLY = true;
                    continue;
                }
                'T' => {
                    TSV_FILE_NAME = hfst_strdup(optarg);
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        if ARCS_ONLY && ENDS_ONLY {
            hfst_error(
                libc::EXIT_FAILURE,
                0,
                "Options '--arcs-only' and '--end-states-only' cannot be used \
at the same time",
            );
            return libc::EXIT_FAILURE;
        }

        check_common_params();
        check_unary_params(argc, argv);
        if FUNCNAME.is_null() {
            let cs = CString::new("id").unwrap();
            FUNCNAME = hfst_strdup(cs.as_ptr());
        }
        if UPPER_BOUND < LOWER_BOUND {
            hfst_warning(
                0,
                0,
                &format!(
                    "Lower bound {} exceeds upper bound {} so reweight will \
never apply",
                    LOWER_BOUND, UPPER_BOUND
                ),
            );
        }
        if !TSV_FILE_NAME.is_null() {
            TSV_FILE = hfst_fopen(&cstr(TSV_FILE_NAME), "r");
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-reweight.reweight-fn]
// [spec:hfst:sem:hfst-reweight.reweight-fn]
unsafe fn reweight(w: f32, i: Option<&str>, o: Option<&str>) -> f32 {
    unsafe {
        if (w < LOWER_BOUND) || (w > UPPER_BOUND) {
            // not within weight bounds, don't apply
            return w;
        }
        if i.is_none() && o.is_none() {
            if ARCS_ONLY {
                return w;
            }
        } else if i.is_some() && o.is_some() {
            let i = i.unwrap();
            let o = o.unwrap();
            if ENDS_ONLY {
                return w;
            }
            if !SYMBOL.is_null() {
                let symbol = cstr(SYMBOL);
                if i != symbol && o != symbol {
                    // symbol doesn't match, don't apply
                    return w;
                }
            }
            if !INPUT_SYMBOL.is_null() && !OUTPUT_SYMBOL.is_null() {
                let isym = cstr(INPUT_SYMBOL);
                let osym = cstr(OUTPUT_SYMBOL);
                if i != isym && o != osym {
                    // input doesn't match, don't apply
                    return w;
                }
            } else if !INPUT_SYMBOL.is_null() {
                if i != cstr(INPUT_SYMBOL) {
                    // input doesn't match, don't apply
                    return w;
                }
            } else if !OUTPUT_SYMBOL.is_null() {
                if o != cstr(OUTPUT_SYMBOL) {
                    // output doesn't match, don't apply
                    return w;
                }
            }
        }
        MULTIPLIER * (FUNC)(w) + ADDITION
    }
}

unsafe fn do_reweight(trans: &mut HfstTransducer) {
    unsafe {
        // [spec:hfst:def:hfst-reweight.original-fn]
        // [spec:hfst:sem:hfst-reweight.original-fn]
        let original = HfstBasicTransducer::from_hfst_transducer(trans);
        let mut replication = HfstBasicTransducer::new();
        let mut state_count: u32 = 1;
        let mut rebuilt: BTreeMap<u32, u32> = BTreeMap::new();
        rebuilt.insert(0, 0); // HfstBasicTransducer initially has state number zero
        if original.is_final_state(0) {
            let nuweight = reweight(original.get_final_weight(0), None, None);
            replication.set_final_weight(0, &nuweight);
        }
        let mut source_state: u32 = 0;
        for state in original.states_and_transitions().iter() {
            if !rebuilt.contains_key(&source_state) {
                replication.add_state(state_count);
                if original.is_final_state(source_state) {
                    let nuweight = reweight(original.get_final_weight(source_state), None, None);
                    replication.set_final_weight(state_count, &nuweight);
                }
                rebuilt.insert(source_state, state_count);
                state_count += 1;
            }
            for arc in state.iter() {
                let target = arc.get_target_state();
                if !rebuilt.contains_key(&target) {
                    replication.add_state(state_count);
                    if original.is_final_state(target) {
                        let nuweight = reweight(original.get_final_weight(target), None, None);
                        replication.set_final_weight(state_count, &nuweight);
                    }
                    rebuilt.insert(target, state_count);
                    state_count += 1;
                }
                let isym = arc.get_input_symbol();
                let osym = arc.get_output_symbol();
                let nuweight = reweight(arc.get_weight(), Some(&isym), Some(&osym));
                let nu = HfstBasicTransition::new_symbols(
                    *rebuilt.get(&target).unwrap(),
                    isym,
                    osym,
                    nuweight,
                );
                replication.add_transition(*rebuilt.get(&source_state).unwrap(), &nu, true);
            }
            source_state += 1;
        }
        *trans = HfstTransducer::new_from_basic(&replication, trans.get_type());
    }
}

// [spec:hfst:def:hfst-reweight.process-stream-fn]
// [spec:hfst:sem:hfst-reweight.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            if trans.get_type() == ImplementationType::FOMA_TYPE {
                hfst_warning(
                    0,
                    0,
                    "Weighting is not supported in this automaton type;\
weights will be discarded",
                );
            }
            let inputname = hfst_get_name(&trans, &cstr(globals::INPUTFILENAME));
            if transducer_n == 1 {
                verbose_printf(&format!("Reweighting {}...\n", inputname));
            } else {
                verbose_printf(&format!("Reweighting {}...{}\n", inputname, transducer_n));
            }
            if TSV_FILE.is_null() {
                do_reweight(&mut trans);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "reweight");
                hfst_set_formula_unary(&mut trans, &src, "W");
            } else {
                libc::rewind(TSV_FILE);
                libc::free(SYMBOL as *mut libc::c_void);
                SYMBOL = std::ptr::null_mut();
                ADDITION = 0.0;
                MULTIPLIER = 1.0;
                let mut line: *mut c_char = std::ptr::null_mut();
                let mut len: libc::size_t = 0;
                let mut linen: usize = 0;
                verbose_printf(&format!("Reading reweights from {}\n", cstr(TSV_FILE_NAME)));
                while hfst_getline(&mut line, &mut len, TSV_FILE) != -1 && !line.is_null() {
                    linen += 1;
                    if *line == b'\n' as c_char {
                        continue;
                    }
                    if *line == b'#' as c_char {
                        continue;
                    }
                    let line_str = CStr::from_ptr(line).to_bytes();
                    let tab_pos = line_str.iter().position(|&b| b == b'\t');
                    let tab = match tab_pos {
                        None => {
                            hfst_error_at_line(
                                libc::EXIT_FAILURE,
                                0,
                                &cstr(TSV_FILE_NAME),
                                linen as u32,
                                "at least one tab required per line",
                            );
                            continue;
                        }
                        Some(p) => p,
                    };
                    // endstr advances from tab+1 to first '\0' or '\n'
                    let mut endstr = tab + 1;
                    while endstr < line_str.len() && line_str[endstr] != b'\n' {
                        endstr += 1;
                    }
                    SYMBOL = hfst_strndup(line, tab);
                    let weightspec = hfst_strndup(line.add(tab + 1), endstr - tab - 1);
                    if *weightspec == b'+' as c_char {
                        ADDITION = hfst_strtoweight(&cstr(weightspec.add(1)));
                    } else {
                        MULTIPLIER = hfst_strtoweight(&cstr(weightspec));
                    }
                    libc::free(weightspec as *mut libc::c_void);
                    verbose_printf(&format!(
                        "Modifying weights {} < w < {} as {} * {}(w) + {} for symbol {}\n",
                        LOWER_BOUND,
                        UPPER_BOUND,
                        MULTIPLIER,
                        cstr(FUNCNAME),
                        ADDITION,
                        cstr(SYMBOL)
                    ));
                    do_reweight(&mut trans);
                } // getline
                libc::free(line as *mut libc::c_void);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "reweight");
                hfst_set_formula_unary(&mut trans, &src, "W");
            } // if tsv_file
            outstream.redirect(trans.remove_epsilons());
        } // foreach transducer
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-reweight.main-fn]
// [spec:hfst:sem:hfst-reweight.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstReweight");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = !globals::INPUTFILE.is_null();
        let output_opened = !globals::OUTFILE.is_null();
        if input_opened {
            libc::fclose(globals::INPUTFILE);
        }
        if output_opened {
            libc::fclose(globals::OUTFILE);
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        verbose_printf(&format!(
            "Modifying weights {} < w < {} as {} * {}(w) + {}\n",
            LOWER_BOUND,
            UPPER_BOUND,
            MULTIPLIER,
            cstr(FUNCNAME),
            ADDITION
        ));
        if !SYMBOL.is_null() {
            verbose_printf(&format!("only if arc has symbol {}\n", cstr(SYMBOL)));
        }
        if !INPUT_SYMBOL.is_null() {
            verbose_printf(&format!("only if input symbol is {}\n", cstr(INPUT_SYMBOL)));
        }
        if !OUTPUT_SYMBOL.is_null() {
            verbose_printf(&format!(
                "only if output symbol is {}\n",
                cstr(OUTPUT_SYMBOL)
            ));
        }
        if ENDS_ONLY {
            verbose_printf("only on final weights, no arcs\n");
        }
        if ARCS_ONLY {
            verbose_printf("only on arc weights, no end states\n");
        }

        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        let type_ = instream.get_type();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        if is_input_stream_in_ol_format(&instream, "hfst-reweight") {
            return libc::EXIT_FAILURE;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
