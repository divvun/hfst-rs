//! Faithful 1:1 port of tools/src/hfst-grep.cc — the Hfst-based grep clone.
//! Bug-for-bug translation of the C++ tool.
//!
//! Building the matcher and matching each line are the library's
//! [`hfst::tools::grep`]; this tool reads the files and prints what matches.
//!
//! Option handling is clap 4 derive through [`crate::cli`]; the tool's state
//! lives in [`CommonOptions`] (the shared `-v/-q/-o/…` fields) and a
//! tool-local [`Options`] built by [`Args::resolve`] and threaded into the
//! processing functions. The GNU grep options this clone has no
//! implementation of (other dialects, case folding, directories, globbing,
//! binary and MSDOS handling) are not declared, so clap rejects them like any
//! unknown option. `--no-messages` joins the `-v`/`-q`/`-s` last-one-wins
//! chain, and the `-A`/`-B` shorts keep their swapped before/after pairing.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::ColourTristate;
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_parse_format_name, hfst_set_program_name, parse_format_name_quiet, parse_u64,
    print_short_help, verbose_print, warning,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::tools::grep::{Dialect, GrepOptions, Matcher};
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

/// hfst-grep's command line.
// [spec:hfst:def:hfst-grep.parse-options-fn]
// [spec:hfst:sem:hfst-grep.parse-options-fn]
// [spec:hfst:req:xfst-cmd.no-dead-surface]
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

    /// PATTERN is a set of newline-separated fixed strings
    #[arg(short = 'F', long = "fixed-strings")]
    fixed_strings: bool,

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

    /// Show only the part of a line matching PATTERN
    #[arg(short = 'O', long = "only-matching")]
    only_matching: bool,

    /// Equivalent to --binary-files=text (all files are handled as text)
    #[arg(short = 'a', long = "text")]
    text: bool,

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
    File,
    MaxCount,
    Text,
    BeforeContext,
    AfterContext,
    Context,
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
                Event::Text => {
                    if print {
                        warning(common, 0, 0, "All files are always handled as text");
                    }
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
            }
        }
        if !options.dialect_fixed_strings && !options.dialect_xerox {
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

    fn apply_io(&self, _opts: &mut CommonOptions) {}

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
            ("file", Event::File),
            ("max_count", Event::MaxCount),
            ("text", Event::Text),
            ("before_context", Event::BeforeContext),
            ("after_context", Event::AfterContext),
            ("context", Event::Context),
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

impl Options {
    /// What the options decide about matching and printing.
    fn grep_options(&self, common: &CommonOptions) -> GrepOptions {
        GrepOptions {
            dialect: if self.dialect_xerox {
                Dialect::Xerox
            } else {
                Dialect::FixedStrings
            },
            colour: common.colour == ColourTristate::COLOUR_ALWAYS,
            match_word: self.match_word,
            match_full_line: self.match_full_line,
            invert: self.invert_matches,
            max_count: self.max_count,
            print_filenames: self.print_filenames,
            filename_null: self.print_filename_null,
            print_line_numbers: self.print_linenumbers,
            only_filenames: self.print_only_matching_filenames
                || self.print_only_unmatching_filenames,
            flush_lines: self.flush_newlines,
        }
    }
}

/// Print the matcher in AT&T format when verbose.
fn show_matcher(common: &CommonOptions, matcher: &Matcher) {
    if common.verbose {
        verbose_print(common, "Resulting FSM:\n");
        hfst::hfst_transducer::write_to(&mut std::io::stderr(), matcher.transducer());
    }
}

// [spec:hfst:req:embed.tools-in-library]
fn build_matcher(common: &CommonOptions, options: &Options, grep: &GrepOptions) -> Matcher {
    // Accepted for option compatibility only: matching does not depend on
    // the backend, but a flag that quietly does nothing is worth saying out
    // loud.
    if options.format != ImplementationType::UNSPECIFIED_TYPE {
        verbose_print(
            common,
            "--format does not affect matching; grep always matches on the tropical backend\n",
        );
    }
    let mut progress = |message: &str| verbose_print(common, message);
    let mut comp = XreCompiler::<hfst_openfst::StdVectorFst>::new();
    comp.set_diagnostic_sink(crate::hfst_commandline::stderr_sink());
    let pattern = options.regexp.clone().unwrap_or_default();
    let compiled = Matcher::compile(&pattern, grep.dialect, &mut comp, &mut progress)
        .inspect(|matcher| show_matcher(common, matcher))
        .and_then(|mut matcher| {
            matcher.extend(grep, &mut progress)?;
            Ok(matcher)
        });
    match compiled {
        Ok(matcher) => {
            show_matcher(common, &matcher);
            matcher
        }
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            unreachable!("error(1, ...) ends the process")
        }
    }
}

// [spec:hfst:def:hfst-grep.main-fn]
// [spec:hfst:sem:hfst-grep.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstGrep");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = args.resolve(&common, false)?;
    verbose_print(&common, &format!("Writing to {}\n", common.output_filename));
    let grep = options.grep_options(&common);
    let matcher = build_matcher(&common, &options, &grep);
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-grep: cannot open output: {e}");
            return Err(1);
        }
    };
    let mut options = options;
    for i in 0..options.infilenames.len() {
        let name = options.infilenames[i].clone();
        let mut reader =
            std::mem::replace(&mut options.infile_readers[i], Box::new(std::io::empty()));
        let matched =
            matcher.match_lines(reader.as_mut(), &name, &grep, &mut *out, &mut |message| {
                verbose_print(&common, message)
            });
        if let Err(e) = matched {
            error(&common, 1, 0, &format!("{e}"));
        }
    }
    let _ = out.flush();

    // The former EXIT_CONTINUE return value: success once processing is done.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Options;
    use crate::globals::{ColourTristate, CommonOptions};
    use hfst::tools::grep::Dialect;

    fn common(colour: ColourTristate) -> CommonOptions {
        let mut common =
            crate::hfst_commandline::hfst_set_program_name("hfst-grep", "0.1", "HfstGrep");
        common.colour = colour;
        common
    }

    // [spec:hfst:req:embed.tools-in-library/test]
    #[test]
    fn options_decide_how_lines_match_and_print() {
        let options = Options {
            dialect_fixed_strings: true,
            invert_matches: true,
            print_only_unmatching_filenames: true,
            max_count: 3,
            ..Options::default()
        };
        let grep = options.grep_options(&common(ColourTristate::COLOUR_ALWAYS));
        assert_eq!(grep.dialect, Dialect::FixedStrings);
        assert!(grep.colour && grep.invert && grep.only_filenames);
        assert_eq!(grep.max_count, 3);
        let xerox = Options {
            dialect_xerox: true,
            dialect_fixed_strings: true,
            ..Options::default()
        };
        let grep = xerox.grep_options(&common(ColourTristate::COLOUR_NEVER));
        assert_eq!(grep.dialect, Dialect::Xerox);
        assert!(!grep.colour && !grep.only_filenames);
    }
}
