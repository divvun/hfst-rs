#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-grep.cc — the Hfst-based grep clone.
//! Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, inc fragments). Bug-for-bug translation of the C++ tool.
//!
//! As in the C++, the optimised-lookup match path is gated behind
//! HFST_OPTIMISED_LOOKUP_CAN_IDENTITY, which is not defined; the active path
//! uses compose/output_project/compare on tropical automata. The functions
//! behind that gate (string_to_utf8, optimise_matcher, the optimised half of
//! match_lines, print_match_line) are still ported faithfully but are never
//! reached at runtime.

use crate::globals;
use crate::globals::ColourTristate;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, error_at_line, extend_options_from_env, hfst_parse_format_name,
    hfst_set_program_name, parse_u64, print_short_help, verbose_print, warning,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
};
use crate::inc::{CaseResult, check_common_params, handle_common_case, handle_error_case};
use hfst::hfst_data_types::{HfstOneLevelPath, HfstTwoLevelPaths, ImplementationType};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::is_epsilon;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;
use std::io::{BufRead, Write};

// add tools-specific variables here
// In the C the per-file inputs were a FILE** array (each from hfst_fopen, or
// stdin); after the io-foundation de-C-ism the names are a Vec<String> and the
// readers are std::io::BufRead readers, parallel to INFILENAMES.
static mut INFILENAMES: Vec<String> = Vec::new();
static mut INFILE_READERS: Vec<Box<dyn BufRead>> = Vec::new();

fn infilenames() -> &'static mut Vec<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(INFILENAMES) }
}
fn infile_readers() -> &'static mut Vec<Box<dyn BufRead>> {
    unsafe { &mut *std::ptr::addr_of_mut!(INFILE_READERS) }
}
// The filename of the file currently being matched (the C kept a char*).
static mut INPUTFILENAME: String = String::new();
static mut LINEN: u64 = 0;
// C used a NULL char* as "no regexp given"; modelled as Option.
static mut REGEXP: Option<String> = None;

fn regexp() -> Option<String> {
    unsafe { (*std::ptr::addr_of!(REGEXP)).clone() }
}
// C: 'FILE *expfile = 0;' — opened by -f but its content is never read (the tool
// keeps a TODO); only its NULL-ness (whether -f was given) is observed. Modelled
// as a bool so the same "was -f given" check survives the FILE* removal.
static mut EXPFILE_GIVEN: bool = false;
#[allow(dead_code)]
static mut EXPFILENAME: Option<String> = None;
static mut DIALECT_XEROX: bool = false;
static mut DIALECT_POSIX_BRE: bool = false;
static mut DIALECT_POSIX_ERE: bool = false;
static mut DIALECT_PERL: bool = false;
static mut DIALECT_FIXED_STRINGS: bool = false;
static mut MATCH_WORD: bool = false;
static mut MATCH_FULL_LINE: bool = false;
static mut LINESEP: u8 = b'\n';
#[allow(dead_code)]
static mut VERY_QUIET: bool = false;
static mut INVERT_MATCHES: bool = false;
static mut MAX_COUNT: u64 = u64::MAX;
#[allow(dead_code)]
static mut MAX_INFINITE: bool = true;
static mut PRINT_OFFSET: bool = false;
static mut PRINT_LINENUMBERS: bool = false;
static mut FLUSH_NEWLINES: bool = false;
static mut PRINT_FILENAMES: bool = false;
static mut PRINT_ONLY_MATCHES: bool = false;
static mut PRINT_ONLY_MATCHING_FILENAMES: bool = false;
static mut PRINT_ONLY_UNMATCHING_FILENAMES: bool = false;
static mut PRINT_ONLY_COUNT: bool = false;
static mut COUNT_MATCHES: bool = false;
static mut PRINT_FILENAME_NULL: bool = false;
static mut BEFORE_CONTEXT: u64 = 0;
static mut AFTER_CONTEXT: u64 = 0;
#[allow(dead_code)]
static mut MATCHES: u64 = 0;
// The matcher pipeline is pinned to the tropical backend
// ([dec:hfst:monomorphic-backends]): grep's output is the matched lines, so
// the -f format (kept for option compatibility) never changes what is
// printed.
static mut MATCHER: *mut HfstTransducer<hfst_openfst::StdVectorFst> = std::ptr::null_mut();
#[allow(dead_code)]
static mut OPTIMISED_MATCHER: *mut HfstTransducer<hfst::transducer::Transducer> =
    std::ptr::null_mut();

