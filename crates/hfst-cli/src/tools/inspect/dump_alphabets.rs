//! Faithful 1:1 port of tools/src/hfst-dump-alphabets.cc — the alphabet dump
//! command-line tool. Option handling is clap 4 derive through
//! [`crate::cli`]; the rest drives the hfst-cli foundation (globals,
//! commandline).

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{hfst_set_program_name, verbose_print};
use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use std::io::Write;

// add tools-specific variables here
// [spec:hfst:def:hfst-dump-alphabets.alphadumpformat]
#[derive(Clone, Copy, PartialEq, Eq)]
enum AlphaDumpFormat {
    Tsv,
    Vislcg3List,
    Vislcg3Tags,
}

/// hfst-dump-alphabets's resolved options (the former tool-specific
/// `static mut`s).
struct Options {
    output_format: AlphaDumpFormat,
    print_seen: bool,
    print_meta: bool,
    only_multichars: bool,
}

// [spec:hfst:def:hfst-dump-alphabets.is-multichar-fn]
// [spec:hfst:sem:hfst-dump-alphabets.is-multichar-fn]
fn is_multichar(s: &str) -> bool {
    if s.len() > 2 {
        return s.starts_with('+') || s.starts_with(' ') || s.starts_with('@');
    }
    false
}

/// hfst-dump-alphabets's command line.
//
// The two exclusion switches keep their upstream names: '-1,
// --include-seen' EXCLUDES the alphabet seen in the automaton and '-2,
// --include-metadata' EXCLUDES the header alphabet — the long names say
// the opposite of what the cases do, and the usage text spelled them
// '--exclude-seen' / '--exclude-metadata' while the getopt table did not
// accept those. Preserved bug-for-bug.
// [spec:hfst:def:hfst-dump-alphabets.parse-options-fn]
// [spec:hfst:sem:hfst-dump-alphabets.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Print alphabets of automaton")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Print alphabet in AFORMAT: tsv, vislcg3-list or vislcg3-tags
    #[arg(short = 'f', long = "format", value_name = "AFORMAT")]
    format: Option<String>,

    /// Ignore alphabets seen in automaton
    #[arg(short = '1', long = "include-seen")]
    exclude_seen: bool,

    /// Ignore alphabets from headers
    #[arg(short = '2', long = "include-metadata")]
    exclude_metadata: bool,

    /// Whether the --format note is printed. The C emitted it from inside
    /// the getopt loop, so it appeared only when -v had already been read;
    /// [`ToolArgs::absorb_matches`] recovers that from the match indices.
    #[arg(skip)]
    announce_format: bool,
}

impl Args {
    /// Case 'f': the AFORMAT vocabulary. `announce` carries the verbose
    /// note the C printed as it read the option, so resolving the value a
    /// second time for the tool body stays silent.
    fn dump_format(&self, common: &CommonOptions, announce: bool) -> (AlphaDumpFormat, bool) {
        let Some(name) = self.format.as_deref() else {
            return (AlphaDumpFormat::Tsv, false);
        };
        let (format, only_multichars, note) = match name {
            "tsv" => (
                AlphaDumpFormat::Tsv,
                false,
                "printing one symbol per line\n",
            ),
            "vislcg3-list" => (
                AlphaDumpFormat::Vislcg3List,
                true,
                "printing LIST x = x ; for VISL CG 3...\n",
            ),
            "vislcg3-tags" => (
                AlphaDumpFormat::Vislcg3Tags,
                true,
                "printing STRICT-TAGS += for VISL CG 3...\n",
            ),
            other => {
                eprintln!("Error: unrecognised format {}", other);
                std::process::exit(1);
            }
        };
        if announce {
            verbose_print(common, note);
        }
        (format, only_multichars)
    }

    fn options(&self, common: &CommonOptions) -> Options {
        let (output_format, only_multichars) = self.dump_format(common, false);
        Options {
            output_format,
            print_seen: !self.exclude_seen,
            print_meta: !self.exclude_metadata,
            only_multichars,
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
        // The C read --format inside its getopt loop, before the parameter
        // checks: the verbose note and the unknown-name refusal both land
        // here for the same ordering.
        self.dump_format(opts, self.announce_format);
        Ok(())
    }

    fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
        // 'f' called verbose_print with whatever verbosity the loop had
        // reached, so '-v -f tsv' printed the note and '-f tsv -v' did not.
        self.announce_format = matches.get_flag("verbose")
            && matches!(
                (matches.index_of("verbose"), matches.index_of("format")),
                (Some(verbose), Some(format)) if verbose < format
            );
    }
}

// [spec:hfst:def:hfst-dump-alphabets.process-stream-fn]
// [spec:hfst:sem:hfst-dump-alphabets.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
) -> i32 {
    // Data output goes to a std stream (the std counterpart of the libc
    // outfile FILE*); `emit` writes a string and ignores errors, matching the
    // old fput/fputs.
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-dump-alphabets: could not open output: {e}");
            return 1;
        }
    };
    let mut emit = |s: &str| {
        let _ = out.write_all(s.as_bytes());
    };
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        if transducer_n < 2 {
            verbose_print(common, "Alphadumping...\n");
        } else {
            verbose_print(common, &format!("Alphadumping... {}\n", transducer_n));
        }
        let any = match instream.read() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("hfst-dump-alphabets: {e}");
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mutt = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&trans)
                .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
            // unsigned int initial_state = 0; // mutt.get_initial_state();
            let transducer_alphabet = match trans.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("hfst-dump-alphabets: {e}");
                    return 1;
                }
            };
            let transducer_knows_alphabet = true;
            let found_alphabet: StringSet = mutt.symbols_used();
            if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                emit(
                    "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                );
                emit("## (some statistics here TODO)\n");
                emit("STRICT-TAGS +=\n");
            } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                emit(
                    "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                );
                emit("## (some statistics here TODO)\n");
            }
            if options.print_meta {
                if transducer_knows_alphabet {
                    for s in transducer_alphabet.iter() {
                        if options.only_multichars && !is_multichar(s) {
                            continue;
                        }
                        if options.output_format == AlphaDumpFormat::Tsv {
                            emit(&format!("{}\n", s));
                        } else if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                            emit(&format!("\t{}\n", s));
                        } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                            emit(&format!("LIST {} = {} ;\n", s, s));
                        }
                    }
                } else {
                    eprintln!("Error: cannot dump non-existent header alphabet");
                    std::process::exit(1);
                }
            }
            if options.print_seen {
                for s in found_alphabet.iter() {
                    if options.only_multichars && !is_multichar(s) {
                        continue;
                    }
                    if options.output_format == AlphaDumpFormat::Tsv {
                        emit(&format!("{}\n", s));
                    } else if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                        emit(&format!("\t{}\n", s));
                    } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                        emit(&format!("LIST {} = {} ;\n", s, s));
                    }
                }
            }
        });
    } // for each automaton
    if options.output_format == AlphaDumpFormat::Vislcg3Tags {
        emit("\t;\n");
    }
    0
}

// [spec:hfst:def:hfst-dump-alphabets.main-fn]
// [spec:hfst:sem:hfst-dump-alphabets.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = args.options(&common);
    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // that calls error(EXIT_FAILURE, ...) is not reproduced here.)
    let instream_res = if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    };
    let mut instream = match instream_res {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "hfst-dump-alphabets: {} is not a valid transducer file: {e}",
                common.input_filename
            );
            return Err(1);
        }
    };
    let _retval = process_stream(&common, &options, &mut instream);

    Ok(())
}
