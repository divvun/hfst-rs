#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-reweight.cc — the transducer reweighting
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! This is a unary tool (it #includes inc/globals-common.h and
//! inc/globals-unary.h and reads a single input stream).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_error, hfst_error_at_line,
    hfst_set_program_name, hfst_strtoweight, hfst_warning, is_input_stream_in_ol_format,
    print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

// add tools-specific variables here
// [spec:hfst:def:hfst-reweight.id-fn]
// [spec:hfst:sem:hfst-reweight.id-fn]
fn id(w: f32) -> f32 {
    w
}
static mut ADDITION: f32 = 0.0;
static mut MULTIPLIER: f32 = 1.0;
static mut FUNCNAME: Option<String> = None;
// [spec:hfst:def:hfst-reweight.func-fn]
// [spec:hfst:sem:hfst-reweight.func-fn]
static mut FUNC: fn(f32) -> f32 = id;
static mut UPPER_BOUND: f32 = f32::MAX;
static mut LOWER_BOUND: f32 = 0.0;
static mut INPUT_SYMBOL: Option<String> = None;
static mut OUTPUT_SYMBOL: Option<String> = None;
static mut SYMBOL: Option<String> = None;
static mut ENDS_ONLY: bool = false;
static mut ARCS_ONLY: bool = false;
static mut TSV_FILE_NAME: Option<String> = None;
static mut TSV_FILE: Option<std::fs::File> = None;

// [spec:hfst:def:hfst-reweight.print-usage-fn]
// [spec:hfst:sem:hfst-reweight.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nReweight transducer weights simply\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
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
\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
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
unless option --end-states-only or --arcs-only is used.\n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-reweight.parse-options-fn]
// [spec:hfst:sem:hfst-reweight.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let tool_opts: [(&'static str, i32, i32); 11] = [
                ("addition", getopt::REQUIRED_ARGUMENT, 'a' as i32),
                ("multiplier", getopt::REQUIRED_ARGUMENT, 'b' as i32),
                ("function", getopt::REQUIRED_ARGUMENT, 'F' as i32),
                ("lower-bound", getopt::REQUIRED_ARGUMENT, 'l' as i32),
                ("upper-bound", getopt::REQUIRED_ARGUMENT, 'u' as i32),
                ("input-symbol", getopt::REQUIRED_ARGUMENT, 'I' as i32),
                ("output-symbol", getopt::REQUIRED_ARGUMENT, 'O' as i32),
                ("symbol", getopt::REQUIRED_ARGUMENT, 'S' as i32),
                ("end-states-only", getopt::NO_ARGUMENT, 'e' as i32),
                ("arcs-only", getopt::NO_ARGUMENT, 'A' as i32),
                ("tsv", getopt::REQUIRED_ARGUMENT, 'T' as i32),
            ];
            for (name, has_arg, val) in tool_opts {
                long_options.push(getopt::GetOpt { name, has_arg, val });
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
            // tool-specific cases
            match c as u8 as char {
                'a' => {
                    ADDITION = hfst_strtoweight(&getopt::optarg());
                    continue;
                }
                'b' => {
                    MULTIPLIER = hfst_strtoweight(&getopt::optarg());
                    continue;
                }
                'F' => {
                    let name = getopt::optarg();
                    FUNCNAME = Some(name.clone());
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
                            hfst_error(1, 0, &format!("Cannot parse {} as function name", name));
                            return 1;
                        }
                    }
                    continue;
                }
                'l' => {
                    LOWER_BOUND = hfst_strtoweight(&getopt::optarg());
                    continue;
                }
                'u' => {
                    UPPER_BOUND = hfst_strtoweight(&getopt::optarg());
                    continue;
                }
                'I' => {
                    INPUT_SYMBOL = Some(getopt::optarg());
                    continue;
                }
                'O' => {
                    OUTPUT_SYMBOL = Some(getopt::optarg());
                    continue;
                }
                'S' => {
                    SYMBOL = Some(getopt::optarg());
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
                    TSV_FILE_NAME = Some(getopt::optarg());
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        if ARCS_ONLY && ENDS_ONLY {
            hfst_error(
                1,
                0,
                "Options '--arcs-only' and '--end-states-only' cannot be used \
at the same time",
            );
            return 1;
        }

        check_common_params();
        check_unary_params(args);
        if FUNCNAME.is_none() {
            FUNCNAME = Some("id".to_string());
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
        if let Some(name) = TSV_FILE_NAME.clone() {
            match std::fs::File::open(&name) {
                Ok(f) => TSV_FILE = Some(f),
                Err(_) => {
                    error(1, 0, &format!("Could not open '{}'", name));
                    return 1;
                }
            }
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
            if let Some(symbol) = SYMBOL.clone() {
                if i != symbol && o != symbol {
                    // symbol doesn't match, don't apply
                    return w;
                }
            }
            if let (Some(isym), Some(osym)) = (INPUT_SYMBOL.clone(), OUTPUT_SYMBOL.clone()) {
                if i != isym && o != osym {
                    // input doesn't match, don't apply
                    return w;
                }
            } else if let Some(isym) = INPUT_SYMBOL.clone() {
                if i != isym {
                    // input doesn't match, don't apply
                    return w;
                }
            } else if let Some(osym) = OUTPUT_SYMBOL.clone() {
                if o != osym {
                    // output doesn't match, don't apply
                    return w;
                }
            }
        }
        MULTIPLIER * (FUNC)(w) + ADDITION
    }
}

