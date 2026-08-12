//! Faithful 1:1 port of tools/src/hfst-tokenize.cc — a replacement for
//! hfst-proc using pmatch: perform matching/lookup/tokenization on text
//! streams. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, inc fragments) and the hfst optimized-lookup pmatch
//! tokenizer ('hfst::pmatch_tokenize', 'hfst::pmatch', 'hfst::pmatch_compiler').
//!
//! This is a unary tool (#includes inc/globals-common.h + inc/globals-unary.h),
//! but like hfst-pmatch it does not use the usual unary
//! HfstInputStream/HfstOutputStream pipeline for output: it reads its single
//! positional argument as the ruleset archive filename, reads lines of stdin
//! (via 'inputfile'), and prints to stdout.
//!
//! The tokenization engine itself (the naive-tokenizer construction and the
//! input-segmentation drivers) lives in 'hfst::pmatch_tokenize'; this binary
//! keeps only option parsing and stream opening.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name, verbose_print};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use crate::inc::{CaseResult, handle_common_case, handle_error_case};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_tokenize::{
    OutputFormat, TokenizeInputSettings, TokenizeSettings, make_naive_tokenizer,
    process_input_stream,
};
use std::io::Write;

const DEFAULT_FORMAT: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

/// hfst-tokenize's own options (the former tool-specific `static mut`s).
struct Options {
    /// Input is apertium-style superblanks (overrides blankline_separated).
    superblanks: bool,
    /// Input is separated by blank lines (as opposed to single newlines).
    blankline_separated: bool,
    keep_newlines: bool,
    tokenizer_filename: String,
    /// 'static TokenizeSettings settings;' — default-constructed as in C++.
    settings: TokenizeSettings,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            superblanks: false,
            blankline_separated: true,
            keep_newlines: false,
            tokenizer_filename: String::new(),
            settings: TokenizeSettings::default(),
        }
    }
}

