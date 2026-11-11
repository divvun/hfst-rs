//! Faithful 1:1 port of tools/src/hfst-substitute.cc — the transducer label
//! modification (relabel arcs) command-line tool. A UNARY tool that
//! additionally reads an optional replacement transducer (-T) and/or a
//! replacement file (-F). Option handling is clap 4 derive through
//! [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_error, hfst_error_at_line, hfst_set_program_name, hfst_warning,
    is_input_stream_in_ol_format, verbose_print,
};
use hfst::hfst_data_types::{ImplementationType, StringPair};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
use hfst::hfst_transducer::HfstTransducer;
use hfst::substitute_driver::{SubstituteEngine, SubstituteRequest};
use hfst::tools::conversion::conversion_needed;
use hfst::tools::format::format_description;
use hfst::tools::substitute::{
    RelabelError, SubstitutionName, name_substituted, relabel_entries, substitute_label,
};
use std::io::{BufRead, BufReader};

/// hfst-substitute's own options (the former tool-specific `static mut`s). The
/// `-C, --do-not-convert` flag lives in [`CommonOptions::allow_transducer_conversion`].
#[derive(Default)]
struct Options {
    from_label: Option<String>,
    from_pair: Option<StringPair>,
    from_file_name: Option<String>,
    /// '-F': the relabel file's text, until its first line that is not
    /// UTF-8, taken by the first transducer read.
    from_file: Option<String>,
    to_label: Option<String>,
    to_pair: Option<StringPair>,
    to_transducer_filename: Option<String>,
    compose: bool,
    in_order: bool,
}

// The substitution the engine is asked for, rebuilt from the options before
// every call: a relabel file rewrites the from/to fields line by line.
fn substitute_request(options: &Options) -> SubstituteRequest {
    SubstituteRequest {
        from_label: options.from_label.clone(),
        from_pair: options.from_pair.clone(),
        to_label: options.to_label.clone(),
        to_pair: options.to_pair.clone(),
        to_transducer_filename: options.to_transducer_filename.clone(),
        compose: options.compose,
    }
}

// The lines of the file `name`, read up to the first that is not UTF-8, as
// 'hfst_getline' read them.
fn read_lines(name: &str) -> std::io::Result<String> {
    let mut reader = BufReader::new(std::fs::File::open(name)?);
    let mut text = String::new();
    while reader.read_line(&mut text).unwrap_or(0) > 0 {}
    Ok(text)
}

/// hfst-substitute's command line.
// [spec:hfst:def:hfst-substitute.parse-options-fn]
// [spec:hfst:sem:hfst-substitute.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Relabel transducer arcs",
    after_help = "LABEL must be a symbol name in single arc in transducer,
or colon separated pair defining an arc.
If TFILE is specified, FLABEL must be a pair.
LABELFILE is a 2 column tsv file where col 1 is FLABEL
and col 2 gives TLABEL specifications.

Examples:
  hfst-substitute -i tr.hfst -o tr_relabeled.hfst -f 'a' -t 'A'
      relabel all symbols 'a' with 'A'
  hfst-substitute -i tr.hfst -o tr_relabeled.hfst -f 'a:b' -t 'A:B'
      relabel all arcs 'a:b' with 'A:B'
  hfst-substitute -i tr.hfst -o tr_relabeled.hfst -f 'a:b' -T repl.hfst
      replace all arcs 'a:b' with transducer repl.hfst"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// replace FLABEL
    #[arg(
        short = 'f',
        long = "from-label",
        value_name = "FLABEL",
        allow_hyphen_values = true
    )]
    from_label: Option<String>,

    /// replace with TLABEL
    #[arg(
        short = 't',
        long = "to-label",
        value_name = "TLABEL",
        allow_hyphen_values = true
    )]
    to_label: Option<String>,

    /// replace with transducer read from TFILE
    #[arg(
        short = 'T',
        long = "to-transducer",
        value_name = "TFILE",
        allow_hyphen_values = true
    )]
    to_transducer: Option<String>,

    /// read replacements from LABELFILE
    #[arg(
        short = 'F',
        long = "from-file",
        value_name = "LABELFILE",
        allow_hyphen_values = true
    )]
    from_file: Option<String>,

    /// keep the order of the replacements (with -F)
    #[arg(short = 'R', long = "in-order")]
    in_order: bool,

    /// require that transducers in TFILE and INFILE have the same type
    #[arg(short = 'C', long = "do-not-convert")]
    do_not_convert: bool,

    /// compose substitutions when possible
    #[arg(short = '9', long = "compose")]
    compose: bool,
}

