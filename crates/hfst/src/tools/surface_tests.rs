//! The operations and reports the compile surface shares with the tools:
//! each operation's result and the name it gives it, and each query's
//! report as data and as the text the tool prints.

use super::compare::{CompareOptions, compare, write_comparison};
use super::dump_alphabets::{AlphabetFormat, Alphabets, is_multichar, write_alphabets, write_end};
use super::fst2strings::{Fst2StringsError, PathFormat, PathQuery, extract_paths, path_line};
use super::guess::{GuessReport, write_guess};
use super::lookup::Lookup;
use super::operations::{
    self, BinaryOptions, BinaryWarning, ComposeOptions, EliminateFlagsError, MetadataError,
    MetadataWarning, Operand, PushDirection, Repetition, SymbolPairError, Tape,
};
use super::optimized_lookup::{OptimizedLookup, OptimizedLookupOptions, OptimizedLookupOutcome};
use super::pmatch::InputSplitter;
use super::substitute::{
    RelabelError, RelabelLineError, Substitution, SubstitutionName, name_substituted,
    relabel_entries, substitute,
};
use super::test_support::{paths_of, regex};
use crate::hfst_data_types::{HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPath, Symbol};
use crate::hfst_lookup_format::{
    CascadeVariant, LookupInputFormat, reported_paths, reported_symbols,
};
use crate::hfst_transducer::AnyTransducer;
use crate::lookup_driver::{
    AmbiguityLimit, FlagPolicy, LookupCascade, LookupEngineOptions, LookupReporter, PairPrintStyle,
};
use crate::substitute_driver::SubstituteReporter;

fn text(write: impl FnOnce(&mut Vec<u8>)) -> String {
    let mut out = Vec::new();
    write(&mut out);
    String::from_utf8(out).expect("utf8")
}

/// The messages an engine gave, kept rather than printed.
#[derive(Default)]
struct Quiet(std::cell::RefCell<Vec<String>>);

impl LookupReporter for Quiet {
    fn verbose(&self, msg: &str) {
        self.0.borrow_mut().push(msg.to_string());
    }

    fn warning(&self, msg: &str) {
        self.0.borrow_mut().push(msg.to_string());
    }
}

impl SubstituteReporter for Quiet {
    fn verbose(&self, msg: &str) {
        self.0.borrow_mut().push(msg.to_string());
    }
}

