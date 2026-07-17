//! Regression locks for the optimized-lookup archive / conversion / expand
//! defects validated in 2026-07 (upstream hfst issues #395, #409, #460, #587).
//!
//! Every test drives the real `hfst` binary end to end (compile -> optimize ->
//! run) over an input that used to be mishandled, and asserts the corrected
//! behaviour:
//!
//! * #395 - `hfst-optimized-lookup` reads EVERY transducer in an archive and
//!   unions their analyses (it used to read only the first member).
//! * #409 - a single-transducer optimized-lookup file is read exactly once
//!   (no spurious "extra transducer" analyses); companion to #395.
//! * #460 - `fst2fst -w` (lookup-optimize) on a prefix acceptor keeps every
//!   output symbol (output-only symbols were dropped upstream).
//! * #587 - `fst2strings -P/-p` (prefix expansion) matches across leading /
//!   interior epsilons instead of comparing against the internal epsilon
//!   marker.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hfst-ol-archive-{name}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Run `hfst <args>` with `stdin`, returning (success, stdout).
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

/// Compile `regex` (with `-S`, single-token strings) to a tropical `.hfst`.
fn build_regex(regex: &str, out: &Path) {
    let (ok, _) = run(
        &[
            "regexp2fst",
            "-S",
            "-f",
            "openfst-tropical",
            "-o",
            out.to_str().expect("utf8 path"),
        ],
        regex.as_bytes(),
    );
    assert!(ok, "regexp2fst failed for {regex}");
}

/// Compile an ATT description to a tropical `.hfst`.
fn build_att(att: &str, out: &Path, dir: &Path) {
    let src = dir.join("in.att");
    std::fs::write(&src, att).expect("write att");
    let (ok, _) = run(
        &[
            "txt2fst",
            src.to_str().expect("utf8 path"),
            "-o",
            out.to_str().expect("utf8 path"),
        ],
        b"",
    );
    assert!(ok, "txt2fst failed");
}

/// Lookup-optimize (`fst2fst -w`) `input` into an optimized-lookup `output`.
fn to_ol(input: &Path, output: &Path) {
    let (ok, _) = run(
        &[
            "fst2fst",
            "-w",
            "-i",
            input.to_str().expect("utf8 path"),
            "-o",
            output.to_str().expect("utf8 path"),
        ],
        b"",
    );
    assert!(ok, "fst2fst -w failed");
}

/// Concatenate the raw bytes of `parts` into a single archive at `out`.
fn concat(parts: &[&Path], out: &Path) {
    let mut merged: Vec<u8> = Vec::new();
    for p in parts {
        merged.extend(std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display())));
    }
    std::fs::write(out, &merged).expect("write concatenated archive");
}

/// hfst/hfst#395: `hfst-optimized-lookup` on an archive holding two transducers
/// must UNION their analyses — the same semantics `hfst-lookup` uses — instead
/// of reading only the first member. Word `cat` matches only the first member,
/// `dog` only the second; both must be found.
#[test]
fn optimized_lookup_unions_all_archive_members() {
    let dir = scratch("395union");
    let a_hfst = dir.join("a.hfst");
    let b_hfst = dir.join("b.hfst");
    let a_ol = dir.join("a.ol");
    let b_ol = dir.join("b.ol");
    let both = dir.join("both.ol");

    build_regex("{cat}:{CAT}", &a_hfst);
    build_regex("{dog}:{DOG}", &b_hfst);
    to_ol(&a_hfst, &a_ol);
    to_ol(&b_hfst, &b_ol);
    concat(&[&a_ol, &b_ol], &both);

    let (ok, out) = run(
        &["optimized-lookup", both.to_str().expect("utf8 path")],
        b"cat\ndog\n",
    );
    assert!(
        ok,
        "optimized-lookup crashed/failed on a two-member archive"
    );
    assert!(
        out.contains("cat\tCAT"),
        "first member (cat->CAT) not matched:\n{out}"
    );
    assert!(
        out.contains("dog\tDOG"),
        "second member (dog->DOG) not matched (only the first FST was read):\n{out}"
    );
    // The second member must NOT report an analysis failure for `dog`.
    assert!(
        !out.contains("dog\tdog\t+?"),
        "the second archive member was ignored:\n{out}"
    );
}

