//! Faithful 1:1 port of tools/src/hfst-edit-metadata.cc — the transducer
//! metadata tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and a
//! tool-local [`Options`], threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, parse_u64, verbose_print, warning};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::collections::BTreeMap;
use std::io::Write;

/// hfst-edit-metadata's command line.
//
// '-p' takes an OPTIONAL argument, so it only ever binds a value written
// as '-pNAME' or '--print-name=NAME'; a following word is an operand, not
// the property name.
// [spec:hfst:def:hfst-edit-metadata.parse-options-fn]
// [spec:hfst:sem:hfst-edit-metadata.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Name a transducer",
    after_help = "If NAME is omitted from --print-name, all values are printed"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Add or replace property ANAME with VALUE
    #[arg(
        short = 'a',
        long = "add",
        value_name = "ANAME=VALUE",
        action = clap::ArgAction::Append
    )]
    add: Vec<String>,

    /// Print the current NAME; without NAME, print every property
    //
    // The default-missing value is a NUL, which no argv string can carry:
    // it marks the bare '-p' while still giving clap a value to index, so
    // the '-a'-versus-bare-'-p' ordering below is recoverable.
    #[arg(
        short = 'p',
        long = "print-name",
        value_name = "NAME",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "\0"
    )]
    print_name: Option<String>,

    /// Truncate added properties' lengths to LEN
    #[arg(short = 't', long = "truncate_length", value_name = "LEN")]
    truncate_length: Option<String>,

    /// Whether every property is printed. Both '-a' and a bare '-p' wrote
    /// this in the C getopt loop and the last write won, so it is recovered
    /// from the match indices rather than from the field values.
    #[arg(skip = true)]
    print_all_properties: bool,
}

impl Args {
    /// Case 'a': split each ANAME=VALUE at its first '=', refusing one
    /// without a separator exactly as the C did.
    fn properties(&self, common: &CommonOptions) -> BTreeMap<String, String> {
        let mut properties = BTreeMap::new();
        for spec in &self.add {
            match spec.find('=') {
                None => {
                    error(
                        common,
                        1,
                        0,
                        &format!("Equals sign `=' missing from {}", spec),
                    );
                }
                Some(idx) => {
                    properties.insert(spec[..idx].to_string(), spec[idx + 1..].to_string());
                }
            }
        }
        properties
    }

    /// Case 't': hfst_strtoul(optarg, 10), fatal on anything else.
    fn truncate_length(&self, common: &CommonOptions) -> u64 {
        match &self.truncate_length {
            Some(len) => parse_u64(common, len, 10),
            None => 0,
        }
    }

    /// The property '-p' names, with the bare-'-p' sentinel read back as
    /// the C's NULL print_property.
    fn named_property(&self) -> Option<String> {
        self.print_name
            .as_deref()
            .filter(|n| *n != "\0")
            .map(str::to_string)
    }

    fn options(&self, common: &CommonOptions) -> Options {
        Options {
            properties: self.properties(common),
            print_all_properties: self.print_all_properties,
            print_property: self.named_property(),
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
        // Both refusals fired inside the C getopt loop, before the
        // parameter checks; run them here for the same ordering.
        self.properties(opts);
        self.truncate_length(opts);
        Ok(())
    }

    // The C loop starts print_all_properties at true, '-a' sets it false
    // and a bare '-p' sets it true, so the flag is decided by whichever
    // came last. A '-p' carrying a value never touches it.
    fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
        let last_add = matches.indices_of("add").and_then(|i| i.max());
        let bare_print = self.print_name.as_deref() == Some("\0");
        self.print_all_properties = match (last_add, bare_print) {
            (None, _) => true,
            (Some(_), false) => false,
            (Some(add), true) => matches
                .index_of("print_name")
                .is_some_and(|print| print > add),
        };
    }
}

/// hfst-edit-metadata's resolved options (the former tool-specific
/// `static mut`s).
struct Options {
    /// '-a, --add=ANAME=VALUE': the properties to add or replace.
    properties: BTreeMap<String, String>,
    /// whether all properties should be printed (the default).
    print_all_properties: bool,
    /// '-p, --print-name[=NAME]': the specific property to print. C used a
    /// NULL char* as "no specific property requested"; modelled as Option.
    print_property: Option<String>,
    /// '-t, --truncate_length=LEN': truncate added property lengths to LEN.
    truncate_length: u64,
}

