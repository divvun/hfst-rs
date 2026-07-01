//! Faithful 1:1 port of tools/src/hfst-expand-equivalences.cc — the transducer
//! label modification tool for equivalence classes. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, tool-metadata,
//! inc fragments).

use hfst::expand_equivalences::{
    FsaLevel, TsvExtensionError, expand_equivalences, read_tsv_extensions,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, error_at_line, extend_options_getenv, hfst_set_program_name,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
};
use std::io::Write;

// Tool-specific static-mut option state, mirroring the C++ file-scope statics.
// C used NULL char* as "unset"; modelled here as Option<String>.
static mut ONLY_FROM_LABEL: Option<String> = None;
static mut ONLY_TO_LABEL: Option<String> = None;
static mut ACX_FILE_NAME: Option<String> = None;
// C: ACX_FILE was a 'FILE*' opened by hfst_fopen and only ever tested for
// non-null (the libxml ACX-parsing body compiles to nothing without libxml).
// Here it is just an "opened" flag.
static mut ACX_FILE_OPENED: bool = false;
static mut TSV_FILE_NAME: Option<String> = None;

// FsaLevel, the TSV reader, and the extension/compose loop now live in
// hfst::expand_equivalences; this tool keeps only the option-driven LEVEL global.
// The TSV file is opened (as a std stream) and parsed in process_stream, so no
// libc TSV handle is held here.
static mut LEVEL: FsaLevel = FsaLevel::First;

fn only_from_label() -> Option<String> {
    unsafe { (*std::ptr::addr_of!(ONLY_FROM_LABEL)).clone() }
}
fn only_to_label() -> Option<String> {
    unsafe { (*std::ptr::addr_of!(ONLY_TO_LABEL)).clone() }
}
fn acx_file_name() -> Option<String> {
    unsafe { (*std::ptr::addr_of!(ACX_FILE_NAME)).clone() }
}
fn tsv_file_name() -> Option<String> {
    unsafe { (*std::ptr::addr_of!(TSV_FILE_NAME)).clone() }
}

// [spec:hfst:def:hfst-expand-equivalences.print-usage-fn]
// [spec:hfst:sem:hfst-expand-equivalences.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = globals::program_name();
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
    let _ = write!(msg, "\n");
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
    print_report_bugs();
    print_more_info();
}

// [spec:hfst:def:hfst-expand-equivalences.parse-options-fn]
// [spec:hfst:sem:hfst-expand-equivalences.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd common cases, then the tool's
            // own cases, then the terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c as u8 {
                b'f' => {
                    ONLY_FROM_LABEL = Some(getopt::optarg());
                    continue;
                }
                b't' => {
                    ONLY_TO_LABEL = Some(getopt::optarg());
                    continue;
                }
                b'a' => {
                    ACX_FILE_NAME = Some(getopt::optarg());
                    continue;
                }
                b'T' => {
                    TSV_FILE_NAME = Some(getopt::optarg());
                    continue;
                }
                b'l' => {
                    let optarg = getopt::optarg();
                    if optarg == "first" || optarg == "upper" || optarg == "input" || optarg == "1"
                    {
                        LEVEL = FsaLevel::First;
                    } else if optarg == "second"
                        || optarg == "lower"
                        || optarg == "output"
                        || optarg == "2"
                    {
                        LEVEL = FsaLevel::Second;
                    } else if optarg == "both" {
                        LEVEL = FsaLevel::Both;
                    } else {
                        error(
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
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-expand-equivalences.check-options-fn]
// [spec:hfst:sem:hfst-expand-equivalences.check-options-fn]
unsafe fn check_options() {
    unsafe {
        if only_from_label().is_some() || only_to_label().is_some() {
            if tsv_file_name().is_some() || acx_file_name().is_some() {
                error(1, 0, "Only one of -a, -T or -f and -t may be given");
            } else if only_from_label().is_none() {
                error(1, 0, "option -t requires -f");
            } else if only_to_label().is_none() {
                error(1, 0, "option -f requires -t");
            }
        } else if tsv_file_name().is_none() && acx_file_name().is_none() {
            error(
                1,
                0,
                "Must give extension specification file with either -a or -t.",
            );
        } else if tsv_file_name().is_some() && acx_file_name().is_some() {
            error(1, 0, "Only one of parameters -a, -t, must be used.");
        } else if tsv_file_name().is_some() {
            // TSV is opened as a std stream and parsed in process_stream via
            // read_tsv_extensions; no libc handle is opened here. A missing file
            // is reported there (slightly later than the C++, which fopen'd it at
            // this point) with the same fatal error.
        } else if let Some(name) = acx_file_name() {
            match std::fs::File::open(&name) {
                Ok(_f) => ACX_FILE_OPENED = true,
                Err(_) => {
                    error(1, 0, &format!("Could not open '{}'", name));
                }
            }
        } else {
            error(1, 0, "Logic error again!");
        }
    }
}

// [spec:hfst:def:hfst-expand-equivalences.process-stream-fn]
// [spec:hfst:sem:hfst-expand-equivalences.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let _ = transducer_n; // C++ counts but never reads it
            let trans = match HfstTransducer::new_from_stream(instream) {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };

            // Collect the (from, to) extension pairs from whichever source the
            // options selected. The TSV parser and the extension/compose loop now
            // live in hfst::expand_equivalences; the per-extension "extending X by
            // Y" and "Applying extensions on N level" -v traces were diagnostic and
            // are not reproduced.
            let mut pairs: Vec<(String, String)> = Vec::new();
            if let Some(from) = only_from_label() {
                let to = only_to_label().unwrap_or_default();
                verbose_printf(&format!(
                    "using single commandline extension {} with {}\n",
                    from, to
                ));
                pairs.push((from, to));
            } else if let Some(tsv_name) = tsv_file_name() {
                verbose_printf(&format!("reading extensions from {}...\n", tsv_name));
                let file = match std::fs::File::open(&tsv_name) {
                    Ok(f) => f,
                    Err(e) => {
                        error(1, 0, &format!("cannot open {}: {}", tsv_name, e));
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
            } else if ACX_FILE_OPENED {
                verbose_printf(&format!(
                    "Reading ACX from {}...\n",
                    acx_file_name().unwrap_or_default()
                ));
                // The libxml ACX-parsing body is gated behind #if HAVE_LIBXML_TREE_H
                // in the C++ source; without libxml it compiles to nothing, which
                // is the path reproduced here (no extensions added).
            } else {
                error(1, 0, "DANGER TERROR HORROR !!!!!!");
                return;
            }

            let mut trans = match expand_equivalences(trans, &pairs, LEVEL) {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            if let Err(e) = outstream.redirect(&mut trans) {
                error(1, 0, &format!("{e}"));
                return;
            }
        } // for each automaton
    }
}

// [spec:hfst:def:hfst-expand-equivalences.main-fn]
// [spec:hfst:sem:hfst-expand-equivalences.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstExpandEquivalences");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        check_options();

        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let type_ = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-expand-equivalences") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream);
        instream.close();
        outstream.close();
        0
    }
}
