use hfst::hfst_transducer::AnyTransducer;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

use super::*;
use crate::format::Format;

fn regex(src: &str) -> Transducer {
    let t = XreCompiler::<StdVectorFst>::new()
        .compile(src)
        .unwrap_or_else(|| panic!("{src} compiles"));
    Transducer::new(AnyTransducer::Tropical(t))
}

// [spec:hfst:req:embed.transducer/test]
#[test]
fn empty_bytes_are_not_a_stream() {
    let error = read_transducers(&[]).err().expect("refused");
    assert_eq!(error.code().name(), "invalid-format");
}

// [spec:hfst:req:embed.transducer/test]
// Converting in place keeps the language, and a stream of the new type
// reads back as it.
#[test]
fn converts_in_place() {
    let mut t = regex("{cat}:{dog}");
    for format in [
        Format::Foma,
        Format::OptimizedLookupWeighted,
        Format::OpenFstTropical,
    ] {
        t.convert(format, false).expect("converts");
        assert_eq!(t.format(), format);
        let read = read_transducers(&t.to_bytes(&WriteOptions::default()).expect("writes"))
            .expect("reads");
        assert_eq!(read[0].format(), format);
    }
    let found = t.lookup("cat", None, None).expect("looks up");
    assert_eq!(found[0].output, "dog");
}

// [spec:hfst:req:embed.operations/test]
// Operands of different types are refused when converting is not asked
// for, and converted as the tools convert them otherwise.
#[test]
fn do_not_convert_refuses_mixed_types() {
    let mut t = regex("a");
    let mut other = regex("b");
    other.convert(Format::Foma, false).expect("converts");
    let options = DisjunctOptions {
        do_not_convert: true,
        ..DisjunctOptions::default()
    };
    let error = t
        .disjunct(Operand::Other(&other), &options)
        .expect_err("refused");
    assert_eq!(error.code().name(), "type-mismatch");
    t.disjunct(Operand::Other(&other), &DisjunctOptions::default())
        .expect("converts and disjuncts");
    assert_eq!(t.format(), Format::OpenFstTropical);
}

