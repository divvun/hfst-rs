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

use std::io::Write as _;
use std::path::{Path, PathBuf};
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

/// The index.xml every zhfst fixture below carries.
const INDEX_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<hfstspeller dtdversion="1.0" hfstversion="3">
  <info>
    <locale>sma</locale>
    <title>Zhfst test speller</title>
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

/// A speller runtime configuration in the shape the Giella spellers ship: the
/// kebab-case keys divvunspell's `SpellerConfig` deserializes, including a null
/// and a nested object.
const SPELLER_CONFIG: &str = r#"{
  "n-best": 100,
  "max-weight": 10000,
  "beam": 14,
  "reweight": {
    "start-penalty": 3,
    "mid-penalty": 1,
    "end-penalty": 1
  },
  "node-pool-size": 128,
  "recase": true,
  "completion-marker": null
}
"#;

/// Write a zhfst (a plain zip) holding the given members, deflated exactly as
/// the Giella build's `zip` writes them.
fn write_zhfst(path: &Path, members: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).expect("create zhfst");
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in members {
        writer.start_file(*name, options).expect("start zip member");
        writer.write_all(bytes).expect("write zip member");
    }
    writer.finish().expect("finish zhfst");
}

/// Write a zhfst carrying the standard index.xml, the shared fixture as both
/// transducers, and optionally a `speller-config.json`.
fn zhfst_fixture(dir: &Path, name: &str, speller_config: Option<&str>) -> PathBuf {
    let transducer = std::fs::read(fixture()).expect("read fixture transducer");
    let mut members: Vec<(&str, &[u8])> = vec![
        ("acceptor.default.hfst", &transducer),
        ("errmodel.default.hfst", &transducer),
        ("index.xml", INDEX_XML.as_bytes()),
    ];
    if let Some(config) = speller_config {
        members.push(("speller-config.json", config.as_bytes()));
    }
    let path = dir.join(name);
    write_zhfst(&path, &members);
    path
}

