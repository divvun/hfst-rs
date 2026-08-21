//! Faithful 1:1 port of tools/src/hfst-grep.cc — the Hfst-based grep clone.
//! Bug-for-bug translation of the C++ tool.
//!
//! As in the C++, the optimised-lookup match path is gated behind
//! HFST_OPTIMISED_LOOKUP_CAN_IDENTITY, which is not defined; the active path
//! uses compose/output_project/compare on tropical automata. The functions
//! behind that gate (string_to_utf8, optimise_matcher, the optimised half of
//! match_lines, print_match_line) are still ported faithfully but are never
//! reached at runtime.
//!
//! Option handling is clap 4 derive through [`crate::cli`]; the tool's state
//! lives in [`CommonOptions`] (the shared `-v/-q/-o/…` fields) and a
//! tool-local [`Options`] built by [`Args::resolve`] and threaded into the
//! processing functions. The old option table survives with its quirks: the
//! GNU-grep options this clone accepts and then rejects keep their exact
//! messages, `--directories` is a spelling of `-d`/`--debug` (its ACTION is
//! swallowed), `--no-messages` joins the `-v`/`-q`/`-s` last-one-wins chain,
//! and the `-A`/`-B` shorts keep their swapped before/after pairing.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::ColourTristate;
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, error_at_line, hfst_parse_format_name, hfst_set_program_name, parse_format_name_quiet,
    parse_u64, print_short_help, verbose_print, warning,
};
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
    #[allow(
        dead_code,
        reason = "the '-z' arm writes it and no reader exists upstream either: \
                  the line loop reads to '\\n' unconditionally. The C assigned \
                  it inside parse_options, which is why the getopt-era port \
                  did not trip this lint."
    )]
    linesep: u8,
    invert_matches: bool,
    max_count: u64,
    #[allow(
        dead_code,
        reason = "'-b' sets it and nothing prints byte offsets, exactly as \
                  upstream: the feature was accepted but never implemented."
    )]
    print_offset: bool,
    print_linenumbers: bool,
    flush_newlines: bool,
    print_filenames: bool,
    #[allow(
        dead_code,
        reason = "'-O'/'--only-matching' sets it and no printer consults it, \
                  exactly as upstream: the feature was accepted but never \
                  implemented."
    )]
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

