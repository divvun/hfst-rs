//! hfst-embed's compile surface against the tools. For the inputs the
//! repository's tests give the tools, each operation written to bytes is what
//! the tool writes, and each query reports what the tool prints.

mod common;

use std::path::{Path, PathBuf};

use common::run_bytes as run;
use hfst_embed::compile::*;
use hfst_embed::format::Format;

/// What `hfst ARGS` writes, which must succeed.
fn tool(args: &[&str], stdin: &[u8]) -> Vec<u8> {
    let (ok, out) = run(args, stdin);
    assert!(ok, "hfst {args:?} failed");
    out
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name)
}

/// A directory of the transducers the tools build from the repository's test
/// sources, by name: each `js/test/fixtures/*.regexp` in tropical OpenFst
/// (`NAME`), foma (`NAME.foma`) and weighted optimized lookup (`NAME.olw`),
/// and the AT&T and lexc fixtures of the library's tests.
struct Inputs {
    dir: tempfile::TempDir,
}

impl Inputs {
    fn new() -> Inputs {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let inputs = Inputs { dir };
        for name in ["words", "flags", "weights", "identity", "many", "nonascii"] {
            let source = fixture(&format!("js/test/fixtures/{name}.regexp"));
            let path = inputs.path(name);
            tool(
                &["regexp2fst", source.to_str().expect("utf8"), "-o", &path],
                b"",
            );
            for (suffix, format) in [("foma", "foma"), ("olw", "olw"), ("olu", "olu")] {
                let to = inputs.path(&format!("{name}.{suffix}"));
                tool(&["fst2fst", "-f", format, &path, "-o", &to], b"");
            }
        }
        for (name, regex) in [
            ("abcd", "a b c d"),
            ("cyc", "[a:b c]* d"),
            ("animals", "{cat}|{dog}|{cats}"),
            ("flagsx", "{cat} \"@P.X.Y@\" | {dog} \"@R.X.Y@\""),
            ("rule", "[c|d|f] a ? s"),
        ] {
            let path = inputs.path(name);
            tool(
                &["regexp2fst", "-o", &path],
                format!("{regex}\n").as_bytes(),
            );
        }
        let att = fixture("crates/hfst/tests/fixtures/test_transducers.att");
        tool(
            &[
                "txt2fst",
                att.to_str().expect("utf8"),
                "-o",
                &inputs.path("att"),
            ],
            b"",
        );
        let lexc = fixture("crates/hfst/tests/fixtures/test_lexc.lexc");
        tool(
            &[
                "lexc",
                lexc.to_str().expect("utf8"),
                "-o",
                &inputs.path("lexc"),
            ],
            b"",
        );
        inputs
    }

    fn path(&self, name: &str) -> String {
        self.dir
            .path()
            .join(format!("{name}.hfst"))
            .to_str()
            .expect("utf8")
            .to_string()
    }

    fn bytes(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.path(name)).expect("the input was written")
    }

    /// The transducers of the input `name`.
    fn all(&self, name: &str) -> Vec<Transducer> {
        read_transducers(&self.bytes(name)).expect("the input reads")
    }

    fn one(&self, name: &str) -> Transducer {
        let mut all = self.all(name);
        assert_eq!(all.len(), 1, "{name} holds one transducer");
        all.remove(0)
    }
}

fn written(t: &Transducer) -> Vec<u8> {
    t.to_bytes(&WriteOptions::default())
        .expect("the transducer writes")
}

// [spec:hfst:req:embed.transducer/test]
// Every type reads, and writes back to the bytes it was read from, with and
// without a header; fst2fst's conversions are what the tool writes.
#[test]
fn reads_and_writes_every_type_as_fst2fst_does() {
    let inputs = Inputs::new();
    for name in ["words", "words.foma", "words.olw", "words.olu"] {
        let bytes = inputs.bytes(name);
        let t = inputs.one(name);
        assert_eq!(written(&t), bytes, "{name} round-trips");
        for (format, flag) in [
            (Format::OpenFstTropical, "openfst-tropical"),
            (Format::Foma, "foma"),
            (Format::OptimizedLookupWeighted, "olw"),
            (Format::OptimizedLookupUnweighted, "olu"),
        ] {
            if name == "words.olu" && format != Format::OptimizedLookupUnweighted {
                // The tool converts unweighted optimized lookup only to its
                // own type.
                continue;
            }
            for header in [true, false] {
                let mut args = vec!["fst2fst", "-f", flag];
                if !header {
                    args.push("-b");
                }
                let path = inputs.path(name);
                args.push(&path);
                let options = WriteOptions {
                    format: Some(format),
                    header,
                    quick: false,
                };
                assert_eq!(
                    t.to_bytes(&options).expect("writes"),
                    tool(&args, b""),
                    "{name} as {flag}, header {header}"
                );
                // The tools do not read a native foma file, which is
                // gzipped, either.
                if header && name != "words.foma" {
                    let bare = WriteOptions {
                        header: false,
                        ..WriteOptions::default()
                    };
                    let read = read_transducers(&t.to_bytes(&bare).expect("writes bare"));
                    assert!(read.is_ok(), "{name} reads back without a header");
                }
            }
        }
    }
}

// [spec:hfst:req:embed.transducer/test]
// A copy changes on its own, and an operation leaves another transducer
// that shares nothing with it as it was.
#[test]
fn copies_are_independent() {
    let inputs = Inputs::new();
    let original = inputs.one("words");
    let mut copy = original.copy().expect("copies");
    copy.invert().expect("inverts");
    assert_eq!(written(&original), inputs.bytes("words"));
    assert_ne!(written(&copy), written(&original));
}

/// A unary operation and the tool arguments that do the same.
type Unary = (
    &'static [&'static str],
    fn(&mut Transducer) -> Result<(), hfst_embed::Error>,
);