static mut FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;

// [spec:hfst:def:hfst-grep.print-usage-fn]
// [spec:hfst:sem:hfst-grep.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] PATTERN [FILE...]\n\
                 Search for PATTERN in each FILE or standard input.\n\
                 Pattern is, by default, a Xerox regular expression (XRE).\n\
                 Example: hfst-grep 'h e l l o %%  w o r l d' menu.h menu.c\n\
                 \n",
        globals::program_name()
    );

    // options, grouped
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "  -9, --format=TYPE       compile expressions to TYPE automata\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "Regexp selection and interpretation:\n\
             \x20 -E, --extended-regexp     PATTERN is an extended regular expression (ERE)\n\
             \x20 -F, --fixed-strings       PATTERN is a set of newline-separated fixed strings\n\
             \x20 -G, --basic-regexp        PATTERN is a basic regular expression (BRE)\n\
             \x20 -P, --perl-regexp         PATTERN is a Perl regular expression\n\
             \x20 -X, --xerox-regexp        PATTERN is a Xerox regulare expression\n\
             \x20 -e, --regexp=PATTERN      use PATTERN for matching\n\
             \x20 -f, --file=FILE           obtain PATTERN from FILE\n\
             \x20 -I, --ignore-case         ignore case distinctions\n\
             \x20 -w, --word-regexp         force PATTERN to match only whole words\n\
             \x20 -x, --line-regexp         force PATTERN to match only whole lines\n\
             \x20 -z, --null-data           a data line ends in 0 byte, not newline\n",
    );
    let _ = write!(
        msg,
        "Miscellaneous options:\n\
             \x20     --no-messages         suppress error messages\n\
             \x20     --invert-match        select non-matching lines\n\
             \n",
    );
    let _ = write!(
        msg,
        "Output control:\n\
             \x20 -m, --max-count=NUM       stop after NUM matches\\n\
             \x20 -b, --byte-offset         print the byte offset with output lines\n\
             \x20 -n, --line-number         print line number with output lines\n\
             \x20     --line-buffered       flush output on every line\n\
             \x20 -H, --with-filename       print the filename for each match\n\
             \x20 -h, --no-filename         suppress the prefixing filename on output\n\
             \x20     --label=LABEL         print LABEL as filename for standard input\n\
             \x20 -o, --only-matching       show only the part of a line matching PATTERN\n\
             \x20     --binary-files=TYPE   assume that binary files are TYPE;\n\
             \x20                           TYPE is `binary', `text', or `without-match'\n\
             \x20 -a, --text                equivalent to --binary-files=text\n\
             \x20 -d, --directories=ACTION  how to handle directories;\n\
             \x20                           ACTION is `read', `recurse', or `skip'\n\
             \x20 -D, --devices=ACTION      how to handle devices, FIFOs and sockets;\n\
             \x20                           ACTION is `read' or `skip'\n\
             \x20 -R, -r, --recursive       equivalent to --directories=recurse\n\
             \x20     --include=FILE_PATTERN  search only files that match FILE_PATTERN\n\
             \x20     --exclude=FILE_PATTERN  skip files and directories matching FILE_PATTERN\n\
             \x20     --exclude-from=FILE   skip files matching any file pattern from FILE\n\
             \x20     --exclude-dir=PATTERN  directories that match PATTERN will be skipped\n\
             \x20 -L, --files-without-match  print only names of FILEs containing  no match\n\
             \x20 -l, --files-with-matches  print only names of FILEs containing matches\n\
             \x20 -c, --count               print only a count of matching lines per FILE\n\
             \x20 -T, --initial-tab         make tabs line up (if needed)\n\
             \x20 -Z, --null                print 0 byte after FILE name\n\
             \n",
    );
    let _ = write!(
        msg,
        "Context control:\n\
             \x20 -B, --before-context=NUM  print NUM lines of leading context\n\
             \x20 -A, --after-context=NUM   print NUM lines of trailing context\n\
             \x20 -C, --context=NUM         print NUM lines of output context\n\
             \x20     --color[=WHEN],\n\
             \x20     --colour[=WHEN]       use markers to highlight the matching strings;\n\
             \x20                           WHEN is `always', `never', or `auto'\n\
             \x20 -U, --binary              do not strip CR characters at EOL (MSDOS)\n\
             \x20 -u, --unix-byte-offsets   report offsets as if CRs were not there (MSDOS)\n\
             \n",
    );

    // parameter details
    let _ = write!(msg, "\n");
    // bug report address
    // external docs
}

