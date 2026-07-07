//! Faithful 1:1 port of tools/src/hfst-reweight.cc — the transducer reweighting
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! This is a unary tool (it #includes inc/globals-common.h and
//! inc/globals-unary.h and reads a single input stream).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_error, hfst_error_at_line, hfst_set_program_name,
    hfst_strtoweight, hfst_warning, is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

// add tools-specific variables here
// [spec:hfst:def:hfst-reweight.id-fn]
// [spec:hfst:sem:hfst-reweight.id-fn]
fn id(w: f32) -> f32 {
    w
}

/// hfst-reweight's own options (the former tool-specific `static mut`s).
struct Options {
    addition: f32,
    multiplier: f32,
    funcname: Option<String>,
    // [spec:hfst:def:hfst-reweight.func-fn]
    // [spec:hfst:sem:hfst-reweight.func-fn]
    func: fn(f32) -> f32,
    upper_bound: f32,
    lower_bound: f32,
    input_symbol: Option<String>,
    output_symbol: Option<String>,
    symbol: Option<String>,
    ends_only: bool,
    arcs_only: bool,
    tsv_file_name: Option<String>,
    tsv_file: Option<std::fs::File>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            addition: 0.0,
            multiplier: 1.0,
            funcname: None,
            func: id,
            upper_bound: f32::MAX,
            lower_bound: 0.0,
            input_symbol: None,
            output_symbol: None,
            symbol: None,
            ends_only: false,
            arcs_only: false,
            tsv_file_name: None,
            tsv_file: None,
        }
    }
}

// [spec:hfst:def:hfst-reweight.print-usage-fn]
// [spec:hfst:sem:hfst-reweight.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nReweight transducer weights simply\n\n",
        common.program_name
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
}