// [spec:hfst:def:hfst-edit-metadata.process-stream-fn]
// [spec:hfst:sem:hfst-edit-metadata.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-edit-metadata: cannot open output: {e}");
            return 1;
        }
    };
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;

        if transducer_n > 1 && (options.print_all_properties || options.print_property.is_some()) {
            eprintln!("--- ");
        }

        if transducer_n == 1 {
            verbose_print(common, &format!("Metadata {}...\n", common.input_filename));
        } else {
            verbose_print(
                common,
                &format!("Metadata {}...{}\n", common.input_filename, transducer_n),
            );
        }

        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mut trans = trans;
            if !options.print_all_properties && options.print_property.is_none() {
                for (key, val) in options.properties.iter() {
                    if key == "type" {
                        warning(
                            common,
                            0,
                            0,
                            "Changing `type' metadata will not change type of transducer in file;\n\
                             having wrong type may cause breakage, use with caution",
                        );
                    } else if key == "version" {
                        warning(
                            common,
                            0,
                            0,
                            "Changing `version' changes parsing semantics for header;\n\
                             use with caution",
                        );
                    } else if key == "character-encoding" && !(val == "utf-8" || val == "UTF-8") {
                        error(
                            common,
                            1,
                            0,
                            "Cannot set `character-encoding' to unsupported value;\n\
                             consider recoding sources of automaton",
                        );
                    }
                    if options.truncate_length > 0 {
                        // C: hfst_strndup(value.c_str(), truncate_length) — copy
                        // up to truncate_length bytes (NUL-terminating early).
                        let bytes = val.as_bytes();
                        let n = (options.truncate_length as usize).min(bytes.len());
                        let truncated = String::from_utf8_lossy(&bytes[..n]).into_owned();
                        trans.set_property(key, &truncated);
                    } else {
                        trans.set_property(key, val);
                    }
                }
                if let Err(e) = outstream.write(&mut trans) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            } else {
                let props = trans.get_properties();
                if options.print_all_properties {
                    for (key, val) in props.iter() {
                        let _ = writeln!(out, "{}: {}", key, val);
                    }
                } else {
                    let pp = options.print_property.clone().unwrap_or_default();
                    let _ = writeln!(out, "{}", props.get(&pp).unwrap());
                }
            }
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-edit-metadata.main-fn]
// [spec:hfst:sem:hfst-edit-metadata.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstEditMetadata");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = args.options(&common);

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

    cli::from_code(process_stream(
        &common,
        &options,
        &mut instream,
        &mut outstream,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, FromArgMatches};

    /// Parse an argv the way [`cli::parse`] does, so the ordering hook runs.
    fn parse(argv: &[&str]) -> Args {
        let matches = Args::command()
            .try_get_matches_from(argv)
            .expect("argv parses");
        let mut args = Args::from_arg_matches(&matches).expect("matches convert");
        args.absorb_matches(&matches);
        args
    }

    // The C loop's last write to print_all_properties wins. The bare '-p'
    // rides on a NUL default-missing value so clap has an index to compare;
    // if a clap upgrade ever stops recording one, the last two cases here
    // are what notices.
    #[test]
    fn add_and_bare_print_resolve_by_position() {
        assert!(parse(&["hfst-edit-metadata"]).print_all_properties);
        assert!(parse(&["hfst-edit-metadata", "-p"]).print_all_properties);
        assert!(parse(&["hfst-edit-metadata", "--print-name=name"]).print_all_properties);
        assert!(!parse(&["hfst-edit-metadata", "-a", "k=v"]).print_all_properties);
        assert!(parse(&["hfst-edit-metadata", "-a", "k=v", "-p"]).print_all_properties);
        assert!(!parse(&["hfst-edit-metadata", "-p", "-a", "k=v"]).print_all_properties);
    }

    /// A bare '-p' means "every property"; only an attached value names one.
    #[test]
    fn only_an_attached_value_names_a_property() {
        assert_eq!(parse(&["hfst-edit-metadata", "-p"]).named_property(), None);
        assert_eq!(
            parse(&["hfst-edit-metadata", "--print-name=name"]).named_property(),
            Some("name".to_string())
        );
    }
}
