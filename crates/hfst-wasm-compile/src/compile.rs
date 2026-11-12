//! The compilers: what `hfst regexp2fst`, `lexc`, `twolc`, `xfst`,
//! `pmatch2fst`, `strings2fst` and `txt2fst` make of their sources, with the
//! tools' options as the fields of an options object, in camelCase. Each is
//! plain Rust over the caller's files and options; `crate::entry` hands
//! JavaScript its sources and results. A compile that fails gives
//! `compile-failed` with every diagnostic it made; one that succeeds gives its
//! diagnostics with its result, which become its warnings.

use std::sync::Arc;

use hfst_embed::compile::{
    self as embed, CompileResult, Compiled, FileResolver, LexcOptions, LexcVerbosity, LogBase,
    Pmatch2FstOptions, RegexpOptions, Separation, Source, StringsOptions, Transducer as Machine,
    TwolcOptions, Txt2FstOptions, XfstOptions,
};
use hfst_embed::format::Format;
use hfst_embed::{Code, Error};
use hfst_wasm_shared::GlueError;

use crate::failure::Failure;
use crate::files::Files;
use crate::options::Options;

const ALGEBRA: [Format; 2] = [Format::OpenFstTropical, Format::Foma];

/// What the standard input is called when a tool reads its source there.
pub const STDIN: &str = "<stdin>";

// [spec:hfst:req:javascript.compile-errors]
/// The value of a compile with its diagnostics, or the `compile-failed`
/// error that carries them. `check` may still refuse the value, with an
/// error that carries them too.
fn finished<T, U>(
    compiled: CompileResult<T>,
    check: impl FnOnce(T) -> Result<U, Error>,
) -> Result<Compiled<U>, Failure> {
    let compiled = compiled.map_err(Failure::failed)?;
    match check(compiled.value) {
        Ok(value) => Ok(Compiled {
            value,
            diagnostics: compiled.diagnostics,
        }),
        Err(error) => Err(Failure::refused(error, compiled.diagnostics)),
    }
}

/// The one transducer a compile made, or the `transducer-count` error that
/// names `call` and what makes one of several, `one_of`.
fn single(
    compiled: CompileResult<Vec<Machine>>,
    call: &str,
    one_of: &str,
) -> Result<Compiled<Machine>, Failure> {
    finished(compiled, |mut all| {
        if all.len() == 1
            && let Some(t) = all.pop()
        {
            return Ok(t);
        }
        Err(Error::new(
            Code::TransducerCount,
            format!(
                "{call} returns one transducer, and the source compiled to {}; {one_of}",
                all.len()
            ),
        ))
    })
}

fn regex_options(o: &Options<'_>) -> Result<(Format, RegexpOptions), GlueError> {
    o.only(&[
        "format",
        "semicolon",
        "disjunct",
        "doNotHarmonize",
        "harmonizeFlags",
        "doNotMinimize",
        "encodeWeights",
        "xeroxComposition",
        "flagIsEpsilon",
        "replacePass",
        "verbose",
    ])?;
    Ok((
        o.format(&Format::ALL, Format::OpenFstTropical)?,
        RegexpOptions {
            separation: if o.flag("semicolon", false)? {
                Separation::Semicolons
            } else {
                Separation::Lines
            },
            disjunct: o.flag("disjunct", false)?,
            harmonize: !o.flag("doNotHarmonize", false)?,
            harmonize_flags: o.flag("harmonizeFlags", false)?,
            minimize: !o.flag("doNotMinimize", false)?,
            flag_is_epsilon: o.flag("flagIsEpsilon", false)?,
            xerox_composition: o.flag("xeroxComposition", false)?,
            encode_weights: o.flag("encodeWeights", false)?,
            replace_pass: o.flag("replacePass", false)?,
            verbose: o.flag("verbose", false)?,
        },
    ))
}

