//! Faithful 1:1 port of tools/src/hfst-kill-paths.cc — the path-killing
//! command-line tool: removes every arc whose input or output symbol matches a
//! given symbol (one --symbol, or a list from a --tsv-file), then removes
//! epsilons. Option handling is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_set_program_name, is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

/// hfst-kill-paths's command line. Upstream spells the long name of -T
/// "tsv" while its help calls it --tsv-file; the registered name is the
/// one that parses, so it is kept.
// [spec:hfst:def:hfst-kill-paths.parse-options-fn]
// [spec:hfst:sem:hfst-kill-paths.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Kill all paths with specific symbols")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Remove arcs with input or output symbol SYM or both
    #[arg(short = 'S', long = "symbol", value_name = "SYM")]
    symbol: Option<String>,

    /// Read kill rules from TFILE, which should contain lines with
    /// tab-separated pairs of SYM; comment lines starting with # and empty
    /// lines are ignored
    #[arg(short = 'T', long = "tsv", value_name = "TFILE")]
    tsv: Option<String>,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The C ran this check after the getopt loop but BEFORE the
        // parameter checks, so it outranks the too-many-files diagnostics.
        if self.symbol.is_none() && self.tsv.is_none() {
            error(opts, 1, 0, "Either --symbol or --tsv-file is required");
            return Err(1);
        }
        Ok(())
    }
}

/// The tool's option-driven state once the kill-rules file, which the C
/// opened after the parameter checks, has been opened.
struct Options {
    /// '-S, --symbol=SYM': the symbol whose arcs to kill.
    symbol: Option<String>,
    /// '-T, --tsv-file=TFILE': the file listing kill symbols.
    tsv_file_name: Option<String>,
    /// The opened kill-rules file (from `tsv_file_name`).
    tsv_file: Option<std::fs::File>,
}

impl Options {
    fn open(args: &Args, common: &CommonOptions) -> Result<Options, i32> {
        let mut options = Options {
            symbol: args.symbol.clone(),
            tsv_file_name: args.tsv.clone(),
            tsv_file: None,
        };
        if let Some(name) = &options.tsv_file_name {
            match std::fs::File::open(name) {
                Ok(f) => options.tsv_file = Some(f),
                Err(_) => {
                    error(common, 1, 0, &format!("Could not open '{}'", name));
                    return Err(1);
                }
            }
        }
        Ok(options)
    }
}

// [spec:hfst:def:hfst-kill-paths.original-fn]
// [spec:hfst:sem:hfst-kill-paths.original-fn]
fn do_killing<B: hfst::backend::AlgebraBackend>(
    symbol: Option<&str>,
    trans: &mut HfstTransducer<B>,
) {
    let symbol = symbol.unwrap_or_default();
    *trans = trans.kill_paths(symbol);
}

// [spec:hfst:def:hfst-kill-paths.process-stream-fn]
// [spec:hfst:sem:hfst-kill-paths.process-stream-fn]
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
            let inputname = hfst_get_name(&trans, &common.input_filename);
            if transducer_n == 1 {
                verbose_print(common, &format!("Path killing {}...\n", inputname));
            } else {
                verbose_print(common, &format!("Path killing {}...{}\n", inputname, transducer_n));
            }
            if options.tsv_file.is_none() {
                do_killing(options.symbol.as_deref(), &mut trans);
                // C: hfst_set_name(trans, trans, "pathkill"); dest and src are the
                // same object, which Rust cannot alias mut+const, so the read side
                // is taken from a copy (name/formula are unchanged by the copy).
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "pathkill");
                hfst_set_formula_unary(&mut trans, &src, "PK");
            } else {
                // C: rewind(tsv_file) — seek the std file back to the start.
                if let Some(tsv_file) = options.tsv_file.as_mut() {
                    let _ = tsv_file.seek(SeekFrom::Start(0));
                }
                options.symbol = None;
                let mut _linen: usize = 0;
                verbose_print(common, &format!(
                    "Reading reweights from {}\n",
                    options.tsv_file_name.clone().unwrap_or_default()
                ));
                if let Some(tsv_file) = options.tsv_file.as_mut() {
                    let mut reader = BufReader::new(tsv_file);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        // C: hfst_getline keeps the trailing newline; Ok(0) at EOF.
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            break;
                        }
                        _linen += 1;
                        let bytes = line.as_bytes();
                        if bytes.first() == Some(&b'\n') {
                            continue;
                        }
                        if bytes.first() == Some(&b'#') {
                            continue;
                        }
                        // const char *endptr = line; advance to '\0' or '\n'
                        let mut endptr = 0usize;
                        while endptr < bytes.len() && bytes[endptr] != b'\n' {
                            endptr += 1;
                        }
                        let sym = String::from_utf8_lossy(&bytes[..endptr]).into_owned();
                        verbose_print(common, &format!("Killing patsh with symbol {}\n", sym));
                        do_killing(Some(&sym), &mut trans);
                    } // getline
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "pathkill");
                hfst_set_formula_unary(&mut trans, &src, "PK");
            } // if tsv_file
            let reduced = match trans.remove_epsilons() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if let Err(e) = outstream.write(reduced) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = writeln!(
                std::io::stderr(),
                "Error: hfst-kill-paths cannot process transducers that are in optimized lookup format."
            );
            return 1;
        });
    } // foreach transducer
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-kill-paths.main-fn]
// [spec:hfst:sem:hfst-kill-paths.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstKillPaths");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = Options::open(&args, &common)?;

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
    verbose_print(&common, "Killing paths\n");
    if let Some(sym) = &options.symbol {
        verbose_print(&common, &format!("only if arc has symbol {}\n", sym));
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

    if is_input_stream_in_ol_format(&instream, "hfst-kill-paths") {
        return Err(1);
    }

    cli::from_code(process_stream(
        &common,
        &mut options,
        &mut instream,
        &mut outstream,
    ))
}
