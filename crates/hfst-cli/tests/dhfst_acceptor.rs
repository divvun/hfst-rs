//! `hfst fst2fst -f dhfst --dhfst-type acceptor` and `hfst bhfst -a
//! FILE.dhfst`, end to end.
//!
//! `tests/fixtures/dhfst/` holds two small acceptors (`acceptor.att` and
//! `acceptor-wide.att`, and `acceptor.hfst` and `acceptor-wide.hfst`, their
//! weighted optimized lookup) and what divvunspell's `dhfst-tools acceptor`
//! writes from each `.hfst`. hfst must write the same bytes, but for the
//! writer the `meta` section names. The first has two arcs on one symbol, an
//! epsilon arc, flag diacritic arcs and distances other than 0, so a `DIST`
//! section; the second has labels past 255, so two-byte checks.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use box_format::{BoxPath, Compression, sync::BoxReader};
use hfst::dhfst_acceptor::AcceptorReader;

fn hfst() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hfst"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dhfst")
        .join(name)
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn output(command: &mut Command) -> Output {
    command.output().expect("run hfst")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// `bytes` split at its `meta` section, which the writer puts last: the
/// bytes before it with its length in the section table zeroed, and its text
/// with the writer value, which must be `writer`, emptied.
fn split_meta(bytes: &[u8], writer: &str) -> (Vec<u8>, String) {
    let u64_at =
        |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes")) as usize;
    let n_sections = u32::from_le_bytes(bytes[12..16].try_into().expect("four bytes")) as usize;
    let record = (0..n_sections)
        .map(|s| 24 + 24 * s)
        .find(|at| &bytes[*at..*at + 4] == b"meta")
        .expect("the file has a meta section");
    let (offset, len) = (u64_at(record + 8), u64_at(record + 16));
    assert_eq!(offset + len, bytes.len(), "meta is last");
    let meta = std::str::from_utf8(&bytes[offset..]).expect("meta is UTF-8");
    let named = format!(",\"writer\":\"{writer}\"}}");
    assert!(meta.ends_with(&named), "{meta} does not name {writer}");
    let mut before = bytes[..offset].to_vec();
    before[record + 16..record + 24].fill(0);
    (before, meta.replace(&named, ",\"writer\":\"\"}"))
}

/// `written` is what dhfst-tools wrote to the fixture `expected` in every
/// section but `meta`, and the two `meta` sections differ only in the writer.
fn assert_dhfst_tools_but_writer(written: &[u8], expected: &str) {
    let hfst = format!("Divvun HFST v{}", env!("CARGO_PKG_VERSION"));
    let (ours, our_meta) = split_meta(written, &hfst);
    let (theirs, their_meta) = split_meta(
        &read(&fixture(expected)),
        "divvunspell dhfst acceptor writer",
    );
    assert!(ours == theirs, "hfst's sections differ from {expected}'s");
    assert_eq!(
        our_meta, their_meta,
        "the meta sections differ beyond the writer"
    );
}

/// `hfst fst2fst -f dhfst --dhfst-type acceptor` of `input` with `extra`
/// options, written to `out`.
fn write_acceptor(input: &Path, out: &Path, extra: &[&str]) -> Output {
    output(
        hfst()
            .args(["fst2fst", "-f", "dhfst", "--dhfst-type", "acceptor"])
            .args(extra)
            .arg("-i")
            .arg(input)
            .arg("-o")
            .arg(out),
    )
}

/// Compile AT&T text to a tropical transducer file.
fn txt2fst(att: &Path, out: &Path) {
    let result = output(hfst().arg("txt2fst").arg("-i").arg(att).arg("-o").arg(out));
    assert!(
        result.status.success(),
        "txt2fst failed: {}",
        stderr(&result)
    );
}

// [spec:hfst:sem:dhfst.acceptor-write/test]
// [spec:hfst:def:dhfst.fst2fst+2/test]
// [spec:hfst:sem:dhfst.acceptor-layout/test]
// [spec:hfst:sem:dhfst.acceptor-distance/test]
#[test]
fn writes_what_dhfst_tools_acceptor_writes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    for name in ["acceptor", "acceptor-wide"] {
        let out = tmp.path().join(format!("{name}.dhfst"));
        let result = write_acceptor(&fixture(&format!("{name}.hfst")), &out, &[]);
        assert!(
            result.status.success(),
            "fst2fst failed: {}",
            stderr(&result)
        );
        assert_dhfst_tools_but_writer(&read(&out), &format!("{name}.dhfst"));
    }
}

