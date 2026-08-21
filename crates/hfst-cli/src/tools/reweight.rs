//! Faithful 1:1 port of tools/src/hfst-reweight.cc — the transducer reweighting
//! command-line tool.
//!
//! This is a unary tool (it #includes inc/globals-common.h and
//! inc/globals-unary.h and reads a single input stream). Option handling is
//! clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_error, hfst_error_at_line, hfst_set_program_name, hfst_strtoweight, hfst_warning,
    is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
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

/// hfst-reweight's command line.
//
// Every weight-valued option carries allow_hyphen_values so a negative
// AVAL/BVAL/LVAL/UVAL is its argument rather than an unknown option, and so
// is '-S -' — the C's getopt handed the literal dash straight to `symbol`.
// [spec:hfst:def:hfst-reweight.parse-options-fn]
// [spec:hfst:sem:hfst-reweight.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Reweight transducer weights simply")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Add AVAL to matching weights
    #[arg(
        short = 'a',
        long = "addition",
        value_name = "AVAL",
        allow_hyphen_values = true
    )]
    addition: Option<String>,

    /// Multiply matching weights by BVAL
    #[arg(
        short = 'b',
        long = "multiplier",
        value_name = "BVAL",
        allow_hyphen_values = true
    )]
    multiplier: Option<String>,

    /// Operate matching weights by FNAME
    #[arg(short = 'F', long = "function", value_name = "FNAME")]
    function: Option<String>,

    /// Match weights greater than LVAL
    #[arg(
        short = 'l',
        long = "lower-bound",
        value_name = "LVAL",
        allow_hyphen_values = true
    )]
    lower_bound: Option<String>,

    /// Match weights less than UVAL
    #[arg(
        short = 'u',
        long = "upper-bound",
        value_name = "UVAL",
        allow_hyphen_values = true
    )]
    upper_bound: Option<String>,

    /// Match arcs with input symbol ISYM
    #[arg(
        short = 'I',
        long = "input-symbol",
        value_name = "ISYM",
        allow_hyphen_values = true
    )]
    input_symbol: Option<String>,

    /// Match arcs with output symbol OSYM
    #[arg(
        short = 'O',
        long = "output-symbol",
        value_name = "OSYM",
        allow_hyphen_values = true
    )]
    output_symbol: Option<String>,

    /// Match arcs with input or output symbol SYM or both
    #[arg(
        short = 'S',
        long = "symbol",
        value_name = "SYM",
        allow_hyphen_values = true
    )]
    symbol: Option<String>,

    /// Match end states only, no arcs
    #[arg(short = 'e', long = "end-states-only")]
    end_states_only: bool,

    /// Match arcs only, no end states
    #[arg(short = 'A', long = "arcs-only")]
    arcs_only: bool,

    /// Read reweighting rules from TFILE. (The C long table spells this
    /// '--tsv' though its help text advertises '--tsv-file'; the accepted
    /// spelling is kept.)
    #[arg(short = 'T', long = "tsv", value_name = "TFILE")]
    tsv: Option<String>,
}

impl Args {
    /// Case 'F': the <cmath> function names the C's else-if chain accepted;
    /// anything else is fatal inside the getopt loop.
    // [spec:hfst:def:hfst-reweight.func-fn]
    // [spec:hfst:sem:hfst-reweight.func-fn]
    fn func(&self, common: &CommonOptions) -> fn(f32) -> f32 {
        let Some(name) = self.function.as_deref() else {
            return id;
        };
        match name {
            "cos" => f32::cos,
            "sin" => f32::sin,
            "tan" => f32::tan,
            "acos" => f32::acos,
            "asin" => f32::asin,
            "atan" => f32::atan,
            "cosh" => f32::cosh,
            "sinh" => f32::sinh,
            "tanh" => f32::tanh,
            "exp" => f32::exp,
            "log" => f32::ln,
            "log10" => f32::log10,
            "sqrt" => f32::sqrt,
            "floor" => f32::floor,
            "ceil" => f32::ceil,
            _ => {
                hfst_error(
                    common,
                    1,
                    0,
                    &format!("Cannot parse {} as function name", name),
                );
                id
            }
        }
    }

    fn weight(&self, common: &CommonOptions, given: &Option<String>, default: f32) -> f32 {
        match given {
            Some(w) => hfst_strtoweight(common, w),
            None => default,
        }
    }

    fn resolve(&self, common: &CommonOptions) -> Options {
        Options {
            addition: self.weight(common, &self.addition, 0.0),
            multiplier: self.weight(common, &self.multiplier, 1.0),
            // The C left funcname NULL until after the parameter checks, then
            // defaulted it to "id" — the name the -v trace prints.
            funcname: Some(self.function.clone().unwrap_or_else(|| "id".to_string())),
            func: self.func(common),
            upper_bound: self.weight(common, &self.upper_bound, f32::MAX),
            lower_bound: self.weight(common, &self.lower_bound, 0.0),
            input_symbol: self.input_symbol.clone(),
            output_symbol: self.output_symbol.clone(),
            symbol: self.symbol.clone(),
            ends_only: self.end_states_only,
            arcs_only: self.arcs_only,
            tsv_file_name: self.tsv.clone(),
            tsv_file: None,
        }
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // Everything the C did inside the getopt loop (the strtod parses and
        // the function-name lookup), then the exclusion test it ran right
        // after the loop and before the parameter checks.
        self.resolve(opts);
        if self.arcs_only && self.end_states_only {
            hfst_error(
                opts,
                1,
                0,
                "Options '--arcs-only' and '--end-states-only' cannot be used \
at the same time",
            );
            return Err(1);
        }
        Ok(())
    }
}

/// hfst-reweight's resolved tool state (the former tool-specific `static mut`s).
struct Options {
    addition: f32,
    multiplier: f32,
    funcname: Option<String>,
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
        if let Some(symbol) = options.symbol.clone()
            && i != symbol
            && o != symbol
        {
            // symbol doesn't match, don't apply
            return w;
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
        } else if let Some(osym) = options.output_symbol.clone()
            && o != osym
        {
            // output doesn't match, don't apply
            return w;
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
    instream: &mut HfstInputStream<'_>,
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
                let lines: Vec<String> = if let Some(tsv_file) = options.tsv_file.as_mut() {
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
                } else {
                    Vec::new()
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
            let _ = writeln!(
                std::io::stderr(),
                "Error: hfst-reweight cannot process transducers that are in optimized lookup format."
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
pub fn run(args: Vec<String>) -> i32 {
    cli::exit_code(execute(args))
}

fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstReweight");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = args.resolve(&common);

    // The bound-order warning and the TSV open ran after the parameter checks.
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
            return Err(1);
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
            return Err(1);
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-reweight") {
        return Err(1);
    }

    cli::from_code(process_stream(
        &common,
        &mut options,
        &mut instream,
        &mut outstream,
    ))
}
