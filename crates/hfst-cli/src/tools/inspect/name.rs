//! Faithful 1:1 port of tools/src/hfst-name.cc — the transducer naming
//! command-line tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and a
//! tool-local [`Options`], threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{hfst_set_program_name, parse_u64, verbose_print};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;

/// hfst-name's resolved options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-n, --name=NAME': the name to set on the transducer.
    transducer_name: String,
    /// whether '-n / --name' was given.
    name_option_given: bool,
    /// '-p, --print-name': only print the current name.
    print_name: bool,
    /// '-t, --truncate_length=LEN': truncate the name to LEN bytes (0 = no limit).
    truncate_length: u64,
}

/// hfst-name's command line.
//
// '--truncate_length' keeps its upstream underscore: that is the long
// name the getopt table carried, and Giella scripts spell it that way.
// '-p' takes no argument here (unlike hfst-edit-metadata's) and, when
// given together with '-n', overrides it — a warning the tool body emits
// after the parameter checks.
// [spec:hfst:def:hfst-name.parse-options-fn]
// [spec:hfst:sem:hfst-name.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Name a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Name the transducer NAME
    #[arg(
        short = 'n',
        long = "name",
        value_name = "NAME",
        allow_hyphen_values = true
    )]
    name: Option<String>,

    /// Only print the current name
    #[arg(short = 'p', long = "print-name")]
    print_name: bool,

    /// Truncate name length to LEN
    #[arg(short = 't', long = "truncate_length", value_name = "LEN")]
    truncate_length: Option<String>,
}

impl Args {
    /// Case 't': hfst_strtoul(optarg, 10), fatal on anything else.
    fn truncate_length(&self, common: &CommonOptions) -> u64 {
        match &self.truncate_length {
            Some(len) => parse_u64(common, len, 10),
            None => 0,
        }
    }

    fn options(&self, common: &CommonOptions) -> Options {
        Options {
            transducer_name: self.name.clone().unwrap_or_default(),
            name_option_given: self.name.is_some(),
            print_name: self.print_name,
            truncate_length: self.truncate_length(common),
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
        // The C rejected a non-numeric LEN inside the getopt loop, before
        // the parameter checks; run it here for the same ordering.
        self.truncate_length(opts);
        Ok(())
    }
}

// [spec:hfst:def:hfst-name.process-stream-fn]
// [spec:hfst:sem:hfst-name.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;

        if transducer_n > 1 && options.print_name {
            eprintln!("---");
        }

        if transducer_n == 1 {
            verbose_print(common, &format!("Naming {}...\n", common.input_filename));
        } else {
            verbose_print(
                common,
                &format!("Naming {}...{}\n", common.input_filename, transducer_n),
            );
        }

        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("hfst-name: {e}");
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mut trans = trans;
            if !options.print_name {
                let name = options.transducer_name.clone();
                if options.truncate_length > 0 {
                    // C: hfst_strndup copies at most TRUNCATE_LENGTH bytes.
                    let n = (options.truncate_length as usize).min(name.len());
                    let truncated = String::from_utf8_lossy(&name.as_bytes()[..n]).into_owned();
                    trans.set_name(&truncated);
                } else {
                    trans.set_name(&name);
                }
                if let Err(e) = outstream.write(&mut trans) {
                    eprintln!("hfst-name: {e}");
                    return 1;
                }
            } else {
                eprintln!("\"{}\"", trans.get_name());
            }
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-name.main-fn]
// [spec:hfst:sem:hfst-name.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstName");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = args.options(&common);

    if !options.print_name && !options.name_option_given {
        eprintln!("Error: hfst-name: use either option --print-name  or --name");
        return Err(1);
    }
    if options.print_name && options.name_option_given {
        eprintln!("Warning: option --print-name overrides option --name");
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

    // here starts the buffer handling part
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            eprintln!("hfst-name: {e}");
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
            eprintln!("hfst-name: {e}");
            return Err(1);
        }
    };

    cli::from_code(process_stream(
        &common,
        &options,
        &mut instream,
        &mut outstream,
    ))
}
