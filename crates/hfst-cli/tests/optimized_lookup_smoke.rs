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

/// An analysed record ends with one blank line and an unanalysable one with
/// three — upstream's punctuation, matched deliberately because this is what
/// Giella pipelines parse. The golden above has its unknown word last, where
/// the trailing blanks are easy to miss; this puts one mid-stream, where a
/// consumer splitting on blank lines sees the two empty records they produce.
#[test]
fn an_unanalysable_word_ends_with_three_blanks() {
    assert_eq!(
        lookup(&[], b"cat\nxyz\ndog\n"),
        "cat\tcat\n\nxyz\txyz\t+?\n\n\n\ndog\tdog\n\n"
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
        "cat\nxyz\txyz\t+?\n\n\n\ndog\n"
    );
}

/// The weighted variants terminate a no-analysis record with ONE blank line
/// where the unweighted ones use three, because upstream bracketed the same
/// copy-pasted block inside `#ifdef WINDOWS` at those two sites and outside it
/// at the others. Both are matched, so this pins the exception against being
/// tidied into uniformity later. The untokenizable word in the same run takes
/// the three-blank path regardless of weighting, since that record is printed
/// before any variant is consulted.
#[test]
fn weighted_no_analysis_ends_with_one_blank() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let regex = dir.join("weighted.hfst");
    let fixture = dir.join("weighted.hfstol");

    run(&["regexp2fst", "-o"], &regex, b"{cat}:{cat+N}::0.5\n");
    let converted = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(["fst2fst", "-f", "olw", "-i"])
        .arg(&regex)
        .arg("-o")
        .arg(&fixture)
        .status()
        .expect("spawn hfst-fst2fst");
    assert!(converted.success(), "conversion exited with {converted:?}");

    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .arg("optimized-lookup")
        .arg(&fixture)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn hfst-optimized-lookup");
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(b"cat\nact\nxyz\n")
        .expect("write queries to stdin");
    let out = child.wait_with_output().expect("wait for the tool");

    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "cat\tcat+N\n\nact\tact\t+?\n\nxyz\txyz\t+?\n\n\n\n"
    );
}

/// Flag diacritics must not reach the printed analysis, and must still filter
/// the traversal.
///
/// Upstream selects a whole parallel class hierarchy (TransducerFd and
/// friends) when the alphabet declares flag diacritics, and blanks every flag
/// symbol's key-table entry while parsing the alphabet, so its analyses never
/// contain one. This port runs the library optimized-lookup engine, whose
/// alphabet keeps flag strings because other callers need them, so the tool
/// filters them at the display layer instead — exactly as hfst-lookup and
/// hfst-flookup do. Before the filter existed, `dog` printed
/// `@U.F.A@dog+N+Sg`.
///
/// `cat` is the filtering half: its path sets F=A and then demands F=B, so a
/// flag-aware traversal rejects it and it comes back `+?`. Both
/// optimized-lookup formats are checked, since the flag axis is orthogonal to
/// the weighted/unweighted one — and they punctuate the `+?` record
/// differently, one blank line against three.
#[test]
fn flags_filter_the_traversal_without_reaching_output() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let regex = dir.join("flagged.hfst");
    run(
        &["regexp2fst", "-o"],
        &regex,
        b"[ \"@U.F.A@\" {dog}:{dog+N+Sg} | \"@U.F.A@\" {cat}:{cat+N+Sg} \"@U.F.B@\" ]\n",
    );

    for (format, expected) in [
        ("olw", "dog\tdog+N+Sg\n\ncat\tcat\t+?\n\n"),
        ("olu", "dog\tdog+N+Sg\n\ncat\tcat\t+?\n\n\n\n"),
    ] {
        let fixture = dir.join(format!("flagged.{format}"));
        let converted = Command::new(env!("CARGO_BIN_EXE_hfst"))
            .args(["fst2fst", "-f", format, "-i"])
            .arg(&regex)
            .arg("-o")
            .arg(&fixture)
            .status()
            .expect("spawn hfst-fst2fst");
        assert!(converted.success(), "conversion exited with {converted:?}");

        let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
            .arg("optimized-lookup")
            .arg(&fixture)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn hfst-optimized-lookup");
        child
            .stdin
            .take()
            .expect("stdin was piped")
            .write_all(b"dog\ncat\n")
            .expect("write queries to stdin");
        let out = child.wait_with_output().expect("wait for the tool");

        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            expected,
            "flag-diacritic handling drifted for the {format} format"
        );
    }
}

/// Compile `source` through `args` (which must end in the output flag) into
/// `out`.
fn run(args: &[&str], out: &std::path::Path, source: &[u8]) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(args)
        .arg(out)
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn compiler");
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(source)
        .expect("write source to stdin");
    let status = child.wait().expect("wait for compiler");
    assert!(status.success(), "compiler exited with {status:?}");
}

/// Two different reasons for having no analysis, told apart. `act` is spelled
/// entirely from the alphabet and simply is not in the language; `xyz` is not
/// spellable at all. Only the second is reported in fast mode, and only the
/// first goes through a lookup — searching for an analysis of a word the
/// alphabet cannot even express is work with a known answer.
#[test]
fn untokenizable_and_unanalysable_are_different() {
    assert_eq!(lookup(&["-f"], b"act\n"), "");
    assert_eq!(lookup(&["-f"], b"xyz\n"), "xyz\txyz\t+?\n\n\n\n");
    // Outside fast mode both report, which is why the distinction stayed
    // invisible until fast mode was fixed. This fixture is unweighted, so both
    // take the three-blank terminator; a weighted transducer would end the
    // no-analysis case with a single blank instead.
    assert_eq!(lookup(&[], b"act\n"), "act\tact\t+?\n\n\n\n");
    assert_eq!(lookup(&[], b"xyz\n"), "xyz\txyz\t+?\n\n\n\n");
}