/// hfst/hfst#395 (weighted archive): the same union must hold for weighted
/// optimized-lookup members with `-w` (show weights).
#[test]
fn optimized_lookup_unions_weighted_archive_with_weights() {
    let dir = scratch("395weighted");
    let a_hfst = dir.join("a.hfst");
    let b_hfst = dir.join("b.hfst");
    let a_ol = dir.join("a.ol");
    let b_ol = dir.join("b.ol");
    let both = dir.join("both.ol");

    build_regex("{cat}:{CAT}", &a_hfst);
    build_regex("{dog}:{DOG}", &b_hfst);
    to_ol(&a_hfst, &a_ol);
    to_ol(&b_hfst, &b_ol);
    concat(&[&a_ol, &b_ol], &both);

    let (ok, out) = run(
        &["optimized-lookup", "-w", both.to_str().expect("utf8 path")],
        b"cat\ndog\n",
    );
    assert!(ok, "optimized-lookup -w failed on a two-member archive");
    assert!(out.contains("CAT"), "cat->CAT missing:\n{out}");
    assert!(out.contains("DOG"), "dog->DOG missing:\n{out}");
    assert!(
        out.contains("0.000000"),
        "expected a printed weight column:\n{out}"
    );
}

/// hfst/hfst#395 (union dedup): with `-u` (unique), duplicate members must
/// collapse to a single analysis; without `-u`, the union keeps every copy.
#[test]
fn optimized_lookup_union_respects_unique_flag() {
    let dir = scratch("395uniq");
    let a_hfst = dir.join("a.hfst");
    let a_ol = dir.join("a.ol");
    let aa = dir.join("aa.ol");

    build_regex("{cat}:{CAT}", &a_hfst);
    to_ol(&a_hfst, &a_ol);
    concat(&[&a_ol, &a_ol], &aa);

    let (ok, uniq) = run(
        &["optimized-lookup", "-u", aa.to_str().expect("utf8 path")],
        b"cat\n",
    );
    assert!(ok, "optimized-lookup -u failed");
    assert_eq!(
        uniq.matches("CAT").count(),
        1,
        "-u must dedup the union across identical members:\n{uniq}"
    );

    let (ok, plain) = run(
        &["optimized-lookup", aa.to_str().expect("utf8 path")],
        b"cat\n",
    );
    assert!(ok, "optimized-lookup failed");
    assert_eq!(
        plain.matches("CAT").count(),
        2,
        "without -u the union keeps both members' copies:\n{plain}"
    );
}

/// hfst/hfst#409 (companion to #395): a SINGLE-transducer optimized-lookup file
/// must be read exactly once — the tool must not claim/replay extra transducers,
/// so exactly one analysis is printed and there is no spurious `+?` failure.
#[test]
fn optimized_lookup_single_transducer_read_once() {
    let dir = scratch("409single");
    let a_hfst = dir.join("a.hfst");
    let a_ol = dir.join("a.ol");

    build_regex("{cat}:{CAT}", &a_hfst);
    to_ol(&a_hfst, &a_ol);

    let (ok, out) = run(
        &["optimized-lookup", a_ol.to_str().expect("utf8 path")],
        b"cat\n",
    );
    assert!(ok, "optimized-lookup failed on a single-transducer file");
    assert_eq!(
        out.matches("CAT").count(),
        1,
        "a single transducer was read more than once:\n{out}"
    );
    assert!(
        !out.contains("+?"),
        "a spurious extra transducer reported an analysis failure:\n{out}"
    );
}

