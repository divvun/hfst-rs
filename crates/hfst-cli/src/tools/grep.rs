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
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::ColourTristate;
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, error_at_line, extend_options_from_env, hfst_parse_format_name, hfst_set_program_name,
    parse_u64, print_short_help, verbose_print, warning,
};
use crate::hfst_getopt::{self as getopt, Getopt};
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

/// hfst-grep's own options (the former tool-specific `static mut`s).
///
/// In the C the per-file inputs were a FILE** array (each from hfst_fopen, or
/// stdin); after the io-foundation de-C-ism the names are a `Vec<String>` and
/// the readers are `std::io::BufRead` readers, parallel to INFILENAMES.
///
/// Upstream's `expfilename`, `very_quiet`, `max_infinite` and `matches` globals
/// are declared and then never read on any path, so they are deliberately not
/// mirrored: a field no option handler writes and no code reads carries no
/// behaviour, and silencing its dead-code warning is how a genuinely inert flag
/// hides (see the --space-separated defect).
struct Options {
    infilenames: Vec<String>,
    infile_readers: Vec<Box<dyn BufRead>>,
    // C used a NULL char* as "no regexp given"; modelled as Option.
    regexp: Option<String>,
    // C: 'FILE *expfile = 0;' — opened by -f but its content is never read (the
    // tool keeps a TODO); only its NULL-ness (whether -f was given) is observed.
    // Modelled as a bool so the same "was -f given" check survives the FILE*
    // removal.
    expfile_given: bool,
    dialect_xerox: bool,
    dialect_posix_bre: bool,
    dialect_posix_ere: bool,
    dialect_perl: bool,
    dialect_fixed_strings: bool,
    match_word: bool,
    match_full_line: bool,
    linesep: u8,
    invert_matches: bool,
    max_count: u64,
    print_offset: bool,
    print_linenumbers: bool,
    flush_newlines: bool,
    print_filenames: bool,
    print_only_matches: bool,
    print_only_matching_filenames: bool,
    print_only_unmatching_filenames: bool,
    print_only_count: bool,
    count_matches: bool,
    print_filename_null: bool,
    before_context: u64,
    after_context: u64,
    format: ImplementationType,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            infilenames: Vec::new(),
            infile_readers: Vec::new(),
            regexp: None,
            expfile_given: false,
            dialect_xerox: false,
            dialect_posix_bre: false,
            dialect_posix_ere: false,
            dialect_perl: false,
            dialect_fixed_strings: false,
            match_word: false,
            match_full_line: false,
            linesep: b'\n',
            invert_matches: false,
            max_count: u64::MAX,
            print_offset: false,
            print_linenumbers: false,
            flush_newlines: false,
            print_filenames: false,
            print_only_matches: false,
            print_only_matching_filenames: false,
            print_only_unmatching_filenames: false,
            print_only_count: false,
            count_matches: false,
            print_filename_null: false,
            before_context: 0,
            after_context: 0,
            format: ImplementationType::UNSPECIFIED_TYPE,
        }
    }
}

/// The matcher pipeline is pinned to the tropical backend
/// ([dec:hfst:monomorphic-backends]): grep's output is the matched lines, so
/// the -f format (kept for option compatibility) never changes what is
/// printed. The C kept `matcher` (+ the gated `optimised_matcher`) as
/// file-scope pointers; here the built matcher is threaded through the pipeline.
struct MatcherState {
    matcher: HfstTransducer<hfst_openfst::StdVectorFst>,
    // The filename of the file currently being matched (the C kept a char*).
    inputfilename: String,
    linen: u64,
}

// [spec:hfst:def:hfst-grep.print-usage-fn]
// [spec:hfst:sem:hfst-grep.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] PATTERN [FILE...]\n\
                 Search for PATTERN in each FILE or standard input.\n\
                 Pattern is, by default, a Xerox regular expression (XRE).\n\
                 Example: hfst-grep 'h e l l o %%  w o r l d' menu.h menu.c\n\
                 \n",
        common.program_name
    );

    // options, grouped
    print_common_program_options(&mut *msg);
    let _ = writeln!(
        msg,
        "  -9, --format=TYPE       compile expressions to TYPE automata"
    );
    let _ = writeln!(msg);
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
    let _ = writeln!(msg);
    // bug report address
    // external docs
}