// [spec:hfst:def:hfst-grep.parse-options-fn]
// [spec:hfst:sem:hfst-grep.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        // use of this function requires options are settable on global scope
        const INVERT_OPT: i32 = 19;
        const LINEBUFFER_OPT: i32 = 20;
        const LABEL_OPT: i32 = 21;
        const BINARYFILES_OPT: i32 = 22;
        const INCLUDE_OPT: i32 = 23;
        const EXCLUDE_OPT: i32 = 24;
        const INCLUDEFROM_OPT: i32 = 25;
        const EXCLUDEFROM_OPT: i32 = 26;
        const COLOR_OPT: i32 = 27;
        extend_options_from_env(args);
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let names: &[(&'static str, i32, i32)] = &[
                ("format", 1, b'9' as i32),
                ("extended-regexp", 0, b'E' as i32),
                ("fixed-strings", 0, b'F' as i32),
                ("basic-regexp", 0, b'G' as i32),
                ("perl-regexp", 0, b'P' as i32),
                ("xerox-regexp", 0, b'X' as i32),
                ("regexp", 1, b'e' as i32),
                ("file", 1, b'f' as i32),
                ("ignore-case", 0, b'I' as i32),
                ("word-regexp", 0, b'w' as i32),
                ("line-regexp", 0, b'x' as i32),
                ("null-data", 0, b'z' as i32),
                ("no-messages", 0, b'q' as i32),
                ("invert-match", 0, INVERT_OPT),
                ("max-count", 1, b'm' as i32),
                ("byte-offset", 0, b'b' as i32),
                ("line-number", 0, b'n' as i32),
                ("line-buffered", 0, LINEBUFFER_OPT),
                ("with-filename", 0, b'H' as i32),
                ("label", 1, LABEL_OPT),
                ("only-matching", 0, b'O' as i32),
                ("binary-files", 1, BINARYFILES_OPT),
                ("text", 0, b'a' as i32),
                ("directories", 1, b'd' as i32),
                ("devices", 1, b'D' as i32),
                ("recursive", 0, b'r' as i32),
                ("include", 1, INCLUDE_OPT),
                ("exclude", 1, EXCLUDE_OPT),
                ("include-from", 1, INCLUDEFROM_OPT),
                ("exclude-from", 1, EXCLUDEFROM_OPT),
                ("files-without-match", 0, b'L' as i32),
                ("files-with-match", 0, b'l' as i32),
                ("count", 0, b'c' as i32),
                ("null", 0, b'Z' as i32),
                ("before-context", 1, b'A' as i32),
                ("after-context", 1, b'B' as i32),
                ("context", 1, b'C' as i32),
                ("colour", 0, COLOR_OPT),
                ("color", 0, COLOR_OPT),
                ("binary", 0, b'u' as i32),
                ("unix-byte-offset", 0, b'U' as i32),
            ];
            for (name, has_arg, val) in names.iter() {
                long_options.push(getopt::GetOpt {
                    name,
                    has_arg: *has_arg,
                    val: *val,
                });
            }
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }

            if c == b'9' as i32 {
                FORMAT = hfst_parse_format_name(&getopt::optarg());
            } else if c == b'E' as i32 {
                error(1, 0, "POSIX ERE syntax not yet supported");
                DIALECT_POSIX_ERE = true;
            } else if c == b'F' as i32 {
                DIALECT_FIXED_STRINGS = true;
            } else if c == b'G' as i32 {
                error(1, 0, "POSIX BRE syntax not yet supported");
                DIALECT_POSIX_BRE = true;
            } else if c == b'P' as i32 {
                error(1, 0, "Perl syntax not yet supported");
                DIALECT_PERL = true;
            } else if c == b'X' as i32 {
                DIALECT_XEROX = true;
            } else if c == b'e' as i32 {
                REGEXP = Some(getopt::optarg());
            } else if c == b'f' as i32 {
                // C: expfile = hfst_fopen(optarg, "r"); the handle is never read,
                // but hfst_fopen validates the file (erroring on failure) — mirror
                // that and record that -f was given.
                let fname = getopt::optarg();
                if fname != "-" && std::fs::File::open(&fname).is_err() {
                    error(1, 0, &format!("Could not open '{}'. ", fname));
                }
                EXPFILE_GIVEN = true;
            } else if c == b'I' as i32 {
                error(1, 0, "Ignore case not supported");
            } else if c == b'w' as i32 {
                MATCH_WORD = true;
            } else if c == b'x' as i32 {
                MATCH_FULL_LINE = true;
            } else if c == b'z' as i32 {
                LINESEP = 0;
            } else if c == INVERT_OPT {
                INVERT_MATCHES = true;
            } else if c == b'm' as i32 {
                MAX_COUNT = parse_u64(&getopt::optarg(), 10);
                COUNT_MATCHES = true;
            } else if c == b'b' as i32 {
                PRINT_OFFSET = true;
            } else if c == b'n' as i32 {
                PRINT_LINENUMBERS = true;
            } else if c == LINEBUFFER_OPT {
                FLUSH_NEWLINES = true;
            } else if c == b'H' as i32 {
                PRINT_FILENAMES = true;
            } else if c == b'O' as i32 {
                PRINT_ONLY_MATCHES = true;
            } else if c == BINARYFILES_OPT {
                error(1, 0, "No binary handling implemented");
            } else if c == b'a' as i32 {
                warning(0, 0, "All files are always handled as text");
            } else if c == b'D' as i32 {
                error(1, 0, "No directory handling implemented");
            } else if c == b'r' as i32 {
                error(1, 0, "No directory handling implemented");
            } else if c == INCLUDE_OPT
                || c == EXCLUDE_OPT
                || c == INCLUDEFROM_OPT
                || c == EXCLUDEFROM_OPT
            {
                error(1, 0, "No directory/globbing implemented");
            } else if c == b'L' as i32 {
                PRINT_ONLY_UNMATCHING_FILENAMES = true;
            } else if c == b'l' as i32 {
                PRINT_ONLY_MATCHING_FILENAMES = true;
            } else if c == b'c' as i32 {
                COUNT_MATCHES = true;
                PRINT_ONLY_COUNT = true;
            } else if c == b'Z' as i32 {
                PRINT_FILENAME_NULL = true;
            } else if c == b'A' as i32 {
                BEFORE_CONTEXT = parse_u64(&getopt::optarg(), 10);
            } else if c == b'B' as i32 {
                AFTER_CONTEXT = parse_u64(&getopt::optarg(), 10);
            } else if c == b'C' as i32 {
                BEFORE_CONTEXT = parse_u64(&getopt::optarg(), 10);
                AFTER_CONTEXT = parse_u64(&getopt::optarg(), 10);
            } else if c == b'u' as i32 || c == b'U' as i32 {
                error(
                    1,
                    0,
                    "MSDOS binary format not supported; use fromdos or dos2unix",
                );
            } else {
                return handle_error_case(c);
            }
        }
        if !DIALECT_FIXED_STRINGS
            && !DIALECT_XEROX
            && !DIALECT_POSIX_BRE
            && !DIALECT_POSIX_ERE
            && !DIALECT_PERL
        {
            warning(0, 0, "Dialect not defined, defaulting to Xerox for now!");
            DIALECT_XEROX = true;
        }
        if FORMAT == ImplementationType::UNSPECIFIED_TYPE {
            FORMAT = ImplementationType::TROPICAL_OPENFST_TYPE;
        }
        if regexp().is_none() && !EXPFILE_GIVEN {
            if args.len() <= getopt::OPTIND {
                print_usage();
                print_short_help();
                return 1;
            } else {
                REGEXP = Some(args[getopt::OPTIND].clone());
                getopt::OPTIND += 1;
            }
        }
        if args.len() == getopt::OPTIND {
            infilenames().push("<stdin>".to_string());
            infile_readers().push(Box::new(std::io::BufReader::new(std::io::stdin())));
        } else {
            for i in getopt::OPTIND..args.len() {
                let name = args[i].clone();
                infilenames().push(name.clone());
                // C: infiles[i] = hfst_fopen(infilenames[i], "r"); open the named
                // file as a buffered std reader, mapping "-" to stdin and erroring
                // on a failed open through the same path.
                if name == "-" {
                    infile_readers().push(Box::new(std::io::BufReader::new(std::io::stdin())));
                } else {
                    match std::fs::File::open(&name) {
                        Ok(f) => infile_readers().push(Box::new(std::io::BufReader::new(f))),
                        Err(_) => {
                            error(1, 0, &format!("Could not open '{}'. ", name));
                        }
                    }
                }
            }
        }
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-grep.string-to-utf8-fn]
// [spec:hfst:sem:hfst-grep.string-to-utf8-fn]
#[allow(dead_code)]
unsafe fn string_to_utf8(p: &str) -> Vec<String> {
    unsafe {
        let mut path: Vec<String> = Vec::new();
        let bytes = p.as_bytes();
        let mut idx = 0usize;
        while idx < bytes.len() {
            let c = bytes[idx];
            let u8len: usize = if c <= 127 {
                1
            } else if (c & (128 + 64 + 32 + 16)) == (128 + 64 + 32 + 16) {
                4
            } else if (c & (128 + 64 + 32)) == (128 + 64 + 32) {
                3
            } else if (c & (128 + 64)) == (128 + 64) {
                2
            } else {
                error_at_line(
                    1,
                    0,
                    &INPUTFILENAME,
                    LINEN as u32,
                    &format!("{} not valid UTF-8\n", &p[idx..]),
                );
                1
            };
            let end = (idx + u8len).min(bytes.len());
            path.push(String::from_utf8_lossy(&bytes[idx..end]).into_owned());
            idx += u8len;
        }
        path
    }
}

