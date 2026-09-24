//! Faithful 1:1 port of tools/src/hfst-repeat.cc — the transducer repetition
//! command-line tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields) and a
//! tool-local [`Options`] built from the parsed [`Args`]. There are no
//! `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_set_program_name, hfst_strtonumber, is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-repeat's command line.
// [spec:hfst:def:hfst-repeat.parse-options-fn]
// [spec:hfst:sem:hfst-repeat.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Repeat transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Repeat at least FNUM times; a positive integer or an infinity as
    /// parsed by strtod(3), 0 if omitted, and less than TNUM
    #[arg(
        short = 'f',
        long = "from",
        value_name = "FNUM",
        allow_hyphen_values = true
    )]
    from: Option<String>,

    /// Repeat at most TNUM times; a positive integer or an infinity as
    /// parsed by strtod(3), Inf if omitted
    #[arg(
        short = 't',
        long = "to",
        value_name = "TNUM",
        allow_hyphen_values = true
    )]
    to: Option<String>,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // Both numbers were parsed inside the C getopt loop, so a
        // non-numeric FNUM/TNUM is rejected before the parameter checks;
        // the range checks run after them, in Options::resolve.
        Options::parse_bounds(self, opts);
        Ok(())
    }
}

/// hfst-repeat's option-driven state (the former tool-specific `static mut`s).
struct Options {
    /// '-f, --from=FNUM': repeat at least FNUM times.
    at_least: u64,
    /// '-t, --to=TNUM': repeat at most TNUM times.
    at_most: u64,
    /// FNUM was parsed as infinity.
    from_infinity: bool,
    /// TNUM was parsed as infinity.
    to_infinity: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            at_least: 0,
            at_most: u32::MAX as u64,
            from_infinity: false,
            to_infinity: true,
        }
    }
}

impl Options {
    /// The 'f' and 't' cases: strtod each bound and note whether it came
    /// out infinite. Fatal on a non-number.
    fn parse_bounds(args: &Args, common: &CommonOptions) -> Options {
        let mut options = Options::default();
        if let Some(from) = &args.from {
            let mut from_inf = false;
            options.at_least = hfst_strtonumber(common, from, Some(&mut from_inf)) as u64;
            options.from_infinity = from_inf;
        }
        if let Some(to) = &args.to {
            let mut to_inf = false;
            options.at_most = hfst_strtonumber(common, to, Some(&mut to_inf)) as u64;
            options.to_infinity = to_inf;
        }
        options
    }

    /// The post-loop validation the C ran AFTER the parameter checks.
    fn resolve(args: &Args, common: &CommonOptions) -> Options {
        let options = Options::parse_bounds(args, common);
        if options.at_least > options.at_most {
            error(
                common,
                1,
                0,
                &format!(
                    "Cannot repeat from {} to {} times\n",
                    options.at_least, options.at_most
                ),
            );
        }
        if options.from_infinity && !options.to_infinity {
            error(
                common,
                1,
                0,
                &format!("Cannot repeat from infinity to {} times\n", options.at_most),
            );
        }
        options
    }
}

// [spec:hfst:def:hfst-repeat.process-stream-fn]
// [spec:hfst:sem:hfst-repeat.process-stream-fn]
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
            let inputname = hfst_get_name(&trans, &common.input_filename);
            if transducer_n == 1 {
                if !options.from_infinity && !options.to_infinity {
                    verbose_print(common, &format!(
                        "Repeating [{}..{}] {}...\n",
                        options.at_least, options.at_most, inputname
                    ));
                } else if options.from_infinity && options.to_infinity {
                    verbose_print(common, &format!("Repeating star {}...\n", inputname));
                } else if !options.from_infinity && options.to_infinity {
                    verbose_print(common, &format!("Repeating [{}..*] {}...\n", options.at_least, inputname));
                } else if options.from_infinity && !options.to_infinity {
                    error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
                }
            } else if !options.from_infinity && !options.to_infinity {
                verbose_print(common, &format!(
                    "Repeating [{}..{}] {}... {}\n",
                    options.at_least, options.at_most, inputname, transducer_n
                ));
            } else if options.from_infinity && options.to_infinity {
                verbose_print(common, &format!(
                    "Repeating star {}... {}\n",
                    inputname, transducer_n
                ));
            } else if !options.from_infinity && options.to_infinity {
                verbose_print(common, &format!(
                    "Repeating [{}..*] {}... {}\n",
                    options.at_least, inputname, transducer_n
                ));
            } else if options.from_infinity && !options.to_infinity {
                error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
            }

            if !options.from_infinity && !options.to_infinity {
                if let Err(e) = trans.repeat_n_to_k(options.at_least as u32, options.at_most as u32) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let composed_name = format!("repeat-{}-to-{}", options.at_least, options.at_most);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &composed_name);
                let composed_name = format!("_{}^{}", options.at_least, options.at_most);
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &composed_name);
            } else if options.from_infinity && options.to_infinity {
                if let Err(e) = trans.repeat_star() {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "repeat-star");
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, "\u{22c6}");
            } else if !options.from_infinity && options.to_infinity {
                if let Err(e) = trans.repeat_n_plus(options.at_least as u32) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let composed_name = format!("repeat-{}-plus", options.at_least);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &composed_name);
                let composed_name = format!("_{}^\u{221e}", options.at_least);
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &composed_name);
            } else if options.from_infinity && !options.to_infinity {
                error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
            }
            if let Err(e) = outstream.write(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = writeln!(
                std::io::stderr(),
                "Error: hfst-repeat cannot process transducers that are in optimized lookup format."
            );
            return 1;
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-repeat.main-fn]
// [spec:hfst:sem:hfst-repeat.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstRepeat");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options::resolve(&args, &common);

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
    if !options.from_infinity && !options.to_infinity {
        verbose_print(
            &common,
            &format!(
                "Repeating from {} to {} times\n",
                options.at_least, options.at_most
            ),
        );
    } else if options.from_infinity && options.to_infinity {
        verbose_print(&common, "Repeating star infinitely\n");
    } else if !options.from_infinity && options.to_infinity {
        verbose_print(
            &common,
            &format!("Repeating from {} to infinite times\n", options.at_least),
        );
    } else if options.from_infinity && !options.to_infinity {
        error(
            &common,
            1,
            0,
            &format!(
                "Repeating at least infinite butno more than {} times?",
                options.at_most
            ),
        );
    }

    // here starts the buffer handling part
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    let ty = instream.get_type();
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-repeat") {
        return Err(1);
    }

    cli::from_code(process_stream(
        &common,
        &options,
        &mut instream,
        &mut outstream,
    ))
}
