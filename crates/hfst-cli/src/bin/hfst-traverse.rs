//! Faithful 1:1 port of tools/src/hfst-traverse.cc — the transducer traversal
//! tool that walks through a transducer arc by arc. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, inc fragments).

use core::ffi::{c_char, c_int};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_readline, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::collections::BTreeMap;
use std::ffi::{CStr, CString};

// add tools-specific variables here
static mut CAVE_MODE: bool = false;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

fn fput(f: &mut dyn std::io::Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

// The C arclabel readline-completion helpers (arclabel_generator /
// arclabel_completion) are gated behind HAVE_DECL_RL_COMPLETION_MATCHES and the
// GNU readline library. The Rust 'hfst_readline' uses plain 'getline' with no
// readline backend, so — exactly as on a build without readline — those #if
// blocks are not compiled in. Their def/sem annotations are carried below for
// traceability; the bodies are intentionally left out to match the
// no-readline configuration the foundation provides.

// [spec:hfst:def:hfst-traverse.arclabel-generator-fn]
// [spec:hfst:sem:hfst-traverse.arclabel-generator-fn]
// (readline-only: not compiled — see note above)

// [spec:hfst:def:hfst-traverse.arclabel-completion-fn]
// [spec:hfst:sem:hfst-traverse.arclabel-completion-fn]
// (readline-only: not compiled — see note above)

// [spec:hfst:def:hfst-traverse.print-usage-fn]
// [spec:hfst:sem:hfst-traverse.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nWalk through the transducer arc by arc\n\n",
                program_name
            ),
        );

        // options, grouped
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-traverse.parse-options-fn]
// [spec:hfst:sem:hfst-traverse.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            long_options.push(getopt::Option {
                name: CString::new("cave").unwrap().into_raw(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'X' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            // add tool-specific options here
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own 'X', then the
            // terminal error arm.
            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == 'X' as c_int {
                CAVE_MODE = true;
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-traverse.main-loop-fn]
// [spec:hfst:sem:hfst-traverse.main-loop-fn]
unsafe fn main_loop(trans: &HfstBasicTransducer) -> c_int {
    unsafe {
        let mut msg = globals::message_writer();
        fput(&mut *msg, "Enter labels to seek all paths\n");
        // record current paths with their end states. The C++ uses a
        // multimap<string, HfstState>; a BTreeMap<(String, usize), HfstState>
        // (keyed on an insertion counter to permit duplicate path strings)
        // preserves both the ordered iteration and the multi-value semantics.
        let mut paths: BTreeMap<(String, usize), u32> = BTreeMap::new();
        let mut counter: usize = 0;
        paths.insert((String::new(), counter), 0);
        counter += 1;
        // (The readline completion / history setup is readline-only; omitted as
        // the foundation uses a plain getline-based readline — see note above.)
        loop {
            // print available paths
            for ((path_str, _), state) in paths.iter() {
                fput(
                    &mut *msg,
                    &format!("On path `{}' are continuations:\n", path_str),
                );
                let transitions = trans.index(*state);
                if transitions.is_empty() {
                    fput(&mut *msg, "<Nothing, you've hit a dead end here>\n");
                }
                for arc in transitions.iter() {
                    fput(
                        &mut *msg,
                        &format!("{}\t{}\n", arc.get_input_symbol(), arc.get_output_symbol()),
                    );
                }
            }
            let label_ptr = hfst_readline("traverse> ");
            if label_ptr.is_null() {
                return 0;
            }
            let label = cstr(label_ptr);
            let mut new_paths: BTreeMap<(String, usize), u32> = BTreeMap::new();
            for ((path_str, _), state) in paths.iter() {
                for arc in trans.index(*state).iter() {
                    if arc.get_input_symbol() == label {
                        let newpath = format!(
                            "{}{}:{} ",
                            path_str,
                            arc.get_input_symbol(),
                            arc.get_output_symbol()
                        );
                        new_paths.insert((newpath, counter), arc.get_target_state());
                        counter += 1;
                    }
                }
            }
            if new_paths.is_empty() {
                if label == "quit" || label.is_empty() {
                    fput(&mut *msg, "Use EOF (Ctrl-D or similar) to quit\n");
                } else if label == "XYZZY" {
                    fput(&mut *msg, "Nothing happens\n");
                }
                fput(&mut *msg, &format!("could not advance with {}\n", label));
            } else {
                paths = new_paths;
            }
            // (add_history is readline-only; omitted — see note above.)
            hfst_cli::hfst_commandline::hfst_free(label_ptr as *mut c_char);
        } // while paths not empty
    }
}

// [spec:hfst:def:hfst-traverse.process-stream-fn]
// [spec:hfst:sem:hfst-traverse.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> c_int {
    unsafe {
        let mut msg = globals::message_writer();
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let _ = transducer_n;
            let trans = HfstTransducer::new_from_stream(instream);
            let mut trans_name = trans.get_name();
            if trans_name.is_empty() {
                trans_name = cstr(globals::INPUTFILENAME);
            }
            // HfstBasicTransducer walkable(trans);
            let walkable = trans.get_basic_transducer();
            if CAVE_MODE {
                fput(
                    &mut *msg,
                    "WELCOME TO ADVENTURE!! WOULD YOU LIKE INSTRUCTIONS?\n\n",
                );
                let yesno_ptr = hfst_readline("");
                let yesno = cstr(yesno_ptr);
                if yesno == "YES" || yesno == "yes" {
                    fput(
                        &mut *msg,
                        "SOMEWHERE NEARBY IS COLOSSAL CAVE \
                         WHERE OTHERS HAVE FOUND\n\
                         FORTUNES IN TREASURES AND GOLD, \
                         THOUGH IT IS RUMORED\n\
                         THAT SOME WHO ENTER ARE NEVER SEEN AGAIN. \
                         MAGIC IS SAID\n\
                         TO WORK IN THE CAVE.  I WILL BE YOUR EYES AND HANDS. \
                         DIRECT\n\
                         ME WITH COMMANDS OF 1 ARC LABEL.\n\
                         (ERRORS, COMPLAINTS, SUGGESTIONS TO HFST-BUGS)\n\
                         (IF STUCK TYPE HELP FOR SOME HINTS)\n\n",
                    );
                }
                if !yesno_ptr.is_null() {
                    hfst_cli::hfst_commandline::hfst_free(yesno_ptr as *mut c_char);
                }
                fput(
                    &mut *msg,
                    "YOU ARE STANDING AT THE END OF A ROAD BEFORE A \
                     SMALL FINITE\n\
                     STATE AUTOMATON . AROUND YOU IS A FOREST. A SMALL\n\
                     STREAM OF ARCS FLOWS OUT OF THE AUTOMATON AND \
                     DOWN A GULLY:\n\n",
                );
            } else {
                fput(
                    &mut *msg,
                    &format!("Traversing automaton {}\n\n", trans_name),
                );
            }
            if walkable.state_vector.is_empty() {
                fput(&mut *msg, "Nowhere to go\n");
                return 0;
            }
            return main_loop(&walkable);
        }
        instream.close();
        0
    }
}

// [spec:hfst:def:hfst-traverse.main-fn]
// [spec:hfst:sem:hfst-traverse.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt and
        // extend_options_getenv reorder/replace it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = cstr(globals::INPUTFILENAME) != "<stdin>";
        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));

        // here starts the buffer handling part
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        // The C constructs an HfstOutputStream from the input type even though
        // this tool never writes to it (traversal only reads). Mirror that
        // construction so the buffer-handling part matches the source.
        let type_ = instream.get_type();
        let _outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        process_stream(&mut instream)
    }
}