/// hfst-grep's command line.
// [spec:hfst:def:hfst-grep.parse-options-fn]
// [spec:hfst:sem:hfst-grep.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Search for PATTERN in each FILE or standard input.\n\
             Pattern is, by default, a Xerox regular expression (XRE).\n\
             Example: hfst-grep 'h e l l o %  w o r l d' menu.h menu.c")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    /// Compile expressions to TYPE automata
    #[arg(
        short = '9',
        long = "format",
        value_name = "TYPE",
        allow_hyphen_values = true
    )]
    format: Option<String>,

    /// PATTERN is an extended regular expression (ERE) — not yet supported
    #[arg(short = 'E', long = "extended-regexp")]
    extended_regexp: bool,

    /// PATTERN is a set of newline-separated fixed strings
    #[arg(short = 'F', long = "fixed-strings")]
    fixed_strings: bool,

    /// PATTERN is a basic regular expression (BRE) — not yet supported
    #[arg(short = 'G', long = "basic-regexp")]
    basic_regexp: bool,

    /// PATTERN is a Perl regular expression — not yet supported
    #[arg(short = 'P', long = "perl-regexp")]
    perl_regexp: bool,

    /// PATTERN is a Xerox regular expression (default)
    #[arg(short = 'X', long = "xerox-regexp")]
    xerox_regexp: bool,

    /// Use PATTERN for matching
    #[arg(
        short = 'e',
        long = "regexp",
        value_name = "PATTERN",
        allow_hyphen_values = true
    )]
    regexp: Option<String>,

    /// Obtain PATTERN from FILE
    #[arg(
        short = 'f',
        long = "file",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    file: Option<String>,

    /// Ignore case distinctions — not supported
    #[arg(short = 'I', long = "ignore-case")]
    ignore_case: bool,

    /// Force PATTERN to match only whole words
    #[arg(short = 'w', long = "word-regexp")]
    word_regexp: bool,

    /// Force PATTERN to match only whole lines
    #[arg(short = 'x', long = "line-regexp")]
    line_regexp: bool,

    /// A data line ends in 0 byte, not newline
    #[arg(short = 'z', long = "null-data")]
    null_data: bool,

    /// Suppress error messages (alias of --quiet)
    #[arg(long = "no-messages")]
    no_messages: bool,

    /// Select non-matching lines
    #[arg(long = "invert-match")]
    invert_match: bool,

    /// Stop after NUM matches
    #[arg(
        short = 'm',
        long = "max-count",
        value_name = "NUM",
        allow_hyphen_values = true
    )]
    max_count: Option<String>,

    /// Print the byte offset with output lines
    #[arg(short = 'b', long = "byte-offset")]
    byte_offset: bool,

    /// Print line number with output lines
    #[arg(short = 'n', long = "line-number")]
    line_number: bool,

    /// Flush output on every line
    #[arg(long = "line-buffered")]
    line_buffered: bool,

    /// Print the filename for each match
    #[arg(short = 'H', long = "with-filename")]
    with_filename: bool,

    /// Print LABEL as filename for standard input — not implemented
    #[arg(long = "label", value_name = "LABEL", allow_hyphen_values = true)]
    label: Option<String>,

    /// Show only the part of a line matching PATTERN
    #[arg(short = 'O', long = "only-matching")]
    only_matching: bool,

    /// Assume that binary files are TYPE — not implemented
    #[arg(long = "binary-files", value_name = "TYPE", allow_hyphen_values = true)]
    binary_files: Option<String>,

    /// Equivalent to --binary-files=text (all files are handled as text)
    #[arg(short = 'a', long = "text")]
    text: bool,

    /// How to handle directories (a spelling of --debug; ACTION is swallowed)
    #[arg(
        long = "directories",
        value_name = "ACTION",
        allow_hyphen_values = true
    )]
    directories: Option<String>,

    /// How to handle devices, FIFOs and sockets — not implemented
    #[arg(
        short = 'D',
        long = "devices",
        value_name = "ACTION",
        allow_hyphen_values = true
    )]
    devices: Option<String>,

    /// Equivalent to --directories=recurse — not implemented
    #[arg(short = 'r', long = "recursive")]
    recursive: bool,

    /// Search only files that match FILE_PATTERN — not implemented
    #[arg(
        long = "include",
        value_name = "FILE_PATTERN",
        allow_hyphen_values = true
    )]
    include: Option<String>,

    /// Skip files and directories matching FILE_PATTERN — not implemented
    #[arg(
        long = "exclude",
        value_name = "FILE_PATTERN",
        allow_hyphen_values = true
    )]
    exclude: Option<String>,

    /// Search only files matching any file pattern from FILE — not implemented
    #[arg(long = "include-from", value_name = "FILE", allow_hyphen_values = true)]
    include_from: Option<String>,

    /// Skip files matching any file pattern from FILE — not implemented
    #[arg(long = "exclude-from", value_name = "FILE", allow_hyphen_values = true)]
    exclude_from: Option<String>,

    /// Print only names of FILEs containing no match
    #[arg(short = 'L', long = "files-without-match")]
    files_without_match: bool,

    /// Print only names of FILEs containing matches
    #[arg(short = 'l', long = "files-with-match")]
    files_with_match: bool,

    /// Print only a count of matching lines per FILE
    #[arg(short = 'c', long = "count")]
    count: bool,

    /// Print 0 byte after FILE name
    #[arg(short = 'Z', long = "null")]
    null: bool,

    /// Print NUM lines of leading context ('-A' is leading here: the old
    /// option table paired the shorts the other way round from GNU grep)
    #[arg(
        short = 'A',
        long = "before-context",
        value_name = "NUM",
        allow_hyphen_values = true
    )]
    before_context: Option<String>,

    /// Print NUM lines of trailing context
    #[arg(
        short = 'B',
        long = "after-context",
        value_name = "NUM",
        allow_hyphen_values = true
    )]
    after_context: Option<String>,

    /// Print NUM lines of output context
    #[arg(
        short = 'C',
        long = "context",
        value_name = "NUM",
        allow_hyphen_values = true
    )]
    context: Option<String>,

    /// Do not strip CR characters at EOL (MSDOS) — not supported
    #[arg(short = 'u', long = "binary")]
    binary: bool,

    /// Report offsets as if CRs were not there (MSDOS) — not supported
    #[arg(short = 'U', long = "unix-byte-offset")]
    unix_byte_offset: bool,

    /// The pattern (unless -e/-f gave one) followed by the input files;
    /// missing files or - read the standard input
    #[arg(value_name = "PATTERN", num_args = 0..)]
    operands: Vec<String>,

    /// The checked option occurrences in command-line order: the C loop
    /// rejected the GNU-grep leftovers, warned for '-a', validated numbers
    /// and the '-f' file, and let '-A'/'-B'/'-C' overwrite each other as it
    /// scanned, so the diagnostics and the context writes replay in that
    /// order.
    #[arg(skip)]
    events: Vec<Event>,
}

/// One checked iteration of the C option loop, in occurrence order.
#[derive(Clone, Copy)]
enum Event {
    Format,
    ExtendedRegexp,
    BasicRegexp,
    PerlRegexp,
    IgnoreCase,
    File,
    MaxCount,
    Label,
    BinaryFiles,
    Text,
    Devices,
    Recursive,
    Globbing,
    BeforeContext,
    AfterContext,
    Context,
    Msdos,
}