// [spec:hfst:def:hfst-tokenize.print-usage-fn]
// [spec:hfst:sem:hfst-tokenize.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [--segment | --xerox | --cg | --giella-cg] [OPTIONS...] RULESET\nperform matching/lookup on text streams\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "  -n, --newline            Newline as input separator (default is blank line)\n\
         \x20 -a, --print-all          Print nonmatching text\n\
         \x20 -w, --print-weight       Print weights (overrides earlier -W option)\n\
         \x20 -W, --no-weights         Don't print weights (default; overrides earlier -w, or -w implied by -g, options)\n\
         \x20 -m, --tokenize-multichar Tokenize multicharacter symbols\n\
         \x20                          (by default only one grapheme is tokenized at a time\n\
         \x20                          regardless of what is present in the alphabet)\n\
         \x20 -b, --beam=B             Output only analyses whose weight is within B from best result\n\
         \x20 -tS, --time-cutoff=S     Limit search after having used S seconds per input\n\
         \x20 -lN, --weight-classes=N  Output no more than N best weight classes\n\
         \x20                          (where analyses with equal weight constitute a class\n\
         \x20 -u, --unique             Remove duplicate analyses\n\
         \x20 -z, --segment            Segmenting / tokenization mode (default)\n\
         \x20 -i, --space-separated    Tokenization with one sentence per line, space-separated tokens\n\
         \x20 -x, --xerox              Xerox output\n\
         \x20 -c, --cg                 Constraint Grammar output\n\
         \x20 -S, --superblanks        Ignore contents of unescaped [] (cf. apertium-destxt); flush on NUL\n\
         \x20 -g, --giella-cg          CG format used in Giella infrastructure (implies -w and -l2,\n\
         \x20                          treats @PMATCH_INPUT_MARK@ as subreading separator,\n\
         \x20                          expects tags to be Multichar_symbols, flush on NUL)\n\
         \x20 -C  --conllu             CoNLL-U format\n\
         \x20 -f, --finnpos            FinnPos output\n\
         \x20 -L, --visl               VISL input and output (implies -W, handles <s> as blocks and <STYLE> inline)\n",
    );
    let _ = write!(
        msg,
        "Use standard streams for input and output (for now).\n\n"
    );
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-tokenize.parse-options-fn]
// [spec:hfst:sem:hfst-tokenize.parse-options-fn]
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        // tool-specific options
        let names: &[(&str, i32, i32)] = &[
            ("newline", getopt::NO_ARGUMENT, b'n' as i32),
            ("keep-newline", getopt::NO_ARGUMENT, b'k' as i32),
            ("print-all", getopt::NO_ARGUMENT, b'a' as i32),
            ("print-weights", getopt::NO_ARGUMENT, b'w' as i32),
            ("no-weights", getopt::NO_ARGUMENT, b'W' as i32),
            ("tokenize-multichar", getopt::NO_ARGUMENT, b'm' as i32),
            ("beam", getopt::REQUIRED_ARGUMENT, b'b' as i32),
            ("time-cutoff", getopt::REQUIRED_ARGUMENT, b't' as i32),
            ("weight-classes", getopt::REQUIRED_ARGUMENT, b'l' as i32),
            ("unique", getopt::NO_ARGUMENT, b'u' as i32),
            ("segment", getopt::NO_ARGUMENT, b'z' as i32),
            // C++ declares this long option as 'd' and only ever reaches the
            // space-separated case through the 'i' in its short-option string
            // "nkawWmub:t:l:zixcSgCfL", so upstream --space-separated silently
            // means --debug. This getopt carries no short string — `val` is the
            // sole channel for both spellings — so 'd' would lose the option to
            // the common --debug case and leave -i unknown. 'i' serves both;
            // --debug keeps 'd' via the common table.
            ("space-separated", getopt::NO_ARGUMENT, b'i' as i32),
            ("xerox", getopt::NO_ARGUMENT, b'x' as i32),
            ("cg", getopt::NO_ARGUMENT, b'c' as i32),
            ("superblanks", getopt::NO_ARGUMENT, b'S' as i32),
            ("giella-cg", getopt::NO_ARGUMENT, b'g' as i32),
            ("gtd", getopt::NO_ARGUMENT, b'g' as i32),
            ("conllu", getopt::NO_ARGUMENT, b'C' as i32),
            ("finnpos", getopt::NO_ARGUMENT, b'f' as i32),
            ("visl", getopt::NO_ARGUMENT, b'L' as i32),
        ];
        for &(name, has_arg, val) in names {
            long_options.push(getopt::GetOpt { name, has_arg, val });
        }
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        if c == b'k' as i32 {
            options.keep_newlines = true;
            options.blankline_separated = false;
        } else if c == b'n' as i32 {
            options.blankline_separated = false;
        } else if c == b'a' as i32 {
            options.settings.print_all = true;
        } else if c == b'w' as i32 {
            options.settings.print_weights = true;
        } else if c == b'W' as i32 {
            options.settings.print_weights = false;
        } else if c == b'm' as i32 {
            options.settings.tokenize_multichar = true;
        } else if c == b't' as i32 {
            options.settings.time_cutoff = opt.optarg().trim().parse::<f64>().unwrap_or(0.0);
            if options.settings.time_cutoff < 0.0 {
                eprintln!("Invalid argument for --time-cutoff");
                return Err(1);
            }
        } else if c == b'u' as i32 {
            options.settings.dedupe = true;
        } else if c == b'b' as i32 {
            options.settings.beam = opt.optarg().trim().parse::<f64>().unwrap_or(0.0) as f32;
            if options.settings.beam < 0.0 {
                eprintln!("Invalid argument for --beam");
                return Err(1);
            }
        } else if c == b'l' as i32 {
            options.settings.max_weight_classes = opt.optarg().trim().parse::<i32>().unwrap_or(0);
            if options.settings.max_weight_classes < 1 {
                eprintln!("Invalid or no argument --weight-classes count");
                return Err(1);
            }
        } else if c == b'z' as i32 {
            options.settings.output_format = OutputFormat::tokenize;
        } else if c == b'i' as i32 {
            options.settings.output_format = OutputFormat::space_separated;
        } else if c == b'x' as i32 {
            options.settings.output_format = OutputFormat::xerox;
        } else if c == b'c' as i32 {
            options.settings.output_format = OutputFormat::cg;
        } else if c == b'C' as i32 {
            options.settings.output_format = OutputFormat::conllu;
        } else if c == b'S' as i32 {
            options.superblanks = true;
        } else if c == b'g' as i32 {
            options.settings.output_format = OutputFormat::giellacg;
            options.settings.print_weights = true;
            options.settings.print_all = true;
            options.settings.dedupe = true;
            options.settings.hack_uncompose = true;
            options.settings.verbose = false;
            if options.settings.max_weight_classes == i32::MAX {
                options.settings.max_weight_classes = 2;
            }
        } else if c == b'L' as i32 {
            options.settings.output_format = OutputFormat::visl;
            options.settings.print_weights = false;
            options.settings.print_all = true;
            options.settings.dedupe = true;
            options.settings.verbose = false;
        } else if c == b'f' as i32 {
            options.settings.output_format = OutputFormat::finnpos;
        } else {
            return Err(handle_error_case(&common, &opt, c));
        }

        if common.verbose {
            options.settings.verbose = true;
        }
    }

    // no more options, we should now be at the input filename
    let argc = args.len();
    if (opt.optind + 1) < argc {
        eprintln!("More than one input file given");
        Err(1)
    } else if (opt.optind + 1) == argc {
        options.tokenizer_filename = args[opt.optind].clone();
        Ok((common, options))
    } else {
        eprintln!("No input file given");
        Err(1)
    }
}

