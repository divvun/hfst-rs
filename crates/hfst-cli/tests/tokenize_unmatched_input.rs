//! `hfst tokenize` against a plain `.hfstol` dictionary must emit input the
//! dictionary cannot analyse, not discard it.
//!
//! The naive tokenizer built for a non-`TOP` transducer has an `others`
//! fallback — `make_exc_list(word_boundary)+` — precisely so that a run of
//! dictionary-external characters still becomes a token. Upstream weights that
//! fallback with `std::numeric_limits<float>::max()`, which is roughly 2^128
//! times the `INFINITE_WEIGHT` cutoff (`NO_TABLE_INDEX as f32`) every `locate`
//! call passes, so `PmatchTransducer::get_analyses` abandons the branch before
//! it can accept. `dogs cot cats` came out as `dogs\ncats`: `cot` vanished
//! although every one of its letters is in the dictionary's alphabet.
//!
//! PORT DIVERGENCE ([dec:hfst:independent-fork]): a tokenizer that discards
//! input is a data-loss bug, so this port weights the fallback with
//! `UNANALYSED_WEIGHT` — the largest weight the runtime admits — and reports
//! the resulting reading with each format's own unknown-material marking.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn run(args: &[&str], stdin: &[u8]) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn hfst");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// A plain weighted `.hfstol` holding `dogs` and `cats` but not `cot` — the
/// shape `hfst tokenize` falls back to its naive tokenizer for, since the
/// transducer carries no `TOP` name. Built per test: nextest runs these in
/// parallel, and a shared path would have one test reading the file another is
/// still writing.
fn dictionary(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hfst-tokenize-unmatched-{test}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let strings = dir.join("dict.txt");
    let hfst = dir.join("dict.hfst");
    let hfstol = dir.join("dict.hfstol");
    std::fs::write(&strings, "dogs:dogs+N+Pl\ncat:cat+N+Sg\ncats:cat+N+Pl\n")
        .expect("write dictionary strings");
    let (ok, _) = run(
        &["strings2fst", "-j", "-i", path(&strings), "-o", path(&hfst)],
        b"",
    );
    assert!(ok, "strings2fst failed to build the dictionary");
    let (ok, _) = run(
        &["fst2fst", "-O", "-i", path(&hfst), "-o", path(&hfstol)],
        b"",
    );
    assert!(ok, "fst2fst -O failed to build the optimized-lookup form");
    hfstol
}

fn path(p: &Path) -> &str {
    p.to_str().expect("utf8 path")
}

const INPUT: &[u8] = b"dogs cot cats\n\n";

/// The reported case: default output is one token per line, and the token the
/// dictionary has no analysis for is one of them.
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.unanalysed-weight/test]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.unanalysed-weight/test]
// [spec:hfst:def:hfst-tokenize.make-naive-tokenizer-fn/test]
// [spec:hfst:sem:hfst-tokenize.make-naive-tokenizer-fn/test]
#[test]
fn default_mode_emits_the_unmatched_run() {
    let dict = dictionary("default");
    let (ok, o) = run(&["tokenize", path(&dict)], INPUT);
    assert!(ok, "hfst tokenize failed on the plain dictionary");
    let tokens: Vec<&str> = o.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        tokens,
        vec!["dogs", "cot", "cats"],
        "tokenize dropped dictionary-external input:\n{o}"
    );
}

/// `-a` keeps the separators as well, so the token stream still reconstructs
/// the input verbatim — and `cot` is a token of its own rather than being
/// glued to the surrounding spaces in one nonmatching blob.
#[test]
fn print_all_reconstructs_input_around_unmatched_run() {
    let dict = dictionary("printall");
    let (ok, o) = run(&["tokenize", "-a", path(&dict)], INPUT);
    assert!(ok, "hfst tokenize -a failed on the plain dictionary");
    let pieces: Vec<&str> = o.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        pieces,
        vec!["dogs", " ", "cot", " ", "cats"],
        "tokenize -a did not segment the unmatched run:\n{o}"
    );
    assert_eq!(
        pieces.concat(),
        "dogs cot cats",
        "tokenize -a no longer reconstructs its input:\n{o}"
    );
}