const UNARY: &[Unary] = &[
    (&["determinize"], |t| {
        t.determinize(&DeterminizeOptions::default())
    }),
    (&["determinize", "-E"], |t| {
        t.determinize(&DeterminizeOptions {
            encode_weights: true,
        })
    }),
    (&["minimize"], |t| t.minimize(&MinimizeOptions::default())),
    (&["minimize", "-E"], |t| {
        t.minimize(&MinimizeOptions {
            encode_weights: true,
        })
    }),
    (&["invert"], |t| t.invert()),
    (&["reverse"], |t| t.reverse()),
    (&["remove-epsilons"], |t| t.remove_epsilons()),
    (&["prune-alphabet"], |t| {
        t.prune_alphabet(&PruneAlphabetOptions::default())
    }),
    (&["prune-alphabet", "-f"], |t| {
        t.prune_alphabet(&PruneAlphabetOptions { force: true })
    }),
    (&["project", "-p", "input"], |t| {
        t.project(&ProjectOptions {
            project: Tape::Input,
        })
    }),
    (&["project"], |t| t.project(&ProjectOptions::default())),
    (&["push-weights", "-p", "initial"], |t| {
        t.push_weights(&PushOptions {
            push: PushDirection::Initial,
        })
    }),
    (&["push-weights"], |t| {
        t.push_weights(&PushOptions::default())
    }),
    (&["push-labels", "-p", "start"], |t| {
        t.push_labels(&PushOptions {
            push: PushDirection::Initial,
        })
    }),
    (&["push-labels"], |t| t.push_labels(&PushOptions::default())),
    (&["realign"], |t| t.realign()),
    (&["repeat", "-f", "1", "-t", "3"], |t| {
        t.repeat(&RepeatOptions {
            from: Count::Finite(1),
            to: Count::Finite(3),
        })
    }),
    (&["repeat", "-f", "2"], |t| {
        t.repeat(&RepeatOptions {
            from: Count::Finite(2),
            to: Count::Infinite,
        })
    }),
    (&["repeat", "-f", "inf"], |t| {
        t.repeat(&RepeatOptions {
            from: Count::Infinite,
            to: Count::Infinite,
        })
    }),
    (&["insert-freely", "-a", "x:y"], |t| {
        t.insert_freely(&InsertFreelyOptions {
            symbol_pair: "x:y".to_string(),
            harmonise: false,
        })
    }),
    (&["eliminate-flags"], |t| {
        t.eliminate_flags(&EliminateFlagsOptions::default())
    }),
    (&["preprocess-for-optimized-lookup-format"], |t| {
        t.preprocess_for_optimized_lookup_format()
    }),
    (&["affix-guessify"], |t| {
        t.affix_guessify(&AffixGuessifyOptions::default())
    }),
    (&["affix-guessify", "-D", "prefix", "-w", "2"], |t| {
        t.affix_guessify(&AffixGuessifyOptions {
            direction: GuessDirection::GuessPrefix,
            weight: 2.0,
        })
    }),
    (&["kill-paths", "-S", "c"], |t| {
        t.kill_paths(&KillPathsOptions {
            symbol: Some("c".to_string()),
            tsv: None,
        })
    }),
    (&["reweight", "-a", "1", "-b", "2"], |t| {
        t.reweight(&ReweightOptions {
            addition: 1.0,
            multiplier: 2.0,
            ..ReweightOptions::default()
        })
    }),
    (&["reweight", "-S", "a", "-F", "sqrt"], |t| {
        t.reweight(&ReweightOptions {
            symbol: Some("a".to_string()),
            function: Some("sqrt".to_string()),
            ..ReweightOptions::default()
        })
    }),
    (
        &[
            "edit-metadata",
            "-a",
            "foo=bar",
            "-a",
            "baz=quux",
            "-t",
            "2",
        ],
        |t| {
            t.edit_metadata(&EditMetadataOptions {
                add: vec![
                    ("foo".to_string(), "bar".to_string()),
                    ("baz".to_string(), "quux".to_string()),
                ],
                truncate_length: 2,
            })
        },
    ),
    (&["name", "-n", "renamed"], |t| {
        t.set_name(&NameOptions {
            name: "renamed".to_string(),
            truncate_length: 0,
        });
        Ok(())
    }),
    (&["expand-equivalences", "-f", "a", "-t", "A"], |t| {
        t.expand_equivalences(&ExpandEquivalencesOptions {
            from: Some("a".to_string()),
            to: Some("A".to_string()),
            ..ExpandEquivalencesOptions::default()
        })
    }),
    (&["substitute", "-f", "a", "-t", "A"], |t| {
        t.substitute(&SubstituteOptions {
            from_label: Some("a".to_string()),
            to_label: Some("A".to_string()),
            ..SubstituteOptions::default()
        })
    }),
    (&["substitute", "-f", "a", "-t", "A", "-9"], |t| {
        t.substitute(&SubstituteOptions {
            from_label: Some("a".to_string()),
            to_label: Some("A".to_string()),
            compose: true,
            ..SubstituteOptions::default()
        })
    }),
];

// [spec:hfst:req:embed.operations/test]
// [spec:hfst:req:embed.tools-in-library/test]
// Each operation writes what its tool writes, in each algebra type.
#[test]
fn unary_operations_write_what_the_tools_write() {
    let inputs = Inputs::new();
    for name in ["words", "weights", "flags", "words.foma", "att", "lexc"] {
        for (args, op) in UNARY {
            let mut all = inputs.all(name);
            if all.len() > 1 && args[0] == "insert-freely" {
                // The tool inserts into the first transducer of a stream
                // only.
                continue;
            }
            let path = inputs.path(name);
            let mut argv: Vec<&str> = args.to_vec();
            argv.push(&path);
            let (ok, expected) = run(&argv, b"");
            let mut bytes = Vec::new();
            let mut result = Ok(());
            for t in all.iter_mut() {
                result = op(t);
                if result.is_err() {
                    break;
                }
                bytes.extend(written(t));
            }
            assert_eq!(result.is_ok(), ok, "hfst {argv:?}: {result:?}");
            if ok {
                assert_eq!(bytes, expected, "hfst {argv:?}");
            }
        }
    }
}

// [spec:hfst:req:embed.operations/test]
// Optimized lookup is refused by the operations as the tools refuse it.
#[test]
fn operations_refuse_optimized_lookup() {
    let inputs = Inputs::new();
    let mut t = inputs.one("words.olw");
    let error = t.invert().expect_err("refused");
    assert_eq!(error.code().name(), "type-mismatch");
    let (ok, _) = run(&["invert", &inputs.path("words.olw")], b"");
    assert!(!ok);
}

/// A binary operation and the tool arguments that do the same.
type Binary = (
    &'static [&'static str],
    fn(&mut Transducer, Operand<'_>) -> Result<(), hfst_embed::Error>,
);

const BINARY: &[Binary] = &[
    (&["compose"], |t, o| {
        t.compose(o, &ComposeOptions::default())
    }),
    (&["compose", "-F"], |t, o| {
        t.compose(
            o,
            &ComposeOptions {
                harmonize_flags: true,
                ..ComposeOptions::default()
            },
        )
    }),
    (&["compose", "-H"], |t, o| {
        t.compose(
            o,
            &ComposeOptions {
                do_not_harmonize: true,
                ..ComposeOptions::default()
            },
        )
    }),
    (&["compose", "--xerox-composition=yes"], |t, o| {
        t.compose(
            o,
            &ComposeOptions {
                xerox_composition: true,
                ..ComposeOptions::default()
            },
        )
    }),
    (&["concatenate"], |t, o| {
        t.concatenate(o, &BinaryOptions::default())
    }),
    (&["concatenate", "-F"], |t, o| {
        t.concatenate(
            o,
            &BinaryOptions {
                harmonize_flags: true,
                ..BinaryOptions::default()
            },
        )
    }),
    (&["conjunct"], |t, o| {
        t.conjunct(o, &BinaryOptions::default())
    }),
    (&["conjunct", "-F"], |t, o| {
        t.conjunct(
            o,
            &BinaryOptions {
                harmonize_flags: true,
                ..BinaryOptions::default()
            },
        )
    }),
    (&["disjunct"], |t, o| {
        t.disjunct(o, &DisjunctOptions::default())
    }),
    (&["priority-disjunct"], |t, o| {
        t.priority_disjunct(o, &ConvertOptions::default())
    }),
    (&["subtract"], |t, o| {
        t.subtract(o, &BinaryOptions::default())
    }),
    (&["subtract", "-F"], |t, o| {
        t.subtract(
            o,
            &BinaryOptions {
                harmonize_flags: true,
                ..BinaryOptions::default()
            },
        )
    }),
];

// [spec:hfst:req:embed.operations/test]
// Each binary operation writes what its tool writes, converting operands of
// different types as the tool converts them.
#[test]
fn binary_operations_write_what_the_tools_write() {
    let inputs = Inputs::new();
    let pairs = [
        ("words", "many"),
        ("animals", "words"),
        ("flags", "flagsx"),
        ("flags", "flags"),
        ("words", "words.foma"),
        ("words.foma", "many"),
        ("abcd", "animals"),
    ];
    for (first, second) in pairs {
        for (args, op) in BINARY {
            let (a, b) = (inputs.path(first), inputs.path(second));
            let mut argv: Vec<&str> = args.to_vec();
            argv.push(&a);
            argv.push(&b);
            let (ok, expected) = run(&argv, b"");
            let mut t = inputs.one(first);
            let other = inputs.one(second);
            let result = op(&mut t, Operand::Other(&other));
            assert_eq!(result.is_ok(), ok, "hfst {argv:?}: {result:?}");
            if ok {
                assert_eq!(written(&t), expected, "hfst {argv:?}");
            }
        }
    }
}