fn lexc_options(o: &Options<'_>) -> Result<(Format, LexcOptions), GlueError> {
    o.only(&[
        "format",
        "alignStrings",
        "withFlags",
        "minimizeFlags",
        "renameFlags",
        "xeroxComposition",
        "flagIsEpsilon",
        "encodeWeights",
        "splitCharacters",
        "werror",
        "warnOneSidedFlags",
        "warnUnusedLexicons",
        "warnRepeatedLexicons",
        "warnMissingLexicons",
        "warnMissingAlphabets",
        "warnUnnecessaryEscapes",
        "quiet",
        "verbose",
    ])?;
    o.not_both("quiet", "verbose")?;
    let verbosity = if o.flag("quiet", false)? {
        LexcVerbosity::Quiet
    } else if o.flag("verbose", false)? {
        LexcVerbosity::Verbose
    } else {
        LexcVerbosity::Normal
    };
    Ok((
        o.format(&Format::ALL, Format::OpenFstTropical)?,
        LexcOptions {
            align_strings: o.flag("alignStrings", false)?,
            with_flags: o.flag("withFlags", false)?,
            minimize_flags: o.flag("minimizeFlags", false)?,
            rename_flags: o.flag("renameFlags", false)?,
            warnings_as_errors: o.flag("werror", false)?,
            warn_one_sided_flags: o.flag("warnOneSidedFlags", false)?,
            warn_unused_lexicons: o.flag("warnUnusedLexicons", false)?,
            warn_repeated_lexicons: o.flag("warnRepeatedLexicons", false)?,
            warn_missing_lexicons: o.flag("warnMissingLexicons", false)?,
            warn_missing_alphabets: o.flag("warnMissingAlphabets", false)?,
            warn_unnecessary_escapes: o.flag("warnUnnecessaryEscapes", false)?,
            xerox_composition: o.flag("xeroxComposition", true)?,
            encode_weights: o.flag("encodeWeights", false)?,
            flag_is_epsilon: o.flag("flagIsEpsilon", false)?,
            split_characters: o.flag("splitCharacters", false)?,
            verbosity,
        },
    ))
}

fn twolc_options(o: &Options<'_>) -> Result<(Format, TwolcOptions), GlueError> {
    o.only(&["format", "resolve", "dontResolveRight", "quiet", "verbose"])?;
    o.not_both("quiet", "verbose")?;
    Ok((
        o.format(&ALGEBRA, Format::OpenFstTropical)?,
        TwolcOptions {
            quiet: o.flag("quiet", false)?,
            verbose: o.flag("verbose", false)?,
            resolve_left_conflicts: o.flag("resolve", false)?,
            resolve_right_conflicts: !o.flag("dontResolveRight", false)?,
        },
    ))
}

fn xfst_options(o: &Options<'_>) -> Result<(Format, XfstOptions), GlueError> {
    o.only(&["format", "printWeight", "restrictedMode", "silent"])?;
    Ok((
        o.format(&ALGEBRA, Format::OpenFstTropical)?,
        XfstOptions {
            print_weight: o.flag("printWeight", false)?,
            restricted_mode: o.flag("restrictedMode", false)?,
            silent: o.flag("silent", false)?,
        },
    ))
}

fn pmatch_options(o: &Options<'_>) -> Result<Pmatch2FstOptions, GlueError> {
    o.only(&["flatten", "cosineDistances", "verbose"])?;
    Ok(Pmatch2FstOptions {
        flatten: o.flag("flatten", false)?,
        include_cosine_distances: o.flag("cosineDistances", false)?,
        include_path: String::new(),
        verbose: o.flag("verbose", false)?,
    })
}

fn strings_options(o: &Options<'_>) -> Result<(Format, StringsOptions), GlueError> {
    o.only(&[
        "format",
        "disjunctStrings",
        "norm",
        "log",
        "log10",
        "pairstrings",
        "hasSpaces",
        "epsilon",
        "multicharSymbols",
        "warnNegativeWeights",
        "werror",
    ])?;
    o.not_both("log", "log10")?;
    let log = if o.flag("log", false)? {
        Some(LogBase::E)
    } else if o.flag("log10", false)? {
        Some(LogBase::Ten)
    } else {
        None
    };
    Ok((
        o.format(&Format::ALL, Format::OpenFstTropical)?,
        StringsOptions {
            epsilon: o.text("epsilon")?.unwrap_or_else(|| "@0@".to_owned()),
            multichar_symbols: o.texts("multicharSymbols")?.unwrap_or_default(),
            pairstrings: o.flag("pairstrings", false)?,
            has_spaces: o.flag("hasSpaces", false)?,
            disjunct: o.flag("disjunctStrings", false)?,
            normalize: o.flag("norm", false)?,
            log,
            warn_negative_weights: o.flag("warnNegativeWeights", true)?,
            warnings_are_errors: o.flag("werror", false)?,
        },
    ))
}