/// The unanalysed token must be distinguishable from an analysis. Every format
/// with an analysis column already has a spelling for unknown material; the
/// fallback reading is reported with that, not as a bare reading a CG grammar
/// would read as a real analysis.
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-unanalysed-location-fn/test]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-unanalysed-location-fn/test]
#[test]
fn analysis_formats_mark_the_unmatched_run_as_unknown() {
    let dict = dictionary("formats");
    let (ok, cg) = run(&["tokenize", "-c", path(&dict)], INPUT);
    assert!(ok, "hfst tokenize -c failed on the plain dictionary");
    assert!(
        cg.contains("\"<cot>\"\n\t\"cot\" ?\n"),
        "cg output did not mark the unmatched run as unknown:\n{cg}"
    );

    let (ok, giella) = run(&["tokenize", "-g", path(&dict)], INPUT);
    assert!(ok, "hfst tokenize -g failed on the plain dictionary");
    assert!(
        giella.contains("\"<cot>\"\n\t\"cot\" ?\n"),
        "giellacg output did not mark the unmatched run as unknown:\n{giella}"
    );

    let (ok, xerox) = run(&["tokenize", "--xerox", path(&dict)], INPUT);
    assert!(ok, "hfst tokenize --xerox failed on the plain dictionary");
    assert!(
        xerox.contains("cot\tcot+?\tinf\n"),
        "xerox output did not mark the unmatched run as unknown:\n{xerox}"
    );

    let (ok, weighted) = run(&["tokenize", "-w", path(&dict)], INPUT);
    assert!(ok, "hfst tokenize -w failed on the plain dictionary");
    assert!(
        weighted.contains("cot\tinf\n"),
        "an unanalysed token should weigh `inf`, not a sentinel integer:\n{weighted}"
    );
}

/// The fallback accepts any run of non-boundary characters, so it covers every
/// analysed token too. It must not surface as an extra empty reading there.
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn/test]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn/test]
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.match-and-print-fn/test]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.match-and-print-fn/test]
#[test]
fn analysed_tokens_do_not_gain_a_fallback_reading() {
    let dict = dictionary("nopollute");
    let (ok, cg) = run(&["tokenize", "-c", path(&dict)], INPUT);
    assert!(ok, "hfst tokenize -c failed on the plain dictionary");
    let dogs = cg
        .split("\"<dogs>\"\n")
        .nth(1)
        .expect("a cohort for dogs")
        .split("\n\n")
        .next()
        .expect("the readings of that cohort");
    assert_eq!(
        dogs, "\t\"dogs\"+N+Pl",
        "the fallback leaked an unanalysed reading into an analysed cohort:\n{cg}"
    );

    let (ok, conllu) = run(&["tokenize", "-C", path(&dict)], INPUT);
    assert!(ok, "hfst tokenize -C failed on the plain dictionary");
    for (n, form) in [(1, "dogs"), (2, "cot"), (3, "cats")] {
        assert!(
            conllu.contains(&format!("{n}\t{form}\t")),
            "conllu lost the FORM column for {form}:\n{conllu}"
        );
    }
}

/// Naming the unanalysed test once also settles a disagreement between the two
/// CG writers. `" ??"` is the unknown marker a pmatch script emits; giellacg
/// has always dropped it where a real reading survives, but plain cg printed
/// it, so a grammar saw a tagless `"w" ??` reading beside every analysis. Over
/// lang-sma's free corpus that was 778,856 spurious readings.
#[test]
fn plain_cg_marks_unknown_like_giellacg() {
    let dir = std::env::temp_dir().join("hfst-tokenize-unmatched-qqmarker");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let src = dir.join("g.pmatch");
    let archive = dir.join("g.pmhfst");
    // `cat` gets a real reading and an unknown marker; `dog` only the marker.
    std::fs::write(
        &src,
        "Define TOP [ {cat}:{cat N} | {cat}:{cat ??} | {dog}:{dog ??} ] EndTag(w) ;\n",
    )
    .expect("write grammar");
    let (ok, _) = run(&["pmatch2fst", "-i", path(&src), "-o", path(&archive)], b"");
    assert!(
        ok,
        "pmatch2fst failed to compile the unknown-marker grammar"
    );

    let (ok, cg) = run(&["tokenise", "-c", path(&archive)], b"cat dog\n\n");
    assert!(ok, "hfst tokenise -c failed on the unknown-marker archive");
    assert_eq!(
        cg, "\"<cat>\"\n\t\"cat\" N\n\n\"<dog>\"\n\t\"dog\" ?\n\n",
        "plain cg did not handle the unknown marker the way giellacg does:\n{cg}"
    );
}

/// Characters outside the dictionary's alphabet entirely take the same path.
#[test]
fn out_of_alphabet_runs_are_emitted_too() {
    let dict = dictionary("outofalpha");
    let (ok, o) = run(&["tokenize", path(&dict)], "dogs кот cats\n\n".as_bytes());
    assert!(ok, "hfst tokenize failed on out-of-alphabet input");
    let tokens: Vec<&str> = o.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        tokens,
        vec!["dogs", "кот", "cats"],
        "tokenize dropped out-of-alphabet input:\n{o}"
    );
}