// [spec:hfst:req:embed.operations/test]
// Shuffling two automata is what `hfst shuffle` writes.
#[test]
fn shuffle_writes_what_the_tool_writes() {
    let inputs = Inputs::new();
    let expected = tool(
        &["shuffle", &inputs.path("abcd"), &inputs.path("animals")],
        b"",
    );
    let mut t = inputs.one("abcd");
    let other = inputs.one("animals");
    t.shuffle(Operand::Other(&other), &ConvertOptions::default())
        .expect("shuffles");
    assert_eq!(written(&t), expected);
}

// [spec:hfst:req:embed.transducer/test]
// [spec:hfst:req:embed.operations/test]
// An operation whose operand is the transducer itself reads it as it was,
// as the tool does with the same file given twice.
#[test]
fn an_operand_can_be_the_transducer_itself() {
    let inputs = Inputs::new();
    for (args, op) in BINARY {
        let a = inputs.path("words");
        let mut argv: Vec<&str> = args.to_vec();
        argv.push(&a);
        argv.push(&a);
        let (ok, expected) = run(&argv, b"");
        let mut t = inputs.one("words");
        let result = op(&mut t, Operand::Itself);
        assert_eq!(result.is_ok(), ok, "hfst {argv:?}");
        if ok {
            assert_eq!(written(&t), expected, "hfst {argv:?}");
        }
    }
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn compose_intersect_writes_what_the_tool_writes() {
    let inputs = Inputs::new();
    let cases: &[(&[&str], ComposeIntersectOptions)] = &[
        (&[], ComposeIntersectOptions::default()),
        (
            &["-f"],
            ComposeIntersectOptions {
                fast: true,
                ..ComposeIntersectOptions::default()
            },
        ),
        (
            &["-I"],
            ComposeIntersectOptions {
                invert: true,
                ..ComposeIntersectOptions::default()
            },
        ),
        (
            &["-a", "-e"],
            ComposeIntersectOptions {
                harmonize: true,
                encode_weights: true,
                ..ComposeIntersectOptions::default()
            },
        ),
    ];
    for (args, options) in cases {
        let (lexicon, rules) = (inputs.path("lexc"), inputs.path("rule"));
        let mut argv = vec!["compose-intersect"];
        argv.extend_from_slice(args);
        argv.push(&lexicon);
        argv.push(&rules);
        let expected = tool(&argv, b"");
        let mut t = inputs.one("lexc");
        let rule = inputs.one("rule");
        let options = ComposeIntersectOptions {
            lexicon_name: lexicon.clone(),
            rules_name: rules.clone(),
            ..options.clone()
        };
        t.compose_intersect(&[&rule], &options).expect("composes");
        assert_eq!(written(&t), expected, "hfst {argv:?}");
    }
}

// [spec:hfst:req:embed.operations/test]
// Substituting with a transducer and from a relabel file names the result
// after the files, as the tool does.
#[test]
fn substitute_by_net_and_file_matches_the_tool() {
    let inputs = Inputs::new();
    let (words, abcd) = (inputs.path("words"), inputs.path("abcd"));
    let expected = tool(&["substitute", "-f", "c", "-T", &abcd, &words], b"");
    let mut t = inputs.one("words");
    let replacement = inputs.one("abcd");
    t.substitute(&SubstituteOptions {
        from_label: Some("c".to_string()),
        to_transducer: Some(NamedOperand {
            transducer: &replacement,
            name: &abcd,
        }),
        ..SubstituteOptions::default()
    })
    .expect("substitutes");
    assert_eq!(written(&t), expected);

    let relabel = inputs.dir.path().join("relabel.tsv");
    let text = "c\tC\n# a comment\nd\tD\n";
    std::fs::write(&relabel, text).expect("write the relabel file");
    let relabel = relabel.to_str().expect("utf8").to_string();
    for in_order in [false, true] {
        let mut argv = vec!["substitute", "-F", &relabel];
        if in_order {
            argv.push("-R");
        }
        argv.push(&words);
        let expected = tool(&argv, b"");
        let mut t = inputs.one("words");
        t.substitute(&SubstituteOptions {
            from_file: Some(NamedText {
                name: relabel.clone(),
                text: text.to_string(),
            }),
            in_order,
            ..SubstituteOptions::default()
        })
        .expect("relabels");
        assert_eq!(written(&t), expected, "in order {in_order}");
    }
}

// [spec:hfst:req:embed.operations/test]
// guessify's guesser and generator are the two transducers the tool writes.
#[test]
fn guessify_writes_what_the_tool_writes() {
    let inputs = Inputs::new();
    for (args, options) in [
        (vec![], GuessifyOptions::default()),
        (
            vec!["-G", "-p", "2"],
            GuessifyOptions {
                default_penalty: 2.0,
                do_not_compile_generator: true,
            },
        ),
    ] {
        let path = inputs.path("words");
        let mut argv = vec!["guessify"];
        argv.extend(args);
        argv.push(&path);
        let expected = tool(&argv, b"");
        let mut t = inputs.one("words");
        let generator = t.guessify(&options).expect("guessifies");
        let mut bytes = written(&t);
        if let Some(generator) = generator {
            bytes.extend(written(&generator));
        }
        assert_eq!(bytes, expected, "hfst {argv:?}");
    }
}

/// The lines `hfst fst2strings -w` prints for `paths`.
fn strings_lines(paths: &[ExtractedPath]) -> String {
    paths
        .iter()
        .map(|p| {
            let identity = p.pairs.iter().all(|(i, o)| i == o);
            if identity {
                format!("{}\t{}\n", p.input, p.weight)
            } else {
                format!("{}:{}\t{}\n", p.input, p.output, p.weight)
            }
        })
        .collect()
}

// [spec:hfst:req:embed.operations/test]
// extract_paths finds the paths fst2strings prints, in its order.
#[test]
fn extract_paths_finds_what_fst2strings_prints() {
    let inputs = Inputs::new();
    let cases: Vec<(Vec<&str>, ExtractPathsOptions)> = vec![
        (vec![], ExtractPathsOptions::default()),
        (
            vec!["-n", "3"],
            ExtractPathsOptions {
                max_strings: 3,
                ..ExtractPathsOptions::default()
            },
        ),
        (
            vec!["-N", "2"],
            ExtractPathsOptions {
                nbest: 2,
                ..ExtractPathsOptions::default()
            },
        ),
        (
            vec!["-b", "0.5"],
            ExtractPathsOptions {
                beam: Some(0.5),
                ..ExtractPathsOptions::default()
            },
        ),
        (
            vec!["-X", "obey-flags"],
            ExtractPathsOptions {
                obey_flags: true,
                ..ExtractPathsOptions::default()
            },
        ),
        (
            vec!["-p", "c"],
            ExtractPathsOptions {
                in_prefix: "c".to_string(),
                ..ExtractPathsOptions::default()
            },
        ),
        (
            vec!["-l", "3"],
            ExtractPathsOptions {
                max_in_length: 3,
                ..ExtractPathsOptions::default()
            },
        ),
    ];
    for name in ["words", "weights", "flags", "words.foma", "words.olw"] {
        for (args, options) in &cases {
            let path = inputs.path(name);
            let mut argv = vec!["fst2strings", "-w"];
            argv.extend(args.iter().copied());
            argv.push(&path);
            let (ok, expected) = run(&argv, b"");
            let found = inputs.one(name).extract_paths(options);
            assert_eq!(found.is_ok(), ok, "hfst {argv:?}: {found:?}");
            if let Ok(paths) = found {
                assert_eq!(
                    strings_lines(&paths),
                    String::from_utf8_lossy(&expected),
                    "hfst {argv:?}"
                );
            }
        }
    }
    let cyclic = inputs.one("cyc");
    assert_eq!(
        cyclic
            .extract_paths(&ExtractPathsOptions::default())
            .expect_err("cyclic")
            .code()
            .name(),
        "cyclic"
    );
}

/// The Xerox lines `hfst lookup` prints for `reports`.
fn lookup_lines(reports: &[LookupReport]) -> String {
    let mut text = String::new();
    for report in reports {
        match &report.outcome {
            LookupOutcome::OutsideAlphabet => {
                text.push_str(&format!("{0}\t{0}+?\tinf\n\n", report.input));
            }
            LookupOutcome::Found { results, .. } if results.is_empty() => {
                text.push_str(&format!("{0}\t{0}+?\tinf\n\n", report.input));
            }
            LookupOutcome::Found { infinite, results } => {
                for r in results {
                    text.push_str(&format!(
                        "{}\t{}\t{:.6}\n",
                        report.input, r.output, r.weight
                    ));
                }
                if *infinite {
                    text.push_str(&format!("{}\t[...cyclic...]\n\n", report.input));
                } else {
                    text.push('\n');
                }
            }
        }
    }
    text
}

const WORDS: &[&str] = &[
    "cat", "mouse", "ca", "mice", "dog", "talo", "xyz", "cats", "",
];

// [spec:hfst:req:embed.operations/test]
// lookup_report finds what `hfst lookup` prints, in every type.
#[test]
fn lookup_reports_what_the_tool_prints() {
    let inputs = Inputs::new();
    let stdin = WORDS.join("\n") + "\n";
    let cases: Vec<(Vec<&str>, LookupOptions)> = vec![
        (vec![], LookupOptions::default()),
        (
            vec!["-b", "0.6"],
            LookupOptions {
                beam: Some(0.6),
                ..LookupOptions::default()
            },
        ),
        (
            vec!["-X", "show-flags"],
            LookupOptions {
                show_flags: true,
                ..LookupOptions::default()
            },
        ),
    ];
    for name in [
        "words",
        "weights",
        "flags",
        "words.foma",
        "words.olw",
        "words.olu",
        "flags.olw",
    ] {
        for (args, options) in &cases {
            let path = inputs.path(name);
            let mut argv = vec!["lookup", "-q"];
            argv.extend(args.iter().copied());
            argv.push(&path);
            let expected = tool(&argv, stdin.as_bytes());
            let t = inputs.one(name);
            let reports = lookup_report(&[&t], WORDS, options).expect("looks up");
            assert_eq!(
                lookup_lines(&reports),
                String::from_utf8_lossy(&expected),
                "hfst {argv:?}"
            );
        }
    }
}

// [spec:hfst:req:embed.operations/test]
// flookup_report finds what `hfst flookup` prints.
#[test]
fn flookup_reports_what_the_tool_prints() {
    let inputs = Inputs::new();
    let stdin = WORDS.join("\n") + "\n";
    for name in ["words", "weights", "words.foma"] {
        for (args, invert) in [(vec![], false), (vec!["-R"], true)] {
            let path = inputs.path(name);
            let mut argv = vec!["flookup", "-q"];
            argv.extend(args.iter().copied());
            argv.push(&path);
            let expected = tool(&argv, stdin.as_bytes());
            let t = inputs.one(name);
            let options = FlookupOptions {
                invert,
                ..FlookupOptions::default()
            };
            let reports = flookup_report(&[&t], WORDS, &options).expect("looks up");
            let mut text = String::new();
            for report in &reports {
                match &report.outcome {
                    LookupOutcome::Found { results, .. } if !results.is_empty() => {
                        for r in results {
                            text.push_str(&format!(
                                "{}\t{}\t{:.6}\n",
                                report.input, r.output, r.weight
                            ));
                        }
                        text.push('\n');
                    }
                    LookupOutcome::OutsideAlphabet | LookupOutcome::Found { .. } => {
                        text.push_str(&format!("{0}\t{0}+?\tinf\n\n", report.input));
                    }
                }
            }
            assert_eq!(text, String::from_utf8_lossy(&expected), "hfst {argv:?}");
        }
    }
}

// [spec:hfst:req:embed.operations/test]
// optimized_lookup_report finds what `hfst optimized-lookup` prints.
#[test]
fn optimized_lookup_reports_what_the_tool_prints() {
    let inputs = Inputs::new();
    let stdin = WORDS.join("\n") + "\n";
    let cases: Vec<(Vec<&str>, OptimizedLookupOptions)> = vec![
        (vec!["-q"], OptimizedLookupOptions::default()),
        (
            vec!["-q", "-u"],
            OptimizedLookupOptions {
                unique: true,
                ..OptimizedLookupOptions::default()
            },
        ),
        (
            vec!["-q", "-n", "1"],
            OptimizedLookupOptions {
                max_analyses: 1,
                ..OptimizedLookupOptions::default()
            },
        ),
        (
            vec!["-q", "-b", "0.5"],
            OptimizedLookupOptions {
                beam: 0.5,
                ..OptimizedLookupOptions::default()
            },
        ),
    ];
    for name in ["words.olw", "weights.olw", "words.olu", "flags.olw"] {
        for (args, options) in &cases {
            let path = inputs.path(name);
            let mut argv = vec!["optimized-lookup"];
            argv.extend(args.iter().copied());
            argv.push(&path);
            let expected = tool(&argv, stdin.as_bytes());
            let t = inputs.one(name);
            let weighted = t.format() == Format::OptimizedLookupWeighted;
            let reports = optimized_lookup_report(&[&t], WORDS, options).expect("looks up");
            let mut text = String::new();
            for report in &reports {
                match &report.analyses {
                    None => text.push_str(&format!("{0}\t{0}\t+?\n\n\n\n", report.input)),
                    Some(analyses) if analyses.is_empty() => {
                        let end = if weighted { "\n\n" } else { "\n\n\n\n" };
                        text.push_str(&format!("{0}\t{0}\t+?{1}", report.input, end));
                    }
                    Some(analyses) => {
                        for (output, weight) in analyses {
                            text.push_str(&format!("{}\t{}", report.input, output));
                            if weighted {
                                text.push_str(&format!("\t{}", weight));
                            }
                            text.push('\n');
                        }
                        text.push('\n');
                    }
                }
            }
            assert_eq!(text, String::from_utf8_lossy(&expected), "hfst {argv:?}");
        }
    }
}

// [spec:hfst:req:embed.operations/test]
// compare, check-alpha, dump-alphabets and summarize report what the tools
// print.
#[test]
fn reports_match_the_tools() {
    let inputs = Inputs::new();
    for (a, b) in [("words", "words"), ("words", "many"), ("flags", "flags")] {
        let (pa, pb) = (inputs.path(a), inputs.path(b));
        let (_, expected) = run(&["compare", &pa, &pb], b"");
        let equal = inputs
            .one(a)
            .compare(&inputs.one(b), &CompareOptions::default())
            .expect("compares");
        let relation = if equal { "==" } else { "!=" };
        let (na, nb) = (inputs.one(a).name(), inputs.one(b).name());
        let name = |n: String, p: &str| if n.is_empty() { p.to_string() } else { n };
        assert_eq!(
            String::from_utf8_lossy(&expected),
            format!("{} {} {}\n", name(na, &pa), relation, name(nb, &pb))
        );
    }
    let (ok, _) = run(
        &["compare", &inputs.path("words"), &inputs.path("words.foma")],
        b"",
    );
    let refused = inputs
        .one("words")
        .compare(&inputs.one("words.foma"), &CompareOptions::default());
    assert_eq!(refused.is_ok(), ok);

    for name in ["words", "lexc", "words.olw"] {
        let path = inputs.path(name);
        let expected = tool(&["dump-alphabets", &path], b"");
        let a = inputs.one(name).dump_alphabets().expect("dumps");
        let lines: String = a
            .declared
            .iter()
            .chain(&a.seen)
            .map(|s| format!("{s}\n"))
            .collect();
        assert_eq!(String::from_utf8_lossy(&expected), lines, "{name}");
    }

    let (words, many) = (inputs.one("words"), inputs.one("many"));
    let comparison = words.check_alpha(&many).expect("checks");
    let printed = String::from_utf8_lossy(&tool(
        &["check-alpha", &inputs.path("words"), &inputs.path("many")],
        b"",
    ))
    .into_owned();
    let only = |a: &[String], b: &[String]| -> Vec<String> {
        a.iter().filter(|s| !b.contains(s)).cloned().collect()
    };
    let first_only = only(&comparison.first_used, &comparison.second_used);
    let second_declared_only = only(&comparison.second_declared, &comparison.first_declared);
    assert!(comparison.mismatch);
    assert!(
        printed.contains(&format!(
            "In first {} but not in second {}:{}\n",
            words.name(),
            many.name(),
            first_only.join(", ")
        )),
        "{printed}"
    );
    assert!(
        printed.contains(&format!(
            "Second {} has but first {} does not: {}\n",
            many.name(),
            words.name(),
            second_declared_only.join(", ")
        )),
        "{printed}"
    );

    for name in ["words", "words.olw"] {
        let summary = inputs.one(name).summarize(true).expect("summarizes");
        let printed = String::from_utf8_lossy(&tool(&["summarize", "-v", &inputs.path(name)], b""))
            .into_owned();
        assert!(
            printed.contains(&format!("# of states: {}\n", summary.states)),
            "{printed}"
        );
        assert!(
            printed.contains(&format!("# of arcs: {}\n", summary.arcs)),
            "{printed}"
        );
    }
}

// [spec:hfst:req:embed.operations/test]
// grep reports the lines `hfst grep` prints.
#[test]
fn grep_reports_what_the_tool_prints() {
    let text = "the cat sat\nno match here\n\ncats and dogs\n";
    // A Xerox expression, the default, reads `cat` as one symbol.
    for (args, pattern, options) in [
        (vec!["cat"], "cat", GrepOptions::default()),
        (vec!["{cat}"], "{cat}", GrepOptions::default()),
        (
            vec!["--invert-match", "{cat}"],
            "{cat}",
            GrepOptions {
                invert_match: true,
                ..GrepOptions::default()
            },
        ),
        (
            vec!["-F", "cat"],
            "cat",
            GrepOptions {
                fixed_strings: true,
                ..GrepOptions::default()
            },
        ),
        (
            vec!["-F", "-w", "cat"],
            "cat",
            GrepOptions {
                fixed_strings: true,
                word_regexp: true,
                ..GrepOptions::default()
            },
        ),
    ] {
        let mut argv = vec!["grep"];
        argv.extend(args);
        let (_, expected) = common::run(&argv, text.as_bytes());
        let report = grep(pattern, text, &options).expect("greps");
        let printed: String = report
            .lines
            .iter()
            .map(|l| format!("{}\n", l.text))
            .collect();
        assert_eq!(printed, expected, "hfst {argv:?}");
    }
}

// [spec:hfst:req:embed.operations/test]
// pair_test reports the verdicts `hfst pair-test` gives.
#[test]
fn pair_test_reports_what_the_tool_reports() {
    let dir = tempfile::tempdir().expect("a directory");
    let rules = dir.path().join("rules.hfst");
    let rules_path = rules.to_str().expect("utf8");
    let grammar = "Alphabet a b ;\nRules\n\"R1\"\na:b <=> _ # ;\n";
    tool(&["twolc", "-o", rules_path], grammar.as_bytes());
    let all = read_transducers(&std::fs::read(&rules).expect("rules")).expect("reads");
    let refs: Vec<&Transducer> = all.iter().collect();
    for (tests, negative) in [("a a:b\n", false), ("a:b a\n", false), ("a:b a\n", true)] {
        let mut argv = vec!["pair-test", rules_path];
        if negative {
            argv.push("-N");
        }
        let (ok, printed) = run(&argv, tests.as_bytes());
        let results = pair_test(
            &refs,
            tests,
            &PairTestOptions {
                negative_test: negative,
                xerox_mode: false,
            },
        )
        .expect("tests");
        assert_eq!(
            results.iter().any(|r| r.failed),
            !ok,
            "{tests:?} {negative}"
        );
        let printed = String::from_utf8_lossy(&printed);
        for result in &results {
            for failure in &result.failures {
                assert!(
                    printed.contains(&format!("Rule {} fails:", failure.rule)),
                    "{printed}"
                );
            }
        }
    }
}

// [spec:hfst:req:embed.operations/test]
// pmatch marks up and locates what `hfst pmatch` prints.
#[test]
fn pmatch_reports_what_the_tool_prints() {
    let dir = tempfile::tempdir().expect("a directory");
    let archive = dir.path().join("animals.pmhfst");
    let archive = archive.to_str().expect("utf8");
    let source = fixture("js/test/fixtures/animals.pmatch");
    tool(
        &["pmatch2fst", source.to_str().expect("utf8"), "-o", archive],
        b"",
    );
    let bytes = std::fs::read(archive).expect("the archive");
    let text = "the cat and the dog\nsaw a horse\n\nand a cat\n";
    for (args, options) in [
        (vec![], PmatchOptions::default()),
        (
            vec!["--newline"],
            PmatchOptions {
                blankline_separated: false,
                ..PmatchOptions::default()
            },
        ),
        (
            vec!["--locate"],
            PmatchOptions {
                locate: true,
                ..PmatchOptions::default()
            },
        ),
    ] {
        let mut argv = vec!["pmatch"];
        argv.extend(args.iter().copied());
        argv.push(archive);
        let expected = tool(&argv, text.as_bytes());
        let mut container =
            hfst_embed::tokenise::read_tokeniser(&bytes, &PACKAGE).expect("the container reads");
        let mut printed = String::new();
        for (_, result) in pmatch(&mut container, text, &options) {
            match result {
                PmatchResult::Marked(marked) => {
                    printed.push_str(&marked);
                    printed.push('\n');
                    if options.blankline_separated {
                        printed.push('\n');
                    }
                }
                PmatchResult::Located(locations) => {
                    for l in &locations {
                        printed.push_str(&format!(
                            "{}|{}|{}|{}|{}\n",
                            l.start, l.length, l.output, l.tag, l.weight
                        ));
                    }
                    if !locations.is_empty() {
                        printed.push('\n');
                    }
                }
            }
        }
        assert_eq!(printed, String::from_utf8_lossy(&expected), "hfst {argv:?}");
    }
}

const PACKAGE: hfst_embed::Package = hfst_embed::Package {
    name: "the parity tests",
    read_transducers: "read_transducers",
    read_optimised_transducer: "read_optimised_transducer",
    read_dhfst: "read_dhfst",
};

// [spec:hfst:req:embed.operations/test]
// guess finds the guesses `hfst guess` prints.
#[test]
fn guess_reports_what_the_tool_prints() {
    let inputs = Inputs::new();
    let guesser_path = inputs.dir.path().join("guesser.hfst");
    let guesser_path = guesser_path.to_str().expect("utf8");
    tool(
        &["guessify", &inputs.path("words"), "-o", guesser_path],
        b"",
    );
    let stdin = "cat\nmouses\nxyz\n";
    let expected = tool(&["guess", guesser_path], stdin.as_bytes());
    let all = read_transducers(&std::fs::read(guesser_path).expect("guesser")).expect("reads");
    let words: Vec<&str> = stdin.lines().collect();
    let reports =
        guess(&all[0], all.get(1), None, &words, &GuessOptions::default()).expect("guesses");
    let mut printed = String::new();
    for (word, report) in words.iter().zip(&reports) {
        let GuessReport::Guesses(guesses) = report else {
            panic!("no model forms were asked for");
        };
        for g in guesses {
            printed.push_str(&format!("{word}\t{g}\n"));
        }
        printed.push('\n');
    }
    assert_eq!(printed, String::from_utf8_lossy(&expected));
}

// [spec:hfst:req:embed.operations/test]
// A transducer that is not a pmatch container tokenises as `hfst tokenize`
// tokenises with it.
#[test]
fn a_naive_tokeniser_formats_what_the_tool_prints() {
    let inputs = Inputs::new();
    let text = "the cat and the mouse.\n";
    let expected = tool(&["tokenize", &inputs.path("words")], text.as_bytes());
    let mut container = naive_tokeniser(&inputs.one("words"), false).expect("a tokeniser");
    let printed = hfst_embed::tokenise::format(
        &mut container,
        text,
        hfst_embed::tokenise::output_format("segment").expect("a format"),
        &hfst_embed::tokenise::FormatOptions::default(),
    )
    .expect("formats");
    assert_eq!(printed, String::from_utf8_lossy(&expected));
}

/// A DHFST fixture's path.
fn dhfst_fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dhfst")
        .join(name)
        .to_str()
        .expect("utf8")
        .to_string()
}

