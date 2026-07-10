//! Integration tests for `hfst-bhfst` — the BHFST speller-archive packer.
//!
//! These drive the `hfst` binary end to end (like `optimized_lookup_smoke.rs`)
//! and re-open the produced archive with box-format's reader to assert the
//! `.bhfst-layout` contract: the two canonical THFST directories with their
//! three members (`alphabet`/`index`/`transition`), plus an optional top-level
//! `meta.json`, all stored UNCOMPRESSED, at 8-byte alignment
//! [spec:hfst:sem:thfst-backend.bhfst-layout].
//!
//! `tests/fixtures/lookup.hfstol` is a committed optimized-lookup transducer; we
//! feed it as BOTH acceptor and errmodel so the auto-convert-to-THFST path is
//! exercised without a second fixture.

use std::path::Path;
use std::process::Command;

use box_format::{BoxPath, Compression, sync::BoxReader};

/// Expected entry paths inside a packed BHFST, in layout order.
const EXPECTED_ENTRIES: &[&str] = &[
    "acceptor.default.thfst/alphabet",
    "acceptor.default.thfst/index",
    "acceptor.default.thfst/transition",
    "errmodel.default.thfst/alphabet",
    "errmodel.default.thfst/index",
    "errmodel.default.thfst/transition",
];

fn hfst() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hfst"))
}

fn fixture() -> String {
    format!(
        "{}/tests/fixtures/lookup.hfstol",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Read an entry's decompressed bytes from the archive, asserting it is a
/// Stored file, and return the bytes.
fn read_stored(reader: &BoxReader, path: &str) -> Vec<u8> {
    let box_path = BoxPath::new(path).expect("valid box path");
    let record = reader
        .find(&box_path)
        .unwrap_or_else(|_| panic!("entry {path} present"));
    let file = record
        .as_file()
        .unwrap_or_else(|| panic!("entry {path} is a file"));
    assert_eq!(
        file.compression,
        Compression::Stored,
        "entry {path} must be Stored (divvunspell mmaps raw offsets)"
    );
    let mut bytes = Vec::new();
    reader
        .decompress(file, &mut bytes)
        .unwrap_or_else(|e| panic!("decompress {path}: {e}"));
    bytes
}

// [spec:hfst:def:thfst-backend.bhfst-layout/test]
// [spec:hfst:sem:thfst-backend.bhfst-layout/test]
// [spec:hfst:def:thfst-backend.meta-json/test]
// [spec:hfst:sem:thfst-backend.meta-json/test]
#[test]
fn pack_with_index_xml_and_reopen() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let index_xml = tmp.path().join("index.xml");
    std::fs::write(
        &index_xml,
        r#"<?xml version="1.0" encoding="UTF-8"?>
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
"#,
    )
    .expect("write index.xml");

    let out = tmp.path().join("out.bhfst");
    let fixture = fixture();
    let status = hfst()
        .arg("bhfst")
        .args(["-a", &fixture, "-e", &fixture])
        .arg("-X")
        .arg(&index_xml)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("run hfst bhfst");
    assert!(status.success(), "pack exited with {status:?}");
    assert!(out.exists(), "output archive was written");

    let reader = BoxReader::open(&out).expect("re-open bhfst");

    // Alignment honoured.
    assert_eq!(reader.alignment(), 8, "archive must be 8-byte aligned");

    // Every canonical THFST member present, Stored, and non-empty.
    for entry in EXPECTED_ENTRIES {
        let bytes = read_stored(&reader, entry);
        assert!(!bytes.is_empty(), "entry {entry} is non-empty");
    }

    // meta.json present, Stored, and carries the rewritten ids (.hfst -> .thfst).
    let meta_bytes = read_stored(&reader, "meta.json");
    let meta: serde_json::Value =
        serde_json::from_slice(&meta_bytes).expect("meta.json is valid JSON");
    assert_eq!(
        meta["acceptor"]["id"], "acceptor.default.thfst",
        "acceptor id rewritten to .thfst"
    );
    assert_eq!(
        meta["errmodel"]["id"], "errmodel.default.thfst",
        "errmodel id rewritten to .thfst"
    );
    assert_eq!(meta["info"]["locale"], "se");
    // `type` attribute preserved on the acceptor.
    assert_eq!(meta["acceptor"]["type"], "general");
    // title is a list of {lang, "$value"} objects.
    assert_eq!(meta["info"]["title"][0]["$value"], "Test speller");
}

// [spec:hfst:sem:thfst-backend.bhfst-layout/test]
// [spec:hfst:sem:thfst-backend.bhfst-tool/test]
#[test]
fn pack_ready_thfst_dirs_with_verbatim_meta() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Produce a ready `.thfst` dir via `hfst fst2fst -f thfst`.
    let acceptor_dir = tmp.path().join("A.thfst");
    let errmodel_dir = tmp.path().join("E.thfst");
    let fixture = fixture();
    for dir in [&acceptor_dir, &errmodel_dir] {
        let status = hfst()
            .args(["fst2fst", "-f", "thfst"])
            .arg(&fixture)
            .arg("-o")
            .arg(dir)
            .status()
            .expect("run hfst fst2fst -f thfst");
        assert!(status.success(), "fst2fst -f thfst exited with {status:?}");
        assert!(
            Path::new(dir).join("alphabet").is_file(),
            "thfst dir has an alphabet"
        );
    }

    // A caller-supplied meta.json is embedded VERBATIM, including an unknown
    // extra field that a strict mirror-struct parse would drop.
    let meta = tmp.path().join("meta.json");
    let meta_text = "{\n  \"info\": {\"locale\": \"se\"},\n  \"custom_extra\": [1, 2, 3]\n}\n";
    std::fs::write(&meta, meta_text).expect("write meta.json");

    let out = tmp.path().join("out.bhfst");
    let status = hfst()
        .arg("bhfst")
        .arg("-a")
        .arg(&acceptor_dir)
        .arg("-e")
        .arg(&errmodel_dir)
        .arg("-m")
        .arg(&meta)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("run hfst bhfst");
    assert!(status.success(), "pack exited with {status:?}");

    let reader = BoxReader::open(&out).expect("re-open bhfst");
    assert_eq!(reader.alignment(), 8);
    for entry in EXPECTED_ENTRIES {
        let _ = read_stored(&reader, entry);
    }
    // Verbatim: the exact bytes we handed in, unknown field and all.
    let meta_bytes = read_stored(&reader, "meta.json");
    assert_eq!(
        meta_bytes,
        meta_text.as_bytes(),
        "meta.json is embedded byte-for-byte verbatim"
    );
}