/// Run `hfst bhfst` with the given arguments, returning its captured output.
fn run_bhfst<I, S>(args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    hfst()
        .arg("bhfst")
        .args(args)
        .output()
        .expect("run hfst bhfst")
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

// -----------------------------------------------------------------------------
// zhfst input + the speller runtime configuration carry-through
// -----------------------------------------------------------------------------

/// A zhfst carrying `speller-config.json` produces a meta.json whose top-level
/// `spellerConfig` key holds that configuration, parsed, as a JSON object —
/// content preserved field for field, nothing renamed, added or dropped.
// [spec:hfst:def:thfst-backend.speller-config/test]
// [spec:hfst:sem:thfst-backend.speller-config/test]
// [spec:hfst:def:thfst-backend.zhfst-input/test]
// [spec:hfst:sem:thfst-backend.zhfst-input/test]
#[test]
fn zhfst_speller_config_rides_in_meta_json() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let zhfst = zhfst_fixture(tmp.path(), "with-config.zhfst", Some(SPELLER_CONFIG));
    let out = tmp.path().join("out.bhfst");

    let output = run_bhfst([
        "-z".as_ref(),
        zhfst.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "pack exited with {:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let reader = BoxReader::open(&out).expect("re-open bhfst");
    assert_eq!(reader.alignment(), 8);
    for entry in EXPECTED_ENTRIES {
        assert!(!read_stored(&reader, entry).is_empty(), "{entry} non-empty");
    }

    let meta: serde_json::Value =
        serde_json::from_slice(&read_stored(&reader, "meta.json")).expect("meta.json is JSON");

    // The metadata the index.xml already carried is untouched.
    assert_eq!(meta["info"]["locale"], "sma");
    assert_eq!(meta["acceptor"]["id"], "acceptor.default.thfst");
    assert_eq!(meta["errmodel"]["id"], "errmodel.default.thfst");

    // The configuration is an OBJECT at the top level, not a string, and is
    // deep-equal to the member the zhfst carried.
    let carried = &meta["spellerConfig"];
    assert!(carried.is_object(), "spellerConfig is a JSON object");
    let source: serde_json::Value =
        serde_json::from_str(SPELLER_CONFIG).expect("fixture config is JSON");
    assert_eq!(carried, &source, "configuration carried through unchanged");
}

/// A zhfst with no `speller-config.json` produces exactly the archive the loose
/// -a/-e/-X path produces from the same members — byte for byte, with no
/// `spellerConfig` key anywhere.
// [spec:hfst:sem:thfst-backend.speller-config/test]
// [spec:hfst:sem:thfst-backend.zhfst-input/test]
#[test]
fn zhfst_without_speller_config_is_unchanged() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let zhfst = zhfst_fixture(tmp.path(), "plain.zhfst", None);
    let from_zhfst = tmp.path().join("from-zhfst.bhfst");
    let output = run_bhfst([
        "-z".as_ref(),
        zhfst.as_os_str(),
        "-o".as_ref(),
        from_zhfst.as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "pack exited with {:?}",
        output.status
    );

    // The same three members, loose on disk, through the pre-existing path.
    let index_xml = tmp.path().join("index.xml");
    std::fs::write(&index_xml, INDEX_XML).expect("write index.xml");
    let from_files = tmp.path().join("from-files.bhfst");
    let fixture = fixture();
    let output = run_bhfst([
        "-a".as_ref(),
        fixture.as_ref(),
        "-e".as_ref(),
        fixture.as_ref(),
        "-X".as_ref(),
        index_xml.as_os_str(),
        "-o".as_ref(),
        from_files.as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "pack exited with {:?}",
        output.status
    );

    assert_eq!(
        std::fs::read(&from_zhfst).expect("read zhfst-built archive"),
        std::fs::read(&from_files).expect("read file-built archive"),
        "a zhfst with no speller-config.json packs byte-identically to the \
         loose-file path"
    );

    let reader = BoxReader::open(&from_zhfst).expect("re-open bhfst");
    let meta_bytes = read_stored(&reader, "meta.json");
    assert!(
        !String::from_utf8_lossy(&meta_bytes).contains("spellerConfig"),
        "no spellerConfig key when the zhfst carried no configuration"
    );
    let meta: serde_json::Value = serde_json::from_slice(&meta_bytes).expect("meta.json is JSON");
    let keys: Vec<&String> = meta
        .as_object()
        .expect("meta.json is an object")
        .keys()
        .collect();
    assert_eq!(
        keys,
        ["acceptor", "errmodel", "info"],
        "the pre-existing keys"
    );
}

/// A `speller-config.json` that is not well-formed JSON fails the conversion,
/// naming the archive and the member, and writes no archive at all.
// [spec:hfst:sem:thfst-backend.speller-config/test]
#[test]
fn malformed_speller_config_fails_the_conversion() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let zhfst = zhfst_fixture(tmp.path(), "bad.zhfst", Some("{ \"n-best\": 100, "));
    let out = tmp.path().join("out.bhfst");

    let output = run_bhfst([
        "-z".as_ref(),
        zhfst.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success(), "a malformed config must not pass");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("speller-config.json")
            && stderr.contains("not a valid speller configuration"),
        "diagnostic names the member and the problem, got: {stderr}"
    );
    assert!(!out.exists(), "no archive is written on failure");
}

/// Well-formed JSON that is not an OBJECT is refused too: divvunspell's
/// SpellerConfig is a struct, and the key has to hold an object.
// [spec:hfst:sem:thfst-backend.speller-config/test]
#[test]
fn non_object_speller_config_fails_the_conversion() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let zhfst = zhfst_fixture(tmp.path(), "array.zhfst", Some("[1, 2, 3]"));
    let out = tmp.path().join("out.bhfst");

    let output = run_bhfst([
        "-z".as_ref(),
        zhfst.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(
        !output.status.success(),
        "a non-object config must not pass"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("expected a JSON object, found an array"),
        "diagnostic names the JSON kind, got: {stderr}"
    );
    assert!(!out.exists(), "no archive is written on failure");
}

/// The loose-file path carries a configuration too, via -c, and an explicit -c
/// overrides the one a zhfst carries.
// [spec:hfst:sem:thfst-backend.speller-config/test]
// [spec:hfst:sem:thfst-backend.bhfst-tool/test]
#[test]
fn speller_config_option_merges_and_overrides() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let index_xml = tmp.path().join("index.xml");
    std::fs::write(&index_xml, INDEX_XML).expect("write index.xml");
    let config = tmp.path().join("speller-config.json");
    std::fs::write(&config, "{\"n-best\": 7}").expect("write speller-config.json");
    let fixture = fixture();

    // -c alongside -X on the loose-file path.
    let out = tmp.path().join("files.bhfst");
    let output = run_bhfst([
        "-a".as_ref(),
        fixture.as_ref(),
        "-e".as_ref(),
        fixture.as_ref(),
        "-X".as_ref(),
        index_xml.as_os_str(),
        "-c".as_ref(),
        config.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "pack exited with {:?}",
        output.status
    );
    let reader = BoxReader::open(&out).expect("re-open bhfst");
    let meta: serde_json::Value =
        serde_json::from_slice(&read_stored(&reader, "meta.json")).expect("meta.json is JSON");
    assert_eq!(meta["spellerConfig"]["n-best"], 7);

    // -c beats the zhfst's own member.
    let zhfst = zhfst_fixture(tmp.path(), "with-config.zhfst", Some(SPELLER_CONFIG));
    let out = tmp.path().join("zhfst.bhfst");
    let output = run_bhfst([
        "-z".as_ref(),
        zhfst.as_os_str(),
        "-c".as_ref(),
        config.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "pack exited with {:?}",
        output.status
    );
    let reader = BoxReader::open(&out).expect("re-open bhfst");
    let meta: serde_json::Value =
        serde_json::from_slice(&read_stored(&reader, "meta.json")).expect("meta.json is JSON");
    assert_eq!(
        meta["spellerConfig"]["n-best"], 7,
        "-c overrides the member"
    );
}

/// A configuration with no metadata to ride in is refused: `spellerConfig`
/// alone is not a meta.json divvunspell can load.
// [spec:hfst:sem:thfst-backend.speller-config/test]
#[test]
fn speller_config_without_metadata_is_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("speller-config.json");
    std::fs::write(&config, SPELLER_CONFIG).expect("write speller-config.json");
    let out = tmp.path().join("out.bhfst");
    let fixture = fixture();

    let output = run_bhfst([
        "-a".as_ref(),
        fixture.as_ref(),
        "-e".as_ref(),
        fixture.as_ref(),
        "-c".as_ref(),
        config.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success(), "a bare config must not pass");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("needs metadata to ride in"),
        "diagnostic explains what is missing"
    );
    assert!(!out.exists(), "no archive is written on failure");
}

/// -z is a mode of its own: combining it with the loose-file options is an
/// error rather than a silent precedence rule.
// [spec:hfst:sem:thfst-backend.zhfst-input/test]
#[test]
fn zhfst_rejects_loose_file_options() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let zhfst = zhfst_fixture(tmp.path(), "plain.zhfst", None);
    let out = tmp.path().join("out.bhfst");
    let fixture = fixture();

    let output = run_bhfst([
        "-z".as_ref(),
        zhfst.as_os_str(),
        "-a".as_ref(),
        fixture.as_ref(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success(), "-z with -a must not pass");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--zhfst cannot be combined with"),
        "diagnostic names the conflict"
    );
}