// [spec:hfst:req:embed.writers/test]
// The text of a stream, in each format and weight setting, is what
// `hfst fst2txt` prints.
#[test]
fn text_is_what_fst2txt_prints() {
    let inputs = Inputs::new();
    let cases: Vec<(Vec<&str>, TextOptions)> = vec![
        (vec![], TextOptions::default()),
        (
            vec!["-n"],
            TextOptions {
                use_numbers: true,
                ..TextOptions::default()
            },
        ),
        (
            vec!["-w"],
            TextOptions {
                print_weights: true,
                ..TextOptions::default()
            },
        ),
        (
            vec!["-D"],
            TextOptions {
                do_not_print_weights: true,
                ..TextOptions::default()
            },
        ),
        (
            vec!["-f", "dot"],
            TextOptions {
                format: TextFormat::Dot,
                ..TextOptions::default()
            },
        ),
        (
            vec!["-f", "prolog"],
            TextOptions {
                format: TextFormat::Prolog,
                ..TextOptions::default()
            },
        ),
        (
            vec!["-f", "pckimmo"],
            TextOptions {
                format: TextFormat::Pckimmo,
                ..TextOptions::default()
            },
        ),
    ];
    for name in [
        "words",
        "weights",
        "flags",
        "words.foma",
        "words.olw",
        "att",
        "lexc",
    ] {
        let all = inputs.all(name);
        let refs: Vec<&Transducer> = all.iter().collect();
        for (args, options) in &cases {
            let path = inputs.path(name);
            let mut argv = vec!["fst2txt"];
            argv.extend(args.iter().copied());
            argv.push(&path);
            let (ok, expected) = run(&argv, b"");
            let written = text(&refs, options);
            assert_eq!(written.is_ok(), ok, "hfst {argv:?}: {written:?}");
            if let Ok(bytes) = written {
                assert_eq!(
                    String::from_utf8_lossy(&bytes),
                    String::from_utf8_lossy(&expected),
                    "hfst {argv:?}"
                );
            }
        }
        if refs.len() == 1 {
            let alone = refs[0].to_text(&TextOptions::default()).expect("writes");
            assert_eq!(alone, tool(&["fst2txt", &inputs.path(name)], b""));
        }
    }
}