// [spec:hfst:def:hfst-tokenize.first-transducer-is-called-top-fn]
// [spec:hfst:sem:hfst-tokenize.first-transducer-is-called-top-fn]
// (Defined in the C++ source but never called there; kept for fidelity.)
#[allow(dead_code)]
fn first_transducer_is_called_top<B: hfst::backend::Backend>(
    dictionary: &HfstTransducer<B>,
) -> bool {
    dictionary.get_name() == "TOP"
}

// [spec:hfst:def:hfst-tokenize.main-fn]
// [spec:hfst:sem:hfst-tokenize.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstTokenize");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let tokenizer_filename = options.tokenizer_filename.clone();
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            tokenizer_filename, common.output_filename
        ),
    );
    let mut file = match std::fs::File::open(&tokenizer_filename) {
        Ok(f) => f,
        Err(_) => {
            eprintln!("Could not open file {}", tokenizer_filename);
            return 1;
        }
    };
    // The C wraps the rest in try/catch on HfstException (and a nested catch
    // on TransducerHeaderException around parse_hfst3_header); the Rust ports
    // currently panic rather than throw, so those catch arms are not
    // reproduced here.
    //
    // To decide whether we're working with something produced by a pmatch
    // ruleset, we want to know whether the first transducer is named TOP. To
    // do this, rather than load the whole thing into a HfstTransducer, we read
    // just the header variables with parse_hfst3_header, then rewind.
    let first_header_attributes = {
        let mut hdr_stream = hfst::transducer::IStream::new(&mut file as &mut dyn std::io::Read);
        match PmatchContainer::parse_hfst3_header(&mut hdr_stream) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("hfst-tokenize: {e}");
                return 1;
            }
        }
    };
    use std::io::Seek;
    let _ = file.seek(std::io::SeekFrom::Start(0));

    let mut stdout = std::io::stdout();
    // Text input is read from the standard input stream (C: 'inputfile()').
    let mut input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-tokenize: cannot open input: {e}");
            return 1;
        }
    };
    // The tool-level input-mode switches, handed to the library driver.
    let input_settings = TokenizeInputSettings {
        superblanks: options.superblanks,
        blankline_separated: options.blankline_separated,
        keep_newlines: options.keep_newlines,
        verbose: common.verbose,
    };
    let mut msg = common.message_writer();
    if first_header_attributes.get("name").map(|s| s.as_str()) != Some("TOP") {
        verbose_print(&common, "No TOP automaton found, using naive tokeniser?\n");
        let mut is = match HfstInputStream::new_filename(&tokenizer_filename) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-tokenize: {e}");
                return 1;
            }
        };
        // C++ built the naive tokenizer's helper transducers in
        // default_format (tropical); the dictionary converts to the same
        // backend at this boundary ([dec:hfst:monomorphic-backends]).
        let _ = DEFAULT_FORMAT;
        let mut dictionary: HfstTransducer<hfst_openfst::StdVectorFst> =
            match is.read().and_then(|any| any.into_typed()) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            };
        let mut container = match make_naive_tokenizer(&mut dictionary) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("hfst-tokenize: {e}");
                return 1;
            }
        };
        container.set_verbose(common.verbose);
        // [#367] Auto-enable multichar (longest-match) tokenization when the
        // transducer carries multichar text symbols, so tokenise matches lookup
        // without -m; -m still forces it, single-grapheme alphabets stay
        // single-codepoint.
        let single_codepoint = if options.settings.tokenize_multichar {
            false
        } else {
            !container.has_multichar_input_symbols()
        };
        container.set_single_codepoint_tokenization(single_codepoint);
        process_input_stream(
            &mut container,
            &mut *input,
            &mut stdout,
            &mut *msg,
            &options.settings,
            &input_settings,
        )
    } else {
        verbose_print(
            &common,
            "TOP automaton seen, treating as pmatch script...\n",
        );
        let mut is = hfst::transducer::IStream::new(&mut file as &mut dyn std::io::Read);
        let mut container = match PmatchContainer::new_from_stream(&mut is) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("hfst-tokenize: {e}");
                return 1;
            }
        };
        container.set_verbose(common.verbose);
        // [#367] Auto-enable multichar (longest-match) tokenization when the
        // transducer carries multichar text symbols, so tokenise matches lookup
        // without -m; -m still forces it, single-grapheme alphabets stay
        // single-codepoint.
        let single_codepoint = if options.settings.tokenize_multichar {
            false
        } else {
            !container.has_multichar_input_symbols()
        };
        container.set_single_codepoint_tokenization(single_codepoint);
        process_input_stream(
            &mut container,
            &mut *input,
            &mut stdout,
            &mut *msg,
            &options.settings,
            &input_settings,
        )
    }
}