// [spec:hfst:req:embed.operations/test]
// A transducer composed with itself reads itself as it was.
#[test]
fn composing_with_itself_reads_the_original() {
    let mut t = regex("a:b | b:c");
    t.compose(Operand::Itself, &ComposeOptions::default())
        .expect("composes");
    let found = t.lookup("a", None, None).expect("looks up");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].output, "c");
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn bad_options_are_refused_as_the_tools_do() {
    let mut t = regex("a");
    let error = t
        .repeat(&RepeatOptions {
            from: Count::Finite(3),
            to: Count::Finite(1),
        })
        .expect_err("refused");
    assert_eq!(error.code().name(), "invalid-argument");
    let error = t
        .kill_paths(&KillPathsOptions::default())
        .expect_err("refused");
    assert_eq!(error.code().name(), "invalid-argument");
    let error = t
        .insert_freely(&InsertFreelyOptions::default())
        .expect_err("refused");
    assert_eq!(error.code().name(), "invalid-argument");
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn summaries_and_alphabets_report_as_data() {
    let t = regex("a:b c %+Tag");
    let summary = t.summarize(true).expect("summarizes");
    assert_eq!((summary.states, summary.arcs), (4, 3));
    assert_eq!(summary.format, Format::OpenFstTropical);
    assert!(!summary.acceptor);
    assert!(summary.details.is_some());
    assert!(
        summary
            .symbol_pairs
            .contains(&("a".to_string(), "b".to_string(), 1))
    );
    let alphabets = t.dump_alphabets().expect("dumps");
    assert!(alphabets.seen.contains(&"+Tag".to_string()));
    let other = regex("a d");
    let comparison = t.check_alpha(&other).expect("checks");
    assert!(comparison.mismatch);
    assert!(comparison.second_used.contains(&"d".to_string()));
    assert!(t.compare(&t, &CompareOptions::default()).expect("compares"));
    assert!(
        !t.compare(&other, &CompareOptions::default())
            .expect("compares")
    );
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn paths_come_back_as_data() {
    let t = regex("{cat}:{dog}::1 | {cow}::2");
    let paths = t
        .extract_paths(&ExtractPathsOptions::default())
        .expect("extracts");
    let shown: Vec<(&str, &str, f32)> = paths
        .iter()
        .map(|p| (p.input.as_str(), p.output.as_str(), p.weight))
        .collect();
    assert!(shown.contains(&("cat", "dog", 1.0)));
    assert!(shown.contains(&("cow", "cow", 2.0)));
    let mut olw = t.copy().expect("copies");
    olw.convert(Format::OptimizedLookupWeighted, false)
        .expect("converts");
    let refused = olw
        .extract_paths(&ExtractPathsOptions {
            nbest: 1,
            ..ExtractPathsOptions::default()
        })
        .expect_err("not for optimized lookup");
    assert_eq!(refused.code().name(), "type-mismatch");
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn lookups_report_as_data() {
    let t = regex("{cat}:{dog}::0.5 | {cat}:{cow}::2");
    let reports =
        lookup_report(&[&t], &["cat", "xyz"], &LookupOptions::default()).expect("looks up");
    let LookupOutcome::Found { results, infinite } = &reports[0].outcome else {
        panic!("cat looks up");
    };
    assert!(!infinite);
    assert_eq!(results[0].output, "dog");
    assert_eq!(results.len(), 2);
    let beam = LookupOptions {
        beam: Some(1.0),
        ..LookupOptions::default()
    };
    let narrowed = lookup_report(&[&t], &["cat"], &beam).expect("looks up");
    let LookupOutcome::Found { results, .. } = &narrowed[0].outcome else {
        panic!("cat looks up");
    };
    assert_eq!(results.len(), 1);
    let flooked = flookup_report(&[&t], &["dog"], &FlookupOptions::default()).expect("looks up");
    let LookupOutcome::Found { results, .. } = &flooked[0].outcome else {
        panic!("dog looks up");
    };
    assert_eq!(results[0].output, "cat");
    let mut olw = t.copy().expect("copies");
    olw.convert(Format::OptimizedLookupWeighted, false)
        .expect("converts");
    let reports =
        optimized_lookup_report(&[&olw], &["cat", "xyz"], &OptimizedLookupOptions::default())
            .expect("looks up");
    assert_eq!(
        reports[0].analyses.as_deref(),
        Some(&[("dog".to_string(), 0.5), ("cow".to_string(), 2.0)][..])
    );
    assert_eq!(reports[1].analyses, None);
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn grep_pair_test_and_guess_report_as_data() {
    let report = grep(
        "cat",
        "a cat\nno dog\n",
        &GrepOptions {
            fixed_strings: true,
            ..GrepOptions::default()
        },
    )
    .expect("greps");
    assert!(report.matched);
    assert_eq!(report.lines.len(), 1);
    assert_eq!(report.lines[0].number, 1);
    assert_eq!(report.lines[0].text, "a {{{cat}}}");

    // A pair string is tested with a word boundary at each end.
    let rule = regex("\"@#@\":0 [a:b | a | b]* \"@#@\":0");
    let results =
        pair_test(&[&rule], "a a:b\n! comment\n", &PairTestOptions::default()).expect("tests");
    assert_eq!(results.len(), 1);
    assert!(!results[0].failed);
    let negative = PairTestOptions {
        negative_test: true,
        ..PairTestOptions::default()
    };
    let results = pair_test(&[&rule], "a a:b\n", &negative).expect("tests");
    assert!(results[0].failed);

    let mut guesser = regex("{cat}:{cat+N} | {dog}:{dog+N}");
    let generator = guesser
        .guessify(&GuessifyOptions::default())
        .expect("guessifies");
    assert!(generator.is_some());
    assert_eq!(guesser.format(), Format::OptimizedLookupWeighted);
    let guesses = guess(&guesser, None, None, &["cat"], &GuessOptions::default()).expect("guesses");
    assert!(matches!(guesses[0], GuessReport::Guesses(_)));
    let refused = guess(&regex("a"), None, None, &["a"], &GuessOptions::default())
        .expect_err("not a guesser");
    assert_eq!(refused.code().name(), "invalid-argument");
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn a_naive_tokeniser_finds_the_dictionarys_words() {
    let dictionary = regex("{cat}:{cat+N} | {dog}:{dog+N}");
    let mut container = naive_tokeniser(&dictionary, false).expect("a tokeniser");
    let tokens = crate::tokenise::tokens(
        &mut container,
        "cat dog",
        &crate::tokenise::TokeniseOptions::default(),
    );
    let analysed: Vec<&str> = tokens
        .iter()
        .filter(|t| t.analysed)
        .map(|t| t.surface.as_str())
        .collect();
    assert_eq!(analysed, ["cat", "dog"]);
}

fn foma(src: &str) -> Transducer {
    let mut t = regex(src);
    t.convert(Format::Foma, false).expect("converts");
    t
}

/// Each unary operation, the name it gives its result, and a call of it.
type Unary = (
    &'static str,
    fn(&mut Transducer) -> Result<(), crate::Error>,
);

const UNARY: &[Unary] = &[
    ("determinize", |t| {
        t.determinize(&DeterminizeOptions::default())
    }),
    ("minimize", |t| t.minimize(&MinimizeOptions::default())),
    ("invert", |t| t.invert()),
    ("reverse", |t| t.reverse()),
    ("remove-epsilons", |t| t.remove_epsilons()),
    ("prune-alphabet", |t| {
        t.prune_alphabet(&PruneAlphabetOptions::default())
    }),
    ("project-2nd", |t| t.project(&ProjectOptions::default())),
    ("push-weights-f", |t| {
        t.push_weights(&PushOptions::default())
    }),
    ("push-labels-f", |t| t.push_labels(&PushOptions::default())),
    ("realign", |t| t.realign()),
    ("repeat-0-plus", |t| t.repeat(&RepeatOptions::default())),
    ("eliminate-flags", |t| {
        t.eliminate_flags(&EliminateFlagsOptions::default())
    }),
    ("fu", |t| t.preprocess_for_optimized_lookup_format()),
    ("pathkill", |t| {
        t.kill_paths(&KillPathsOptions {
            symbol: None,
            tsv: Some("b\n# not this\n".to_string()),
        })
    }),
    ("reweight", |t| {
        t.reweight(&ReweightOptions {
            tsv: Some("a\t2\n".to_string()),
            ..ReweightOptions::default()
        })
    }),
    ("insert-freely", |t| {
        t.insert_freely(&InsertFreelyOptions {
            symbol_pair: "x:y".to_string(),
            harmonise: true,
        })
    }),
];

// [spec:hfst:req:embed.operations/test]
// Every unary operation runs in both algebra types, in place, and names its
// result after the transducer as its tool does.
#[test]
fn unary_operations_run_in_both_algebra_types() {
    for (name, op) in UNARY {
        for mut t in [regex("a:b c"), foma("a:b c")] {
            let format = t.format();
            op(&mut t).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(t.format(), format, "{name} keeps the type");
            assert_eq!(t.name(), format!("{name}(UNNAMED)"), "{name}");
        }
    }
}

// [spec:hfst:req:embed.operations/test]
// The operations that build their result rather than change the transducer
// still leave it in the type it had.
#[test]
fn rebuilding_operations_replace_the_transducer() {
    let mut t = regex("{cat}:{dog} | {cow}");
    t.affix_guessify(&AffixGuessifyOptions::default())
        .expect("guessifies");
    assert_eq!(t.format(), Format::OpenFstTropical);
    let mut t = regex("a b");
    t.expand_equivalences(&ExpandEquivalencesOptions {
        tsv: Some("a\tA\tB\n".to_string()),
        level: Level::Both,
        ..ExpandEquivalencesOptions::default()
    })
    .expect("expands");
    // a stands for itself, A or B on both sides: three by three paths.
    let paths = t
        .extract_paths(&ExtractPathsOptions::default())
        .expect("extracts");
    assert_eq!(paths.len(), 9);
    let refused = regex("a")
        .expand_equivalences(&ExpandEquivalencesOptions {
            from: Some("a".to_string()),
            ..ExpandEquivalencesOptions::default()
        })
        .expect_err("needs to");
    assert_eq!(refused.code().name(), "invalid-argument");
}

/// Each binary operation, the name it gives its result, and a call of it.
type Binary = (
    &'static str,
    fn(&mut Transducer, Operand<'_>) -> Result<(), crate::Error>,
);

const BINARY: &[Binary] = &[
    ("compose", |t, o| t.compose(o, &ComposeOptions::default())),
    ("concatenate", |t, o| {
        t.concatenate(o, &BinaryOptions::default())
    }),
    ("intersect", |t, o| t.conjunct(o, &BinaryOptions::default())),
    ("union", |t, o| t.disjunct(o, &DisjunctOptions::default())),
    ("union", |t, o| {
        t.priority_disjunct(o, &ConvertOptions::default())
    }),
    ("subtract", |t, o| t.subtract(o, &BinaryOptions::default())),
];

// [spec:hfst:req:embed.operations/test]
// A binary operation takes an operand of the other algebra type, converting
// one of the two as the tools do, and an operand that is the transducer
// itself.
#[test]
fn binary_operations_convert_their_operands() {
    for (name, op) in BINARY {
        let mut t = regex("a");
        let other = foma("a");
        op(&mut t, Operand::Other(&other)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(t.format(), Format::OpenFstTropical, "{name}");
        assert_eq!(t.name(), format!("{name}(UNNAMED, UNNAMED)"));
        assert_eq!(other.format(), Format::Foma, "{name} leaves the operand");

        let mut t = foma("a");
        op(&mut t, Operand::Itself).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(t.format(), Format::Foma, "{name}");

        let mut ol = regex("a");
        ol.convert(Format::OptimizedLookupUnweighted, false)
            .expect("converts");
        let refused = op(&mut regex("a"), Operand::Other(&ol)).expect_err("refused");
        assert_eq!(refused.code().name(), "type-mismatch", "{name}");
    }
    let mut t = regex("a b");
    let other = regex("[a|b] c");
    t.shuffle(Operand::Other(&other), &ConvertOptions::default())
        .expect("shuffles");
    assert_eq!(t.name(), "shuffle(UNNAMED, UNNAMED)");
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn compose_intersect_names_the_lexicon_and_the_rules() {
    let mut lexicon = regex("{cat}:{cats}");
    let rule = foma("[c|a|t|s]*");
    lexicon
        .compose_intersect(
            &[&rule],
            &ComposeIntersectOptions {
                lexicon_name: "lexicon.hfst".to_string(),
                rules_name: "rules.hfst".to_string(),
                ..ComposeIntersectOptions::default()
            },
        )
        .expect("composes");
    assert_eq!(
        lexicon.name(),
        "compose(lexicon.hfst, intersect(rules.hfst))"
    );
    assert_eq!(lexicon.format(), Format::OpenFstTropical);
}

// [spec:hfst:req:embed.operations/test]
// Each way of naming what to substitute, and the refusals the tool gives.
#[test]
fn substitutions_take_labels_files_and_transducers() {
    let mut t = regex("a b");
    t.substitute(&SubstituteOptions {
        from_label: Some("a".to_string()),
        to_label: Some("@0@".to_string()),
        ..SubstituteOptions::default()
    })
    .expect("substitutes");
    assert_eq!(t.name(), "substitute-a-with-@_EPSILON_SYMBOL_@(UNNAMED)");
    let mut t = regex("a b");
    t.substitute(&SubstituteOptions {
        from_file: Some(NamedText {
            name: "f.tsv".to_string(),
            text: "a\tc\n".to_string(),
        }),
        ..SubstituteOptions::default()
    })
    .expect("relabels");
    assert_eq!(t.name(), "substitute-from-f.tsv(UNNAMED)");
    let replacement = foma("x y");
    let mut t = regex("a b");
    t.substitute(&SubstituteOptions {
        from_label: Some("a".to_string()),
        to_transducer: Some(NamedOperand {
            transducer: &replacement,
            name: "net.hfst",
        }),
        ..SubstituteOptions::default()
    })
    .expect("substitutes a net");
    assert_eq!(t.name(), "substitute-a-with-net-net.hfst(UNNAMED)");
    for refused in [
        SubstituteOptions::default(),
        SubstituteOptions {
            from_label: Some("a".to_string()),
            ..SubstituteOptions::default()
        },
        SubstituteOptions {
            from_file: Some(NamedText {
                name: "bad.tsv".to_string(),
                text: "no tab\n".to_string(),
            }),
            ..SubstituteOptions::default()
        },
    ] {
        let error = regex("a").substitute(&refused).expect_err("refused");
        assert_eq!(error.code().name(), "invalid-argument");
    }
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn metadata_names_and_properties_change_in_place() {
    let mut t = regex("a");
    t.set_name(&NameOptions {
        name: "renamed".to_string(),
        truncate_length: 0,
    });
    t.edit_metadata(&EditMetadataOptions {
        add: vec![
            ("note".to_string(), "first".to_string()),
            ("note".to_string(), "second".to_string()),
        ],
        truncate_length: 0,
    })
    .expect("edits");
    assert_eq!(t.name(), "renamed");
    assert!(
        t.properties()
            .contains(&("note".to_string(), "second".to_string()))
    );
    let refused = t
        .edit_metadata(&EditMetadataOptions {
            add: vec![("character-encoding".to_string(), "latin1".to_string())],
            truncate_length: 0,
        })
        .expect_err("refused");
    assert_eq!(refused.code().name(), "invalid-argument");
}

// [spec:hfst:req:embed.transducer/test]
// Written without a header, a stream is the backend's own format; written in
// another type, it is converted and named as fst2fst names it.
#[test]
fn write_options_choose_the_header_and_the_type() {
    let t = regex("a");
    let with = t.to_bytes(&WriteOptions::default()).expect("writes");
    assert!(with.starts_with(b"HFST\0"));
    let without = t
        .to_bytes(&WriteOptions {
            header: false,
            ..WriteOptions::default()
        })
        .expect("writes");
    assert!(!without.starts_with(b"HFST\0"));
    let converted = t
        .to_bytes(&WriteOptions {
            format: Some(Format::OptimizedLookupWeighted),
            quick: true,
            ..WriteOptions::default()
        })
        .expect("writes");
    let read = read_transducers(&converted).expect("reads");
    assert_eq!(read[0].format(), Format::OptimizedLookupWeighted);
    assert_eq!(read[0].name(), "convert(UNNAMED)");
    assert_eq!(t.name(), "", "writing leaves the transducer as it was");
}

// [spec:hfst:req:embed.operations/test]
#[test]
fn lookup_options_shape_the_reports() {
    let t = regex("{cat}:{dog}::0.5 | {cat}:{cow}::2");
    let apertium = LookupOptions {
        input_format: LookupInput::Apertium,
        ..LookupOptions::default()
    };
    let reports = lookup_report(&[&t], &["c\\at[x]"], &apertium).expect("looks up");
    assert_eq!(reports[0].input, "cat");
    assert_eq!(reports[0].markup, "[x]");
    let mut ol = t.copy().expect("copies");
    ol.convert(Format::OptimizedLookupWeighted, false)
        .expect("converts");
    let refused =
        flookup_report(&[&ol], &["dog"], &FlookupOptions::default()).expect_err("refused");
    assert_eq!(refused.code().name(), "type-mismatch");
    let forced = flookup_report(
        &[&ol],
        &["cat"],
        &FlookupOptions {
            force_ol: true,
            invert: true,
            ..FlookupOptions::default()
        },
    )
    .expect("looks up");
    assert!(matches!(forced[0].outcome, LookupOutcome::Found { .. }));
    let not_obeying = LookupOptions {
        obey_flags: false,
        ..LookupOptions::default()
    };
    let refused = lookup_report(&[&ol], &["cat"], &not_obeying).expect_err("refused");
    assert_eq!(refused.code().name(), "type-mismatch");
    let unique = OptimizedLookupOptions {
        unique: true,
        max_analyses: 1,
        ..OptimizedLookupOptions::default()
    };
    let reports = optimized_lookup_report(&[&ol], &["cat"], &unique).expect("looks up");
    assert_eq!(reports[0].analyses.as_ref().map(Vec::len), Some(1));
    let summary = ol.summarize(true).expect("summarizes");
    let details = summary.details.expect("details");
    assert!(details.first_input_symbols.is_none());
}

/// The JavaScript tests' pmatch tokeniser as a container.
fn container() -> hfst::pmatch::PmatchContainer {
    use hfst::file_resolver::{FileResolver, MemoryFiles};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::pmatch_compiler::{PmatchCompiler, write_archive};
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../js/test/fixtures/animals.pmatch"
    ))
    .expect("the fixture is in the repository");
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let mut definitions = compiler.compile(&source).expect("the fixture compiles");
    let files = MemoryFiles::default();
    let sink = files.create("archive").expect("memory takes a file");
    let mut stream = HfstOutputStream::from_writer(sink, ImplementationType::HFST_OLW_TYPE, true)
        .expect("a stream");
    write_archive(&mut definitions, &mut stream, false, &mut std::io::sink())
        .expect("the archive writes");
    stream.close();
    let bytes = files.get("archive").expect("written");
    crate::tokenise::read_tokeniser(&bytes, &crate::TEST_PACKAGE).expect("reads")
}

// [spec:hfst:req:embed.operations/test]
// pmatch marks matches up, or locates them, piece by piece.
#[test]
fn pmatch_marks_up_and_locates() {
    let mut matcher = container();
    let text = "the cat\n\nthe dog\n";
    let marked = pmatch(&mut matcher, text, &PmatchOptions::default());
    assert_eq!(marked.len(), 2);
    let PmatchResult::Marked(first) = &marked[0].1 else {
        panic!("not in locate mode");
    };
    assert!(first.contains("cat"), "{first}");
    let located = pmatch(
        &mut matcher,
        text,
        &PmatchOptions {
            locate: true,
            blankline_separated: false,
            ..PmatchOptions::default()
        },
    );
    assert_eq!(located.len(), 3);
    let PmatchResult::Located(locations) = &located[0].1 else {
        panic!("in locate mode");
    };
    assert!(locations.iter().all(|l| l.output != "@_NONMATCHING_@"));
}