// [spec:hfst:req:embed.writers/test]
// Each DHFST file is what `hfst fst2fst -f dhfst` writes of the same input.
#[test]
fn dhfst_is_what_fst2fst_writes() {
    let dir = tempfile::tempdir().expect("a directory");
    let out = dir.path().join("out.dhfst");
    let out = out.to_str().expect("utf8");
    let cases: &[(&str, &[&str], DhfstOptions)] = &[
        (
            "errmodel.hfst",
            &["--dhfst-type", "errmodel"],
            DhfstOptions::new(DhfstKind::ErrorModel),
        ),
        (
            "errmodel.hfst",
            &["--dhfst-type", "errmodel", "--max-fallback-depth", "1"],
            DhfstOptions {
                max_fallback_depth: Some(1),
                ..DhfstOptions::new(DhfstKind::ErrorModel)
            },
        ),
        (
            "errmodel.hfst",
            &["--dhfst-type", "errmodel", "--unbounded-fallback"],
            DhfstOptions {
                max_fallback_depth: None,
                ..DhfstOptions::new(DhfstKind::ErrorModel)
            },
        ),
        (
            "acceptor.hfst",
            &["--dhfst-type", "acceptor"],
            DhfstOptions::new(DhfstKind::Acceptor),
        ),
        (
            "acceptor-wide.hfst",
            &["--dhfst-type", "acceptor", "-Q"],
            DhfstOptions {
                quick: true,
                ..DhfstOptions::new(DhfstKind::Acceptor)
            },
        ),
    ];
    for (input, args, options) in cases {
        let path = dhfst_fixture(input);
        let mut argv = vec!["fst2fst", "-f", "dhfst"];
        argv.extend(args.iter().copied());
        argv.extend(["-i", &path, "-o", out]);
        tool(&argv, b"");
        let expected = std::fs::read(out).expect("the tool wrote");
        let t = read_transducers(&std::fs::read(&path).expect("the fixture"))
            .expect("reads")
            .remove(0);
        let options = DhfstOptions {
            source_name: input.to_string(),
            ..options.clone()
        };
        assert_eq!(
            t.to_dhfst(&options).expect("writes"),
            expected,
            "hfst {argv:?}"
        );
    }
}