impl Args {
    /// The '@0@' epsilon spelling plus the empty-label diagnostic of the C's
    /// 'f' and 't' cases; `source` picks which of the two messages fires.
    fn resolve_label(
        &self,
        opts: &CommonOptions,
        given: &Option<String>,
        source: bool,
    ) -> Result<Option<String>, i32> {
        let Some(label) = given else {
            return Ok(None);
        };
        let label = substitute_label(label);
        if label.is_empty() {
            let message = if source {
                format!(
                    "argument of source label option is empty;\n\
                     if you REALLY want to replace epsilons with something, use @0@ or {}",
                    internal_epsilon
                )
            } else {
                format!(
                    "argument of target label option is empty;\n\
                     if you want to substitute something with epsilons, use @0@ or {}",
                    internal_epsilon
                )
            };
            hfst_error(opts, 1, 0, &message);
            return Err(1);
        }
        Ok(Some(label))
    }

    /// The 'F' and 'T' cases probed their file with hfst_fopen at parse time.
    fn check_openable(&self, opts: &CommonOptions, given: &Option<String>) -> ToolResult {
        if let Some(fname) = given
            && std::fs::File::open(fname).is_err()
        {
            error(opts, 1, 0, &format!("Could not open '{}'", fname));
            return Err(1);
        }
        Ok(())
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        if self.do_not_convert {
            opts.allow_transducer_conversion = false;
        }
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The label/file checks ran inside the C getopt loop, the two
        // must-give checks right after it — all before the parameter checks.
        self.resolve_label(opts, &self.from_label, true)?;
        self.resolve_label(opts, &self.to_label, false)?;
        self.check_openable(opts, &self.from_file)?;
        self.check_openable(opts, &self.to_transducer)?;
        if self.from_label.is_none() && self.from_file.is_none() {
            hfst_error(
                opts,
                1,
                0,
                "Must state name of labels to rewrite with -f or -F",
            );
            return Err(1);
        }
        if self.to_label.is_none() && self.to_transducer.is_none() && self.from_file.is_none() {
            hfst_error(opts, 1, 0, "Must give target labels with -t, -T or -F");
            return Err(1);
        }
        Ok(())
    }
}

// (The former parse_options getopt loop is replaced by the clap derive
// struct above plus `resolve_options` below.)
fn resolve_options(common: &CommonOptions, args: &Args) -> Result<Options, i32> {
    let from_label = args.resolve_label(common, &args.from_label, true)?;
    let to_label = args.resolve_label(common, &args.to_label, false)?;
    let from_file = match &args.from_file {
        Some(fname) => match read_lines(fname) {
            Ok(text) => Some(text),
            Err(_) => {
                error(common, 1, 0, &format!("Could not open '{}'", fname));
                return Err(1);
            }
        },
        None => None,
    };
    Ok(Options {
        from_pair: from_label.as_deref().and_then(label_to_stringpair),
        from_label,
        from_file_name: args.from_file.clone(),
        from_file,
        to_pair: to_label.as_deref().and_then(label_to_stringpair),
        to_label,
        to_transducer_filename: args.to_transducer.clone(),
        compose: args.compose,
        in_order: args.in_order,
    })
}

