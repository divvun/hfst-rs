//! Faithful 1:1 port of tools/src/hfst-preprocess-for-optimized-lookup-format.cc
//! — the transducer preprocessing tool (the C++ source is the epsilon-removal /
//! rebuild tool). Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::Write;

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.print-usage-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nRemove epsilons from a transducer\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.parse-options-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let inputname = hfst_get_name(&trans, &globals::input_filename());
            if transducer_n == 1 {
                verbose_printf(&format!("Removing epsilons {}...\n", inputname));
            } else {
                verbose_printf(&format!(
                    "Removing epsilons {}...{}\n",
                    inputname, transducer_n
                ));
            }
            trans.remove_epsilons();
            if transducer_n == 1 {
                verbose_printf(&format!("Rebuilding and fixing {}...\n", inputname));
            } else {
                verbose_printf(&format!(
                    "Rebuilding and fisting {}...{}\n",
                    inputname, transducer_n
                ));
            }
            // C++: HfstBasicTransducer original(trans); — the
            // HfstBasicTransducer(const HfstTransducer&) conversion constructor.
            let original: HfstBasicTransducer = trans.get_basic_transducer();
            let replication = original.renumber_states();
            trans = HfstTransducer::new_from_basic(&replication, trans.get_type());
            // C: hfst_set_name(trans, trans, "fu"); the dest and src are the same
            // object, which Rust cannot alias mut+const, so the read side is taken
            // from a copy (name/formula are unchanged by the copy).
            let src = trans.clone();
            hfst_set_name_unary(&mut trans, &src, "fu");
            hfst_set_formula_unary(&mut trans, &src, "FU");
            trans.remove_epsilons();
            outstream.redirect(&mut trans);
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.main-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstPreprocessForOptimizedLookupFormat");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let type_ = instream.get_type();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        process_stream(&mut instream, &mut outstream)
    }
}
