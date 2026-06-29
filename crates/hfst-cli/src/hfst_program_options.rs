//! Faithful 1:1 port of tools/src/hfst-program-options.{h,cc} — the shared
//! help-text printers used by every hfst tool's '--help' output, plus the
//! getopt macro payloads (HFST_GETOPT_*_SHORT / *_LONG) the bins splice into
//! their own 'getopt_long' option tables. The "erros" typo in the common
//! options text and the 'input2' -> '1' mapping in the binary long options are
//! preserved bug-for-bug.

use crate::hfst_commandline::GETOPT_COLOUR;
use crate::hfst_getopt::{NO_ARGUMENT, OPTIONAL_ARGUMENT, Option, REQUIRED_ARGUMENT};
use core::ffi::c_int;
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
// getopt macro payloads. In C these are '#define's spliced into each tool's
// 'option long_options[]' initialiser; here the SHORT strings are consts and
// the LONG lists are helper fns returning the same Option entries the bin then
// concatenates (with its tool-specific options and a terminating {0,0,0,0})
// into its own 'getopt_long' table.
// ----------------------------------------------------------------------------

pub const HFST_GETOPT_COMMON_SHORT: &str = ":hVvqsd";
pub const HFST_GETOPT_UNARY_SHORT: &str = "i:o:";
pub const HFST_GETOPT_BINARY_SHORT: &str = "1:2:o:C";

// HFST_GETOPT_COMMON_LONG
pub fn hfst_getopt_common_long() -> [Option; 8] {
    [
        Option {
            name: c"help".as_ptr(),
            has_arg: NO_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'h' as c_int,
        },
        Option {
            name: c"version".as_ptr(),
            has_arg: NO_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'V' as c_int,
        },
        Option {
            name: c"verbose".as_ptr(),
            has_arg: NO_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'v' as c_int,
        },
        Option {
            name: c"quiet".as_ptr(),
            has_arg: NO_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'q' as c_int,
        },
        Option {
            name: c"silent".as_ptr(),
            has_arg: NO_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b's' as c_int,
        },
        Option {
            name: c"debug".as_ptr(),
            has_arg: NO_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'd' as c_int,
        },
        Option {
            name: c"color".as_ptr(),
            has_arg: OPTIONAL_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: GETOPT_COLOUR,
        },
        Option {
            name: c"colour".as_ptr(),
            has_arg: OPTIONAL_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: GETOPT_COLOUR,
        },
    ]
}

// HFST_GETOPT_UNARY_LONG
pub fn hfst_getopt_unary_long() -> [Option; 2] {
    [
        Option {
            name: c"input".as_ptr(),
            has_arg: REQUIRED_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'i' as c_int,
        },
        Option {
            name: c"output".as_ptr(),
            has_arg: REQUIRED_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'o' as c_int,
        },
    ]
}

// HFST_GETOPT_BINARY_LONG
//
// 'input2' is mapped to the '1' option value, bug-for-bug with the C macro.
pub fn hfst_getopt_binary_long() -> [Option; 4] {
    [
        Option {
            name: c"input1".as_ptr(),
            has_arg: REQUIRED_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'1' as c_int,
        },
        Option {
            name: c"input2".as_ptr(),
            has_arg: REQUIRED_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'1' as c_int,
        },
        Option {
            name: c"output".as_ptr(),
            has_arg: REQUIRED_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'o' as c_int,
        },
        Option {
            name: c"do-not-convert".as_ptr(),
            has_arg: NO_ARGUMENT,
            flag: std::ptr::null_mut(),
            val: b'C' as c_int,
        },
    ]
}
