//! Faithful 1:1 port of tools/src/hfst-lexc-compiler.cc — the lexc compilation
//! command-line tool, driving the hfst::lexc::LexcCompiler library API.
//! Option handling is clap 4 derive through [`crate::cli`]; the -W/-x/-X/-f
//! vocabularies replay in command-line order so their diagnostics keep the C
//! getopt loop's sequencing (and so --Werror and -Wall interleave the way the
//! last writer wins).
//!
//! Compile lexc files into a transducer.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_parse_format_name, hfst_set_program_name, hfst_warning, parse_format_name_quiet,
    redirect_converting, verbose_print,
};
use crate::hfst_tool_metadata::{hfst_set_formula, hfst_set_name};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::lexc::LexcCompiler;
use std::io::Write;

/// hfst-lexc-compiler's own options (the former tool-specific `static mut`s).
struct Options {
    /// The lexc input filenames (a "<stdin>" entry is the standard-input sentinel).
    // The C kept a parallel FILE* array (LEXCFILES) of fopen'd lexc inputs; but
    // the file content is read by filename via std::fs::read_to_string in
    // lexc_streams, and the only thing the FILE* was used for was the stdin
    // sentinel. After the io-foundation de-C-ism that array is gone — the
    // "<stdin>" filename serves as the sentinel directly.
    lexcfilenames: Vec<String>,
    lexccount: u32,
    is_input_stdin: bool,
    format: ImplementationType,
    align_strings: bool,
    with_flags: bool,
    minimize_flags: bool,
    rename_flags: bool,
    treat_warnings_as_errors: bool,
    warn_everything: bool,
    warn_missing_lexicons: bool,
    warn_unused_lexicons: bool,
    warn_repeated_lexicons: bool,
    warn_one_sided_flags: bool,
    warn_missing_alphabets: bool,
    warn_unnecessary_escapes: bool,
    /// Compatibility with Xerox tools is the default.
    xerox_composition: bool,
    encode_weights: bool,
    /// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition'
    /// file-static global; now threaded into the lexc compiler via
    /// 'set_flag_is_epsilon').
    flag_is_epsilon: bool,
    split_characters: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            lexcfilenames: Vec::new(),
            lexccount: 0,
            is_input_stdin: true,
            format: ImplementationType::UNSPECIFIED_TYPE,
            align_strings: false,
            with_flags: false,
            minimize_flags: false,
            rename_flags: false,
            treat_warnings_as_errors: false,
            warn_everything: false,
            warn_missing_lexicons: false,
            warn_unused_lexicons: false,
            warn_repeated_lexicons: false,
            warn_one_sided_flags: false,
            warn_missing_alphabets: false,
            warn_unnecessary_escapes: false,
            xerox_composition: true,
            encode_weights: false,
            flag_is_epsilon: false,
            split_characters: false,
        }
    }
}

fn eput(s: &str) {
    let _ = std::io::stderr().write_all(s.as_bytes());
}

/// hfst-lexc's command line.
// [spec:hfst:def:hfst-lexc-compiler.parse-options-fn]
// [spec:hfst:sem:hfst-lexc-compiler.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Compile lexc files into transducer",
    after_help = "If INFILE or OUTFILE are omitted or -, standard streams will be used
The possible values for FORMAT are { sfst, openfst-tropical,
foma, optimized-lookup-unweighted, optimized-lookup-weighted }.
BOOL is one of {true,ON,yes} or {false,OFF,no}.
Xfst variables are {flag-is-epsilon (default OFF)}.
The -W switches are: all, error, [no-]one-sided-flags,
[no-]repeated-lexicons, [no-]missing-lexicons, [no-]unused-lexicons,
[no-]missing-alphabets, [no-]unnecessary-escapes.

Examples:
  hfst-lexc -o cat.hfst cat.lexc               Compile single-file lexicon
  hfst-lexc -o L.hfst Root.lexc 2.lexc 3.lexc  Compile multi-file lexicon