// [spec:hfst:def:thfst-backend.bhfst-tool/test]
// [spec:hfst:sem:thfst-backend.bhfst-tool/test]
#[test]
fn info_prints_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let index_xml = tmp.path().join("index.xml");
    std::fs::write(
        &index_xml,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<hfstspeller dtdversion="1.0" hfstversion="3">
  <info>
    <locale>fi</locale>
    <title>Info test</title>
    <description>d</description>
    <producer>p</producer>
  </info>
  <acceptor type="general" id="acceptor.default.hfst">
    <title>t</title><description>d</description>
  </acceptor>
  <errmodel id="errmodel.default.hfst">
    <title>t</title><description>d</description>
  </errmodel>
</hfstspeller>
"#,
    )
    .expect("write index.xml");

    let out = tmp.path().join("out.bhfst");
    let fixture = fixture();
    let status = hfst()
        .arg("bhfst")
        .args(["-a", &fixture, "-e", &fixture])
        .arg("-X")
        .arg(&index_xml)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("pack");
    assert!(status.success());

    let output = hfst()
        .arg("bhfst")
        .arg("-I")
        .arg(&out)
        .output()
        .expect("run hfst bhfst -I");
    assert!(
        output.status.success(),
        "info exited with {:?}",
        output.status
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    // The printed metadata is the converted meta.json with rewritten ids.
    let value: serde_json::Value =
        serde_json::from_str(printed.trim()).expect("info prints valid JSON");
    assert_eq!(value["acceptor"]["id"], "acceptor.default.thfst");
    assert_eq!(value["info"]["locale"], "fi");
}