fn att_options(o: &Options<'_>) -> Result<(Format, Txt2FstOptions), GlueError> {
    o.only(&[
        "format",
        "epsilon",
        "disjunct",
        "checkNegativeEpsilonCycles",
        "warnNegativeWeights",
    ])?;
    Ok((
        o.format(&Format::ALL, Format::OpenFstTropical)?,
        Txt2FstOptions {
            prolog: false,
            epsilon: o.text("epsilon")?.unwrap_or_else(|| "@0@".to_owned()),
            check_negative_epsilon_cycles: o.flag("checkNegativeEpsilonCycles", false)?,
            warn_negative_weights: o.flag("warnNegativeWeights", true)?,
            disjunct: o.flag("disjunct", false)?,
        },
    ))
}

fn prolog_options(o: &Options<'_>) -> Result<(Format, Txt2FstOptions), GlueError> {
    o.only(&["format", "checkNegativeEpsilonCycles"])?;
    Ok((
        o.format(&Format::ALL, Format::OpenFstTropical)?,
        Txt2FstOptions {
            prolog: true,
            check_negative_epsilon_cycles: o.flag("checkNegativeEpsilonCycles", false)?,
            ..Txt2FstOptions::default()
        },
    ))
}

// [spec:hfst:req:javascript.compile-module]
// [spec:hfst:req:javascript.compile-files]
/// The transducer `hfst regexp2fst` writes of the regular expression
/// `source`. With `disjunct` a source of several expressions compiles to
/// their union; without it, a source of more than one fails with
/// `transducer-count`.
pub fn regex(
    source: Source<'_>,
    files: &Files,
    o: &Options<'_>,
) -> Result<Compiled<Machine>, Failure> {
    let (format, options) = regex_options(o)?;
    single(
        embed::compile_regexps(source, format, &options, Arc::new(files.clone())),
        "regex",
        "the option disjunct compiles a source of several expressions to their union",
    )
}

// [spec:hfst:req:javascript.compile-module]
// [spec:hfst:req:javascript.compile-files]
/// The transducer `hfst lexc` writes of the lexc `sources`, compiled
/// together and named after the first.
pub fn lexc(
    sources: &[Source<'_>],
    files: &Files,
    o: &Options<'_>,
) -> Result<Compiled<Machine>, Failure> {
    let (format, options) = lexc_options(o)?;
    finished(
        embed::compile_lexc(sources, format, &options, Arc::new(files.clone())),
        Ok,
    )
}

// [spec:hfst:req:javascript.compile-module]
/// The rule transducers `hfst twolc` writes of the grammar `source`, in
/// tropical OpenFst or foma. A twolc grammar names no file, so it reads none.
pub fn twolc(source: Source<'_>, o: &Options<'_>) -> Result<Compiled<Vec<Machine>>, Failure> {
    let (format, options) = twolc_options(o)?;
    finished(embed::compile_twolc(source, format, &options), Ok)
}

/// What an xfst script left behind, with the bytes of each file it wrote.
pub struct Ran {
    pub output: String,
    pub stack: Vec<Machine>,
    pub definitions: Vec<(String, Machine)>,
    pub written: Vec<(String, Vec<u8>)>,
}

// [spec:hfst:req:javascript.compile-module]
// [spec:hfst:req:javascript.compile-files]
/// Runs the xfst script `source` as `hfst xfst -F` runs it, with `input` as
/// the standard input its commands read. Each file it writes lands among its
/// files and comes back with what it printed, its stack and its definitions.
/// A command that fails is a diagnostic, and the script fails only when it
/// stops on one.
pub fn xfst(
    source: Source<'_>,
    input: &str,
    files: &Files,
    o: &Options<'_>,
) -> Result<Compiled<Ran>, Failure> {
    let (format, options) = xfst_options(o)?;
    let compiled = embed::compile_xfst(source, input, format, &options, Arc::new(files.clone()));
    finished(compiled, |ran| {
        let written = ran
            .written
            .into_iter()
            .map(|file| {
                let bytes = files.read(&file).unwrap_or_default();
                (file, bytes)
            })
            .collect();
        Ok(Ran {
            output: ran.output,
            stack: ran.stack,
            definitions: ran.definitions,
            written,
        })
    })
}

// [spec:hfst:req:javascript.compile-module]
// [spec:hfst:req:javascript.compile-files]
/// The archive `hfst pmatch2fst` writes of the pmatch script `source`. A
/// relative `@bin"FILE"` or `@txt"FILE"` is found beside the source's name,
/// as the tool finds one beside its input.
pub fn pmatch(
    source: Source<'_>,
    files: &Files,
    o: &Options<'_>,
) -> Result<Compiled<Vec<u8>>, Failure> {
    let mut options = pmatch_options(o)?;
    if source.name != STDIN
        && let Some(slash) = source.name.rfind('/')
    {
        options.include_path = source.name[..=slash].to_owned();
    }
    finished(
        embed::compile_pmatch(source, &options, Arc::new(files.clone())),
        Ok,
    )
}

/// A compiler of text that names no file: `hfst strings2fst`, and
/// `hfst txt2fst` reading AT&T or Prolog text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Text {
    Strings,
    Att,
    Prolog,
}