// [spec:hfst:sem:dhfst.fst2fst+2/test]
// [spec:hfst:sem:dhfst.acceptor-meta/test]
#[test]
fn converts_other_formats_as_olw_does_and_reports() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // The tropical source of the fixture, under the fixture's file name so
    // that the meta section names the same source.
    let tropical = tmp.path().join("acceptor.hfst");
    txt2fst(&fixture("acceptor.att"), &tropical);
    let out = tmp.path().join("out.dhfst");
    let result = write_acceptor(&tropical, &out, &["-v"]);
    assert!(
        result.status.success(),
        "fst2fst failed: {}",
        stderr(&result)
    );
    assert_dhfst_tools_but_writer(&read(&out), "acceptor.dhfst");
    // With -o the report goes to standard output.
    let report = String::from_utf8_lossy(&result.stdout).into_owned() + &stderr(&result);
    assert!(report.contains("Writing a DHFST acceptor"), "{report}");
    assert!(
        report.contains("8 states numbered below 9, 12 arcs (3 free)"),
        "{report}"
    );
    assert!(report.contains("all equal to the source"), "{report}");

    // To standard output, and from standard input the source has no name.
    let piped = output(
        hfst()
            .args(["fst2fst", "-f", "dhfst", "--dhfst-type", "acceptor"])
            .stdin(std::fs::File::open(fixture("acceptor.hfst")).expect("open the fixture")),
    );
    assert!(piped.status.success(), "fst2fst failed: {}", stderr(&piped));
    let reader = AcceptorReader::parse(&piped.stdout).expect("the output is a DHFST acceptor");
    assert!(reader.meta().is_some_and(|m| m.contains("\"source\":\"\"")));
}

// [spec:hfst:sem:dhfst.fst2fst+2/test]
// [spec:hfst:sem:dhfst.acceptor-source/test]
#[test]
fn refuses_a_transducer_and_writes_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let att = tmp.path().join("transducer.att");
    std::fs::write(&att, "0\t1\ta\tb\t0\n1\t0\n").expect("write transducer.att");
    let transducer = tmp.path().join("transducer.hfst");
    txt2fst(&att, &transducer);
    let out = tmp.path().join("refused.dhfst");
    let result = write_acceptor(&transducer, &out, &[]);
    assert!(
        !result.status.success() && stderr(&result).contains("is not an acceptor arc"),
        "{}",
        stderr(&result)
    );
    assert!(!out.exists(), "nothing is written when the writer refuses");
    let result = write_acceptor(
        &fixture("acceptor.hfst"),
        &out,
        &["--max-fallback-depth", "2"],
    );
    assert!(
        !result.status.success() && stderr(&result).contains("apply only to --dhfst-type errmodel"),
        "{}",
        stderr(&result)
    );
    assert!(!out.exists());
}

const INDEX_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<hfstspeller dtdversion="1.0" hfstversion="3">
  <info>
    <locale>se</locale>
    <title>Test speller</title>
    <description>A tiny test speller.</description>
    <producer>rust-hfst tests</producer>
  </info>
  <acceptor type="general" id="acceptor.default.hfst">
    <title>Test acceptor</title>
    <description>Test dictionary.</description>
  </acceptor>
  <errmodel id="errmodel.default.hfst">
    <title>Test errmodel</title>
    <description>Test edit distance.</description>
  </errmodel>
</hfstspeller>
"#;

