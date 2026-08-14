//! Golden smoke test for `hfst-optimized-lookup`.
//!
//! `tests/fixtures/lookup.hfstol` is a committed optimized-lookup transducer
//! accepting {cat, cats, dog, dogs} (built with `hfst-strings2fst -j |
//! hfst-fst2fst -O`); `tests/fixtures/lookup.golden` is the tool's output for a
//! fixed query set. This is the regression oracle for `librarify.ol-reuse`:
//! gutting the tool to call the library OL engine (`crate::transducer`) must
//! preserve this exact output. The tool engine and the library engine were
//! verified to agree on the analyses (only the per-tool Xerox formatting is
//! tool-specific), so byte-equality here is the right contract.

use std::io::Write;
use std::process::{Command, Stdio};

const QUERIES: &[u8] = b"cat\ncats\ndog\ndogs\nxyz\n";

fn lookup(args: &[&str], queries: &[u8]) -> String {
    let dir = env!("CARGO_MANIFEST_DIR");
    let fixture = format!("{dir}/tests/fixtures/lookup.hfstol");

    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .arg("optimized-lookup")
        .args(args)
        .arg(&fixture)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn hfst-optimized-lookup");

    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(queries)
        .expect("write queries to stdin");

    let out = child
        .wait_with_output()
        .expect("wait for hfst-optimized-lookup");
    assert!(out.status.success(), "tool exited with {:?}", out.status);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn optimized_lookup_matches_golden() {
    let golden = include_str!("fixtures/lookup.golden");
    assert_eq!(
        lookup(&[], QUERIES),
        golden,
        "hfst-optimized-lookup output drifted from the golden oracle"
    );
}

/// One blank line terminates every record, whether the word was analysed or
/// not. PORT DIVERGENCE: upstream prints the `+?` record with its blank-line
/// terminator and then unconditionally prints two more (hfst-optimized-lookup.cc
/// lines 651-659 for tokenization failure, 1276-1284 for no analyses), so an
/// unknown word is followed by three blanks where a known word is followed by
/// one. Anything splitting the stream on blank lines reads two empty records
/// out of upstream after every unknown word. The golden above has its unknown
/// word last, where the difference is invisible; this puts one mid-stream.
#[test]
fn an_unknown_word_terminates_like_any_other() {
    assert_eq!(
        lookup(&[], b"cat\nxyz\ndog\n"),
        "cat\tcat\n\nxyz\txyz\t+?\n\ndog\tdog\n\n"
    );
}

/// Fast mode streams each analysis out as it is found, with no prepend column
/// and no blank-line terminator; the accumulate-then-format path is what it
/// skips. This is unweighted-only — `Transducer::note_analysis` is the sole
/// variant with a `beFast` branch, and the sole one whose `printAnalyses` is
/// `!beFast`-guarded — so porting the guard without the streaming half printed
/// nothing whatsoever.
///
/// The unknown word is unaffected: tokenization failure prints its `+?` record
/// from a path upstream never guarded on `beFast`.
#[test]
fn fast_mode_streams_analyses_without_the_prepend_column() {
    assert_eq!(
        lookup(&["-f"], b"cat\nxyz\ndog\n"),
        "cat\nxyz\txyz\t+?\n\ndog\n"
    );
}

/// Two different reasons for having no analysis, told apart. `act` is spelled
/// entirely from the alphabet and simply is not in the language; `xyz` is not
/// spellable at all. Only the second is reported in fast mode, and only the
/// first goes through a lookup — searching for an analysis of a word the
/// alphabet cannot even express is work with a known answer.
#[test]
fn untokenizable_and_unanalysable_are_different() {
    assert_eq!(lookup(&["-f"], b"act\n"), "");
    assert_eq!(lookup(&["-f"], b"xyz\n"), "xyz\txyz\t+?\n\n");
    // Outside fast mode both report, which is why the distinction stayed
    // invisible until fast mode was fixed.
    assert_eq!(lookup(&[], b"act\n"), "act\tact\t+?\n\n");
    assert_eq!(lookup(&[], b"xyz\n"), "xyz\txyz\t+?\n\n");
}
