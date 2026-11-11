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
    is_input_stream_in_ol_format, last_os_error_code, verbose_print,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::tools::metadata::hfst_get_name;
use hfst::tools::reweight::{
    ReweightError, Reweighting, TsvError, finish_reweighted, reweight_by_tsv, weight_function,
};
use std::io::{BufRead, BufReader, Write};

/// The lines of the file `name`, read up to the first that is not UTF-8.
fn read_lines(name: &str) -> std::io::Result<String> {
    let mut reader = BufReader::new(std::fs::File::open(name)?);
    let mut text = String::new();
    while reader.read_line(&mut text).unwrap_or(0) > 0 {}
    Ok(text)
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
    /// Case 'F': the function names the C's else-if chain accepted;
    /// anything else is fatal inside the getopt loop.
    fn func(&self, common: &CommonOptions) -> fn(f32) -> f32 {
        let Some(name) = self.function.as_deref() else {
            return Reweighting::default().function;
        };
        weight_function(name).unwrap_or_else(|| {
            hfst_error(
                common,
                1,
                0,
                &format!("Cannot parse {} as function name", name),
            );
            Reweighting::default().function
        })
    }

    fn weight(&self, common: &CommonOptions, given: &Option<String>, default: f32) -> f32 {
        match given {
            Some(w) => hfst_strtoweight(common, w),
            None => default,
        }
    }

    fn resolve(&self, common: &CommonOptions) -> Options {
        Options {
            rule: Reweighting {
                addition: self.weight(common, &self.addition, 0.0),
                multiplier: self.weight(common, &self.multiplier, 1.0),
                function: self.func(common),
                upper_bound: self.weight(common, &self.upper_bound, f32::MAX),
                lower_bound: self.weight(common, &self.lower_bound, 0.0),
                symbol: self.symbol.clone(),
                input_symbol: self.input_symbol.clone(),
                output_symbol: self.output_symbol.clone(),
                ends_only: self.end_states_only,
                arcs_only: self.arcs_only,
            },
            // The C left funcname NULL until after the parameter checks, then
            // defaulted it to "id" — the name the -v trace prints.
            funcname: self.function.clone().unwrap_or_else(|| "id".to_string()),
            tsv: None,
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

/// hfst-reweight's resolved tool state.
struct Options {
    rule: Reweighting,
    funcname: String,
    /// '-T': the rules file's name and text.
    tsv: Option<(String, String)>,
}

/// Reweight `trans` by each rule of the TSV file `name`, whose text is
/// `text`, reporting a refused line as the tool reports it.
fn reweight_by_tsv_file<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &Options,
    (name, text): (&str, &str),
    trans: &mut HfstTransducer<B>,
) -> hfst::error::Result<()> {
    verbose_print(common, &format!("Reading reweights from {}\n", name));
    let reweighted = reweight_by_tsv(trans, &options.rule, text, &mut |rule| {
        verbose_print(
            common,
            &format!(
                "Modifying weights {} < w < {} as {} * {}(w) + {} for symbol {}\n",
                rule.lower_bound,
                rule.upper_bound,
                rule.multiplier,
                options.funcname,
                rule.addition,
                rule.symbol.clone().unwrap_or_default()
            ),
        );
    });
    match reweighted {
        Ok(()) => Ok(()),
        Err(ReweightError::Operation(e)) => Err(e),
        Err(ReweightError::Tsv {
            line,
            error: error @ TsvError::NoTab,
        }) => {
            hfst_error_at_line(common, 1, 0, name, line, &error.to_string());
            Ok(())
        }
        Err(ReweightError::Tsv {
            error: error @ TsvError::NotAWeight(_),
            ..
        }) => {
            hfst_error(common, 1, last_os_error_code(), &error.to_string());
            Ok(())
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
// The rules and the reweighting are the library's; the tool reads the stream
// and the TSV file and writes the result.
fn process_stream(
    common: &CommonOptions,
    options: &Options,
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
            let reweighted = match &options.tsv {
                None => options.rule.apply(&mut trans),
                Some((name, text)) => reweight_by_tsv_file(common, options, (name, text), &mut trans),
            };
            let written = reweighted.and_then(|()| {
                finish_reweighted(&mut trans)?;
                outstream.write(&trans)?;
                Ok(())
            });
            if let Err(e) = written {
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
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstReweight");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = args.resolve(&common);

    // The bound-order warning and the TSV open ran after the parameter checks.
    if options.rule.upper_bound < options.rule.lower_bound {
        hfst_warning(
            &common,
            0,
            0,
            &format!(
                "Lower bound {} exceeds upper bound {} so reweight will \
never apply",
                options.rule.lower_bound, options.rule.upper_bound
            ),
        );
    }
    if let Some(name) = &args.tsv {
        match read_lines(name) {
            Ok(text) => options.tsv = Some((name.clone(), text)),
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
            options.rule.lower_bound,
            options.rule.upper_bound,
            options.rule.multiplier,
            options.funcname,
            options.rule.addition
        ),
    );
    if let Some(symbol) = &options.rule.symbol {
        verbose_print(&common, &format!("only if arc has symbol {}\n", symbol));
    }
    if let Some(isym) = &options.rule.input_symbol {
        verbose_print(&common, &format!("only if input symbol is {}\n", isym));
    }
    if let Some(osym) = &options.rule.output_symbol {
        verbose_print(&common, &format!("only if output symbol is {}\n", osym));
    }
    if options.rule.ends_only {
        verbose_print(&common, "only on final weights, no arcs\n");
    }
    if options.rule.arcs_only {
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
        &options,
        &mut instream,
        &mut outstream,
    ))
}
