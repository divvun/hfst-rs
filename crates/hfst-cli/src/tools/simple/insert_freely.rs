//! Faithful 1:1 port of tools/src/hfst-insert-freely.cc — the freely-insert
//! a symbol (pair) command-line tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
//! the parsed [`Args`] and threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_set_program_name, is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst::hfst_data_types::StringPair;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
use std::io::Write;

/// hfst-insert-freely's command line.
// [spec:hfst:def:hfst-insert-freely.parse-options-fn]
// [spec:hfst:sem:hfst-insert-freely.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Freely insert a symbol (pair)")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Symbol pair SYM: either a single alphabetic symbol or two symbols
    /// separated by a colon, :
    #[arg(short = 'a', long = "symbol-pair", value_name = "SYM")]
    symbol_pair: Option<String>,

    /// Harmonise
    #[arg(short = 'H')]
    harmonise: bool,

    /// Harmonise; upstream's long spelling takes a required argument that
    /// nothing reads, while its short-option string gives -H none, so
    /// '--harmonise SYM' swallows SYM and '-H SYM' leaves it as the input
    /// operand. The two spellings are separate args so both keep their
    /// upstream arity.
    #[arg(long = "harmonise", value_name = "ARG")]
    harmonise_long: Option<String>,
}

impl Args {
    /// Either spelling of -H/--harmonise sets the flag; the long form's
    /// argument is discarded, as upstream discards it.
    fn harmonise_flags(&self) -> bool {
        self.harmonise || self.harmonise_long.is_some()
    }
}

impl Args {
    /// Case 'a': "@0@" stands for the internal epsilon, and an empty label
    /// is fatal (the C checks AFTER building the pair from it).
    fn label(&self, common: &CommonOptions) -> Option<StringPair> {
        let lbl = self.symbol_pair.as_deref()?;
        // This will probably break for unicode
        let lbl = if lbl == "@0@" {
            internal_epsilon.to_string()
        } else {
            lbl.to_string()
        };
        let pair = label_to_stringpair(&lbl);
        if lbl.is_empty() {
            error(
                common,
                1,
                0,
                &format!(
                    "argument of source label option is empty;\nif you REALLY want to replace epsilons with something, use @0@ or {}",
                    internal_epsilon
                ),
            );
        }
        pair
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
        // The empty-label rejection happened inside the C getopt loop,
        // before the parameter checks; run it here for the same ordering.
        self.label(opts);
        Ok(())
    }
}

// [spec:hfst:def:hfst-insert-freely.process-stream-fn]
// [spec:hfst:sem:hfst-insert-freely.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    symbol_pair: &StringPair,
    harmonise_flags: bool,
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
            let _inputname = hfst_get_name(&trans, &common.input_filename);
            if transducer_n == 1 {
                // If harmonize is true, then identity and unknown symbols in the
                // transducer will be expanded by the symbols in symbol pair.
                // Otherwise they aren't.
                if let Err(e) = trans.insert_freely_pair(symbol_pair, harmonise_flags) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                // C: hfst_set_name(trans, trans, "insert-freely") and
                // hfst_set_formula(trans, trans, "Id"); dest and src are the
                // same object, so the read side is taken from a copy.
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "insert-freely");
                hfst_set_formula_unary(&mut trans, &src, "Id");
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
                "Error: hfst-insert-freely cannot process transducers that are in optimized lookup format."
            );
            return 1;
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-insert-freely.main-fn]
// [spec:hfst:sem:hfst-insert-freely.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
    let (common, args) = cli::parse::<Args>(common, args)?;
    // The C dereferenced a null pair in process_stream when -a was absent
    // or its argument carried no delimiting colon; name what is missing.
    let Some(symbol_pair) = args.label(&common) else {
        let msg = match args.symbol_pair.as_deref() {
            None => String::from("no symbol pair given; use -a SYM:SYM"),
            Some(label) => {
                format!("symbol pair '{label}' has no colon; give it as SYM:SYM")
            }
        };
        error(&common, 1, 0, &msg);
        return Err(1);
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

    // here starts the buffer handling part
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
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
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-insert-freely") {
        return Err(1);
    }

    cli::from_code(process_stream(
        &common,
        &symbol_pair,
        args.harmonise_flags(),
        &mut instream,
        &mut outstream,
    ))
}