impl Text {
    /// The kind the wrapper names, as its call is named.
    pub fn named(kind: &str) -> Option<(Text, &'static str)> {
        match kind {
            "strings" => Some((Text::Strings, "compileStrings")),
            "att" => Some((Text::Att, "parseAtt")),
            "prolog" => Some((Text::Prolog, "parseProlog")),
            _ => None,
        }
    }
}

// [spec:hfst:req:javascript.compile-module]
/// The transducer `hfst strings2fst` writes of the strings `source`, one a
/// line, or `hfst txt2fst` of the AT&T or Prolog text `source`, named after
/// it. With `disjunctStrings`, or AT&T's `disjunct`, the strings or the
/// transducers compile to their union; without it, a source of more than
/// one fails with `transducer-count`, as a Prolog text of more than one
/// network always does.
pub fn text(kind: Text, source: Source<'_>, o: &Options<'_>) -> Result<Compiled<Machine>, Failure> {
    let (compiled, call, one_of) = match kind {
        Text::Strings => {
            let (format, options) = strings_options(o)?;
            (
                embed::compile_strings(source, format, &options),
                "compileStrings",
                "the option disjunctStrings compiles a source of several lines to their union",
            )
        }
        Text::Att => {
            let (format, options) = att_options(o)?;
            (
                embed::compile_text(source, format, &options),
                "parseAtt",
                "the option disjunct reads a text of several transducers as their union",
            )
        }
        Text::Prolog => {
            let (format, options) = prolog_options(o)?;
            (
                embed::compile_text(source, format, &options),
                "parseProlog",
                "give the text of one network",
            )
        }
    };
    single(compiled, call, one_of)
}

#[cfg(test)]
mod tests {
    use hfst_embed::compile::Severity;

    use super::*;
    use crate::options::{Value, given};
    use crate::transducer::Transducer;

    fn source<'a>(name: &'a str, text: &'a str) -> Source<'a> {
        Source { name, text }
    }

    fn none() -> Files {
        Files::of(Vec::new())
    }

    fn paths(t: Machine) -> Vec<String> {
        crate::tests::paths(&Transducer::new(t))
    }

    fn failed<T>(compiled: Result<Compiled<T>, Failure>) -> (&'static str, String, Vec<String>) {
        match compiled {
            Err(Failure::Compile {
                code,
                message,
                diagnostics,
            }) => (
                code,
                message,
                diagnostics.iter().map(|d| d.message().to_owned()).collect(),
            ),
            Err(Failure::Glue(error)) => (error.code(), error.message().to_owned(), Vec::new()),
            Err(Failure::Thrown(_)) => panic!("nothing was thrown"),
            Ok(_) => panic!("the compile succeeded"),
        }
    }

