//! Faithful 1:1 port of tools/src/hfst-program-options.{h,cc} — the shared
//! help-text printers used by every hfst tool's '--help' output, plus the
//! getopt macro payloads (HFST_GETOPT_*_SHORT / *_LONG) the bins splice into
//! their own 'getopt_long' option tables. The "erros" typo in the common
//! options text and the 'input2' -> '1' mapping in the binary long options are
//! preserved bug-for-bug.

use crate::hfst_commandline::GETOPT_COLOUR;
use crate::hfst_getopt::{GetOpt, NO_ARGUMENT, OPTIONAL_ARGUMENT, REQUIRED_ARGUMENT};
use std::io::Write;

// All programs
// [spec:hfst:def:hfst-program-options.print-common-program-options-fn]
// [spec:hfst:sem:hfst-program-options.print-common-program-options-fn]
pub fn print_common_program_options(file: &mut dyn Write) {
    let _ = file.write_all(
        b"Common options:\n  -h, --help             Print help message\n  -V, --version          Print version info\n  -v, --verbose          Print verbosely while processing\n  -q, --quiet            Only print fatal erros and requested output\n  -s, --silent           Alias of --quiet\n      --colour[=WHEN]    Print in colour WHEN:\n      --color[=WHEN]     always, never, auto (default)\n",
    );
}

// One transducer to one transducer:
//   compatible
//   determinize
//   head
//   invert
//   minimize
//   project
//   push-weights
//   remove-epsilons
//   repeat
//   reverse
//   symbols
//   tail
//   unweighted2weighted
//   weighted2unweighted
// [spec:hfst:def:hfst-program-options.print-common-unary-program-options-fn]
// [spec:hfst:sem:hfst-program-options.print-common-unary-program-options-fn]
pub fn print_common_unary_program_options(file: &mut dyn Write) {
    let _ = file.write_all(
        b"Input/Output options:\n  -i, --input=INFILE     Read input transducer from INFILE\n  -o, --output=OUTFILE   Write output transducer to OUTFILE\n",
    );
}

// [spec:hfst:def:hfst-program-options.print-common-unary-program-parameter-instructions-fn]
// [spec:hfst:sem:hfst-program-options.print-common-unary-program-parameter-instructions-fn]
pub fn print_common_unary_program_parameter_instructions(file: &mut dyn Write) {
    let _ = file.write_all(
        b"If OUTFILE or INFILE is missing or -, standard streams will be used.\nFormat of result depends on format of INFILE\n",
    );
}

// One transducer to text:
//   fst2txt
//   fst2strings
// [spec:hfst:def:hfst-program-options.print-common-unary-string-program-options-fn]
// [spec:hfst:sem:hfst-program-options.print-common-unary-string-program-options-fn]
//
// Declared in hfst-program-options.h but never defined in the C sources; ported
// as an empty-body stub for the same absent semantics (nothing is printed).
pub fn print_common_unary_string_program_options(_file: &mut dyn Write) {}

// Two transducers to one transducer
//   compose
//   concatenate
//   conjunct
//   disjunct,
// [spec:hfst:def:hfst-program-options.print-common-binary-program-options-fn]
// [spec:hfst:sem:hfst-program-options.print-common-binary-program-options-fn]
pub fn print_common_binary_program_options(file: &mut dyn Write) {
    let _ = file.write_all(
        b"Input/Output options:\n  -1, --input1=INFILE1   Read first input transducer from INFILE1\n  -2, --input2=INFILE2   Read second input transducer from INFILE2\n  -C, --do-not-convert   Do not allow transducers to be converted into the same type\n  -o, --output=OUTFILE   Write results to OUTFILE\n",
    );
}

// [spec:hfst:def:hfst-program-options.print-common-binary-program-parameter-instructions-fn]
// [spec:hfst:sem:hfst-program-options.print-common-binary-program-parameter-instructions-fn]
pub fn print_common_binary_program_parameter_instructions(file: &mut dyn Write) {
    let _ = file.write_all(
        b"If OUTFILE, or either INFILE1 or INFILE2 is missing or -,\nstandard streams will be used.\nINFILE1, INFILE2, or both, must be specified.\nFormat of result depends on format of INFILE1 and INFILE2;\nboth should have the same format.\n",
    );
    let _ = file.write_all(
        b"\nThe operation is applied pairwise for INFILE1 and INFILE2\nthat must have the same number of transducers.\nIf INFILE2 has only one transducer, the operation is applied for\neach transducer in INFILE1 keeping the second transducer constant.\n",
    );
}

// ----------------------------------------------------------------------------
// getopt long-option tables. In C these were '#define'd 'option long_options[]'
// fragments each tool spliced in; here they are helper fns returning the GetOpt
// entries the bin concatenates with its tool-specific options into its own
// 'getopt_long' table. (The C SHORT optstrings are gone — the Rust getopt
// derives short options from the table's single-character names.)
// ----------------------------------------------------------------------------

// HFST_GETOPT_COMMON_LONG
pub fn hfst_getopt_common_long() -> [GetOpt; 8] {
    [
        GetOpt {
            name: "help",
            has_arg: NO_ARGUMENT,
            val: b'h' as i32,
        },
        GetOpt {
            name: "version",
            has_arg: NO_ARGUMENT,
            val: b'V' as i32,
        },
        GetOpt {
            name: "verbose",
            has_arg: NO_ARGUMENT,
            val: b'v' as i32,
        },
        GetOpt {
            name: "quiet",
            has_arg: NO_ARGUMENT,
            val: b'q' as i32,
        },
        GetOpt {
            name: "silent",
            has_arg: NO_ARGUMENT,
            val: b's' as i32,
        },
        GetOpt {
            name: "debug",
            has_arg: NO_ARGUMENT,
            val: b'd' as i32,
        },
        GetOpt {
            name: "color",
            has_arg: OPTIONAL_ARGUMENT,
            val: GETOPT_COLOUR,
        },
        GetOpt {
            name: "colour",
            has_arg: OPTIONAL_ARGUMENT,
            val: GETOPT_COLOUR,
        },
    ]
}

// HFST_GETOPT_UNARY_LONG
pub fn hfst_getopt_unary_long() -> [GetOpt; 2] {
    [
        GetOpt {
            name: "input",
            has_arg: REQUIRED_ARGUMENT,
            val: b'i' as i32,
        },
        GetOpt {
            name: "output",
            has_arg: REQUIRED_ARGUMENT,
            val: b'o' as i32,
        },
    ]
}

// HFST_GETOPT_BINARY_LONG
//
// 'input2' is mapped to the '1' option value, bug-for-bug with the C macro.
pub fn hfst_getopt_binary_long() -> [GetOpt; 5] {
    [
        GetOpt {
            name: "input1",
            has_arg: REQUIRED_ARGUMENT,
            val: b'1' as i32,
        },
        GetOpt {
            name: "input2",
            has_arg: REQUIRED_ARGUMENT,
            val: b'1' as i32,
        },
        // The C binary tools' getopt SHORT string carried "1:2:"; with the
        // long table's 'input2'->'1' mapping being a preserved upstream bug,
        // the short '-2' needs its own entry for the fallback matcher.
        GetOpt {
            name: "2",
            has_arg: REQUIRED_ARGUMENT,
            val: b'2' as i32,
        },
        GetOpt {
            name: "output",
            has_arg: REQUIRED_ARGUMENT,
            val: b'o' as i32,
        },
        GetOpt {
            name: "do-not-convert",
            has_arg: NO_ARGUMENT,
            val: b'C' as i32,
        },
    ]
}