// [spec:hfst:req:embed.writers/test]
// A THFST directory written through files in memory, or on the filesystem,
// holds what `hfst fst2fst -f thfst` writes.
#[test]
fn thfst_is_what_fst2fst_writes() {
    let inputs = Inputs::new();
    let dir = tempfile::tempdir().expect("a directory");
    for name in ["words", "weights", "words.foma", "words.olw"] {
        let by_tool = dir.path().join(format!("{name}.tool.thfst"));
        let by_tool = by_tool.to_str().expect("utf8");
        tool(
            &["fst2fst", "-f", "thfst", &inputs.path(name), "-o", by_tool],
            b"",
        );
        let t = inputs.one(name);
        let memory = hfst::file_resolver::MemoryFiles::default();
        t.write_thfst("out.thfst", false, &memory).expect("writes");
        let on_disk = dir.path().join(format!("{name}.embed.thfst"));
        let on_disk = on_disk.to_str().expect("utf8");
        t.write_thfst(on_disk, false, &hfst::file_resolver::FsResolver)
            .expect("writes");
        for member in ["alphabet", "index", "transition"] {
            let expected = std::fs::read(format!("{by_tool}/{member}")).expect("the tool wrote");
            assert_eq!(
                memory.get(&format!("out.thfst/{member}")),
                Some(expected.clone()),
                "{name} {member}"
            );
            let written = std::fs::read(format!("{on_disk}/{member}")).expect("written");
            assert_eq!(written, expected, "{name} {member} on disk");
        }
    }
}