Using weights:
  LEXICON Root
  cat # \"weight: 2\" ;    Define weight for a word
  <[dog::1]+> # ;        Use weights in regular expressions

Using weights has an effect only if FORMAT is weighted, i.e.
{ openfst-tropical, optimized-lookup-weighted }."
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    /// compile into FORMAT transducer
    #[arg(
        short = 'f',
        long = "format",
        value_name = "FORMAT",
        allow_hyphen_values = true
    )]
    format: Option<String>,

    /// align characters in input and output strings
    #[arg(short = 'A', long = "alignStrings")]
    align_strings: bool,

    /// encode weights when minimizing (default is false)
    #[arg(short = 'E', long = "encode-weights")]
    encode_weights: bool,

    /// use flags to hyperminimize result
    #[arg(short = 'F', long = "withFlags")]
    with_flags: bool,

    /// if --withFlags is used, minimize the number of flags
    #[arg(short = 'M', long = "minimizeFlags")]
    minimize_flags: bool,

    /// if --withFlags and --minimizeFlags are used, rename flags (for
    /// testing)
    #[arg(short = 'R', long = "renameFlags")]
    rename_flags: bool,

    /// Whether flag diacritics are treated as ordinary symbols in composition
    /// (default is true)
    #[arg(
        short = 'x',
        long = "xerox-composition",
        value_name = "BOOL",
        allow_hyphen_values = true
    )]
    xerox_composition: Option<String>,

    /// toggle xfst compatibility option VARIABLE
    #[arg(
        short = 'X',
        long = "xfst",
        value_name = "VARIABLE",
        allow_hyphen_values = true
    )]
    xfst: Option<String>,

    /// Warning switch, e.g. -Wall or -Werror. (The C long table spells this
    /// '--Wstuff'; the accepted spelling is kept.)
    #[arg(
        short = 'W',
        long = "Wstuff",
        value_name = "SWITCH",
        action = clap::ArgAction::Append,
        allow_hyphen_values = true
    )]
    warnings: Vec<String>,

    /// deprecated: treat warnings as errors (use -Werror -Wall instead).
    /// The C option table gave the '--Werror' long the option value 'Q', so
    /// '-Q' has always been an (unadvertised) spelling of it.
    #[arg(short = 'Q', long = "Werror")]
    werror_deprecated: bool,

    /// disable unicode character parsing for multichars
    #[arg(short = '9', long = "split-characters")]
    split_characters: bool,

    /// The lexc source files; missing or - reads the standard input
    #[arg(value_name = "INFILE", num_args = 0..)]
    infiles: Vec<String>,

    /// The vocabulary-checked option occurrences in command-line order: the C
    /// loop's -W/-Q arms overwrite the same warning flags (and -f/-x/-X print
    /// their diagnostics as scanned), so the last writer wins and the
    /// diagnostics must replay in that order.
    #[arg(skip)]
    events: Vec<Event>,
}

/// One vocabulary-checked iteration of the C option loop, in occurrence order.
#[derive(Clone, Copy)]
enum Event {
    Format,
    XeroxComposition,
    Xfst,
    /// Index into the `warnings` occurrence vector.
    Warning(usize),
    /// The deprecated '--Werror' long option (the C's 'Q' case).
    WerrorDeprecated,
}