/// hfst/hfst#460: lookup-optimize (`fst2fst -w`) on a prefix acceptor whose
/// continuation symbols appear ONLY on the output side must keep those outputs.
/// Input `a` maps to two multi-character output continuations (`+by`, `+person`)
/// that never occur as input symbols; both must survive the OL conversion and be
/// returned by lookup. Upstream, output-only symbols were dropped (mapped to
/// epsilon) during the HfstBasicTransducer -> hfst_ol conversion.
#[test]
fn lookup_optimize_keeps_prefix_acceptor_outputs() {
    let dir = scratch("460prefix");
    let hfst = dir.join("pa.hfst");
    let ol = dir.join("pa.ol");

    // Two output-only continuations sharing the same input arc.
    build_att(
        "0\t1\ta\t+by\t0.0\n0\t2\ta\t+person\t0.0\n1\t0.0\n2\t0.0\n",
        &hfst,
        &dir,
    );
    to_ol(&hfst, &ol);

    // The optimized `.ol` must still enumerate both continuations.
    let (ok, strings) = run(&["fst2strings", ol.to_str().expect("utf8 path")], b"");
    assert!(ok, "fst2strings on the optimized acceptor failed");
    assert!(
        strings.contains("a:+by") && strings.contains("a:+person"),
        "OL conversion dropped an output-only continuation:\n{strings}"
    );

    // And lookup must return both outputs for input `a`.
    let (ok, look) = run(
        &["optimized-lookup", ol.to_str().expect("utf8 path")],
        b"a\n",
    );
    assert!(ok, "optimized-lookup failed on the optimized acceptor");
    assert!(
        look.contains("+by") && look.contains("+person"),
        "lookup on the optimized acceptor lost an output:\n{look}"
    );
}

/// hfst/hfst#587: `fst2strings -P` (output prefix expansion) must match across a
/// LEADING epsilon. Here input `abc` maps to output `BC` via `a:<eps> b:B c:C`,
/// so the printed output starts with `B` even though the path's first output
/// symbol is the internal epsilon marker. `-P "B"` / `-P "BC"` must match; a
/// non-matching prefix must not.
#[test]
fn fst2strings_output_prefix_matches_across_leading_epsilon() {
    let dir = scratch("587outeps");
    let hfst = dir.join("eps.hfst");

    build_att(
        "0\t1\ta\t@_EPSILON_SYMBOL_@\t0.0\n1\t2\tb\tB\t0.0\n2\t3\tc\tC\t0.0\n3\t0.0\n",
        &hfst,
        &dir,
    );

    // Sanity: the full string is `abc:BC`.
    let (ok, all) = run(&["fst2strings", hfst.to_str().expect("utf8 path")], b"");
    assert!(ok, "fst2strings failed");
    assert!(all.contains("abc:BC"), "unexpected baseline output:\n{all}");

    for prefix in ["B", "BC"] {
        let (ok, out) = run(
            &[
                "fst2strings",
                "-P",
                prefix,
                hfst.to_str().expect("utf8 path"),
            ],
            b"",
        );
        assert!(ok, "fst2strings -P {prefix} failed");
        assert!(
            out.contains("abc:BC"),
            "-P {prefix:?} did not match across the leading epsilon:\n{out}"
        );
    }

    // A prefix that genuinely does not match must still be rejected.
    let (ok, out) = run(
        &["fst2strings", "-P", "Z", hfst.to_str().expect("utf8 path")],
        b"",
    );
    assert!(ok, "fst2strings -P Z failed");
    assert!(
        !out.contains("abc:BC"),
        "-P Z must not match output BC:\n{out}"
    );
}

/// hfst/hfst#587 (input side): `-p` (input prefix) must likewise match across a
/// leading epsilon on the input tape. Input `bc` is produced via `@eps:B a:...`
/// so the printed input starts with `b` even though the first input symbol is
/// the epsilon marker.
#[test]
fn fst2strings_input_prefix_matches_across_leading_epsilon() {
    let dir = scratch("587ineps");
    let hfst = dir.join("ineps.hfst");

    build_att(
        "0\t1\t@_EPSILON_SYMBOL_@\tX\t0.0\n1\t2\tb\tB\t0.0\n2\t3\tc\tC\t0.0\n3\t0.0\n",
        &hfst,
        &dir,
    );

    let (ok, all) = run(&["fst2strings", hfst.to_str().expect("utf8 path")], b"");
    assert!(ok, "fst2strings failed");
    assert!(all.contains("bc:XBC"), "unexpected baseline output:\n{all}");

    let (ok, out) = run(
        &["fst2strings", "-p", "b", hfst.to_str().expect("utf8 path")],
        b"",
    );
    assert!(ok, "fst2strings -p b failed");
    assert!(
        out.contains("bc:XBC"),
        "-p b did not match across the leading input epsilon:\n{out}"
    );
}