unsafe fn do_reweight(trans: &mut HfstTransducer) -> hfst::error::Result<()> {
    // [spec:hfst:def:hfst-reweight.original-fn]
    // [spec:hfst:sem:hfst-reweight.original-fn]
    let original = HfstBasicTransducer::from_hfst_transducer(trans);
    let replication = original.transform_weights(|w, i, o| unsafe { reweight(w, i, o) });
    *trans = HfstTransducer::new_from_basic(&replication, trans.get_type())?;
    Ok(())
}

// [spec:hfst:def:hfst-reweight.process-stream-fn]
// [spec:hfst:sem:hfst-reweight.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(t) => t,
                Err(e) => {
                    hfst_error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if trans.get_type() == ImplementationType::FOMA_TYPE {
                hfst_warning(
                    0,
                    0,
                    "Weighting is not supported in this automaton type;\
weights will be discarded",
                );
            }
            let inputname = hfst_get_name(&trans, &globals::input_filename());
            if transducer_n == 1 {
                verbose_printf(&format!("Reweighting {}...\n", inputname));
            } else {
                verbose_printf(&format!("Reweighting {}...{}\n", inputname, transducer_n));
            }
            if TSV_FILE.is_none() {
                if let Err(e) = do_reweight(&mut trans) {
                    hfst_error(1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "reweight");
                hfst_set_formula_unary(&mut trans, &src, "W");
            } else {
                // C: rewind(tsv_file) — seek the std file back to the start.
                let tsv_file = TSV_FILE.as_mut().unwrap();
                let _ = tsv_file.seek(SeekFrom::Start(0));
                SYMBOL = None;
                ADDITION = 0.0;
                MULTIPLIER = 1.0;
                let mut linen: usize = 0;
                verbose_printf(&format!(
                    "Reading reweights from {}\n",
                    TSV_FILE_NAME.clone().unwrap_or_default()
                ));
                let mut reader = BufReader::new(tsv_file);
                let mut line = String::new();
                loop {
                    line.clear();
                    // C: hfst_getline keeps the trailing newline; Ok(0) at EOF.
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    linen += 1;
                    let line_str = line.as_bytes();
                    if line_str.first() == Some(&b'\n') {
                        continue;
                    }
                    if line_str.first() == Some(&b'#') {
                        continue;
                    }
                    let tab_pos = line_str.iter().position(|&b| b == b'\t');
                    let tab = match tab_pos {
                        None => {
                            hfst_error_at_line(
                                1,
                                0,
                                &TSV_FILE_NAME.clone().unwrap_or_default(),
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
                    // SYMBOL = strndup(line, tab); kept as the substring before the tab.
                    let sym = String::from_utf8_lossy(&line_str[..tab]).into_owned();
                    SYMBOL = Some(sym);
                    let weightspec =
                        String::from_utf8_lossy(&line_str[tab + 1..endstr]).into_owned();
                    if weightspec.as_bytes().first() == Some(&b'+') {
                        ADDITION = hfst_strtoweight(&weightspec[1..]);
                    } else {
                        MULTIPLIER = hfst_strtoweight(&weightspec);
                    }
                    verbose_printf(&format!(
                        "Modifying weights {} < w < {} as {} * {}(w) + {} for symbol {}\n",
                        LOWER_BOUND,
                        UPPER_BOUND,
                        MULTIPLIER,
                        FUNCNAME.clone().unwrap_or_default(),
                        ADDITION,
                        SYMBOL.clone().unwrap_or_default()
                    ));
                    if let Err(e) = do_reweight(&mut trans) {
                        hfst_error(1, 0, &format!("{e}"));
                        return 1;
                    }
                } // getline
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "reweight");
                hfst_set_formula_unary(&mut trans, &src, "W");
            } // if tsv_file
            let reduced = match trans.remove_epsilons() {
                Ok(t) => t,
                Err(e) => {
                    hfst_error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if let Err(e) = outstream.redirect(reduced) {
                hfst_error(1, 0, &format!("{e}"));
                return 1;
            }
        } // foreach transducer
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-reweight.main-fn]
// [spec:hfst:sem:hfst-reweight.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstReweight");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        verbose_printf(&format!(
            "Modifying weights {} < w < {} as {} * {}(w) + {}\n",
            LOWER_BOUND,
            UPPER_BOUND,
            MULTIPLIER,
            FUNCNAME.clone().unwrap_or_default(),
            ADDITION
        ));
        if let Some(symbol) = SYMBOL.clone() {
            verbose_printf(&format!("only if arc has symbol {}\n", symbol));
        }
        if let Some(isym) = INPUT_SYMBOL.clone() {
            verbose_printf(&format!("only if input symbol is {}\n", isym));
        }
        if let Some(osym) = OUTPUT_SYMBOL.clone() {
            verbose_printf(&format!("only if output symbol is {}\n", osym));
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
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(s) => s,
            Err(e) => {
                hfst_error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(s) => s,
            Err(e) => {
                hfst_error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-reweight") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
