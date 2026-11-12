//! The writers fst2fst and fst2txt share with the compile surface: DHFST
//! files, THFST members, and text.

use super::dhfst::{DhfstKind, DhfstOptions, write_dhfst};
use super::fst2txt::{TextError, TextFormat, TextNote, TextOptions, write_text};
use super::test_support::regex;
use crate::file_resolver::MemoryFiles;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_transducer::AnyTransducer;

/// The text of `ts` as one stream, and the notes it gave.
fn text_of(ts: &[AnyTransducer], options: &TextOptions) -> (String, Vec<TextNote>) {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    for (i, t) in ts.iter().enumerate() {
        write_text(t, i + 1, options, &mut out, &mut |n| notes.push(n)).expect("written");
    }
    (String::from_utf8(out).expect("utf8"), notes)
}

// [spec:hfst:def:hfst-fst2txt.fst-text-format/test]
// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn text_formats_go_by_the_tools_names() {
    for (name, format) in [
        ("att", TextFormat::Att),
        ("AT&T", TextFormat::Att),
        ("OpenFst", TextFormat::Att),
        ("graphviz", TextFormat::Dot),
        ("pckimmo", TextFormat::Pckimmo),
        ("Prolog", TextFormat::Prolog),
    ] {
        assert_eq!(TextFormat::from_name(name), Some(format), "{name}");
    }
    assert_eq!(TextFormat::from_name("PCKIMMO"), None);
    let options = TextOptions::default();
    assert!(options.weights_of(ImplementationType::TROPICAL_OPENFST_TYPE));
    assert!(!options.weights_of(ImplementationType::FOMA_TYPE));
    let forced = TextOptions {
        print_weights: true,
        do_not_print_weights: true,
        ..TextOptions::default()
    };
    assert!(forced.weights_of(ImplementationType::FOMA_TYPE));
}

// [spec:hfst:sem:hfst-fst2txt.process-stream-fn/test]
// [spec:hfst:req:embed.writers/test]
// A stream's transducers are written one after another, a `--` line
// between them, and Prolog names each for its place.
#[test]
fn a_stream_is_written_transducer_by_transducer() {
    let mut named = regex("b");
    named.set_name("bee");
    let stream = [
        AnyTransducer::Tropical(regex("a::0.5")),
        AnyTransducer::Tropical(named),
    ];
    let (att, notes) = text_of(&stream, &TextOptions::default());
    assert_eq!(
        att,
        "0\t1\ta\ta\t0.500000\n1\t0.000000\n--\n0\t1\tb\tb\t0.000000\n1\t0.000000\n"
    );
    assert!(notes.is_empty());
    let prolog = TextOptions {
        format: TextFormat::Prolog,
        do_not_print_weights: true,
        ..TextOptions::default()
    };
    let (text, notes) = text_of(&stream, &prolog);
    assert!(text.starts_with("network(NO_NAME_1)."), "{text}");
    assert!(text.contains("--\nnetwork(NO_NAME_2)."), "{text}");
    assert_eq!(
        notes,
        [
            TextNote::Named("NO_NAME_1".to_string()),
            TextNote::Renamed("NO_NAME_2".to_string())
        ]
    );
    assert_eq!(
        notes[1].to_string(),
        "Renaming transducer into 'NO_NAME_2'..."
    );
    let dot = TextOptions {
        format: TextFormat::Dot,
        ..TextOptions::default()
    };
    let (text, _) = text_of(&stream[1..], &dot);
    assert!(
        text.starts_with("// This graph generated with hfst-fst2txt\ndigraph \"bee\" {\n"),
        "{text}"
    );
    let failed = TextError::Write(std::io::Error::other("full"));
    assert_eq!(
        failed.to_string(),
        "Error encountered when writing in text format: full"
    );
}

// [spec:hfst:sem:dhfst.fst2fst+2/test]
// [spec:hfst:req:embed.writers/test]
// Either kind of DHFST file is written, checked, and reported on, and the
// source name rides in its meta section.
#[test]
fn dhfst_files_are_written_with_their_report() {
    let errmodel = write_dhfst(
        AnyTransducer::Tropical(regex("[a:b | b:a | a | b]*")),
        &DhfstOptions {
            source_name: "e.hfst".to_string(),
            ..DhfstOptions::new(DhfstKind::ErrorModel)
        },
    )
    .expect("an error model");
    assert!(errmodel.bytes.starts_with(b"DHFST\x01"));
    assert!(
        errmodel.report.starts_with("DHFST source: "),
        "{}",
        errmodel.report
    );
    assert!(
        errmodel.report.contains("depth bound 4"),
        "{}",
        errmodel.report
    );
    assert!(
        errmodel
            .report
            .ends_with(&format!("wrote {} bytes of DHFST\n", errmodel.bytes.len()))
    );
    assert!(errmodel.bytes.windows(6).any(|w| w == b"e.hfst"));
    let acceptor = write_dhfst(
        AnyTransducer::Tropical(regex("{cat} | {dog}")),
        &DhfstOptions::new(DhfstKind::Acceptor),
    )
    .expect("an acceptor");
    assert!(acceptor.bytes.starts_with(b"DHFST\x02"));
    assert!(
        acceptor.report.starts_with("DHFST acceptor: "),
        "{}",
        acceptor.report
    );
}

// [spec:hfst:req:embed.writers/test]
// THFST members written through files in memory land under the directory's
// name, in the order the directory writer writes them.
#[test]
fn thfst_members_write_through_a_resolver() {
    let converted = AnyTransducer::Tropical(regex("{cat}"))
        .convert(ImplementationType::THFST_TYPE, "")
        .expect("converts");
    let AnyTransducer::Thfst(thfst) = converted else {
        panic!("a THFST transducer");
    };
    let members = thfst.thfst_members().expect("members");
    let files = MemoryFiles::default();
    members.write_to("x.thfst", &files).expect("written");
    assert_eq!(files.get("x.thfst/index"), Some(members.index.clone()));
    assert_eq!(files.get("x.thfst/transition"), Some(members.transition));
    assert_eq!(files.get("x.thfst/alphabet"), Some(members.alphabet));
}