// [spec:hfst:def:hfst-grep.parse-options-fn]
// [spec:hfst:sem:hfst-grep.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }

        if c == b'9' as i32 {
            options.format = hfst_parse_format_name(&common, &opt.optarg());
        } else if c == b'E' as i32 {
            error(&common, 1, 0, "POSIX ERE syntax not yet supported");
            options.dialect_posix_ere = true;
        } else if c == b'F' as i32 {
            options.dialect_fixed_strings = true;
        } else if c == b'G' as i32 {
            error(&common, 1, 0, "POSIX BRE syntax not yet supported");
            options.dialect_posix_bre = true;
        } else if c == b'P' as i32 {
            error(&common, 1, 0, "Perl syntax not yet supported");
            options.dialect_perl = true;
        } else if c == b'X' as i32 {
            options.dialect_xerox = true;
        } else if c == b'e' as i32 {
            options.regexp = Some(opt.optarg());
        } else if c == b'f' as i32 {
            // C: expfile = hfst_fopen(optarg, "r"); the handle is never read,
            // but hfst_fopen validates the file (erroring on failure) — mirror
            // that and record that -f was given.
            let fname = opt.optarg();
            if fname != "-" && std::fs::File::open(&fname).is_err() {
                error(&common, 1, 0, &format!("Could not open '{}'. ", fname));
            }
            options.expfile_given = true;
        } else if c == b'I' as i32 {
            error(&common, 1, 0, "Ignore case not supported");
        } else if c == b'w' as i32 {
            options.match_word = true;
        } else if c == b'x' as i32 {
            options.match_full_line = true;
        } else if c == b'z' as i32 {
            options.linesep = 0;
        } else if c == INVERT_OPT {
            options.invert_matches = true;
        } else if c == b'm' as i32 {
            options.max_count = parse_u64(&common, &opt.optarg(), 10);
            options.count_matches = true;
        } else if c == b'b' as i32 {
            options.print_offset = true;
        } else if c == b'n' as i32 {
            options.print_linenumbers = true;
        } else if c == LINEBUFFER_OPT {
            options.flush_newlines = true;
        } else if c == b'H' as i32 {
            options.print_filenames = true;
        } else if c == b'O' as i32 {
            options.print_only_matches = true;
        } else if c == BINARYFILES_OPT {
            error(&common, 1, 0, "No binary handling implemented");
        } else if c == b'a' as i32 {
            warning(&common, 0, 0, "All files are always handled as text");
        } else if c == b'D' as i32 || c == b'r' as i32 {
            error(&common, 1, 0, "No directory handling implemented");
        } else if c == INCLUDE_OPT
            || c == EXCLUDE_OPT
            || c == INCLUDEFROM_OPT
            || c == EXCLUDEFROM_OPT
        {
            error(&common, 1, 0, "No directory/globbing implemented");
        } else if c == b'L' as i32 {
            options.print_only_unmatching_filenames = true;
        } else if c == b'l' as i32 {
            options.print_only_matching_filenames = true;
        } else if c == b'c' as i32 {
            options.count_matches = true;
            options.print_only_count = true;
        } else if c == b'Z' as i32 {
            options.print_filename_null = true;
        } else if c == b'A' as i32 {
            options.before_context = parse_u64(&common, &opt.optarg(), 10);
        } else if c == b'B' as i32 {
            options.after_context = parse_u64(&common, &opt.optarg(), 10);
        } else if c == b'C' as i32 {
            options.before_context = parse_u64(&common, &opt.optarg(), 10);
            options.after_context = parse_u64(&common, &opt.optarg(), 10);
        } else if c == b'u' as i32 || c == b'U' as i32 {
            error(
                &common,
                1,
                0,
                "MSDOS binary format not supported; use fromdos or dos2unix",
            );
        } else {
            return Err(handle_error_case(&common, &opt, c));
        }
    }
    if !options.dialect_fixed_strings
        && !options.dialect_xerox
        && !options.dialect_posix_bre
        && !options.dialect_posix_ere
        && !options.dialect_perl
    {
        warning(
            &common,
            0,
            0,
            "Dialect not defined, defaulting to Xerox for now!",
        );
        options.dialect_xerox = true;
    }
    if options.format == ImplementationType::UNSPECIFIED_TYPE {
        options.format = ImplementationType::TROPICAL_OPENFST_TYPE;
    }
    if options.regexp.is_none() && !options.expfile_given {
        if args.len() <= opt.optind {
            print_usage(&common);
            print_short_help(&common);
            return Err(1);
        } else {
            options.regexp = Some(args[opt.optind].clone());
            opt.optind += 1;
        }
    }
    if args.len() == opt.optind {
        options.infilenames.push("<stdin>".to_string());
        options
            .infile_readers
            .push(Box::new(std::io::BufReader::new(std::io::stdin())));
    } else {
        for i in opt.optind..args.len() {
            let name = args[i].clone();
            options.infilenames.push(name.clone());
            // C: infiles[i] = hfst_fopen(infilenames[i], "r"); open the named
            // file as a buffered std reader, mapping "-" to stdin and erroring
            // on a failed open through the same path.
            if name == "-" {
                options
                    .infile_readers
                    .push(Box::new(std::io::BufReader::new(std::io::stdin())));
            } else {
                match std::fs::File::open(&name) {
                    Ok(f) => options
                        .infile_readers
                        .push(Box::new(std::io::BufReader::new(f))),
                    Err(_) => {
                        error(&common, 1, 0, &format!("Could not open '{}'. ", name));
                    }
                }
            }
        }
    }
    check_common_params(&mut common);
    Ok((common, options))
}