impl Args {
    /// Replay the C option loop over the ordered occurrences. `print` guards
    /// the non-fatal diagnostics (the --Werror deprecation line, the
    /// ambiguous-format warning) so the second pass does not repeat them.
    fn resolve(&self, common: &CommonOptions, print: bool) -> Result<Options, i32> {
        let mut options = Options {
            align_strings: self.align_strings,
            encode_weights: self.encode_weights,
            with_flags: self.with_flags,
            minimize_flags: self.minimize_flags,
            rename_flags: self.rename_flags,
            split_characters: self.split_characters,
            ..Options::default()
        };
        for event in &self.events {
            match event {
                Event::Format => {
                    let name = self.format.as_deref().unwrap_or_default();
                    options.format = if print {
                        hfst_parse_format_name(common, name)
                    } else {
                        parse_format_name_quiet(name)
                    };
                }
                Event::XeroxComposition => {
                    let argument = self.xerox_composition.as_deref().unwrap_or_default();
                    if argument == "yes" || argument == "true" || argument == "ON" {
                        options.xerox_composition = true;
                    } else if argument == "no" || argument == "false" || argument == "OFF" {
                        options.xerox_composition = false;
                    } else {
                        eput(&format!(
                            "Error: unknown option to --xerox-composition: '{}'\n",
                            argument
                        ));
                        return Err(1);
                    }
                }
                Event::Xfst => {
                    let argument = self.xfst.as_deref().unwrap_or_default();
                    if argument == "flag-is-epsilon" {
                        options.flag_is_epsilon = true;
                    } else {
                        eput(&format!(
                            "Error: unknown option to --xfst: '{}'\n",
                            argument
                        ));
                        return Err(1);
                    }
                }
                Event::WerrorDeprecated => {
                    options.treat_warnings_as_errors = true;
                    options.warn_one_sided_flags = true;
                    options.warn_missing_lexicons = true;
                    options.warn_unused_lexicons = true;
                    options.warn_repeated_lexicons = true;
                    // compatibility?? might change later:
                    options.warn_unnecessary_escapes = false;
                    options.warn_missing_alphabets = false;
                    if print {
                        eput("Warning: --Werror is deprecated, use -Werror -Wall instead\n");
                    }
                }
                Event::Warning(k) => {
                    let optarg = self.warnings[*k].as_str();
                    if optarg == "error" {
                        options.treat_warnings_as_errors = true;
                    } else if optarg == "all" {
                        options.warn_one_sided_flags = true;
                        options.warn_everything = true;
                        options.warn_missing_lexicons = true;
                        options.warn_unused_lexicons = true;
                        options.warn_repeated_lexicons = true;
                        options.warn_missing_alphabets = true;
                        options.warn_unnecessary_escapes = true;
                        options.warn_missing_alphabets = true;
                    } else if optarg == "one-sided-flags" {
                        options.warn_one_sided_flags = true;
                    } else if optarg == "no-one-sided-flags" {
                        options.warn_one_sided_flags = false;
                    } else if optarg == "unused-lexicons" {
                        options.warn_unused_lexicons = true;
                    } else if optarg == "no-unused-lexicons" {
                        options.warn_unused_lexicons = false;
                    } else if optarg == "repeated-lexicons" {
                        options.warn_repeated_lexicons = true;
                    } else if optarg == "no-repeated-lexicons" {
                        options.warn_repeated_lexicons = false;
                    } else if optarg == "missing-lexicons" {
                        options.warn_missing_lexicons = true;
                    } else if optarg == "no-missing-lexicons" {
                        options.warn_missing_lexicons = false;
                    } else if optarg == "missing-alphabets" {
                        options.warn_missing_alphabets = true;
                    } else if optarg == "no-missing-alphabets" {
                        options.warn_missing_alphabets = false;
                    } else if optarg == "unnecessary-escapes" {
                        options.warn_unnecessary_escapes = true;
                    } else if optarg == "no-unnecessary-escapes" {
                        options.warn_unnecessary_escapes = false;
                    } else {
                        eput(&format!("Unknown warning option {}\n", optarg));
                        return Err(1);
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
        let ids: &[(&str, Event)] = &[
            ("format", Event::Format),
            ("xerox_composition", Event::XeroxComposition),
            ("xfst", Event::Xfst),
            ("werror_deprecated", Event::WerrorDeprecated),
        ];
        let mut ordered: Vec<(usize, Event)> = ids
            .iter()
            .filter(|(id, _)| {
                matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine)
            })
            .filter_map(|(id, event)| matches.index_of(id).map(|i| (i, *event)))
            .collect();
        if matches.value_source("warnings") == Some(clap::parser::ValueSource::CommandLine)
            && let Some(indices) = matches.indices_of("warnings")
        {
            for (k, i) in indices.enumerate() {
                ordered.push((i, Event::Warning(k)));
            }
        }
        ordered.sort_by_key(|(i, _)| *i);
        self.events = ordered.into_iter().map(|(_, event)| event).collect();
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The vocabulary rejections happened inside the C loop, before the
        // parameter checks.
        self.resolve(opts, true)?;
        Ok(())
    }
}

// [spec:hfst:def:hfst-lexc-compiler.lexc-streams-fn]
// [spec:hfst:sem:hfst-lexc-compiler.lexc-streams-fn]
fn lexc_streams<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &Options,
    lexc: &mut LexcCompiler<B>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let lexcfilenames = &options.lexcfilenames;
    for name in &lexcfilenames[..options.lexccount as usize] {
        verbose_print(common, &format!("Parsing lexc file {}\n", name));
        if name == "<stdin>" {
            // The new Rust LexcCompiler::parse takes the source text, so we
            // read the whole of standard input into a string (mirroring the
            // C++ 'lexc.parse(stdin)').
            let mut source = String::new();
            use std::io::Read;
            let _ = std::io::stdin().read_to_string(&mut source);
            lexc.set_source_name(name);
            if let Err(e) = lexc.parse(&source) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        } else {
            // Read the named file's contents into a string (mirroring the
            // C++ 'lexc.parse(filename)').
            let source = std::fs::read_to_string(name).unwrap_or_default();
            lexc.set_source_name(name);
            if let Err(e) = lexc.parse(&source) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
    }
    verbose_print(common, "Compiling... ");
    let compiled = match lexc.compile_lexical() {
        Ok(c) => c,
        Err(e) => {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    };
    let Some(mut res) = compiled else {
        if options.lexccount == 1 {
            error(
                common,
                1,
                0,
                &format!(
                    "The file {} did not compile cleanly.\n(if there are no error messages above, try -v or -d to get more info)",
                    lexcfilenames[0]
                ),
            );
        } else {
            error(
                common,
                1,
                0,
                &format!(
                    "The files {}... did not compile cleanly.\n(if there are no error messages above, try -v or -d to get more info)",
                    lexcfilenames[0]
                ),
            );
        }
        return 1;
    };
    hfst_set_name(&mut res, &lexcfilenames[0], "lexc");
    hfst_set_formula(&mut res, &lexcfilenames[0], "L");
    verbose_print(common, "\nWriting... ");
    if let Err(e) = redirect_converting(outstream, &mut res) {
        error(common, 1, 0, &format!("{e}"));
        return 1;
    }
    verbose_print(common, "done\n");
    // C++ 'delete res' — owned value drops at end of scope.
    outstream.close();

    0
}

// [spec:hfst:def:hfst-lexc-compiler.main-fn]
// [spec:hfst:sem:hfst-lexc-compiler.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    cli::exit_code(execute(args))
}

fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstLexc");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = args.resolve(&common, false)?;

    // The C ran these right after check-params-common.h: the default-format
    // warning, then the operand resolution with its per-file open checks.
    if options.format == ImplementationType::UNSPECIFIED_TYPE {
        if !common.silent {
            hfst_warning(&common, 0, 0, "Defaulting to OpenFst tropical type");
        }
        options.format = ImplementationType::TROPICAL_OPENFST_TYPE;
    }
    if !args.infiles.is_empty() {
        for name in &args.infiles {
            // C: lexcfiles.push(hfst_fopen(name, "r")); a "-" resolved to
            // stdin, otherwise the named file was opened (erroring on
            // failure). The content is read by filename later, so only
            // validate openability and record "<stdin>" for "-".
            if name == "-" {
                options.lexcfilenames.push("<stdin>".to_string());
            } else {
                if std::fs::File::open(name).is_err() {
                    error(&common, 1, 0, &format!("Could not open '{}'. ", name));
                    return Err(1);
                }
                options.lexcfilenames.push(name.clone());
            }
            options.lexccount += 1;
        }
        options.is_input_stdin = false;
    } else {
        options.lexcfilenames.push("<stdin>".to_string());
        options.is_input_stdin = true;
        options.lexccount += 1;
    }

    // close buffers, we use streams
    verbose_print(&common, "Reading from ");
    for i in 0..(options.lexccount as usize) {
        verbose_print(&common, &format!("{}, ", options.lexcfilenames[i]));
    }
    verbose_print(&common, &format!("writing to {}\n", common.output_filename));
    // here starts the buffer handling part
    let output_opened = common.output_filename != "<stdout>";
    let outstream_res = if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, options.format, true)
    } else {
        HfstOutputStream::new(options.format, true)
    };
    let mut outstream = match outstream_res {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    // The parsed --format is matched ONCE into the compiler's backend
    // type ([dec:hfst:monomorphic-backends]); optimized-lookup formats
    // compile at tropical and convert at the write.
    cli::from_code(match options.format {
        ImplementationType::SFST_TYPE
        | ImplementationType::TROPICAL_OPENFST_TYPE
        | ImplementationType::FOMA_TYPE
        | ImplementationType::XFSM_TYPE
        | ImplementationType::HFST_OL_TYPE
        | ImplementationType::HFST_OLW_TYPE
        | ImplementationType::THFST_TYPE
        | ImplementationType::HFST2_TYPE
        | ImplementationType::UNSPECIFIED_TYPE
        | ImplementationType::ERROR_TYPE => {
            run_typed::<hfst_openfst::StdVectorFst>(&common, &options, &mut outstream)
        }
    })
}

fn run_typed<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &Options,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut lexc = LexcCompiler::<B>::new_with_flags(options.with_flags, options.align_strings);
    lexc.set_minimize_flags(options.minimize_flags);
    lexc.set_rename_flags(options.rename_flags);
    lexc.set_flag_is_epsilon(options.flag_is_epsilon);
    lexc.set_xerox_composition(options.xerox_composition);
    lexc.set_encode_weights(options.encode_weights);
    // lexc.with_flags = with_flags;
    if common.silent {
        lexc.set_verbosity(0);
    } else {
        lexc.set_verbosity(if common.verbose { 2 } else { 1 });
    }
    if options.treat_warnings_as_errors {
        lexc.set_treat_warnings_as_errors(true);
    }
    lexc.set_warning("-Wone-sided-flags", options.warn_one_sided_flags);
    lexc.set_warning("-Wunused-lexicons", options.warn_unused_lexicons);
    lexc.set_warning("-Wrepeated-lexicons", options.warn_repeated_lexicons);
    lexc.set_warning("-Wmissing-lexicons", options.warn_missing_lexicons);
    lexc.set_warning("-Wmissing-alphabets", options.warn_missing_alphabets);
    lexc.set_warning("-Wunnecessary-escapes", options.warn_unnecessary_escapes);
    if !common.silent && common.verbose {
        let mut line = String::from("Warning settings: ");
        if options.treat_warnings_as_errors {
            line.push_str(" -Werror (fail on all warnings)");
        }
        if options.warn_one_sided_flags {
            line.push_str(" -Wone-sided-flags");
        }
        if options.warn_unused_lexicons {
            line.push_str(" -Wunused-lexicons");
        }
        if options.warn_repeated_lexicons {
            line.push_str(" -Wrepeated-lexicons");
        }
        if options.warn_missing_lexicons {
            line.push_str(" -Wmissing-lexicons");
        }
        if options.warn_missing_alphabets {
            line.push_str(" -Wmissing-alphabets");
        }
        if options.warn_unnecessary_escapes {
            line.push_str(" -Wunnecessary-escapes");
        }
        line.push('\n');
        print!("{}", line);
    }
    if options.split_characters {
        eput("Warningn: Disabling unicode character tokenisation\n");
        lexc.set_split_characters(true);
    }
    // The C++ also frees the filename buffers here; the Rust owners drop
    // automatically.
    lexc_streams(common, options, &mut lexc, outstream)
}
