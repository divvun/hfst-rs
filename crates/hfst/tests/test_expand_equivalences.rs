// Tests for hfst::expand_equivalences — the equivalence-class extension logic
// lifted from hfst-expand-equivalences. These are librarify regressions (not a
// C++ test-suite port): read_tsv_extensions reproduces the tool's TSV grammar
// (de-C-ified from its getline/strstr/strndup loop), and expand_equivalences
// applies the extensions at each FSA level.

use std::io::Cursor;

use hfst::expand_equivalences::{FsaLevel, expand_equivalences, read_tsv_extensions};
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::hfst_transducer::HfstTransducer;

// The tropical transition-data symbol coding lives in process-global statics
// guarded by mutexes; cargo runs #[test]s as parallel threads in one process, so
// the transducer-building tests serialize through this lock (the pure-string TSV
// tests do not touch the symbol tables and need no lock). into_inner() recovers a
// poisoned lock so one failing test does not cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn read_tsv_extensions_parses_fields_comments_and_blanks() {
    // A comment line, a blank line, a multi-`to` line, and a single-`to` line.
    let tsv = "# a comment line\n\
               \n\
               a\tb\tc\n\
               x\ty\n";
    let pairs = read_tsv_extensions(Cursor::new(tsv)).expect("should parse");
    assert_eq!(
        pairs,
        vec![
            ("a".to_string(), "b".to_string()),
            ("a".to_string(), "c".to_string()),
            ("x".to_string(), "y".to_string()),
        ]
    );
}

#[test]
fn read_tsv_extensions_requires_a_tab() {
    let err = read_tsv_extensions(Cursor::new("noTabHere\n")).unwrap_err();
    assert_eq!(err.line, 1);
    assert!(err.message.contains("At least one tab"));
}

#[test]
fn read_tsv_extensions_rejects_empty_first_field() {
    let err = read_tsv_extensions(Cursor::new("\tb\n")).unwrap_err();
    assert_eq!(err.line, 1);
    assert!(err.message.contains("First field is empty"));
}

#[test]
fn read_tsv_extensions_rejects_trailing_tab_empty_field() {
    // "a\tb\t" -> the fields after the first tab are ["b", ""]; the trailing
    // empty is the same error the C++ raised via its final strndup(.., 0).
    let err = read_tsv_extensions(Cursor::new("a\tb\t\n")).unwrap_err();
    assert_eq!(err.line, 1);
    assert!(err.message.contains("Extension field seems empty"));
}

#[test]
fn read_tsv_extensions_comment_needs_no_tab() {
    // A '#' line that DOES contain a tab is data, not a comment — matching the
    // C++ gate, where the comment check only fires when strstr finds no tab.
    let pairs = read_tsv_extensions(Cursor::new("#x\ty\n")).expect("parses");
    assert_eq!(pairs, vec![("#x".to_string(), "y".to_string())]);
}

#[test]
fn expand_equivalences_applies_extensions_at_each_level() {
    let _g = serialized();
    let pairs = [("a".to_string(), "b".to_string())];
    let empty = HfstTransducer::new_type(TROPICAL_OPENFST_TYPE);
    let a_acc = HfstTransducer::new_symbol("a", TROPICAL_OPENFST_TYPE);

    // Second level composes the input with (identity | a:b)*, so the "a" acceptor
    // gains an a:b path and is no longer the bare acceptor.
    let extended = expand_equivalences(a_acc.clone(), &pairs, FsaLevel::Second);
    assert!(
        !extended.compare_default(&empty),
        "result must be non-empty"
    );
    assert!(
        !extended.compare_default(&a_acc),
        "Second-level extension must change the transducer"
    );

    // First and Both levels also produce non-empty transducers.
    for level in [FsaLevel::First, FsaLevel::Both] {
        let r = expand_equivalences(a_acc.clone(), &pairs, level);
        assert!(!r.compare_default(&empty), "result must be non-empty");
    }
}