/// Whether the archive has a member at `path`.
fn has_member(reader: &BoxReader, path: &str) -> bool {
    reader
        .find(&BoxPath::new(path).expect("a valid box path"))
        .is_ok()
}

/// The bytes of a Stored archive member.
fn stored(reader: &BoxReader, path: &str) -> Vec<u8> {
    let record = reader
        .find(&BoxPath::new(path).expect("a valid box path"))
        .unwrap_or_else(|_| panic!("{path} is in the archive"));
    let file = record.as_file().expect("the member is a file");
    assert_eq!(file.compression, Compression::Stored, "{path} is Stored");
    let mut bytes = Vec::new();
    reader
        .decompress(file, &mut bytes)
        .expect("read the member");
    bytes
}

/// `hfst bhfst -a acceptor -e errmodel -X index_xml -o out`.
fn bhfst(acceptor: &Path, errmodel: &Path, index_xml: &Path, out: &Path) -> Output {
    output(
        hfst()
            .arg("bhfst")
            .arg("-a")
            .arg(acceptor)
            .arg("-e")
            .arg(errmodel)
            .arg("-X")
            .arg(index_xml)
            .arg("-o")
            .arg(out),
    )
}

// [spec:hfst:def:dhfst.bhfst-member+2/test]
// [spec:hfst:sem:dhfst.bhfst-member+2/test]
#[test]
fn bhfst_stores_a_dhfst_acceptor_unchanged() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let index_xml = tmp.path().join("index.xml");
    std::fs::write(&index_xml, INDEX_XML).expect("write index.xml");
    let acceptor = fixture("acceptor.dhfst");
    for (errmodel, dhfst_errmodel) in [
        (fixture("errmodel.dhfst"), true),
        (fixture("errmodel.hfst"), false),
    ] {
        let out = tmp.path().join("out.bhfst");
        let result = bhfst(&acceptor, &errmodel, &index_xml, &out);
        assert!(result.status.success(), "bhfst failed: {}", stderr(&result));
        let reader = BoxReader::open(&out).expect("open the archive");
        assert_eq!(reader.alignment(), 8);
        assert!(stored(&reader, "acceptor.default.dhfst") == read(&acceptor));
        assert!(!has_member(&reader, "acceptor.default.thfst/alphabet"));
        assert_eq!(
            has_member(&reader, "errmodel.default.dhfst"),
            dhfst_errmodel
        );
        assert_eq!(
            has_member(&reader, "errmodel.default.thfst/alphabet"),
            !dhfst_errmodel
        );
        let meta: serde_json::Value =
            serde_json::from_slice(&stored(&reader, "meta.json")).expect("meta.json is JSON");
        assert_eq!(meta["acceptor"]["id"], "acceptor.default.dhfst");
        let errmodel_id = if dhfst_errmodel {
            "errmodel.default.dhfst"
        } else {
            "errmodel.default.thfst"
        };
        assert_eq!(meta["errmodel"]["id"], errmodel_id);
    }
}

// [spec:hfst:sem:dhfst.bhfst-member+2/test]
#[test]
fn bhfst_refuses_a_broken_or_misplaced_dhfst_acceptor() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let index_xml = tmp.path().join("index.xml");
    std::fs::write(&index_xml, INDEX_XML).expect("write index.xml");
    let acceptor = fixture("acceptor.dhfst");
    let broken = tmp.path().join("broken.dhfst");
    std::fs::write(&broken, &read(&acceptor)[..100]).expect("write broken.dhfst");
    let out = tmp.path().join("out.bhfst");
    for (a, e, reason) in [
        (
            fixture("errmodel.hfst"),
            acceptor.clone(),
            "is a DHFST acceptor (type 2); an error model is type 1",
        ),
        (
            broken,
            fixture("errmodel.dhfst"),
            "is not a DHFST acceptor divvunspell can load",
        ),
    ] {
        let result = bhfst(&a, &e, &index_xml, &out);
        assert!(
            !result.status.success() && stderr(&result).contains(reason),
            "{reason}: {}",
            stderr(&result)
        );
    }
}