// [spec:hfst:req:embed.tools-in-library/test]
// An operation names and gives a formula to its result after the
// transducer's own, `UNNAMED` and `.` standing in for none.
#[test]
fn operations_name_their_results() {
    let mut t = regex("a:b");
    operations::invert(&mut t).expect("inverts");
    assert_eq!(t.get_name(), "invert(UNNAMED)");
    assert_eq!(t.get_property("formulaic-definition"), "\u{207b}\u{00b9} .");
    operations::determinize(&mut t, false).expect("determinizes");
    assert_eq!(t.get_name(), "determinize(invert(UNNAMED))");
    assert_eq!(
        t.get_property("formulaic-definition"),
        "\u{2336} \u{207b}\u{00b9} ."
    );
    operations::project(&mut t, Tape::Input).expect("projects");
    assert_eq!(paths_of(&t), ["b:b 0"]);
    operations::push_weights(&mut t, PushDirection::Initial).expect("pushes");
    assert!(t.get_name().starts_with("push-weights-i("));
    operations::prune_alphabet(&mut t, false).expect("prunes");
    assert!(t.get_name().starts_with("prune-alphabet("));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn repetitions_take_their_bounds_and_names() {
    assert_eq!(
        Repetition::from_bounds(Some(1), Some(3)),
        Ok(Repetition::Between { from: 1, to: 3 })
    );
    assert_eq!(Repetition::from_bounds(None, None), Ok(Repetition::Star));
    assert_eq!(
        Repetition::from_bounds(Some(2), None),
        Ok(Repetition::AtLeast(2))
    );
    assert!(Repetition::from_bounds(Some(3), Some(1)).is_err());
    assert!(Repetition::from_bounds(None, Some(1)).is_err());
    for (repetition, name) in [
        (
            Repetition::Between { from: 1, to: 2 },
            "repeat-1-to-2(UNNAMED)",
        ),
        (Repetition::Star, "repeat-star(UNNAMED)"),
        (Repetition::AtLeast(1), "repeat-1-plus(UNNAMED)"),
    ] {
        let mut t = regex("a");
        operations::repeat(&mut t, repetition).expect("repeats");
        assert_eq!(t.get_name(), name);
    }
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn labels_and_flags_refused_with_tool_texts() {
    assert_eq!(
        operations::insert_freely_pair(""),
        Err(SymbolPairError::Empty)
    );
    let pair = operations::insert_freely_pair("x:y").expect("a pair");
    assert_eq!((pair.0.as_str(), pair.1.as_str()), ("x", "y"));
    let mut t = regex("a");
    operations::insert_freely(&mut t, &pair, false).expect("inserts");
    assert_eq!(t.get_name(), "insert-freely(UNNAMED)");
    let mut flagged = regex("\"@P.X.Y@\" a");
    let refused =
        operations::eliminate_flags(&mut flagged, Some("NOPE")).expect_err("no such feature");
    assert!(matches!(refused, EliminateFlagsError::NoSuchFeature(_)));
    assert!(refused.to_string().contains("does not occur"));
    operations::eliminate_flags(&mut flagged, Some("X")).expect("eliminates");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn metadata_is_set_in_key_order_with_warnings() {
    let mut t = regex("a");
    let mut warned = Vec::new();
    operations::edit_metadata(
        &mut t,
        [("type", "x"), ("version", "9"), ("foo", "barbaz")],
        3,
        &mut |w| warned.push(w),
    )
    .expect("edits");
    assert_eq!(warned, [MetadataWarning::Type, MetadataWarning::Version]);
    assert_eq!(t.get_property("foo"), "bar");
    let refused =
        operations::edit_metadata(&mut t, [("character-encoding", "latin1")], 0, &mut |_| {});
    assert_eq!(refused, Err(MetadataError::UnsupportedEncoding));
    operations::name(&mut t, "na\u{00ef}ve", 3);
    assert_eq!(t.get_name(), "na\u{fffd}");
    operations::name(&mut t, "whole", 0);
    assert_eq!(t.get_name(), "whole");
}

// [spec:hfst:req:embed.tools-in-library/test]
// A borrowed operand stays as it is, and is copied only for the flag
// harmonizing that changes it.
#[test]
fn binary_operations_borrow_their_second_operand() {
    let second = regex("b");
    let mut first = regex("a");
    let mut operand = Operand::Borrowed(&second);
    operations::concatenate(
        &mut first,
        &mut operand,
        &BinaryOptions::default(),
        &mut |_| {},
    )
    .expect("concatenates");
    assert!(matches!(operand, Operand::Borrowed(_)));
    assert_eq!(paths_of(&first), ["ab:ab 0"]);
    assert_eq!(first.get_name(), "concatenate(UNNAMED, UNNAMED)");

    let flagged = regex("\"@P.X.Y@\" b");
    let mut first = regex("\"@R.X.Y@\" a");
    let mut operand = Operand::Borrowed(&flagged);
    let mut warned = Vec::new();
    operations::subtract(
        &mut first,
        &mut operand,
        &BinaryOptions::default(),
        &mut |w| warned.push(w),
    )
    .expect("subtracts");
    assert_eq!(warned[0], BinaryWarning::SubtrahendHasFlags);
    assert!(matches!(warned[1], BinaryWarning::FlagsNotHarmonized(_)));

    let mut first = regex("\"@R.X.Y@\" a");
    let mut operand = Operand::Borrowed(&flagged);
    let options = ComposeOptions {
        binary: BinaryOptions {
            harmonize: true,
            harmonize_flags: true,
        },
        ..ComposeOptions::default()
    };
    operations::compose(&mut first, &mut operand, &options, None, &mut |_| {}).expect("composes");
    assert!(matches!(operand, Operand::Owned(_)));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn relabel_files_read_line_by_line() {
    let entries = relabel_entries("a\tb\n# comment\n\nc:d\te\n").expect("reads");
    assert_eq!(
        entries,
        [
            ("a".to_string(), "b".to_string()),
            ("c:d".to_string(), "e".to_string())
        ]
    );
    for (text, line, error) in [
        ("a\tb\nnope\n", 2, RelabelLineError::NoTab),
        ("\tb\n", 1, RelabelLineError::EmptyFrom),
        ("a\t\n", 1, RelabelLineError::EmptyTo),
    ] {
        assert_eq!(relabel_entries(text), Err(RelabelError { line, error }));
    }
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn substitutions_relabel_and_name_their_results() {
    let mut t = regex("a b");
    substitute(
        &mut t,
        Substitution::Label {
            from: "a",
            to: "c",
            compose: false,
        },
        &Quiet::default(),
    )
    .expect("substitutes");
    assert_eq!(paths_of(&t), ["cb:cb 0"]);
    assert_eq!(t.get_name(), "substitute-a-with-c(UNNAMED)");
    let entries = [("b".to_string(), "d".to_string())];
    substitute(
        &mut t,
        Substitution::Relabel {
            entries: &entries,
            in_order: false,
            compose: false,
            file_name: "f.tsv",
        },
        &Quiet::default(),
    )
    .expect("relabels");
    assert_eq!(paths_of(&t), ["cd:cd 0"]);
    assert!(t.get_name().starts_with("substitute-from-f.tsv("));
    name_substituted(
        &mut t,
        SubstitutionName::Transducer {
            from: "c",
            file_name: "n.hfst",
        },
    );
    assert!(t.get_name().starts_with("substitute-c-with-net-n.hfst("));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn pmatch_input_splits_into_lines_or_paragraphs() {
    let text = "one\ntwo\n\nthree\n";
    assert_eq!(
        InputSplitter::split(text, false),
        ["one", "two", "", "three"]
    );
    assert_eq!(InputSplitter::split(text, true), ["one\ntwo", "three"]);
    assert_eq!(InputSplitter::split("a\n\n", true), ["a"]);
}

fn olw(src: &str) -> AnyTransducer {
    AnyTransducer::Tropical(regex(src))
        .convert(
            crate::hfst_data_types::ImplementationType::HFST_OLW_TYPE,
            "",
        )
        .expect("converts")
}

// [spec:hfst:req:embed.tools-in-library/test]
// Each display variant of `hfst optimized-lookup` selects and orders as the
// tool prints.
#[test]
fn optimized_lookup_selects_as_each_variant_prints() {
    let t = olw("{cat}:{dog}::2 | {cat}:{cow}::1 | {cat}:{dog}::3");
    let mut lookup = OptimizedLookup::default();
    lookup.push_borrowed(&t).expect("pushes");
    assert!(lookup.weighted());
    let look = |options: &OptimizedLookupOptions| lookup.look_up("cat", options, &mut |_| {});
    let OptimizedLookupOutcome::Analyses(all) = look(&OptimizedLookupOptions::default()) else {
        panic!("cat tokenizes");
    };
    assert_eq!(all[0], ("cow".to_string(), 1.0));
    let unique = OptimizedLookupOptions {
        unique: true,
        ..OptimizedLookupOptions::default()
    };
    let OptimizedLookupOutcome::Analyses(unique) = look(&unique) else {
        panic!("cat tokenizes");
    };
    assert_eq!(unique.len(), 2);
    let beam = OptimizedLookupOptions {
        beam: 0.5,
        ..OptimizedLookupOptions::default()
    };
    assert_eq!(
        look(&beam),
        OptimizedLookupOutcome::Analyses(vec![("cow".to_string(), 1.0)])
    );
    assert_eq!(
        lookup.look_up("xyz", &OptimizedLookupOptions::default(), &mut |_| {}),
        OptimizedLookupOutcome::Untokenizable
    );
    let printed = text(|out| {
        let outcome = look(&beam);
        lookup
            .write(out, "cat", &outcome, &beam, true)
            .expect("written");
        lookup
            .write(
                out,
                "xyz",
                &OptimizedLookupOutcome::Untokenizable,
                &beam,
                true,
            )
            .expect("written");
    });
    assert_eq!(printed, "cat\tcow\t1\n\nxyz\txyz\t+?\n\n\n\n");
}

fn two_level(pairs: &[(&str, &str)], weight: f32) -> HfstTwoLevelPath {
    HfstTwoLevelPath {
        first: weight,
        second: pairs
            .iter()
            .map(|(i, o)| (Symbol::new(i), Symbol::new(o)))
            .collect(),
    }
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn paths_print_as_fst2strings_prints_them() {
    let path = two_level(
        &[
            ("c", "c"),
            ("a", "o"),
            ("@_EPSILON_SYMBOL_@", "w"),
            ("@P.X.Y@", "@P.X.Y@"),
        ],
        1.5,
    );
    let plain = PathFormat::default();
    assert_eq!(path_line(&path, &plain), "ca:cow\n");
    let shown = PathFormat {
        print_weights: true,
        print_pairs: true,
        show_flags: true,
        epsilon_format: "0".to_string(),
        ..PathFormat::default()
    };
    assert_eq!(path_line(&path, &shown), "ca:o0:w@P.X.Y@\t1.5\n");
    let spaced = PathFormat {
        print_spaces: true,
        quote_special: true,
        ..PathFormat::default()
    };
    let quoted = two_level(&[("a b", "a b")], 0.0);
    assert_eq!(path_line(&quoted, &spaced), "a@_SPACE_@b \n");
}

// The selector carries each path's strings over to the paths that extend it,
// as a walk's order allows. Random paths come in no such order: they come
// sorted, so `ba` follows `ab`, and judging `ba` on `ab`'s first symbol would
// read it as `aa` and drop it.
// [spec:hfst:sem:hfst-fst2strings.callback.operator-fn+1/test]
#[test]
fn random_paths_are_each_judged_whole() {
    let t = regex("{aa} | {ab} | {ba} | {bb}");
    let query = PathQuery {
        random: 200,
        in_prefix: "b".to_string(),
        ..PathQuery::default()
    };
    let mut found = std::collections::BTreeSet::new();
    extract_paths(
        &t,
        &query,
        &mut |p| {
            found.insert(p.second.iter().map(|(i, _)| i.as_str()).collect::<String>());
        },
        &mut |_| {},
    )
    .expect("finds");
    assert_eq!(found, ["ba", "bb"].map(String::from).into());
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn paths_are_selected_as_fst2strings_selects_them() {
    let t = regex("{cat}::1 | {cow}::2 | {dog}::3");
    let mut found = Vec::new();
    let query = PathQuery {
        in_prefix: "c".to_string(),
        ..PathQuery::default()
    };
    extract_paths(&t, &query, &mut |p| found.push(p.first), &mut |_| {}).expect("finds");
    assert_eq!(found, [1.0, 2.0]);
    let mut found = Vec::new();
    let nbest = PathQuery {
        nbest: 1,
        ..PathQuery::default()
    };
    extract_paths(&t, &nbest, &mut |p| found.push(p.first), &mut |_| {}).expect("finds");
    assert_eq!(found, [1.0]);
    let cyclic = regex("a*");
    let refused = extract_paths(&cyclic, &PathQuery::default(), &mut |_| {}, &mut |_| {});
    assert!(matches!(refused, Err(Fst2StringsError::Cyclic)));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn comparisons_report_as_the_tool_prints() {
    let mut first = regex("\"@P.X.Y@\" a");
    let second = regex("a");
    let options = CompareOptions {
        harmonize: true,
        eliminate_flags: true,
    };
    let equal = compare(
        &mut first,
        &mut Operand::Borrowed(&second),
        &options,
        &mut || {},
    )
    .expect("compares");
    assert!(equal);
    let printed = text(|out| {
        write_comparison(out, ("a", "b"), (1, 1), true).expect("written");
        write_comparison(out, ("a", "b"), (2, 1), false).expect("written");
    });
    assert_eq!(printed, "a == b\na[2] != b[1]\n");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn alphabets_dump_in_each_format() {
    let alphabets = Alphabets::of(&AnyTransducer::Tropical(regex("a %+Tag"))).expect("dumps");
    assert!(alphabets.seen.contains("+Tag"));
    assert!(is_multichar("+Tag"));
    assert!(!is_multichar("a"));
    let tsv = text(|out| {
        write_alphabets(out, &alphabets, AlphabetFormat::Tsv, (false, true)).expect("written");
    });
    assert_eq!(tsv, "+Tag\na\n");
    let tags = text(|out| {
        write_alphabets(out, &alphabets, AlphabetFormat::Vislcg3Tags, (false, true))
            .expect("written");
        write_end(out, AlphabetFormat::Vislcg3Tags).expect("written");
    });
    assert!(tags.ends_with("STRICT-TAGS +=\n\t+Tag\n\t;\n"), "{tags}");
    let list = text(|out| {
        write_alphabets(out, &alphabets, AlphabetFormat::Vislcg3List, (false, true))
            .expect("written");
    });
    assert!(list.ends_with("LIST +Tag = +Tag ;\n"), "{list}");
}

fn one_level(symbols: &[&str], weight: f32) -> HfstOneLevelPath {
    HfstOneLevelPath {
        first: weight,
        second: symbols.iter().copied().map(Symbol::new).collect(),
    }
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn lookups_report_within_the_beam() {
    let mut kvs = HfstOneLevelPaths::new();
    kvs.insert(one_level(&["a"], 1.0));
    kvs.insert(one_level(&["b", "@P.X.Y@", "@_EPSILON_SYMBOL_@"], 1.5));
    kvs.insert(one_level(&["c"], 3.0));
    assert_eq!(reported_paths(&kvs, -1.0).count(), 3);
    assert_eq!(reported_paths(&kvs, 0.5).count(), 2);
    let second = reported_paths(&kvs, 0.5).nth(1).expect("a second result");
    assert_eq!(reported_symbols(second, false), ["b"]);
    assert_eq!(reported_symbols(second, true), ["b", "@P.X.Y@"]);

    let t = AnyTransducer::Tropical(regex("{cat}:{dog}::0.5"));
    let mut cascade = LookupCascade::new();
    cascade.begin_transducer(&t, "", &Quiet::default());
    cascade
        .push_borrowed(&t, &Quiet::default())
        .expect("pushes");
    let engine = LookupEngineOptions {
        obey_flags: true,
        show_flags: false,
        print_pairs: false,
        print_space: false,
        quote_special: false,
        epsilon_format: String::new(),
        beam: -1.0,
        time_cutoff: 0.0,
        infinite_cutoff: 5,
        cascade: CascadeVariant::Union,
        flags: FlagPolicy::InLookup,
        ambiguity: AmbiguityLimit::MaxResults {
            max_number: -1,
            default_max: 5,
        },
        pair_style: PairPrintStyle::Lookup,
    };
    let lookup = Lookup::new(cascade, engine, LookupInputFormat::Utf8TokenInput).expect("ready");
    let found = lookup
        .look_up(
            "cat",
            &Quiet::default(),
            &mut std::io::sink(),
            &mut std::io::sink(),
        )
        .expect("looks up");
    assert!(!found.outside_alphabet);
    let first = found.results.iter().next().expect("a result");
    assert_eq!(reported_symbols(first, false).concat(), "dog");
    let unknown = lookup
        .look_up(
            "xyz",
            &Quiet::default(),
            &mut std::io::sink(),
            &mut std::io::sink(),
        )
        .expect("looks up");
    assert!(unknown.results.is_empty());
}

fn engine(obey_flags: bool, flags: FlagPolicy) -> LookupEngineOptions {
    LookupEngineOptions {
        obey_flags,
        show_flags: false,
        print_pairs: false,
        print_space: false,
        quote_special: false,
        epsilon_format: String::new(),
        beam: -1.0,
        time_cutoff: 0.0,
        infinite_cutoff: 5,
        cascade: CascadeVariant::Union,
        flags,
        ambiguity: AmbiguityLimit::Cycles,
        pair_style: PairPrintStyle::Flookup,
    }
}

// [spec:hfst:req:embed.tools-in-library/test]
// flookup inverts each transducer unless told to look up as it is, through
// tropical OpenFst for optimized lookup, which it refuses unless forced;
// optimized lookup prints no pairs, and lookup obeys its flags.
#[test]
fn lookup_cascades_refuse_as_the_tools_do() {
    use super::lookup::{CascadeError, FlookupCascade};
    let quiet = Quiet::default();
    let ol = || {
        AnyTransducer::Tropical(regex("{cat}:{dog}"))
            .convert(
                crate::hfst_data_types::ImplementationType::HFST_OLW_TYPE,
                "",
            )
            .expect("converts")
    };
    let mut cascade = FlookupCascade::new(false, false);
    assert!(matches!(
        cascade.push(ol(), "", &quiet),
        Err(CascadeError::InvertsOptimizedLookup)
    ));
    let lookup_inverted = |as_is: bool, force_ol: bool, t: AnyTransducer, word: &str| {
        let mut cascade = FlookupCascade::new(as_is, force_ol);
        cascade.push(t, "", &quiet).expect("pushes");
        let cascade = cascade.finish(false).expect("finishes");
        let lookup = Lookup::new(
            cascade,
            engine(true, FlagPolicy::PostFilter),
            LookupInputFormat::Utf8TokenInput,
        )
        .expect("ready");
        let found = lookup
            .look_up(word, &quiet, &mut std::io::sink(), &mut std::io::sink())
            .expect("looks up");
        found
            .results
            .iter()
            .map(|path| reported_symbols(path, false).concat())
            .collect::<Vec<_>>()
    };
    assert_eq!(lookup_inverted(true, false, ol(), "cat"), ["dog"]);
    assert_eq!(lookup_inverted(false, true, ol(), "dog"), ["cat"]);
    let tropical = AnyTransducer::Tropical(regex("{cat}:{dog}"));
    assert_eq!(lookup_inverted(false, false, tropical, "dog"), ["cat"]);

    let mut cascade = FlookupCascade::new(true, false);
    cascade.push(ol(), "", &quiet).expect("pushes");
    assert!(matches!(
        cascade.finish(true),
        Err(CascadeError::PairsOnOptimizedLookup)
    ));

    let t = ol();
    let mut cascade = LookupCascade::new();
    cascade.begin_transducer(&t, "", &quiet);
    cascade.push_borrowed(&t, &quiet).expect("pushes");
    assert!(matches!(
        Lookup::new(
            cascade,
            engine(false, FlagPolicy::InLookup),
            LookupInputFormat::Utf8TokenInput
        ),
        Err(CascadeError::IgnoresFlagsOnOptimizedLookup)
    ));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn guesses_print_as_the_tool_prints() {
    let printed = text(|out| {
        write_guess(
            out,
            "cats",
            &GuessReport::Guesses(vec!["cat+N".to_string()]),
        )
        .expect("written");
        write_guess(
            out,
            "dogs",
            &GuessReport::Paradigms(vec!["dogs\tdog+N\tdog".to_string()]),
        )
        .expect("written");
    });
    assert_eq!(printed, "cats\tcat+N\n\ndogs\tdog+N\tdog\n\n");
}
