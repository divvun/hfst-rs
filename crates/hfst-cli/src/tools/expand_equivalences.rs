//! Faithful 1:1 port of tools/src/hfst-expand-equivalences.cc — the transducer
//! label modification tool for equivalence classes. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, tool-metadata,
//! inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, error_at_line, extend_options_from_env, hfst_set_program_name,
    is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
};
use hfst::expand_equivalences::{
    FsaLevel, TsvExtensionError, expand_equivalences, read_tsv_extensions,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-expand-equivalences's own options (the former tool-specific `static mut`s).
///
/// C used NULL char* as "unset"; modelled here as `Option<String>`. The C++
/// `ACX_FILE` was a `FILE*` opened by `hfst_fopen` and only ever tested for
/// non-null (the libxml ACX-parsing body compiles to nothing without libxml);
/// here it is just an "opened" flag.
struct Options {
    only_from_label: Option<String>,
    only_to_label: Option<String>,
    acx_file_name: Option<String>,
    acx_file_opened: bool,
    tsv_file_name: Option<String>,
    // FsaLevel, the TSV reader, and the extension/compose loop now live in
    // hfst::expand_equivalences; this tool keeps only the option-driven LEVEL.
    // The TSV file is opened (as a std stream) and parsed in process_stream, so
    // no libc TSV handle is held here.
    level: FsaLevel,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            only_from_label: None,
            only_to_label: None,
            acx_file_name: None,
            acx_file_opened: false,
            tsv_file_name: None,
            level: FsaLevel::First,
        }
    }
}

// [spec:hfst:def:hfst-expand-equivalences.print-usage-fn]
// [spec:hfst:sem:hfst-expand-equivalences.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = &common.program_name;
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nExtend transducer arcs for equivalence classes\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Eqv. class extension options:\n\
         \x20 -f, --from=ISYM     convert single symbol ISYM to allow OSYM\n\
         \x20 -t, --to=OSYM       convert to OSYM\n\
         \x20 -a, --acx=ACXFILE   read extensions in acx format from ACXFILE\n\
         \x20 -T, --tsv=TSVFILE   read extensions in tsv format from TSVFILE\n\
         \x20 -l, --level=LEVEL   perform extensions on LEVEL of fsa\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "Either ACXFILE, TSVFILE or both ISYM and OSYM must be specified.\n\
         LEVEL should be either {{upper, first, 1, input, surface}}, \
         {{lower, second, 2, output, analysis}} or both.\n\
         If LEVEL is omitted, default is first.\n"
    );
    let _ = write!(
        msg,
        "Examples:\n\
         \x20 {} -o rox.hfst -a romanian.acx ro.hfst  extend romanian char\
         equivalences\n\n",
        program_name
    );
}