    // [spec:hfst:req:javascript.compile-errors/test]
    #[test]
    fn failed_compiles_carry_their_diagnostics() {
        let (code, message, said) =
            failed(regex(source("s.xre", "[a"), &none(), &given("regex", &[])));
        assert_eq!(code, "compile-failed");
        assert!(message.contains("s.xre"), "{message}");
        assert!(said.iter().any(|m| m.contains("RightBracket")), "{said:?}");
        let (code, _, said) = failed(lexc(
            &[source("a.lexc", "LEXICON Root\n< [ > # ;\n")],
            &none(),
            &given("compileLexc", &[]),
        ));
        assert_eq!(code, "compile-failed");
        assert!(!said.is_empty(), "lexc says why");
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn several_where_one_is_wanted_is_transducer_count() {
        let (code, message, _) = failed(regex(
            source("s.xre", "a\nb\n"),
            &none(),
            &given("regex", &[]),
        ));
        assert_eq!(code, "transducer-count");
        assert!(message.contains("disjunct"), "{message}");
        let union = regex(
            source("s.xre", "a\nb\n"),
            &none(),
            &given("regex", &[("disjunct", true.into())]),
        )
        .expect("their union");
        assert_eq!(paths(union.value), ["a", "b"]);
        let (code, _, _) = failed(text(
            Text::Strings,
            source("w", "cat\ndog\n"),
            &given("compileStrings", &[]),
        ));
        assert_eq!(code, "transducer-count");
        let (code, _, _) = failed(text(
            Text::Att,
            source("t.att", "0\t1\ta\ta\n1\n--\n0\t1\tb\tb\n1\n"),
            &given("parseAtt", &[]),
        ));
        assert_eq!(code, "transducer-count");
    }

    // [spec:hfst:req:javascript.compile-files/test]
    #[test]
    fn compiles_read_only_given_files() {
        let files = Files::of(vec![("w.txt".to_owned(), b"cat\ndog\n".to_vec())]);
        let read = regex(
            source("s.xre", "@txt\"w.txt\""),
            &files,
            &given("regex", &[]),
        )
        .expect("reads");
        assert_eq!(paths(read.value), ["cat", "dog"]);
        let (code, _, said) = failed(regex(
            source("s.xre", "@txt\"x.txt\""),
            &files,
            &given("regex", &[]),
        ));
        assert_eq!(code, "compile-failed");
        assert!(said.iter().any(|m| m.contains("x.txt")), "{said:?}");
    }

    // [spec:hfst:req:javascript.compile-files/test]
    #[test]
    fn pmatch_finds_a_file_beside_its_source() {
        let files = Files::of(vec![("dir/w.txt".to_owned(), b"cat\n".to_vec())]);
        let script = "Define TOP @txt\"w.txt\" ;";
        let archive = pmatch(
            source("dir/s.pmatch", script),
            &files,
            &given("compilePmatch", &[]),
        )
        .expect("beside the source");
        assert!(!archive.value.is_empty());
        let (code, _, _) = failed(pmatch(
            source(STDIN, script),
            &files,
            &given("compilePmatch", &[]),
        ));
        assert_eq!(
            code, "compile-failed",
            "the standard input has no directory"
        );
    }

    // [spec:hfst:req:javascript.compile-files/test]
    #[test]
    fn xfst_returns_written_files_and_failed_commands() {
        let script = "regex a b ;\nsave stack out.hfst\nset quit-on-fail OFF\npop stack\npop stack\nregex c ;\nprint words\n";
        let ran = xfst(
            source("s.xfst", script),
            "",
            &none(),
            &given("compileXfst", &[]),
        )
        .expect("runs");
        assert!(ran.value.output.ends_with("c\n"), "{:?}", ran.value.output);
        let [(name, bytes)] = &ran.value.written[..] else {
            panic!("one file written: {:?}", ran.value.written.len());
        };
        assert_eq!(name, "out.hfst");
        let saved = embed::read_transducers(bytes).expect("a stream");
        assert_eq!(saved.len(), 1);
        assert!(
            ran.diagnostics
                .iter()
                .any(|d| d.severity() == Severity::Error && d.message().contains("empty stack")),
            "the failed pop is among the diagnostics"
        );
        let foma = xfst(
            source("s.xfst", "regex a ;\n"),
            "",
            &none(),
            &given("compileXfst", &[("format", "foma".into())]),
        )
        .expect("foma");
        assert_eq!(foma.value.stack.len(), 1);
        assert_eq!(foma.value.stack[0].format(), Format::Foma);
        let refused = xfst(
            source("s.xfst", "regex a ;\n"),
            "",
            &none(),
            &given(
                "compileXfst",
                &[("format", "optimized-lookup-weighted".into())],
            ),
        );
        assert_eq!(
            failed(refused).0,
            "invalid-argument",
            "xfst works in an algebra type"
        );
    }

    // [spec:hfst:req:javascript.compile-errors/test]
    #[test]
    fn a_compile_that_succeeds_keeps_its_warnings() {
        let lines = "cat\ndog\t-1\n";
        let compiled = text(
            Text::Strings,
            source("w.txt", lines),
            &given("compileStrings", &[("disjunctStrings", true.into())]),
        )
        .expect("compiles");
        let warning = compiled
            .diagnostics
            .iter()
            .find(|d| d.severity() == Severity::Warning)
            .expect("a negative weight warns");
        let span = warning.primary_span().expect("on its line");
        assert_eq!(&lines[span], "dog\t-1");
        let quiet = text(
            Text::Strings,
            source("w.txt", lines),
            &given(
                "compileStrings",
                &[
                    ("disjunctStrings", true.into()),
                    ("warnNegativeWeights", false.into()),
                ],
            ),
        )
        .expect("compiles");
        assert!(quiet.diagnostics.is_empty(), "the warning turned off");
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn each_compiler_takes_its_tools_options() {
        let foma = regex(
            source("s.xre", "a"),
            &none(),
            &given("regex", &[("format", "foma".into())]),
        )
        .expect("foma");
        assert_eq!(foma.value.format(), Format::Foma);
        let rules = twolc(
            source("<twolc>", "Alphabet a b ;\nRules\n\"R1\"\na:b <=> _ # ;\n"),
            &given("compileTwolc", &[]),
        )
        .expect("rules");
        assert_eq!(rules.value.len(), 1);
        assert_eq!(
            rules.value[0].name(),
            "\"R1\"",
            "named as twolc names a rule"
        );
        let refused = twolc(
            source("<twolc>", "Alphabet a ;\n"),
            &given(
                "compileTwolc",
                &[("format", "optimized-lookup-weighted".into())],
            ),
        );
        assert_eq!(failed(refused).0, "invalid-argument");
        let both = given(
            "compileLexc",
            &[("quiet", true.into()), ("verbose", true.into())],
        );
        let lexicon = [source("a.lexc", "LEXICON Root\ncat # ;\n")];
        assert_eq!(failed(lexc(&lexicon, &none(), &both)).0, "invalid-argument");
        let both = given(
            "compileStrings",
            &[("log", true.into()), ("log10", true.into())],
        );
        assert_eq!(
            failed(text(Text::Strings, source("w", "a\n"), &both)).0,
            "invalid-argument"
        );
        let pairs = text(
            Text::Strings,
            source("w", "c:d a t\n"),
            &given(
                "compileStrings",
                &[("pairstrings", true.into()), ("hasSpaces", true.into())],
            ),
        )
        .expect("pair strings");
        assert_eq!(paths(pairs.value), ["cat:dat"]);
        let multichar = text(
            Text::Strings,
            source("w", "ab+N\n"),
            &given(
                "compileStrings",
                &[("multicharSymbols", Value::List(vec!["+N".into()]))],
            ),
        )
        .expect("a multichar symbol");
        assert_eq!(multichar.value.summarize(false).expect("summary").arcs, 3);
        let unknown = regex(
            source("s.xre", "a"),
            &none(),
            &given("regex", &[("semicolons", true.into())]),
        );
        let (code, message, _) = failed(unknown);
        assert_eq!(code, "invalid-argument");
        assert!(message.contains("semicolons"), "{message}");
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn text_reads_back_what_the_writers_write() {
        let t = text(
            Text::Att,
            source("t.att", "0\t1\ta\tb\t0.5\n1\t0\n"),
            &given("parseAtt", &[]),
        )
        .expect("AT&T");
        assert_eq!(
            t.value.name(),
            "text(t.att)",
            "named after its source, as txt2fst names it"
        );
        let written = t
            .value
            .to_text(&hfst_embed::compile::TextOptions {
                format: hfst_embed::compile::TextFormat::Prolog,
                ..Default::default()
            })
            .expect("Prolog");
        let written = String::from_utf8(written).expect("UTF-8");
        let back = text(
            Text::Prolog,
            source("t.prolog", &written),
            &given("parseProlog", &[]),
        )
        .expect("Prolog");
        assert_eq!(paths(back.value), ["a:b"]);
    }
}