// [spec:hfst:def:hfst-reweight.parse-options-fn]
// [spec:hfst:sem:hfst-reweight.parse-options-fn]
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
        // tool-specific cases
        match c as u8 as char {
            'a' => {
                options.addition = hfst_strtoweight(&common, &opt.optarg());
                continue;
            }
            'b' => {
                options.multiplier = hfst_strtoweight(&common, &opt.optarg());
                continue;
            }
            'F' => {
                let name = opt.optarg();
                options.funcname = Some(name.clone());
                match name.as_str() {
                    "cos" => options.func = f32::cos,
                    "sin" => options.func = f32::sin,
                    "tan" => options.func = f32::tan,
                    "acos" => options.func = f32::acos,
                    "asin" => options.func = f32::asin,
                    "atan" => options.func = f32::atan,
                    "cosh" => options.func = f32::cosh,
                    "sinh" => options.func = f32::sinh,
                    "tanh" => options.func = f32::tanh,
                    "exp" => options.func = f32::exp,
                    "log" => options.func = f32::ln,
                    "log10" => options.func = f32::log10,
                    "sqrt" => options.func = f32::sqrt,
                    "floor" => options.func = f32::floor,
                    "ceil" => options.func = f32::ceil,
                    _ => {
                        hfst_error(
                            &common,
                            1,
                            0,
                            &format!("Cannot parse {} as function name", name),
                        );
                        return Err(1);
                    }
                }
                continue;
            }
            'l' => {
                options.lower_bound = hfst_strtoweight(&common, &opt.optarg());
                continue;
            }
            'u' => {
                options.upper_bound = hfst_strtoweight(&common, &opt.optarg());
                continue;
            }
            'I' => {
                options.input_symbol = Some(opt.optarg());
                continue;
            }
            'O' => {
                options.output_symbol = Some(opt.optarg());
                continue;
            }
            'S' => {
                options.symbol = Some(opt.optarg());
                continue;
            }
            'e' => {
                options.ends_only = true;
                continue;
            }
            'A' => {
                options.arcs_only = true;
                continue;
            }
            'T' => {
                options.tsv_file_name = Some(opt.optarg());
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    if options.arcs_only && options.ends_only {
        hfst_error(
            &common,
            1,
            0,
            "Options '--arcs-only' and '--end-states-only' cannot be used \
at the same time",
        );
        return Err(1);
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    if options.funcname.is_none() {
        options.funcname = Some("id".to_string());
    }
    if options.upper_bound < options.lower_bound {
        hfst_warning(
            &common,
            0,
            0,
            &format!(
                "Lower bound {} exceeds upper bound {} so reweight will \
never apply",
                options.lower_bound, options.upper_bound
            ),
        );
    }
    if let Some(name) = options.tsv_file_name.clone() {
        match std::fs::File::open(&name) {
            Ok(f) => options.tsv_file = Some(f),
            Err(_) => {
                error(&common, 1, 0, &format!("Could not open '{}'", name));
                return Err(1);
            }
        }
    }
    Ok((common, options))
}

// [spec:hfst:def:hfst-reweight.reweight-fn]
// [spec:hfst:sem:hfst-reweight.reweight-fn]
fn reweight(options: &Options, w: f32, i: Option<&str>, o: Option<&str>) -> f32 {
    if (w < options.lower_bound) || (w > options.upper_bound) {
        // not within weight bounds, don't apply
        return w;
    }
    if i.is_none() && o.is_none() {
        if options.arcs_only {
            return w;
        }
    } else if i.is_some() && o.is_some() {
        let i = i.unwrap();
        let o = o.unwrap();
        if options.ends_only {
            return w;
        }
        if let Some(symbol) = options.symbol.clone() {
            if i != symbol && o != symbol {
                // symbol doesn't match, don't apply
                return w;
            }
        }
        if let (Some(isym), Some(osym)) =
            (options.input_symbol.clone(), options.output_symbol.clone())
        {
            if i != isym && o != osym {
                // input doesn't match, don't apply
                return w;
            }
        } else if let Some(isym) = options.input_symbol.clone() {
            if i != isym {
                // input doesn't match, don't apply
                return w;
            }
        } else if let Some(osym) = options.output_symbol.clone() {
            if o != osym {
                // output doesn't match, don't apply
                return w;
            }
        }
    }
    options.multiplier * (options.func)(w) + options.addition
}

fn do_reweight<B: hfst::backend::AlgebraBackend>(
    options: &Options,
    trans: &mut HfstTransducer<B>,
) -> hfst::error::Result<()> {
    // [spec:hfst:def:hfst-reweight.original-fn]
    // [spec:hfst:sem:hfst-reweight.original-fn]
    let original = HfstBasicTransducer::from_hfst_transducer(trans);
    let replication = original.transform_weights(|w, i, o| reweight(options, w, i, o));
    *trans = HfstTransducer::new_from_basic(&replication)?;
    Ok(())
}

// [spec:hfst:def:hfst-reweight.process-stream-fn]
// [spec:hfst:sem:hfst-reweight.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_algebra!(any, trans => {
            let mut trans = trans;
            if trans.get_type() == ImplementationType::FOMA_TYPE {
                hfst_warning(
                    common,
                    0,
                    0,
                    "Weighting is not supported in this automaton type;\
        weights will be discarded",
                );
            }
            let inputname = hfst_get_name(&trans, &common.input_filename);
            if transducer_n == 1 {
                verbose_print(common, &format!("Reweighting {}...\n", inputname));
            } else {
                verbose_print(common, &format!("Reweighting {}...{}\n", inputname, transducer_n));
            }
            if options.tsv_file.is_none() {
                if let Err(e) = do_reweight(options, &mut trans) {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "reweight");
                hfst_set_formula_unary(&mut trans, &src, "W");
            } else {
                // C: rewind(tsv_file) — seek the std file back to the start.
                options.symbol = None;
                options.addition = 0.0;
                options.multiplier = 1.0;
                let mut linen: usize = 0;
                verbose_print(common, &format!(
                    "Reading reweights from {}\n",
                    options.tsv_file_name.clone().unwrap_or_default()
                ));
                // Read the file's lines up front so the per-line body can keep
                // mutating `options` (the C code kept the file handle open and
                // rewound it; the borrow checker forbids holding a reader over
                // `options.tsv_file` while the loop mutates other `options`
                // fields, so we snapshot the lines instead). Each line keeps its
                // trailing newline, matching hfst_getline.
                let lines: Vec<String> = {
                    let tsv_file = options.tsv_file.as_mut().unwrap();
                    let _ = tsv_file.seek(SeekFrom::Start(0));
                    let mut reader = BufReader::new(tsv_file);
                    let mut acc: Vec<String> = Vec::new();
                    let mut line = String::new();
                    loop {
                        line.clear();
                        // C: hfst_getline keeps the trailing newline; Ok(0) at EOF.
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            break;
                        }
                        acc.push(line.clone());
                    }
                    acc
                };
                for line in &lines {
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
                                common,
                                1,
                                0,
                                &options.tsv_file_name.clone().unwrap_or_default(),
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
                    options.symbol = Some(sym);
                    let weightspec =
                        String::from_utf8_lossy(&line_str[tab + 1..endstr]).into_owned();
                    if weightspec.as_bytes().first() == Some(&b'+') {
                        options.addition = hfst_strtoweight(common, &weightspec[1..]);
                    } else {
                        options.multiplier = hfst_strtoweight(common, &weightspec);
                    }
                    verbose_print(common, &format!(
                        "Modifying weights {} < w < {} as {} * {}(w) + {} for symbol {}\n",
                        options.lower_bound,
                        options.upper_bound,
                        options.multiplier,
                        options.funcname.clone().unwrap_or_default(),
                        options.addition,
                        options.symbol.clone().unwrap_or_default()
                    ));
                    if let Err(e) = do_reweight(options, &mut trans) {
                        hfst_error(common, 1, 0, &format!("{e}"));
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
                    hfst_error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if let Err(e) = outstream.redirect(reduced) {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = write!(
                std::io::stderr(),
                "Error: hfst-reweight cannot process transducers that are in optimized lookup format.\n"
            );
            return 1;
        });
    } // foreach transducer
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-reweight.main-fn]
// [spec:hfst:sem:hfst-reweight.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstReweight");
    let (common, mut options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    verbose_print(
        &common,
        &format!(
            "Modifying weights {} < w < {} as {} * {}(w) + {}\n",
            options.lower_bound,
            options.upper_bound,
            options.multiplier,
            options.funcname.clone().unwrap_or_default(),
            options.addition
        ),
    );
    if let Some(symbol) = options.symbol.clone() {
        verbose_print(&common, &format!("only if arc has symbol {}\n", symbol));
    }
    if let Some(isym) = options.input_symbol.clone() {
        verbose_print(&common, &format!("only if input symbol is {}\n", isym));
    }
    if let Some(osym) = options.output_symbol.clone() {
        verbose_print(&common, &format!("only if output symbol is {}\n", osym));
    }
    if options.ends_only {
        verbose_print(&common, "only on final weights, no arcs\n");
    }
    if options.arcs_only {
        verbose_print(&common, "only on arc weights, no end states\n");
    }

    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(s) => s,
        Err(e) => {
            hfst_error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    let ty = instream.get_type();
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(s) => s,
        Err(e) => {
            hfst_error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-reweight") {
        return 1;
    }

    process_stream(&common, &mut options, &mut instream, &mut outstream)
}