// [spec:hfst:def:hfst-substitute.process-stream-fn]
// [spec:hfst:sem:hfst-substitute.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    instream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut to_any: Option<hfst::hfst_transducer::AnyTransducer> = None;
    let mut output_type = ImplementationType::UNSPECIFIED_TYPE;

    if options.to_transducer_filename.is_some() {
        let to_fname = options.to_transducer_filename.clone().unwrap();
        // (C wraps the ctor in try/catch on NotTransducerStreamException; the
        // Rust ctor panics on a bad file rather than throwing.)
        let mut tostream = match HfstInputStream::new_filename(&to_fname) {
            Ok(s) => s,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let mut to_transducer = match tostream.read() {
            Ok(t) => t,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        tostream.close();
        let to_transducer_type = to_transducer.get_type();
        let instream_type = instream.get_type();
        if let Some(note) = conversion_needed(instream_type, to_transducer_type) {
            if common.allow_transducer_conversion {
                hfst_warning(
                    common,
                    0,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; {}",
                        common.input_filename,
                        to_fname,
                        note.output_choice()
                    ),
                );
                output_type = note.target();
                to_transducer = match to_transducer.convert(output_type, "") {
                    Ok(t) => t,
                    Err(e) => {
                        hfst_error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
            } else {
                hfst_error(
                    common,
                    1,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; \
                         formats {} and {} are not compatible for substitution \
                         (--do-not-convert was requested)",
                        common.input_filename,
                        to_fname,
                        format_description(instream_type),
                        format_description(to_transducer_type)
                    ),
                );
            }
        } else {
            output_type = instream.get_type();
        }
        to_any = Some(to_transducer);
    } else {
        output_type = instream.get_type();
    }

    let output_named = common.output_filename != "<stdout>";
    let mut outstream = match if output_named {
        HfstOutputStream::new_filename(&common.output_filename, output_type, true)
    } else {
        HfstOutputStream::new(output_type, true)
    } {
        Ok(s) => s,
        Err(e) => {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    // The resolved output type is matched ONCE into the loop's backend
    // type parameter ([dec:hfst:monomorphic-backends]); OL streams were
    // rejected before this point.
    match output_type {
        #[cfg(feature = "foma")]
        ImplementationType::FOMA_TYPE => process_loop::<hfst::backend_foma::FomaTransducer>(
            common,
            options,
            instream,
            &mut outstream,
            to_any,
        ),
        _ => process_loop::<hfst_openfst::StdVectorFst>(
            common,
            options,
            instream,
            &mut outstream,
            to_any,
        ),
    }
}

fn process_loop<B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer>(
    common: &CommonOptions,
    options: &mut Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
    to_any: Option<hfst::hfst_transducer::AnyTransducer>,
) -> i32 {
    let mut transducer_n: usize = 0;

    let to_transducer: Option<HfstTransducer<B>> = match to_any {
        Some(a) => match a.into_typed() {
            Ok(t) => Some(t),
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        },
        None => None,
    };
    let mut engine: SubstituteEngine<B> = SubstituteEngine::new(to_transducer);
    let reporter = crate::CliVerboseReporter::new(common);

    while instream.is_good() {
        transducer_n += 1;
        let mut trans: HfstTransducer<B> = match instream.read().and_then(|any| any.into_typed()) {
            Ok(t) => t,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let inputname = {
            let n = trans.get_name();
            if n.is_empty() {
                common.input_filename.clone()
            } else {
                n
            }
        };
        if transducer_n == 1 {
            verbose_print(
                common,
                &format!("performing substitutions in {}...\n", inputname),
            );
        } else {
            verbose_print(
                common,
                &format!(
                    "performing substitutions in {}... {}\n",
                    inputname, transducer_n
                ),
            );
        }
        // initialize delayed substitutor automaton
        engine.begin_transducer();
        if let Some(from_file) = options.from_file.as_mut() {
            let text = std::mem::take(from_file);
            let from_file_name = options.from_file_name.clone().unwrap_or_default();
            verbose_print(
                common,
                &format!("reading substitutions from {}...\n", from_file_name),
            );
            let entries = match relabel_entries(&text) {
                Ok(entries) => entries,
                Err(RelabelError { line, error }) => {
                    hfst_error_at_line(common, 1, 0, &from_file_name, line, &error.to_string());
                    return 1;
                }
            };
            for (fl, tl) in entries {
                options.from_pair = label_to_stringpair(&fl);
                options.to_pair = label_to_stringpair(&tl);
                options.from_label = Some(fl);
                options.to_label = Some(tl);

                if let Err(e) = engine.apply_relabel_entry(
                    &substitute_request(options),
                    &mut trans,
                    transducer_n,
                    options.in_order,
                    &reporter,
                ) {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }

            if let Err(e) = engine.flush_batched(&mut trans, options.in_order) {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
        // if not from file
        else if let Err(e) = engine.do_substitute(
            &substitute_request(options),
            &mut trans,
            transducer_n,
            &reporter,
        ) {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if engine.is_delayed()
            && let Err(e) = engine.perform_delayed(&mut trans, &reporter)
        {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        let naming = if options.from_file.is_some() {
            Some(SubstitutionName::Relabel {
                file_name: options.from_file_name.as_deref().unwrap_or_default(),
            })
        } else if let (Some(from), Some(to)) = (&options.from_label, &options.to_label) {
            Some(SubstitutionName::Label { from, to })
        } else if let (Some(from), Some(file_name)) =
            (&options.from_label, &options.to_transducer_filename)
        {
            Some(SubstitutionName::Transducer { from, file_name })
        } else {
            None
        };
        if let Some(naming) = naming {
            name_substituted(&mut trans, naming);
        }
        if let Err(e) = outstream.write(&trans) {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }
    // delete to_transducer
    engine.release_to_transducer();
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-substitute.main-fn]
// [spec:hfst:sem:hfst-substitute.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSubstitute");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = resolve_options(&common, &args)?;

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    // (the C allocated the two batched substitution maps here; they are now
    // fields of the library engine, allocated with it in process_loop.)

    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing.)
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

    if is_input_stream_in_ol_format(&instream, "hfst-substitute") {
        return Err(1);
    }

    cli::from_code(process_stream(&common, &mut options, &mut instream))
}
