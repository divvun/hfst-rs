//! `hfst fst2fst -f dhfst` and `hfst bhfst -e FILE.dhfst`, end to end.
//!
//! `tests/fixtures/dhfst/` holds a small error model (`errmodel.att`, and
//! `errmodel.hfst`, its weighted optimized lookup) and what divvunspell's
//! `dhfst-tools write` writes from `errmodel.hfst` with the default depth
//! bound, `--max-depth 1` and `--unbounded`. hfst must write the same bytes,
//! but for the writer the `meta` section names. The model has explicit arcs, blockers, defaults of every kind, fallback
//! chains of four, and a pair with two arcs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use box_format::{BoxPath, Compression, sync::BoxReader};
use hfst::dhfst::DhfstReader;

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

/// `bytes` with the `meta` section's writer value, which must be `writer`,
/// cut out, and the section's length in the section table zeroed. The writer
/// puts `meta` last, so nothing else moves.
fn without_writer(bytes: &[u8], writer: &str) -> Vec<u8> {
    let u64_at =
        |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes")) as usize;
    let n_sections = u32::from_le_bytes(bytes[12..16].try_into().expect("four bytes")) as usize;
    let record = (0..n_sections)
        .map(|s| 24 + 24 * s)
        .find(|at| &bytes[*at..*at + 4] == b"meta")
        .expect("the file has a meta section");
    let (offset, len) = (u64_at(record + 8), u64_at(record + 16));
    assert_eq!((offset + len).div_ceil(8) * 8, bytes.len(), "meta is last");
    let meta = std::str::from_utf8(&bytes[offset..offset + len]).expect("meta is UTF-8");
    let rest = meta
        .strip_prefix(&format!("{{\"writer\":\"{writer}\""))
        .unwrap_or_else(|| panic!("{meta} does not name {writer} as the writer"));
    let mut out = bytes[..offset].to_vec();
    out[record + 16..record + 24].fill(0);
    out.extend_from_slice(rest.as_bytes());
    out
}

/// `written` is what dhfst-tools wrote to the fixture `expected` in every
/// section but `meta`, and the two `meta` sections differ only in the writer.
fn assert_dhfst_tools_but_writer(written: &[u8], expected: &str) {
    let hfst = format!("Divvun HFST v{}", env!("CARGO_PKG_VERSION"));
    assert!(
        without_writer(written, &hfst)
            == without_writer(&read(&fixture(expected)), "divvun-fst 1.0.0-beta.13"),
        "hfst's output differs from {expected} beyond the writer"
    );
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

/// `hfst fst2fst -f dhfst` of `input` with `extra` options, written to a file
/// in `dir`; answers the bytes.
fn write_dhfst(dir: &Path, input: &Path, extra: &[&str]) -> Vec<u8> {
    let out = dir.join("out.dhfst");
    let result = output(
        hfst()
            .args(["fst2fst", "-f", "dhfst"])
            .args(extra)
            .arg("-i")
            .arg(input)
            .arg("-o")
            .arg(&out),
    );
    assert!(
        result.status.success(),
        "fst2fst failed: {}",
        stderr(&result)
    );
    read(&out)
}

// [spec:hfst:sem:dhfst.write+1/test]
// [spec:hfst:def:dhfst.fst2fst/test]
#[test]
fn writes_what_dhfst_tools_writes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = fixture("errmodel.hfst");
    for (extra, expected) in [
        (&[][..], "errmodel.dhfst"),
        (&["--max-fallback-depth", "1"][..], "errmodel.depth1.dhfst"),
        (&["--unbounded-fallback"][..], "errmodel.unbounded.dhfst"),
    ] {
        assert_dhfst_tools_but_writer(&write_dhfst(tmp.path(), &input, extra), expected);
    }
}

// [spec:hfst:sem:dhfst.fst2fst+1/test]
// [spec:hfst:sem:dhfst.source-model/test]
#[test]
fn converts_other_formats_as_olw_does() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // The tropical source of the fixture, under the fixture's file name so
    // that the meta section names the same source.
    let tropical = tmp.path().join("errmodel.hfst");
    txt2fst(&fixture("errmodel.att"), &tropical);
    assert_dhfst_tools_but_writer(&write_dhfst(tmp.path(), &tropical, &[]), "errmodel.dhfst");
}

// [spec:hfst:sem:dhfst.fst2fst+1/test]
// [spec:hfst:sem:dhfst.meta+1/test]
#[test]
fn writes_standard_output_and_reports_with_verbose() {
    let input = fixture("errmodel.hfst");
    let result = output(
        hfst()
            .args(["fst2fst", "-v", "-f", "dhfst", "-i"])
            .arg(&input),
    );
    assert!(
        result.status.success(),
        "fst2fst failed: {}",
        stderr(&result)
    );
    assert_dhfst_tools_but_writer(&result.stdout, "errmodel.dhfst");
    assert!(stderr(&result).contains("read back from the written bytes: all equal to the source"));

    // From standard input the source has no name.
    let piped = output(
        hfst()
            .args(["fst2fst", "-f", "dhfst"])
            .stdin(std::fs::File::open(&input).expect("open the fixture")),
    );
    assert!(piped.status.success(), "fst2fst failed: {}", stderr(&piped));
    let reader = DhfstReader::parse(&piped.stdout).expect("the output is DHFST");
    assert!(reader.meta().is_some_and(|m| m.contains("\"source\":\"\"")));
}