// [spec:hfst:def:hfst-expand-equivalences.parse-options-fn]
// [spec:hfst:sem:hfst-expand-equivalences.parse-options-fn]
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
        long_options.push(getopt::GetOpt {
            name: "from",
            has_arg: 1, // required_argument
            val: b'f' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "to",
            has_arg: 1,
            val: b't' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "acx",
            has_arg: 1,
            val: b'a' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "tsv",
            has_arg: 1,
            val: b'T' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "level",
            has_arg: 1,
            val: b'l' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd common cases, then the tool's
        // own cases, then the terminal error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match c as u8 {
            b'f' => {
                options.only_from_label = Some(opt.optarg());
                continue;
            }
            b't' => {
                options.only_to_label = Some(opt.optarg());
                continue;
            }
            b'a' => {
                options.acx_file_name = Some(opt.optarg());
                continue;
            }
            b'T' => {
                options.tsv_file_name = Some(opt.optarg());
                continue;
            }
            b'l' => {
                let optarg = opt.optarg();
                if optarg == "first" || optarg == "upper" || optarg == "input" || optarg == "1" {
                    options.level = FsaLevel::First;
                } else if optarg == "second"
                    || optarg == "lower"
                    || optarg == "output"
                    || optarg == "2"
                {
                    options.level = FsaLevel::Second;
                } else if optarg == "both" {
                    options.level = FsaLevel::Both;
                } else {
                    error(
                        &common,
                        1,
                        0,
                        "The option for level parameter must be one of:\n\
                         upper, first, input; second, lower, output; both, \
                         1 or 2.",
                    );
                }
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-expand-equivalences.check-options-fn]
// [spec:hfst:sem:hfst-expand-equivalences.check-options-fn]
fn check_options(common: &CommonOptions, options: &mut Options) {
    if options.only_from_label.is_some() || options.only_to_label.is_some() {
        if options.tsv_file_name.is_some() || options.acx_file_name.is_some() {
            error(common, 1, 0, "Only one of -a, -T or -f and -t may be given");
        } else if options.only_from_label.is_none() {
            error(common, 1, 0, "option -t requires -f");
        } else if options.only_to_label.is_none() {
            error(common, 1, 0, "option -f requires -t");
        }
    } else if options.tsv_file_name.is_none() && options.acx_file_name.is_none() {
        error(
            common,
            1,
            0,
            "Must give extension specification file with either -a or -t.",
        );
    } else if options.tsv_file_name.is_some() && options.acx_file_name.is_some() {
        error(common, 1, 0, "Only one of parameters -a, -t, must be used.");
    } else if options.tsv_file_name.is_some() {
        // TSV is opened as a std stream and parsed in process_stream via
        // read_tsv_extensions; no libc handle is opened here. A missing file
        // is reported there (slightly later than the C++, which fopen'd it at
        // this point) with the same fatal error.
    } else if let Some(name) = options.acx_file_name.clone() {
        match std::fs::File::open(&name) {
            Ok(_f) => options.acx_file_opened = true,
            Err(_) => {
                error(common, 1, 0, &format!("Could not open '{}'", name));
            }
        }
    } else {
        error(common, 1, 0, "Logic error again!");
    }
}

// [spec:hfst:def:hfst-expand-equivalences.process-stream-fn]
// [spec:hfst:sem:hfst-expand-equivalences.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let _ = transducer_n; // C++ counts but never reads it
        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_algebra!(any, trans => {
            // Collect the (from, to) extension pairs from whichever source the
            // options selected. The TSV parser and the extension/compose loop now
            // live in hfst::expand_equivalences; the per-extension "extending X by
            // Y" and "Applying extensions on N level" -v traces were diagnostic and
            // are not reproduced.
            let mut pairs: Vec<(String, String)> = Vec::new();
            if let Some(from) = options.only_from_label.clone() {
                let to = options.only_to_label.clone().unwrap_or_default();
                verbose_print(common, &format!(
                    "using single commandline extension {} with {}\n",
                    from, to
                ));
                pairs.push((from, to));
            } else if let Some(tsv_name) = options.tsv_file_name.clone() {
                verbose_print(common, &format!("reading extensions from {}...\n", tsv_name));
                let file = match std::fs::File::open(&tsv_name) {
                    Ok(f) => f,
                    Err(e) => {
                        error(common, 1, 0, &format!("cannot open {}: {}", tsv_name, e));
                        return;
                    }
                };
                match read_tsv_extensions(std::io::BufReader::new(file)) {
                    Ok(p) => pairs = p,
                    Err(TsvExtensionError { line, message }) => {
                        error_at_line(1, 0, &tsv_name, line, &message);
                        return;
                    }
                }
            } else if options.acx_file_opened {
                verbose_print(common, &format!(
                    "Reading ACX from {}...\n",
                    options.acx_file_name.clone().unwrap_or_default()
                ));
                // The libxml ACX-parsing body is gated behind #if HAVE_LIBXML_TREE_H
                // in the C++ source; without libxml it compiles to nothing, which
                // is the path reproduced here (no extensions added).
            } else {
                error(common, 1, 0, "DANGER TERROR HORROR !!!!!!");
                return;
            }

            let mut trans = match expand_equivalences(trans, &pairs, options.level) {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return;
                }
            };
            if let Err(e) = outstream.redirect(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = writeln!(
                std::io::stderr(),
                "Error: hfst-expand-equivalences cannot process transducers that are in optimized lookup format."
            );
            return;
        });
    } // for each automaton
}

// [spec:hfst:def:hfst-expand-equivalences.main-fn]
// [spec:hfst:sem:hfst-expand-equivalences.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstExpandEquivalences");
    let (common, mut options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    check_options(&common, &mut options);

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
            return 1;
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
            return 1;
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-expand-equivalences") {
        return 1;
    }

    process_stream(&common, &options, &mut instream, &mut outstream);
    instream.close();
    outstream.close();
    0
}