// [spec:hfst:def:hfst-grep.read-matcher-fn]
// [spec:hfst:sem:hfst-grep.read-matcher-fn]
#[allow(dead_code)]
unsafe fn read_matcher_stream(instream: &mut HfstInputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        MATCHER = Box::into_raw(Box::new(HfstTransducer::new()));
        while instream.is_good() {
            transducer_n += 1;
            // one dispatch per read: everything joins the tropical matcher
            let mut trans: HfstTransducer<hfst_openfst::StdVectorFst> =
                match instream.read().and_then(|any| any.into_typed()) {
                    Ok(t) => t,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
            let mut inputname = trans.get_name();
            if inputname.is_empty() {
                inputname = INPUTFILENAME.clone();
            }
            if transducer_n == 1 {
                verbose_print(&format!("Reading matcher {}...\n", inputname));
            } else {
                verbose_print(&format!(
                    "Reading matcher {}...{}\n",
                    inputname, transducer_n
                ));
            }
            if transducer_n > 1 {
                verbose_print("and disjuncting...\n");
            }
            if let Err(e) = trans.input_project() {
                error(1, 0, &format!("{e}"));
                return 1;
            }
            if let Err(e) = (*MATCHER).disjunct(&trans, true) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        verbose_print("minimising matchers...\n");
        if let Err(e) = (*MATCHER).minimize() {
            error(1, 0, &format!("{e}"));
            return 1;
        }
        instream.close();
        0
    }
}

unsafe fn read_matcher(expression: &str) {
    unsafe {
        // (FORMAT is parsed for option compatibility; the matcher runs on
        // the tropical backend regardless — matching is weight-independent.)
        let _ = FORMAT;
        MATCHER = Box::into_raw(Box::new(HfstTransducer::new()));
        if DIALECT_XEROX {
            let mut comp = XreCompiler::<hfst_openfst::StdVectorFst>::new();
            verbose_print(&format!(
                "parsing {} as Xerox style regular expression...\n",
                expression
            ));
            let mut trans = comp.compile(expression).unwrap();
            if let Err(e) = trans.input_project() {
                error(1, 0, &format!("{e}"));
                return;
            }
            if let Err(e) = (*MATCHER).disjunct(&trans, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
        } else if DIALECT_FIXED_STRINGS {
            verbose_print(&format!(
                "parsing {} as fixed string of UTF-8 symbols...\n",
                expression
            ));
            let t = HfstTokenizer::new();
            let trans = match HfstTransducer::new_tokenized_pair(expression, expression, &t) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            if let Err(e) = (*MATCHER).disjunct(&trans, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
        } else {
            error(1, 0, "dialect unsupported");
        }
        verbose_print("minimizing...\n");
        if let Err(e) = (*MATCHER).minimize() {
            error(1, 0, &format!("{e}"));
            return;
        }
        if globals::VERBOSE {
            verbose_print("Resulting FSM:\n");
            // C: std::cerr << *matcher;
            hfst::hfst_transducer::operator_shl_os(&mut std::io::stderr(), &*MATCHER);
        }
    }
}

// [spec:hfst:def:hfst-grep.extend-matcher-with-options-fn]
// [spec:hfst:sem:hfst-grep.extend-matcher-with-options-fn]
unsafe fn extend_matcher_with_options() {
    unsafe {
        if globals::COLOUR == ColourTristate::COLOUR_ALWAYS {
            verbose_print("Adding color codes to match boundaries...\n");
            let color_start = match HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "[31m") {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            let color_end = match HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "[00m") {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            let mut coloured = color_start;
            if let Err(e) = coloured.concatenate(&*MATCHER, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
            if let Err(e) = coloured.concatenate(&color_end, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
            MATCHER = Box::into_raw(Box::new(coloured));
        } else {
            // bracket matches for now
            verbose_print("Adding brackets to match boundaries...\n");
            let color_start = match HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "{{{") {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            let color_end = match HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "}}}") {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            let mut coloured = color_start;
            if let Err(e) = coloured.concatenate(&*MATCHER, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
            if let Err(e) = coloured.concatenate(&color_end, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
            MATCHER = Box::into_raw(Box::new(coloured));
        }
        if MATCH_WORD {
            verbose_print("Delimiting matcher to word boundaries (currently space)...\n");
            let non_word_char_left = match HfstTransducer::new_symbol(" ") {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            let non_word_char_right = match HfstTransducer::new_symbol(" ") {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            let mut word_bounded = non_word_char_left;
            if let Err(e) = word_bounded.concatenate(&*MATCHER, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
            if let Err(e) = word_bounded.concatenate(&non_word_char_right, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
            MATCHER = Box::into_raw(Box::new(word_bounded));
        }
        if !MATCH_FULL_LINE {
            verbose_print("Extending matcher for repetitions and rest...\n");
            let mut left_any = match HfstTransducer::new_symbol("@_IDENTITY_SYMBOL_@") {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            let mut right_any = match HfstTransducer::new_symbol("@_IDENTITY_SYMBOL_@") {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return;
                }
            };
            if let Err(e) = left_any.repeat_star() {
                error(1, 0, &format!("{e}"));
                return;
            }
            if let Err(e) = right_any.repeat_star() {
                error(1, 0, &format!("{e}"));
                return;
            }
            let mut one_match = left_any;
            if let Err(e) = one_match.concatenate(&*MATCHER, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
            if let Err(e) = one_match.concatenate(&right_any, true) {
                error(1, 0, &format!("{e}"));
                return;
            }
            MATCHER = Box::into_raw(Box::new(one_match));
            if let Err(e) = (*MATCHER).repeat_plus() {
                error(1, 0, &format!("{e}"));
                return;
            }
        }
        verbose_print("Minimising extended matcher...\n");
        if let Err(e) = (*MATCHER).minimize() {
            error(1, 0, &format!("{e}"));
            return;
        }
        if globals::VERBOSE {
            verbose_print("Resulting FSM:\n");
            hfst::hfst_transducer::operator_shl_os(&mut std::io::stderr(), &*MATCHER);
        }
    }
}

// [spec:hfst:def:hfst-grep.print-match-line-fn]
// [spec:hfst:sem:hfst-grep.print-match-line-fn]
#[allow(dead_code)]
unsafe fn print_match_line(path: &HfstOneLevelPath, out: &mut dyn Write) {
    unsafe {
        if PRINT_ONLY_MATCHING_FILENAMES || PRINT_ONLY_UNMATCHING_FILENAMES {
            return;
        }
        if PRINT_FILENAMES {
            let _ = out.write_all(INPUTFILENAME.as_bytes());
            if PRINT_FILENAME_NULL {
                let _ = out.write_all(&[0u8]);
            } else {
                let _ = out.write_all(b": ");
            }
        }
        if PRINT_LINENUMBERS {
            let _ = write!(out, "{}: ", LINEN);
        }
        for s in &path.second {
            let _ = out.write_all(s.as_bytes());
        }
        let _ = out.write_all(b"\n");
    }
}

// [spec:hfst:def:hfst-grep.print-match-transducer-fn]
// [spec:hfst:sem:hfst-grep.print-match-transducer-fn]
unsafe fn print_match_transducer(
    path: &HfstTransducer<hfst_openfst::StdVectorFst>,
    out: &mut dyn Write,
) {
    unsafe {
        let mut p: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        if let Err(e) = path.extract_paths(&mut p, 1, -1) {
            error(1, 0, &format!("{e}"));
            return;
        }
        if PRINT_ONLY_MATCHING_FILENAMES || PRINT_ONLY_UNMATCHING_FILENAMES {
            return;
        }
        if PRINT_FILENAMES {
            let _ = out.write_all(INPUTFILENAME.as_bytes());
            if PRINT_FILENAME_NULL {
                let _ = out.write_all(&[0u8]);
            } else {
                let _ = out.write_all(b": ");
            }
        }
        if PRINT_LINENUMBERS {
            let _ = write!(out, "{}: ", LINEN);
        }
        if let Some(first) = p.iter().next() {
            for s in &first.second {
                if !is_epsilon(&s.0) {
                    let _ = out.write_all(s.0.as_bytes());
                }
            }
        }
        let _ = out.write_all(b"\n");
    }
}

/// @return true if matches in @a infile
// [spec:hfst:def:hfst-grep.match-lines-fn]
// [spec:hfst:sem:hfst-grep.match-lines-fn]
unsafe fn match_lines(infile: &mut dyn BufRead, infilename: &str, out: &mut dyn Write) -> bool {
    unsafe {
        verbose_print(&format!("matching against {}...\n", infilename));
        let mut matched = false;
        let mut matches_n: usize = 0;
        // #ifndef HFST_OPTIMISED_LOOKUP_CAN_IDENTITY
        let tokeniser = HfstTokenizer::new();
        loop {
            // C: hfst_getline reads a raw line (bytes); cstr does a lossy UTF-8
            // conversion. read_until(b'\n') mirrors getline's byte semantics.
            let mut raw_bytes: Vec<u8> = Vec::new();
            match infile.read_until(b'\n', &mut raw_bytes) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            LINEN += 1;
            let mut line = String::from_utf8_lossy(&raw_bytes).into_owned();
            // C: scan to the first '\n' and replace it with '\0' (truncate there).
            if let Some(pos) = line.find('\n') {
                line.truncate(pos);
            }
            verbose_print(&format!("matching {}...\n", line));
            // #else branch (active: HFST_OPTIMISED_LOOKUP_CAN_IDENTITY undefined)
            if line.is_empty() {
                continue;
            }
            let line_str = line;
            let mut line_trans: HfstTransducer<hfst_openfst::StdVectorFst> =
                match HfstTransducer::new_tokenized_pair(&line_str, &line_str, &tokeniser) {
                    Ok(t) => t,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return false;
                    }
                };
            verbose_print("composing...\n");
            let mut results_t = match HfstTransducer::new_copy(&line_trans) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return false;
                }
            };
            if let Err(e) = results_t.compose(&*MATCHER, true) {
                error(1, 0, &format!("{e}"));
                return false;
            }
            if let Err(e) = results_t.output_project() {
                error(1, 0, &format!("{e}"));
                return false;
            }
            let empty: HfstTransducer<hfst_openfst::StdVectorFst> = HfstTransducer::new();
            let is_empty = match results_t.compare_default(&empty) {
                Ok(b) => b,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return false;
                }
            };
            if is_empty {
                verbose_print("no matches\n");
                if INVERT_MATCHES {
                    print_match_transducer(&line_trans, &mut *out);
                }
            } else {
                verbose_print("matches\n");
                if !INVERT_MATCHES {
                    print_match_transducer(&results_t, &mut *out);
                }
                matched = true;
                matches_n += 1;
            }
            let _ = &mut line_trans;
            if FLUSH_NEWLINES {
                let _ = out.flush();
            }
            if (MAX_COUNT > 0) && (matches_n as u64 >= MAX_COUNT) {
                break;
            }
        }
        if INVERT_MATCHES { !matched } else { matched }
    }
}

// [spec:hfst:def:hfst-grep.optimise-matcher-fn]
// [spec:hfst:sem:hfst-grep.optimise-matcher-fn]
#[allow(dead_code)]
unsafe fn optimise_matcher() {
    unsafe {
        verbose_print("Optimising...\n");
        // C: HfstTransducer(*matcher).convert(HFST_OL_TYPE) — the typed
        // algebra->OL conversion now.
        OPTIMISED_MATCHER = Box::into_raw(Box::new(match (*MATCHER).to_ol(false, "") {
            Ok(t) => t,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return;
            }
        }));
    }
}

// [spec:hfst:def:hfst-grep.main-fn]
// [spec:hfst:sem:hfst-grep.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstGrep");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        verbose_print(&format!("Writing to {}\n", globals::output_filename()));
        read_matcher(&regexp().unwrap_or_default());
        extend_matcher_with_options();
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-grep: cannot open output: {e}");
                return 1;
            }
        };
        // #if HFST_OPTIMISED_LOOKUP_CAN_IDENTITY_SYMBOL: optimise_matcher();
        for i in 0..infilenames().len() {
            INPUTFILENAME = infilenames()[i].clone();
            LINEN = 0;
            let name = infilenames()[i].clone();
            let reader = &mut infile_readers()[i];
            match_lines(reader.as_mut(), &name, &mut *out);
        }
        let _ = out.flush();

        retval
    }
}
