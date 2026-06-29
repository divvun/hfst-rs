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

use hfst::hfst_data_types::{HfstOneLevelPath, HfstTwoLevelPaths, ImplementationType};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::is_epsilon;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;
use hfst_cli::globals;
use hfst_cli::globals::ColourTristate;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, error_at_line, extend_options_getenv, hfst_parse_format_name,
    hfst_set_program_name, hfst_setlocale, hfst_strdup, hfst_strndup, hfst_strtoul,
    print_more_info, print_report_bugs, print_short_help, verbose_printf, warning,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options,
};
use hfst_cli::inc::{CaseResult, check_common_params, handle_common_case, handle_error_case};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};
use std::io::{BufRead, Write};

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

// add tools-specific variables here
static mut INFILENAMES: *mut *mut c_char = std::ptr::null_mut();
// In the C the per-file inputs were a FILE** array (each from hfst_fopen, or
// stdin); after the io-foundation de-C-ism they are std::io::BufRead readers,
// parallel to INFILENAMES.
static mut INFILE_READERS: Vec<Box<dyn BufRead>> = Vec::new();

fn infile_readers() -> &'static mut Vec<Box<dyn BufRead>> {
    unsafe { &mut *std::ptr::addr_of_mut!(INFILE_READERS) }
}
static mut INFILE_N: libc::c_uint = 0;
static mut INPUTFILENAME: *mut c_char = std::ptr::null_mut();
static mut LINEN: libc::c_ulong = 0;
static mut REGEXP: *mut c_char = std::ptr::null_mut();
// C: 'FILE *expfile = 0;' — opened by -f but its content is never read (the tool
// keeps a TODO); only its NULL-ness (whether -f was given) is observed. Modelled
// as a bool so the same "was -f given" check survives the FILE* removal.
static mut EXPFILE_GIVEN: bool = false;
#[allow(dead_code)]
static mut EXPFILENAME: *mut c_char = std::ptr::null_mut();
static mut DIALECT_XEROX: bool = false;
static mut DIALECT_POSIX_BRE: bool = false;
static mut DIALECT_POSIX_ERE: bool = false;
static mut DIALECT_PERL: bool = false;
static mut DIALECT_FIXED_STRINGS: bool = false;
static mut MATCH_WORD: bool = false;
static mut MATCH_FULL_LINE: bool = false;
static mut LINESEP: c_char = b'\n' as c_char;
#[allow(dead_code)]
static mut VERY_QUIET: bool = false;
static mut INVERT_MATCHES: bool = false;
static mut MAX_COUNT: libc::c_ulong = u64::MAX as libc::c_ulong;
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
static mut BEFORE_CONTEXT: libc::c_ulong = 0;
static mut AFTER_CONTEXT: libc::c_ulong = 0;
#[allow(dead_code)]
static mut MATCHES: libc::c_ulong = 0;
static mut MATCHER: *mut HfstTransducer = std::ptr::null_mut();
#[allow(dead_code)]
static mut OPTIMISED_MATCHER: *mut HfstTransducer = std::ptr::null_mut();

static mut FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;