// [spec:hfst:sem:dhfst.fst2fst+1/test]
#[test]
fn refuses_what_it_cannot_write() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = fixture("errmodel.hfst");
    let out = tmp.path().join("refused.dhfst");
    for (args, reason) in [
        (
            &["-f", "olw", "--max-fallback-depth", "2"][..],
            "apply only to -f dhfst",
        ),
        (
            &[
                "-f",
                "dhfst",
                "--max-fallback-depth",
                "2",
                "--unbounded-fallback",
            ][..],
            "mutually exclusive",
        ),
        (&["-f", "dhfst", "-b"][..], "-b does not apply"),
    ] {
        let result = output(
            hfst()
                .arg("fst2fst")
                .args(args)
                .arg("-i")
                .arg(&input)
                .arg("-o")
                .arg(&out),
        );
        assert!(
            !result.status.success() && stderr(&result).contains(reason),
            "{args:?}"
        );
    }

    let twice = tmp.path().join("twice.hfst");
    std::fs::write(&twice, [read(&input), read(&input)].concat()).expect("write twice.hfst");
    let flagged_att = tmp.path().join("flagged.att");
    std::fs::write(
        &flagged_att,
        "0\t1\t@P.CASE.NOM@\t@P.CASE.NOM@\t0\n1\t1\ta\tb\t1\n1\t0\n",
    )
    .expect("write flagged.att");
    let flagged = tmp.path().join("flagged.hfst");
    txt2fst(&flagged_att, &flagged);
    for (source, reason) in [(&twice, "more than one"), (&flagged, "flag diacritic")] {
        let result = output(
            hfst()
                .args(["fst2fst", "-f", "dhfst", "-i"])
                .arg(source)
                .arg("-o")
                .arg(&out),
        );
        assert!(
            !result.status.success() && stderr(&result).contains(reason),
            "{reason}"
        );
        assert!(!out.exists(), "nothing is written when the writer refuses");
    }
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

/// `hfst bhfst -a acceptor -e errmodel [-X index_xml] -o out`.
fn bhfst(acceptor: &Path, errmodel: &Path, index_xml: Option<&Path>, out: &Path) -> Output {
    let mut command = hfst();
    command
        .arg("bhfst")
        .arg("-a")
        .arg(acceptor)
        .arg("-e")
        .arg(errmodel);
    if let Some(index_xml) = index_xml {
        command.arg("-X").arg(index_xml);
    }
    output(command.arg("-o").arg(out))
}

// [spec:hfst:def:dhfst.bhfst-member/test]
// [spec:hfst:sem:dhfst.bhfst-member/test]
#[test]
fn bhfst_stores_dhfst_error_model_unchanged() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let index_xml = tmp.path().join("index.xml");
    std::fs::write(&index_xml, INDEX_XML).expect("write index.xml");
    let acceptor = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lookup.hfstol");
    let errmodel = fixture("errmodel.dhfst");
    let out = tmp.path().join("out.bhfst");
    let result = bhfst(&acceptor, &errmodel, Some(&index_xml), &out);
    assert!(result.status.success(), "bhfst failed: {}", stderr(&result));

    let reader = BoxReader::open(&out).expect("open the archive");
    assert_eq!(reader.alignment(), 8);
    for member in ["alphabet", "index", "transition"] {
        assert!(!stored(&reader, &format!("acceptor.default.thfst/{member}")).is_empty());
    }
    assert!(stored(&reader, "errmodel.default.dhfst") == read(&errmodel));
    let thfst_errmodel = BoxPath::new("errmodel.default.thfst/alphabet").expect("a valid box path");
    assert!(
        reader.find(&thfst_errmodel).is_err(),
        "no THFST error model beside the DHFST one"
    );
    let meta: serde_json::Value =
        serde_json::from_slice(&stored(&reader, "meta.json")).expect("meta.json is JSON");
    assert_eq!(meta["errmodel"]["id"], "errmodel.default.dhfst");
    assert_eq!(meta["acceptor"]["id"], "acceptor.default.thfst");
}

// [spec:hfst:sem:dhfst.bhfst-member/test]
#[test]
fn bhfst_refuses_a_broken_or_misplaced_dhfst() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dhfst = fixture("errmodel.dhfst");
    let broken = tmp.path().join("broken.dhfst");
    std::fs::write(&broken, &read(&dhfst)[..100]).expect("write broken.dhfst");
    let out = tmp.path().join("out.bhfst");
    for (acceptor, errmodel, reason) in [
        (dhfst.clone(), dhfst.clone(), "only --errmodel can be DHFST"),
        (fixture("errmodel.hfst"), broken, "not a DHFST error model"),
    ] {
        let result = bhfst(&acceptor, &errmodel, None, &out);
        assert!(
            !result.status.success() && stderr(&result).contains(reason),
            "{reason}"
        );
    }
}