impl Args {
    /// Replay the C option loop over the ordered occurrences, then the
    /// post-loop resolution (dialect default, pattern and input files).
    /// `print` gates the non-fatal diagnostics so the second pass after a
    /// successful validate stays silent; every rejection is fatal (`error`
    /// with a nonzero status exits).
    fn resolve(&self, common: &CommonOptions, print: bool) -> Result<Options, i32> {
        let mut options = Options {
            dialect_fixed_strings: self.fixed_strings,
            dialect_xerox: self.xerox_regexp,
            match_word: self.word_regexp,
            match_full_line: self.line_regexp,
            linesep: if self.null_data { 0 } else { b'\n' },
            invert_matches: self.invert_match,
            print_offset: self.byte_offset,
            print_linenumbers: self.line_number,
            flush_newlines: self.line_buffered,
            print_filenames: self.with_filename,
            print_only_matches: self.only_matching,
            print_only_matching_filenames: self.files_with_match,
            print_only_unmatching_filenames: self.files_without_match,
            regexp: self.regexp.clone(),
            expfile_given: self.file.is_some(),
            ..Options::default()
        };
        if self.count {
            options.count_matches = true;
            options.print_only_count = true;
        }
        options.print_filename_null = self.null;
        for event in &self.events {
            match event {
                Event::Format => {
                    let optarg = self.format.as_deref().unwrap_or_default();
                    options.format = if print {
                        hfst_parse_format_name(common, optarg)
                    } else {
                        parse_format_name_quiet(optarg)
                    };
                }
                Event::ExtendedRegexp => {
                    error(common, 1, 0, "POSIX ERE syntax not yet supported");
                    options.dialect_posix_ere = true;
                    return Err(1);
                }
                Event::BasicRegexp => {
                    error(common, 1, 0, "POSIX BRE syntax not yet supported");
                    options.dialect_posix_bre = true;
                    return Err(1);
                }
                Event::PerlRegexp => {
                    error(common, 1, 0, "Perl syntax not yet supported");
                    options.dialect_perl = true;
                    return Err(1);
                }
                Event::IgnoreCase => {
                    error(common, 1, 0, "Ignore case not supported");
                    return Err(1);
                }
                Event::File => {
                    // C: expfile = hfst_fopen(optarg, "r"); the handle is
                    // never read, but hfst_fopen validates the file (erroring
                    // on failure).
                    let fname = self.file.as_deref().unwrap_or_default();
                    if fname != "-" && std::fs::File::open(fname).is_err() {
                        error(common, 1, 0, &format!("Could not open '{}'. ", fname));
                        return Err(1);
                    }
                }
                Event::MaxCount => {
                    options.max_count =
                        parse_u64(common, self.max_count.as_deref().unwrap_or_default(), 10);
                    options.count_matches = true;
                }
                Event::Label => {
                    // The option table declared --label but the switch had no
                    // arm for it, so it fell into the getopt-cases-error.h
                    // 'default' — an "invalid option" naming the unprintable
                    // option value 21.
                    print_short_help(common);
                    error(common, 1, 0, &format!("invalid option -{}", '\u{15}'));
                    return Err(1);
                }
                Event::BinaryFiles => {
                    error(common, 1, 0, "No binary handling implemented");
                    return Err(1);
                }
                Event::Text => {
                    if print {
                        warning(common, 0, 0, "All files are always handled as text");
                    }
                }
                Event::Devices | Event::Recursive => {
                    error(common, 1, 0, "No directory handling implemented");
                    return Err(1);
                }
                Event::Globbing => {
                    error(common, 1, 0, "No directory/globbing implemented");
                    return Err(1);
                }
                Event::BeforeContext => {
                    options.before_context = parse_u64(
                        common,
                        self.before_context.as_deref().unwrap_or_default(),
                        10,
                    );
                }
                Event::AfterContext => {
                    options.after_context = parse_u64(
                        common,
                        self.after_context.as_deref().unwrap_or_default(),
                        10,
                    );
                }
                Event::Context => {
                    let optarg = self.context.as_deref().unwrap_or_default();
                    options.before_context = parse_u64(common, optarg, 10);
                    options.after_context = parse_u64(common, optarg, 10);
                }
                Event::Msdos => {
                    error(
                        common,
                        1,
                        0,
                        "MSDOS binary format not supported; use fromdos or dos2unix",
                    );
                    return Err(1);
                }
            }
        }
        if !options.dialect_fixed_strings
            && !options.dialect_xerox
            && !options.dialect_posix_bre
            && !options.dialect_posix_ere
            && !options.dialect_perl
        {
            if print {
                warning(
                    common,
                    0,
                    0,
                    "Dialect not defined, defaulting to Xerox for now!",
                );
            }
            options.dialect_xerox = true;
        }
        if options.format == ImplementationType::UNSPECIFIED_TYPE {
            options.format = ImplementationType::TROPICAL_OPENFST_TYPE;
        }
        let mut files: &[String] = &self.operands;
        if options.regexp.is_none() && !options.expfile_given {
            match files.split_first() {
                None => {
                    // C: print_usage + the short help, exit 1. The usage text
                    // is clap's now.
                    if print {
                        let mut msg = common.message_writer();
                        let mut cmd = <Args as clap::CommandFactory>::command()
                            .bin_name(common.program_name.clone());
                        let _ = write!(msg, "{}", cmd.render_help());
                    }
                    print_short_help(common);
                    return Err(1);
                }
                Some((pattern, rest)) => {
                    options.regexp = Some(pattern.clone());
                    files = rest;
                }
            }
        }
        if files.is_empty() {
            options.infilenames.push("<stdin>".to_string());
            options
                .infile_readers
                .push(Box::new(std::io::BufReader::new(std::io::stdin())));
        } else {
            for name in files {
                options.infilenames.push(name.clone());
                // C: infiles[i] = hfst_fopen(infilenames[i], "r"); open the
                // named file as a buffered std reader, mapping "-" to stdin
                // and erroring on a failed open through the same path.
                if name == "-" {
                    options
                        .infile_readers
                        .push(Box::new(std::io::BufReader::new(std::io::stdin())));
                } else {
                    match std::fs::File::open(name) {
                        Ok(f) => options
                            .infile_readers
                            .push(Box::new(std::io::BufReader::new(f))),
                        Err(_) => {
                            error(common, 1, 0, &format!("Could not open '{}'. ", name));
                            return Err(1);
                        }
                    }
                }
            }
        }
        Ok(options)
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        // '--directories' rode the same option value as '-d'/'--debug' in the
        // old table, so giving it (with any ACTION) turned debug mode on.
        if self.directories.is_some() {
            opts.debug = true;
        }
    }

    fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
        use clap::parser::ValueSource;
        let given = |id: &str| matches.value_source(id) == Some(ValueSource::CommandLine);
        // '--no-messages' was the C loop's 'q' arm under another long name, so
        // it joins the -v/-q/-s last-one-wins chain by occurrence index.
        if given("no_messages") {
            let nm = matches.index_of("no_messages").unwrap_or(0);
            let verbose_at = matches.index_of("verbose").filter(|_| given("verbose"));
            if verbose_at.is_none_or(|v| nm > v) {
                self.common.verbose = false;
                self.common.quiet = true;
            }
        }
        let ids: &[(&str, Event)] = &[
            ("format", Event::Format),
            ("extended_regexp", Event::ExtendedRegexp),
            ("basic_regexp", Event::BasicRegexp),
            ("perl_regexp", Event::PerlRegexp),
            ("ignore_case", Event::IgnoreCase),
            ("file", Event::File),
            ("max_count", Event::MaxCount),
            ("label", Event::Label),
            ("binary_files", Event::BinaryFiles),
            ("text", Event::Text),
            ("devices", Event::Devices),
            ("recursive", Event::Recursive),
            ("include", Event::Globbing),
            ("exclude", Event::Globbing),
            ("include_from", Event::Globbing),
            ("exclude_from", Event::Globbing),
            ("before_context", Event::BeforeContext),
            ("after_context", Event::AfterContext),
            ("context", Event::Context),
            ("binary", Event::Msdos),
            ("unix_byte_offset", Event::Msdos),
        ];
        let mut ordered: Vec<(usize, Event)> = ids
            .iter()
            .filter(|(id, _)| given(id))
            .filter_map(|(id, event)| matches.index_of(id).map(|i| (i, *event)))
            .collect();
        ordered.sort_by_key(|(i, _)| *i);
        self.events = ordered.into_iter().map(|(_, event)| event).collect();
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The rejections, number checks and file checks happened inside the C
        // loop, and the pattern/input resolution before check-params-common.
        self.resolve(opts, true)?;
        Ok(())
    }
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
        // C: comp.compile returned NULL on a parse failure (an empty pattern
        // included) and the next line dereferenced it — a crash upstream. Fail
        // cleanly instead: XRE has no empty expression, so nothing sound can
        // be built from one.
        let Some(mut trans) = comp.compile(expression) else {
            if expression.is_empty() {
                error(
                    common,
                    1,
                    0,
                    "empty pattern: XRE parsing failed (an empty Xerox regular expression is not valid)",
                );
            } else {
                error(common, 1, 0, &format!("XRE parsing failed: {}", expression));
            }
            return;
        };
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
pub fn run(args: Vec<String>) -> i32 {
    cli::exit_code(execute(args))
}

fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstGrep");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = args.resolve(&common, false)?;
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
            return Err(1);
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
    Ok(())
}