// [spec:hfst:def:hfst-grep.print-usage-fn]
// [spec:hfst:sem:hfst-grep.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] PATTERN [FILE...]\n\
                 Search for PATTERN in each FILE or standard input.\n\
                 Pattern is, by default, a Xerox regular expression (XRE).\n\
                 Example: hfst-grep 'h e l l o %%  w o r l d' menu.h menu.c\n\
                 \n",
                program_name
            ),
        );

        // options, grouped
        print_common_program_options(&mut *msg);
        fput(
            &mut *msg,
            "  -9, --format=TYPE       compile expressions to TYPE automata\n",
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
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
        fput(
            &mut *msg,
            "Miscellaneous options:\n\
             \x20     --no-messages         suppress error messages\n\
             \x20     --invert-match        select non-matching lines\n\
             \n",
        );
        fput(
            &mut *msg,
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
        fput(
            &mut *msg,
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
        fput(&mut *msg, "\n");
        // bug report address
        print_report_bugs();
        // external docs
        print_more_info();
    }
}

// [spec:hfst:def:hfst-grep.parse-options-fn]
// [spec:hfst:sem:hfst-grep.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        // use of this function requires options are settable on global scope
        const INVERT_OPT: c_int = 19;
        const LINEBUFFER_OPT: c_int = 20;
        const LABEL_OPT: c_int = 21;
        const BINARYFILES_OPT: c_int = 22;
        const INCLUDE_OPT: c_int = 23;
        const EXCLUDE_OPT: c_int = 24;
        const INCLUDEFROM_OPT: c_int = 25;
        const EXCLUDEFROM_OPT: c_int = 26;
        const COLOR_OPT: c_int = 27;
        extend_options_getenv(&mut argc, &mut argv);
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let names: &[(&str, c_int, c_int)] = &[
                ("format", 1, b'9' as c_int),
                ("extended-regexp", 0, b'E' as c_int),
                ("fixed-strings", 0, b'F' as c_int),
                ("basic-regexp", 0, b'G' as c_int),
                ("perl-regexp", 0, b'P' as c_int),
                ("xerox-regexp", 0, b'X' as c_int),
                ("regexp", 1, b'e' as c_int),
                ("file", 1, b'f' as c_int),
                ("ignore-case", 0, b'I' as c_int),
                ("word-regexp", 0, b'w' as c_int),
                ("line-regexp", 0, b'x' as c_int),
                ("null-data", 0, b'z' as c_int),
                ("no-messages", 0, b'q' as c_int),
                ("invert-match", 0, INVERT_OPT),
                ("max-count", 1, b'm' as c_int),
                ("byte-offset", 0, b'b' as c_int),
                ("line-number", 0, b'n' as c_int),
                ("line-buffered", 0, LINEBUFFER_OPT),
                ("with-filename", 0, b'H' as c_int),
                ("label", 1, LABEL_OPT),
                ("only-matching", 0, b'O' as c_int),
                ("binary-files", 1, BINARYFILES_OPT),
                ("text", 0, b'a' as c_int),
                ("directories", 1, b'd' as c_int),
                ("devices", 1, b'D' as c_int),
                ("recursive", 0, b'r' as c_int),
                ("include", 1, INCLUDE_OPT),
                ("exclude", 1, EXCLUDE_OPT),
                ("include-from", 1, INCLUDEFROM_OPT),
                ("exclude-from", 1, EXCLUDEFROM_OPT),
                ("files-without-match", 0, b'L' as c_int),
                ("files-with-match", 0, b'l' as c_int),
                ("count", 0, b'c' as c_int),
                ("null", 0, b'Z' as c_int),
                ("before-context", 1, b'A' as c_int),
                ("after-context", 1, b'B' as c_int),
                ("context", 1, b'C' as c_int),
                ("colour", 0, COLOR_OPT),
                ("color", 0, COLOR_OPT),
                ("binary", 0, b'u' as c_int),
                ("unix-byte-offset", 0, b'U' as c_int),
            ];
            // Keep the CStrings alive for the lifetime of getopt_long.
            let name_storage: Vec<CString> = names
                .iter()
                .map(|(n, _, _)| CString::new(*n).unwrap())
                .collect();
            for (i, (_, has_arg, val)) in names.iter().enumerate() {
                long_options.push(getopt::Option {
                    name: name_storage[i].as_ptr(),
                    has_arg: *has_arg,
                    flag: std::ptr::null_mut(),
                    val: *val,
                });
            }
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}EFGPXe:f:IwxzqmbnOad:D:rLlcZA:B:C:uU9:",
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

            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }

            if c == b'9' as c_int {
                FORMAT = hfst_parse_format_name(&cstr(getopt::OPTARG));
            } else if c == b'E' as c_int {
                error(libc::EXIT_FAILURE, 0, "POSIX ERE syntax not yet supported");
                DIALECT_POSIX_ERE = true;
            } else if c == b'F' as c_int {
                DIALECT_FIXED_STRINGS = true;
            } else if c == b'G' as c_int {
                error(libc::EXIT_FAILURE, 0, "POSIX BRE syntax not yet supported");
                DIALECT_POSIX_BRE = true;
            } else if c == b'P' as c_int {
                error(libc::EXIT_FAILURE, 0, "Perl syntax not yet supported");
                DIALECT_PERL = true;
            } else if c == b'X' as c_int {
                DIALECT_XEROX = true;
            } else if c == b'e' as c_int {
                REGEXP = hfst_strdup(getopt::OPTARG);
            } else if c == b'f' as c_int {
                // C: expfile = hfst_fopen(optarg, "r"); the handle is never read,
                // but hfst_fopen validates the file (erroring on failure) — mirror
                // that and record that -f was given.
                let fname = cstr(getopt::OPTARG);
                if fname != "-" && std::fs::File::open(&fname).is_err() {
                    error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!("Could not open '{}'. ", fname),
                    );
                }
                EXPFILE_GIVEN = true;
            } else if c == b'I' as c_int {
                error(libc::EXIT_FAILURE, 0, "Ignore case not supported");
            } else if c == b'w' as c_int {
                MATCH_WORD = true;
            } else if c == b'x' as c_int {
                MATCH_FULL_LINE = true;
            } else if c == b'z' as c_int {
                LINESEP = 0;
            } else if c == INVERT_OPT {
                INVERT_MATCHES = true;
            } else if c == b'm' as c_int {
                MAX_COUNT = hfst_strtoul(&cstr(getopt::OPTARG), 10) as libc::c_ulong;
                COUNT_MATCHES = true;
            } else if c == b'b' as c_int {
                PRINT_OFFSET = true;
            } else if c == b'n' as c_int {
                PRINT_LINENUMBERS = true;
            } else if c == LINEBUFFER_OPT {
                FLUSH_NEWLINES = true;
            } else if c == b'H' as c_int {
                PRINT_FILENAMES = true;
            } else if c == b'O' as c_int {
                PRINT_ONLY_MATCHES = true;
            } else if c == BINARYFILES_OPT {
                error(libc::EXIT_FAILURE, 0, "No binary handling implemented");
            } else if c == b'a' as c_int {
                warning(0, 0, "All files are always handled as text");
            } else if c == b'D' as c_int {
                error(libc::EXIT_FAILURE, 0, "No directory handling implemented");
            } else if c == b'r' as c_int {
                error(libc::EXIT_FAILURE, 0, "No directory handling implemented");
            } else if c == INCLUDE_OPT
                || c == EXCLUDE_OPT
                || c == INCLUDEFROM_OPT
                || c == EXCLUDEFROM_OPT
            {
                error(libc::EXIT_FAILURE, 0, "No directory/globbing implemented");
            } else if c == b'L' as c_int {
                PRINT_ONLY_UNMATCHING_FILENAMES = true;
            } else if c == b'l' as c_int {
                PRINT_ONLY_MATCHING_FILENAMES = true;
            } else if c == b'c' as c_int {
                COUNT_MATCHES = true;
                PRINT_ONLY_COUNT = true;
            } else if c == b'Z' as c_int {
                PRINT_FILENAME_NULL = true;
            } else if c == b'A' as c_int {
                BEFORE_CONTEXT = hfst_strtoul(&cstr(getopt::OPTARG), 10) as libc::c_ulong;
            } else if c == b'B' as c_int {
                AFTER_CONTEXT = hfst_strtoul(&cstr(getopt::OPTARG), 10) as libc::c_ulong;
            } else if c == b'C' as c_int {
                BEFORE_CONTEXT = hfst_strtoul(&cstr(getopt::OPTARG), 10) as libc::c_ulong;
                AFTER_CONTEXT = hfst_strtoul(&cstr(getopt::OPTARG), 10) as libc::c_ulong;
            } else if c == b'u' as c_int || c == b'U' as c_int {
                error(
                    libc::EXIT_FAILURE,
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
        if REGEXP.is_null() && !EXPFILE_GIVEN {
            if (argc - getopt::OPTIND) <= 0 {
                print_usage();
                print_short_help();
                return libc::EXIT_FAILURE;
            } else {
                REGEXP = libc::strdup(*argv.offset(getopt::OPTIND as isize));
                getopt::OPTIND += 1;
            }
        }
        if (argc - getopt::OPTIND) == 0 {
            INFILENAMES = libc::malloc(std::mem::size_of::<*mut c_char>()) as *mut *mut c_char;
            INFILE_N = 1;
            let stdin_name = CString::new("<stdin>").unwrap();
            *INFILENAMES.offset(0) = libc::strdup(stdin_name.as_ptr());
            infile_readers().push(Box::new(std::io::BufReader::new(std::io::stdin())));
        } else {
            let count = (argc - getopt::OPTIND) as usize;
            INFILENAMES =
                libc::malloc(std::mem::size_of::<*mut c_char>() * count) as *mut *mut c_char;
            INFILE_N = (argc - getopt::OPTIND) as libc::c_uint;
            for i in 0..(argc - getopt::OPTIND) {
                *INFILENAMES.offset(i as isize) =
                    libc::strdup(*argv.offset((getopt::OPTIND + i) as isize));
                // C: infiles[i] = hfst_fopen(infilenames[i], "r"); open the named
                // file as a buffered std reader, mapping "-" to stdin and erroring
                // on a failed open through the same path.
                let name = cstr(*INFILENAMES.offset(i as isize));
                if name == "-" {
                    infile_readers().push(Box::new(std::io::BufReader::new(std::io::stdin())));
                } else {
                    match std::fs::File::open(&name) {
                        Ok(f) => infile_readers().push(Box::new(std::io::BufReader::new(f))),
                        Err(_) => {
                            error(
                                libc::EXIT_FAILURE,
                                0,
                                &format!("Could not open '{}'. ", name),
                            );
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
unsafe fn string_to_utf8(p: *mut c_char) -> Vec<String> {
    unsafe {
        let mut path: Vec<String> = Vec::new();
        let mut p = p;
        while !p.is_null() && *p != b'\0' as c_char {
            let c = *p as u8;
            let mut u8len: u16 = 1;
            if c <= 127 {
                u8len = 1;
            } else if (c & (128 + 64 + 32 + 16)) == (128 + 64 + 32 + 16) {
                u8len = 4;
            } else if (c & (128 + 64 + 32)) == (128 + 64 + 32) {
                u8len = 3;
            } else if (c & (128 + 64)) == (128 + 64) {
                u8len = 2;
            } else {
                error_at_line(
                    libc::EXIT_FAILURE,
                    0,
                    &cstr(INPUTFILENAME),
                    LINEN as u32,
                    &format!("{} not valid UTF-8\n", cstr(p)),
                );
            }
            let nextu8 = hfst_strndup(p, u8len as usize);
            path.push(cstr(nextu8));
            p = p.offset(u8len as isize);
            libc::free(nextu8 as *mut libc::c_void);
        }
        path
    }
}

// [spec:hfst:def:hfst-grep.read-matcher-fn]
// [spec:hfst:sem:hfst-grep.read-matcher-fn]
#[allow(dead_code)]
unsafe fn read_matcher_stream(instream: &mut HfstInputStream) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        MATCHER = Box::into_raw(Box::new(HfstTransducer::new_type(instream.get_type())));
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let mut inputname =
                libc::strdup(CString::new(trans.get_name()).unwrap_or_default().as_ptr());
            if libc::strlen(inputname) == 0 {
                inputname = libc::strdup(INPUTFILENAME);
            }
            if transducer_n == 1 {
                verbose_printf(&format!("Reading matcher {}...\n", cstr(inputname)));
            } else {
                verbose_printf(&format!(
                    "Reading matcher {}...{}\n",
                    cstr(inputname),
                    transducer_n
                ));
            }
            if transducer_n > 1 {
                verbose_printf("and disjuncting...\n");
            }
            (*MATCHER).disjunct(trans.input_project(), true);
        }
        verbose_printf("minimising matchers...\n");
        (*MATCHER).minimize();
        instream.close();
        libc::EXIT_SUCCESS
    }
}

unsafe fn read_matcher(expression: &str) {
    unsafe {
        MATCHER = Box::into_raw(Box::new(HfstTransducer::new_type(FORMAT)));
        if DIALECT_XEROX {
            let mut comp = XreCompiler::new(FORMAT);
            verbose_printf(&format!(
                "parsing {} as Xerox style regular expression...\n",
                expression
            ));
            let mut trans = comp.compile(expression).unwrap();
            (*MATCHER).disjunct(trans.input_project(), true);
        } else if DIALECT_FIXED_STRINGS {
            verbose_printf(&format!(
                "parsing {} as fixed string of UTF-8 symbols...\n",
                expression
            ));
            let t = HfstTokenizer::new();
            let trans = HfstTransducer::new_tokenized_pair(expression, expression, &t, FORMAT);
            (*MATCHER).disjunct(&trans, true);
        } else {
            error(libc::EXIT_FAILURE, 0, "dialect unsupported");
        }
        verbose_printf("minimizing...\n");
        (*MATCHER).minimize();
        if globals::VERBOSE {
            verbose_printf("Resulting FSM:\n");
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
            verbose_printf("Adding color codes to match boundaries...\n");
            let color_start = HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "[31m", FORMAT);
            let color_end = HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "[00m", FORMAT);
            let mut coloured = color_start;
            coloured.concatenate(&*MATCHER, true);
            coloured.concatenate(&color_end, true);
            MATCHER = Box::into_raw(Box::new(coloured));
        } else {
            // bracket matches for now
            verbose_printf("Adding brackets to match boundaries...\n");
            let color_start = HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "{{{", FORMAT);
            let color_end = HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "}}}", FORMAT);
            let mut coloured = color_start;
            coloured.concatenate(&*MATCHER, true);
            coloured.concatenate(&color_end, true);
            MATCHER = Box::into_raw(Box::new(coloured));
        }
        if MATCH_WORD {
            verbose_printf("Delimiting matcher to word boundaries (currently space)...\n");
            let non_word_char_left = HfstTransducer::new_symbol(" ", FORMAT);
            let non_word_char_right = HfstTransducer::new_symbol(" ", FORMAT);
            let mut word_bounded = non_word_char_left;
            word_bounded.concatenate(&*MATCHER, true);
            word_bounded.concatenate(&non_word_char_right, true);
            MATCHER = Box::into_raw(Box::new(word_bounded));
        }
        if !MATCH_FULL_LINE {
            verbose_printf("Extending matcher for repetitions and rest...\n");
            let mut left_any = HfstTransducer::new_symbol("@_IDENTITY_SYMBOL_@", FORMAT);
            let mut right_any = HfstTransducer::new_symbol("@_IDENTITY_SYMBOL_@", FORMAT);
            left_any.repeat_star();
            right_any.repeat_star();
            let mut one_match = left_any;
            one_match.concatenate(&*MATCHER, true);
            one_match.concatenate(&right_any, true);
            MATCHER = Box::into_raw(Box::new(one_match));
            (*MATCHER).repeat_plus();
        }
        verbose_printf("Minimising extended matcher...\n");
        (*MATCHER).minimize();
        if globals::VERBOSE {
            verbose_printf("Resulting FSM:\n");
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
            fput(&mut *out, &cstr(INPUTFILENAME));
            if PRINT_FILENAME_NULL {
                let _ = out.write_all(&[0u8]);
            } else {
                fput(&mut *out, ": ");
            }
        }
        if PRINT_LINENUMBERS {
            fput(&mut *out, &format!("{}: ", LINEN));
        }
        for s in &path.second {
            fput(&mut *out, s);
        }
        fput(&mut *out, "\n");
    }
}