// [spec:hfst:req:embed.writers/test]
// An archive packed from files is what `hfst bhfst` packs from the same
// files, from the filesystem or from memory.
#[test]
fn bhfst_is_what_the_tool_packs() {
    let dir = tempfile::tempdir().expect("a directory");
    let index_xml = dir.path().join("index.xml");
    std::fs::write(
        &index_xml,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<hfstspeller dtdversion="1.0" hfstversion="3">
  <info><locale>se</locale><title>T</title><description>D</description><producer>p</producer></info>
  <acceptor type="general" id="acceptor.default.hfst"><title>A</title><description>a</description></acceptor>
  <errmodel id="errmodel.default.hfst"><title>E</title><description>e</description></errmodel>
</hfstspeller>
"#,
    )
    .expect("write index.xml");
    let config = dir.path().join("config.json");
    std::fs::write(&config, r#"{"beam": 3}"#).expect("write the config");
    let out = dir.path().join("out.bhfst");
    let (acceptor, errmodel) = (
        dhfst_fixture("acceptor.hfst"),
        dhfst_fixture("errmodel.dhfst"),
    );
    let index_xml = index_xml.to_str().expect("utf8").to_string();
    let config = config.to_str().expect("utf8").to_string();
    let out = out.to_str().expect("utf8");
    tool(
        &[
            "bhfst", "-a", &acceptor, "-e", &errmodel, "-X", &index_xml, "-c", &config, "-o", out,
        ],
        b"",
    );
    let expected = std::fs::read(out).expect("the tool wrote");
    let speller = SpellerFiles {
        acceptor: acceptor.clone(),
        errmodel: errmodel.clone(),
        meta: MetaFile::IndexXml(index_xml.clone()),
        speller_config: Some(config.clone()),
    };
    let packed = pack_bhfst(&speller, &hfst::file_resolver::FsResolver).expect("packs");
    assert_eq!(packed, expected);

    let memory = hfst::file_resolver::MemoryFiles::default();
    for name in [&acceptor, &errmodel, &index_xml, &config] {
        memory.insert(name.as_str(), std::fs::read(name).expect("a file"));
    }
    assert_eq!(pack_bhfst(&speller, &memory).expect("packs"), expected);
    let printed = tool(&["bhfst", "-I", out], b"");
    let mut metadata = bhfst_metadata(&packed).expect("metadata");
    if !metadata.ends_with(b"\n") {
        metadata.push(b'\n');
    }
    assert_eq!(metadata, printed);
}

fn filesystem() -> std::sync::Arc<dyn hfst::file_resolver::FileResolver> {
    std::sync::Arc::new(hfst::file_resolver::FsResolver)
}

/// The bytes of a stream of `ts`, as written.
fn stream(ts: &[Transducer]) -> Vec<u8> {
    ts.iter().flat_map(written).collect()
}

/// The text of the file at `path`, and the path, as a compile's source.
fn read_source(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

const FORMATS: [(&str, Format); 4] = [
    ("openfst-tropical", Format::OpenFstTropical),
    ("foma", Format::Foma),
    ("olw", Format::OptimizedLookupWeighted),
    ("olu", Format::OptimizedLookupUnweighted),
];

// [spec:hfst:req:embed.compile/test]
// Each file of expressions compiles in every type to what regexp2fst writes.
#[test]
fn regexps_compile_as_regexp2fst_compiles_them() {
    let cases: Vec<(Vec<&str>, RegexpOptions)> = vec![
        (vec![], RegexpOptions::default()),
        (
            vec!["-j"],
            RegexpOptions {
                disjunct: true,
                ..RegexpOptions::default()
            },
        ),
        (
            vec!["-S"],
            RegexpOptions {
                separation: Separation::Semicolons,
                ..RegexpOptions::default()
            },
        ),
        (
            vec!["-H", "-F", "-E", "-M"],
            RegexpOptions {
                harmonize: false,
                harmonize_flags: true,
                encode_weights: true,
                minimize: false,
                ..RegexpOptions::default()
            },
        ),
        (
            vec!["--xerox-composition=yes", "-X", "flag-is-epsilon"],
            RegexpOptions {
                xerox_composition: true,
                flag_is_epsilon: true,
                ..RegexpOptions::default()
            },
        ),
    ];
    for name in ["words", "flags", "weights", "many", "unify", "nonascii"] {
        let path = fixture(&format!("js/test/fixtures/{name}.regexp"));
        let path = path.to_str().expect("utf8");
        let text = read_source(path);
        for (args, options) in &cases {
            for (flag, format) in FORMATS {
                let mut argv = vec!["regexp2fst", "-f", flag];
                argv.extend(args.iter().copied());
                argv.push(path);
                let expected = tool(&argv, b"");
                let compiled = compile_regexps(
                    Source {
                        name: path,
                        text: &text,
                    },
                    format,
                    options,
                    filesystem(),
                )
                .unwrap_or_else(|e| panic!("hfst {argv:?}: {e}"));
                assert_eq!(stream(&compiled.value), expected, "hfst {argv:?}");
            }
        }
    }
}

// [spec:hfst:req:embed.compile/test]
// lexc sources compile in every type to what lexc writes, named after the
// first.
#[test]
fn lexc_compiles_as_the_tool_compiles() {
    let dir = tempfile::tempdir().expect("a directory");
    let second = dir.path().join("more.lexc");
    std::fs::write(&second, "LEXICON Root\nmouse # ;\n").expect("write a source");
    let second = second.to_str().expect("utf8").to_string();
    let first = fixture("crates/hfst/tests/fixtures/test_lexc.lexc");
    let first = first.to_str().expect("utf8").to_string();
    let cases: Vec<(Vec<&str>, LexcOptions)> = vec![
        (vec![], LexcOptions::default()),
        (
            vec!["-A", "-F"],
            LexcOptions {
                align_strings: true,
                with_flags: true,
                ..LexcOptions::default()
            },
        ),
        (
            vec!["-F", "-M", "-R"],
            LexcOptions {
                with_flags: true,
                minimize_flags: true,
                rename_flags: true,
                ..LexcOptions::default()
            },
        ),
    ];
    for files in [vec![first.clone()], vec![first.clone(), second.clone()]] {
        let texts: Vec<String> = files.iter().map(|f| read_source(f)).collect();
        let sources: Vec<Source<'_>> = files
            .iter()
            .zip(&texts)
            .map(|(name, text)| Source { name, text })
            .collect();
        for (args, options) in &cases {
            for (flag, format) in FORMATS {
                let mut argv = vec!["lexc", "-q", "-f", flag];
                argv.extend(args.iter().copied());
                argv.extend(files.iter().map(String::as_str));
                let expected = tool(&argv, b"");
                let quiet = LexcOptions {
                    verbosity: LexcVerbosity::Quiet,
                    ..options.clone()
                };
                let compiled = compile_lexc(&sources, format, &quiet, filesystem())
                    .unwrap_or_else(|e| panic!("hfst {argv:?}: {e}"));
                assert_eq!(written(&compiled.value), expected, "hfst {argv:?}");
            }
        }
    }
}

// [spec:hfst:req:embed.compile/test]
// A twolc grammar's rule transducers are what twolc writes, in either
// algebra type.
#[test]
fn twolc_rules_are_what_twolc_writes() {
    let grammars = [
        "Alphabet a b ;\nRules\n\"R1\"\na:b <=> _ # ;\n",
        "Alphabet a b c a:b ;\nSets\nCns = a b ;\nRules\n\"R1\"\nCns:0 <=> _ c ;\n\"R2\"\na:b => c _ ;\n",
    ];
    for grammar in grammars {
        for (flag, format, args, options) in [
            (
                "openfst",
                Format::OpenFstTropical,
                vec![],
                TwolcOptions::default(),
            ),
            ("foma", Format::Foma, vec![], TwolcOptions::default()),
            (
                "openfst",
                Format::OpenFstTropical,
                vec!["-R", "-D"],
                TwolcOptions {
                    resolve_left_conflicts: true,
                    resolve_right_conflicts: false,
                    ..TwolcOptions::default()
                },
            ),
        ] {
            let mut argv = vec!["twolc", "-q", "-f", flag];
            argv.extend(args);
            let expected = tool(&argv, grammar.as_bytes());
            let quiet = TwolcOptions {
                quiet: true,
                ..options
            };
            let compiled = compile_twolc(
                Source {
                    name: "<twolc>",
                    text: grammar,
                },
                format,
                &quiet,
            )
            .unwrap_or_else(|e| panic!("hfst {argv:?}: {e}"));
            assert_eq!(stream(&compiled.value), expected, "hfst {argv:?}");
        }
    }
}

// [spec:hfst:req:embed.compile/test]
// A pmatch script's archive is what pmatch2fst writes.
#[test]
fn pmatch_archives_are_what_pmatch2fst_writes() {
    let dir = tempfile::tempdir().expect("a directory");
    let out = dir.path().join("out.pmhfst");
    let out = out.to_str().expect("utf8");
    for name in ["animals", "tokenise"] {
        let path = fixture(&format!("js/test/fixtures/{name}.pmatch"));
        let path = path.to_str().expect("utf8");
        for (args, options) in [
            (vec![], Pmatch2FstOptions::default()),
            (
                vec!["--flatten"],
                Pmatch2FstOptions {
                    flatten: true,
                    ..Pmatch2FstOptions::default()
                },
            ),
        ] {
            let mut argv = vec!["pmatch2fst"];
            argv.extend(args);
            argv.extend([path, "-o", out]);
            tool(&argv, b"");
            let expected = std::fs::read(out).expect("the archive");
            let text = read_source(path);
            let compiled = compile_pmatch(
                Source {
                    name: path,
                    text: &text,
                },
                &options,
                filesystem(),
            )
            .unwrap_or_else(|e| panic!("hfst {argv:?}: {e}"));
            assert_eq!(compiled.value, expected, "hfst {argv:?}");
        }
    }
}

// [spec:hfst:req:embed.compile/test]
// Strings, and AT&T and Prolog text, compile in every type to what
// strings2fst and txt2fst write.
#[test]
fn strings_and_text_compile_as_the_tools_do() {
    let dir = tempfile::tempdir().expect("a directory");
    let strings = dir.path().join("s.txt");
    std::fs::write(&strings, "cat\ndog\t1.5\nc:d a t\t2\n").expect("write strings");
    let strings = strings.to_str().expect("utf8").to_string();
    let text = read_source(&strings);
    for (flag, format) in FORMATS {
        for (args, options) in [
            (vec![], StringsOptions::default()),
            (
                vec!["-j", "--norm"],
                StringsOptions {
                    disjunct: true,
                    normalize: true,
                    ..StringsOptions::default()
                },
            ),
            (
                vec!["-p", "-S"],
                StringsOptions {
                    pairstrings: true,
                    has_spaces: true,
                    ..StringsOptions::default()
                },
            ),
        ] {
            let mut argv = vec!["strings2fst", "-f", flag];
            argv.extend(args);
            argv.push(&strings);
            let expected = tool(&argv, b"");
            let compiled = compile_strings(
                Source {
                    name: &strings,
                    text: &text,
                },
                format,
                &options,
            )
            .unwrap_or_else(|e| panic!("hfst {argv:?}: {e}"));
            assert_eq!(stream(&compiled.value), expected, "hfst {argv:?}");
        }
    }

    let prolog = dir.path().join("t.prolog");
    let words = inputs_text(&["fst2txt", "-f", "prolog"], "words");
    std::fs::write(&prolog, words).expect("write Prolog");
    let prolog = prolog.to_str().expect("utf8").to_string();
    let att = fixture("crates/hfst/tests/fixtures/test_transducers.att");
    let att = att.to_str().expect("utf8").to_string();
    for (flag, format) in FORMATS {
        for (path, args, options) in [
            (&att, vec![], Txt2FstOptions::default()),
            (
                &att,
                vec!["-j"],
                Txt2FstOptions {
                    disjunct: true,
                    ..Txt2FstOptions::default()
                },
            ),
            (
                &prolog,
                vec!["-p"],
                Txt2FstOptions {
                    prolog: true,
                    ..Txt2FstOptions::default()
                },
            ),
        ] {
            let mut argv = vec!["txt2fst", "-f", flag];
            argv.extend(args);
            argv.push(path);
            let expected = tool(&argv, b"");
            let text = read_source(path);
            let compiled = compile_text(
                Source {
                    name: path,
                    text: &text,
                },
                format,
                &options,
            )
            .unwrap_or_else(|e| panic!("hfst {argv:?}: {e}"));
            assert_eq!(stream(&compiled.value), expected, "hfst {argv:?}");
        }
    }
}

/// What `hfst ARGS` prints of the input transducer `name`.
fn inputs_text(args: &[&str], name: &str) -> Vec<u8> {
    let inputs = Inputs::new();
    let path = inputs.path(name);
    let mut argv = args.to_vec();
    argv.push(&path);
    tool(&argv, b"")
}

/// What `hfst xfst ARGS -F s.xfst` prints of `script` in a directory of its
/// own, whether it succeeded, and the directory.
fn xfst_tool(args: &[&str], script: &str) -> (bool, Vec<u8>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(dir.path().join("s.xfst"), script).expect("write the script");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_hfst"))
        .arg("xfst")
        .args(args)
        .args(["-F", "s.xfst"])
        .current_dir(dir.path())
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .expect("hfst xfst");
    (out.status.success(), out.stdout, dir)
}

// [spec:hfst:req:embed.compile/test]
// [spec:hfst:req:embed.compile-files/test]
// An xfst script prints what `hfst xfst -F` prints, and writes the files it
// writes, through the resolver rather than to the working directory. A
// command that fails is an error among the diagnostics, and stops the
// script only when quit-on-fail is on.
#[test]
fn xfst_scripts_run_as_the_tool_runs_them() {
    let script = "regex a b c ;\nprint words\ndefine X [d|e] ;\nregex X ;\nprint net\n\
        push defined X\nprint stack\nsave stack out.hfst\nsave defined defs.hfst\n\
        set quit-on-fail OFF\nregex a:b ;\nset print-weight ON\nprint words\n\
        pop stack\npop stack\npop stack\npop stack\npop stack\nregex c ;\nprint size\n";
    for (flag, format, args, options) in [
        (
            "openfst-tropical",
            Format::OpenFstTropical,
            vec![],
            XfstOptions::default(),
        ),
        ("foma", Format::Foma, vec![], XfstOptions::default()),
        (
            "openfst-tropical",
            Format::OpenFstTropical,
            vec!["-w"],
            XfstOptions {
                print_weight: true,
                ..XfstOptions::default()
            },
        ),
    ] {
        let mut argv = vec!["-f", flag];
        argv.extend(args);
        let (ok, printed, dir) = xfst_tool(&argv, script);
        assert!(ok, "xfst {argv:?}");
        let memory = hfst::file_resolver::MemoryFiles::default();
        let compiled = compile_xfst(
            Source {
                name: "s.xfst",
                text: script,
            },
            "",
            format,
            &options,
            std::sync::Arc::new(memory.clone()),
        )
        .unwrap_or_else(|e| panic!("xfst {argv:?}: {e}"));
        let result = compiled.value;
        assert_eq!(
            result.output,
            String::from_utf8_lossy(&printed),
            "xfst {argv:?}"
        );
        assert_eq!(result.written, ["out.hfst", "defs.hfst"]);
        for name in &result.written {
            let on_disk = std::fs::read(dir.path().join(name)).expect("the tool wrote");
            assert_eq!(memory.get(name), Some(on_disk), "{name}");
        }
        assert_eq!(result.stack.len(), 1);
        assert_eq!(result.definitions.len(), 1);
        assert_eq!(result.definitions[0].0, "X");
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|d| d.is_error() && d.message().contains("empty stack")),
            "{:?}",
            compiled.diagnostics
        );
    }
    let stopping = "regex a ;\npop stack\npop stack\nregex b ;\nprint words\n";
    let (ok, _, _) = xfst_tool(&[], stopping);
    assert!(!ok);
    let failed = compile_xfst(
        Source {
            name: "s.xfst",
            text: stopping,
        },
        "",
        Format::OpenFstTropical,
        &XfstOptions::default(),
        std::sync::Arc::new(hfst::file_resolver::MemoryFiles::default()),
    )
    .err()
    .expect("the script stops");
    assert!(failed.diagnostics.iter().any(Diagnostic::is_error));
}