// [spec:hfst:def:hfst-grep.string-to-utf8-fn]
// [spec:hfst:sem:hfst-grep.string-to-utf8-fn]
#[allow(dead_code)]
fn string_to_utf8(state: &MatcherState, p: &str) -> Vec<String> {
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
                &state.inputfilename,
                state.linen as u32,
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

// [spec:hfst:def:hfst-grep.read-matcher-fn]
// [spec:hfst:sem:hfst-grep.read-matcher-fn]
#[allow(dead_code)]
fn read_matcher_stream(
    common: &CommonOptions,
    state: &mut MatcherState,
    instream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut transducer_n: usize = 0;
    state.matcher = HfstTransducer::new();
    while instream.is_good() {
        transducer_n += 1;
        // one dispatch per read: everything joins the tropical matcher
        let mut trans: HfstTransducer<hfst_openfst::StdVectorFst> =
            match instream.read().and_then(|any| any.into_typed()) {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
        let mut inputname = trans.get_name();
        if inputname.is_empty() {
            inputname = state.inputfilename.clone();
        }
        if transducer_n == 1 {
            verbose_print(common, &format!("Reading matcher {}...\n", inputname));
        } else {
            verbose_print(
                common,
                &format!("Reading matcher {}...{}\n", inputname, transducer_n),
            );
        }
        if transducer_n > 1 {
            verbose_print(common, "and disjuncting...\n");
        }
        if let Err(e) = trans.input_project() {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if let Err(e) = state.matcher.disjunct(&trans, true) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }
    verbose_print(common, "minimising matchers...\n");
    if let Err(e) = state.matcher.minimize() {
        error(common, 1, 0, &format!("{e}"));
        return 1;
    }
    instream.close();
    0
}

fn read_matcher(
    common: &CommonOptions,
    options: &Options,
    state: &mut MatcherState,
    expression: &str,
) {
    // (FORMAT is parsed for option compatibility; the matcher runs on
    // the tropical backend regardless — matching is weight-independent.)
    let _ = options.format;
    state.matcher = HfstTransducer::new();
    if options.dialect_xerox {
        let mut comp = XreCompiler::<hfst_openfst::StdVectorFst>::new();
        verbose_print(
            common,
            &format!(
                "parsing {} as Xerox style regular expression...\n",
                expression
            ),
        );
        let mut trans = comp.compile(expression).unwrap();
        if let Err(e) = trans.input_project() {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        if let Err(e) = state.matcher.disjunct(&trans, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
    } else if options.dialect_fixed_strings {
        verbose_print(
            common,
            &format!(
                "parsing {} as fixed string of UTF-8 symbols...\n",
                expression
            ),
        );
        let t = HfstTokenizer::new();
        let trans = match HfstTransducer::new_tokenized_pair(expression, expression, &t) {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        if let Err(e) = state.matcher.disjunct(&trans, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
    } else {
        error(common, 1, 0, "dialect unsupported");
    }
    verbose_print(common, "minimizing...\n");
    if let Err(e) = state.matcher.minimize() {
        error(common, 1, 0, &format!("{e}"));
        return;
    }
    if common.verbose {
        verbose_print(common, "Resulting FSM:\n");
        // C: std::cerr << *matcher;
        hfst::hfst_transducer::write_to(&mut std::io::stderr(), &state.matcher);
    }
}

// [spec:hfst:def:hfst-grep.extend-matcher-with-options-fn]
// [spec:hfst:sem:hfst-grep.extend-matcher-with-options-fn]
fn extend_matcher_with_options(
    common: &CommonOptions,
    options: &Options,
    state: &mut MatcherState,
) {
    if common.colour == ColourTristate::COLOUR_ALWAYS {
        verbose_print(common, "Adding color codes to match boundaries...\n");
        let color_start = match HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "[31m") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        let color_end = match HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "[00m") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        let mut coloured = color_start;
        if let Err(e) = coloured.concatenate(&state.matcher, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        if let Err(e) = coloured.concatenate(&color_end, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        state.matcher = coloured;
    } else {
        // bracket matches for now
        verbose_print(common, "Adding brackets to match boundaries...\n");
        let color_start = match HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "{{{") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        let color_end = match HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "}}}") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        let mut coloured = color_start;
        if let Err(e) = coloured.concatenate(&state.matcher, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        if let Err(e) = coloured.concatenate(&color_end, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        state.matcher = coloured;
    }
    if options.match_word {
        verbose_print(
            common,
            "Delimiting matcher to word boundaries (currently space)...\n",
        );
        let non_word_char_left = match HfstTransducer::new_symbol(" ") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        let non_word_char_right = match HfstTransducer::new_symbol(" ") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        let mut word_bounded = non_word_char_left;
        if let Err(e) = word_bounded.concatenate(&state.matcher, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        if let Err(e) = word_bounded.concatenate(&non_word_char_right, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        state.matcher = word_bounded;
    }
    if !options.match_full_line {
        verbose_print(common, "Extending matcher for repetitions and rest...\n");
        let mut left_any = match HfstTransducer::new_symbol("@_IDENTITY_SYMBOL_@") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        let mut right_any = match HfstTransducer::new_symbol("@_IDENTITY_SYMBOL_@") {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return;
            }
        };
        if let Err(e) = left_any.repeat_star() {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        if let Err(e) = right_any.repeat_star() {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        let mut one_match = left_any;
        if let Err(e) = one_match.concatenate(&state.matcher, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        if let Err(e) = one_match.concatenate(&right_any, true) {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
        state.matcher = one_match;
        if let Err(e) = state.matcher.repeat_plus() {
            error(common, 1, 0, &format!("{e}"));
            return;
        }
    }
    verbose_print(common, "Minimising extended matcher...\n");
    if let Err(e) = state.matcher.minimize() {
        error(common, 1, 0, &format!("{e}"));
        return;
    }
    if common.verbose {
        verbose_print(common, "Resulting FSM:\n");
        hfst::hfst_transducer::write_to(&mut std::io::stderr(), &state.matcher);
    }
}

// [spec:hfst:def:hfst-grep.print-match-line-fn]
// [spec:hfst:sem:hfst-grep.print-match-line-fn]
#[allow(dead_code)]
fn print_match_line(
    options: &Options,
    state: &MatcherState,
    path: &HfstOneLevelPath,
    out: &mut dyn Write,
) {
    if options.print_only_matching_filenames || options.print_only_unmatching_filenames {
        return;
    }
    if options.print_filenames {
        let _ = out.write_all(state.inputfilename.as_bytes());
        if options.print_filename_null {
            let _ = out.write_all(&[0u8]);
        } else {
            let _ = out.write_all(b": ");
        }
    }
    if options.print_linenumbers {
        let _ = write!(out, "{}: ", state.linen);
    }
    for s in &path.second {
        let _ = out.write_all(s.as_bytes());
    }
    let _ = out.write_all(b"\n");
}

// [spec:hfst:def:hfst-grep.print-match-transducer-fn]
// [spec:hfst:sem:hfst-grep.print-match-transducer-fn]
fn print_match_transducer(
    common: &CommonOptions,
    options: &Options,
    state: &MatcherState,
    path: &HfstTransducer<hfst_openfst::StdVectorFst>,
    out: &mut dyn Write,
) {
    let mut p: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
    if let Err(e) = path.extract_paths(&mut p, 1, -1) {
        error(common, 1, 0, &format!("{e}"));
        return;
    }
    if options.print_only_matching_filenames || options.print_only_unmatching_filenames {
        return;
    }
    if options.print_filenames {
        let _ = out.write_all(state.inputfilename.as_bytes());
        if options.print_filename_null {
            let _ = out.write_all(&[0u8]);
        } else {
            let _ = out.write_all(b": ");
        }
    }
    if options.print_linenumbers {
        let _ = write!(out, "{}: ", state.linen);
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

/// @return true if matches in @a infile
// [spec:hfst:def:hfst-grep.match-lines-fn]
// [spec:hfst:sem:hfst-grep.match-lines-fn]
fn match_lines(
    common: &CommonOptions,
    options: &Options,
    state: &mut MatcherState,
    infile: &mut dyn BufRead,
    infilename: &str,
    out: &mut dyn Write,
) -> bool {
    verbose_print(common, &format!("matching against {}...\n", infilename));
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
        state.linen += 1;
        let mut line = String::from_utf8_lossy(&raw_bytes).into_owned();
        // C: scan to the first '\n' and replace it with '\0' (truncate there).
        if let Some(pos) = line.find('\n') {
            line.truncate(pos);
        }
        verbose_print(common, &format!("matching {}...\n", line));
        // #else branch (active: HFST_OPTIMISED_LOOKUP_CAN_IDENTITY undefined)
        if line.is_empty() {
            continue;
        }
        let line_str = line;
        let mut line_trans: HfstTransducer<hfst_openfst::StdVectorFst> =
            match HfstTransducer::new_tokenized_pair(&line_str, &line_str, &tokeniser) {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return false;
                }
            };
        verbose_print(common, "composing...\n");
        let mut results_t = match HfstTransducer::new_copy(&line_trans) {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return false;
            }
        };
        if let Err(e) = results_t.compose(&state.matcher, true) {
            error(common, 1, 0, &format!("{e}"));
            return false;
        }
        if let Err(e) = results_t.output_project() {
            error(common, 1, 0, &format!("{e}"));
            return false;
        }
        let empty: HfstTransducer<hfst_openfst::StdVectorFst> = HfstTransducer::new();
        let is_empty = match results_t.compare_default(&empty) {
            Ok(b) => b,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return false;
            }
        };
        if is_empty {
            verbose_print(common, "no matches\n");
            if options.invert_matches {
                print_match_transducer(common, options, state, &line_trans, &mut *out);
            }
        } else {
            verbose_print(common, "matches\n");
            if !options.invert_matches {
                print_match_transducer(common, options, state, &results_t, &mut *out);
            }
            matched = true;
            matches_n += 1;
        }
        let _ = &mut line_trans;
        if options.flush_newlines {
            let _ = out.flush();
        }
        if (options.max_count > 0) && (matches_n as u64 >= options.max_count) {
            break;
        }
    }
    if options.invert_matches {
        !matched
    } else {
        matched
    }
}

// [spec:hfst:def:hfst-grep.optimise-matcher-fn]
// [spec:hfst:sem:hfst-grep.optimise-matcher-fn]
#[allow(dead_code)]
fn optimise_matcher(
    common: &CommonOptions,
    state: &MatcherState,
) -> Option<HfstTransducer<hfst::transducer::Transducer>> {
    verbose_print(common, "Optimising...\n");
    // C: HfstTransducer(*matcher).convert(HFST_OL_TYPE) — the typed
    // algebra->OL conversion now.
    match state.matcher.to_ol(false, "") {
        Ok(t) => Some(t),
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            None
        }
    }
}

// [spec:hfst:def:hfst-grep.main-fn]
// [spec:hfst:sem:hfst-grep.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstGrep");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    verbose_print(&common, &format!("Writing to {}\n", common.output_filename));
    let mut state = MatcherState {
        matcher: HfstTransducer::new(),
        inputfilename: String::new(),
        linen: 0,
    };
    read_matcher(
        &common,
        &options,
        &mut state,
        &options.regexp.clone().unwrap_or_default(),
    );
    extend_matcher_with_options(&common, &options, &mut state);
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-grep: cannot open output: {e}");
            return 1;
        }
    };
    // #if HFST_OPTIMISED_LOOKUP_CAN_IDENTITY_SYMBOL: optimise_matcher();
    let mut options = options;
    for i in 0..options.infilenames.len() {
        state.inputfilename = options.infilenames[i].clone();
        state.linen = 0;
        let name = options.infilenames[i].clone();
        let mut reader =
            std::mem::replace(&mut options.infile_readers[i], Box::new(std::io::empty()));
        match_lines(
            &common,
            &options,
            &mut state,
            reader.as_mut(),
            &name,
            &mut *out,
        );
    }
    let _ = out.flush();

    // The former EXIT_CONTINUE return value: success once processing is done.
    0
}