// [spec:hfst:def:hfst-grep.print-match-transducer-fn]
// [spec:hfst:sem:hfst-grep.print-match-transducer-fn]
unsafe fn print_match_transducer(path: &HfstTransducer, out: &mut dyn Write) {
    unsafe {
        let mut p: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        path.extract_paths(&mut p, 1, -1);
        if PRINT_ONLY_MATCHING_FILENAMES || PRINT_ONLY_UNMATCHING_FILENAMES {
            return;
        }
        if PRINT_FILENAMES {
            fput(&mut *out, &cstr(INPUTFILENAME));
            if PRINT_FILENAME_NULL {
                let _ = out.write_all(&[0u8]);
            } else {
                fput(&mut *out, ": ");
            }
        }
        if PRINT_LINENUMBERS {
            fput(&mut *out, &format!("{}: ", LINEN));
        }
        if let Some(first) = p.iter().next() {
            for s in &first.second {
                if !is_epsilon(&s.0) {
                    fput(&mut *out, &s.0);
                }
            }
        }
        fput(&mut *out, "\n");
    }
}

/// @return true if matches in @a infile
// [spec:hfst:def:hfst-grep.match-lines-fn]
// [spec:hfst:sem:hfst-grep.match-lines-fn]
unsafe fn match_lines(
    infile: &mut dyn BufRead,
    infilename: *mut c_char,
    out: &mut dyn Write,
) -> bool {
    unsafe {
        verbose_printf(&format!("matching against {}...\n", cstr(infilename)));
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
            verbose_printf(&format!("matching {}...\n", line));
            // #else branch (active: HFST_OPTIMISED_LOOKUP_CAN_IDENTITY undefined)
            if line.is_empty() {
                continue;
            }
            let line_str = line;
            let mut line_trans =
                HfstTransducer::new_tokenized_pair(&line_str, &line_str, &tokeniser, FORMAT);
            verbose_printf("composing...\n");
            let mut results_t = HfstTransducer::new_copy(&line_trans);
            results_t.compose(&*MATCHER, true);
            results_t.output_project();
            let empty = HfstTransducer::new_type(FORMAT);
            if results_t.compare_default(&empty) {
                verbose_printf("no matches\n");
                if INVERT_MATCHES {
                    print_match_transducer(&line_trans, &mut *out);
                }
            } else {
                verbose_printf("matches\n");
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
            if (MAX_COUNT > 0) && (matches_n as libc::c_ulong >= MAX_COUNT) {
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
        verbose_printf("Optimising...\n");
        OPTIMISED_MATCHER =
            HfstTransducer::convert_static(&*MATCHER, ImplementationType::HFST_OL_TYPE);
    }
}

// [spec:hfst:def:hfst-grep.main-fn]
// [spec:hfst:sem:hfst-grep.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_setlocale();
        hfst_set_program_name(&argv0, "0.1", "HfstGrep");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        verbose_printf(&format!("Writing to {}\n", cstr(globals::OUTFILENAME)));
        read_matcher(&cstr(REGEXP));
        extend_matcher_with_options();
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-grep: cannot open output: {e}");
                return libc::EXIT_FAILURE;
            }
        };
        // #if HFST_OPTIMISED_LOOKUP_CAN_IDENTITY_SYMBOL: optimise_matcher();
        for i in 0..INFILE_N {
            INPUTFILENAME = *INFILENAMES.offset(i as isize);
            LINEN = 0;
            let name = *INFILENAMES.offset(i as isize);
            let reader = &mut infile_readers()[i as usize];
            match_lines(reader.as_mut(), name, &mut *out);
        }
        let _ = out.flush();

        libc::free(globals::OUTFILENAME as *mut libc::c_void);
        retval
    }
}
