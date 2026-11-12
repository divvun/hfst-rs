//! Packing BHFST archives in memory: sources, meta.json, zhfst input, and
//! the archive read back.

use std::io::Write;

use super::bhfst::{
    BhfstError, MetaFile, MetaSource, PackSource, SourceInput, SpellerFiles, build_meta_json,
    dhfst_meta_ids, is_dhfst, meta_json_of, pack, pack_files, pack_zhfst, parse_speller_config,
    read_zhfst, resolve_source,
};
use super::test_support::{regex, stream_bytes};
use crate::dhfst_header::DhfstType;
use crate::file_resolver::MemoryFiles;
use crate::hfst_data_types::ImplementationType;
use box_format::core::ArchiveReader;
use box_format::{BoxPath, Record};

const INDEX_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<hfstspeller dtdversion="1.0" hfstversion="3">
  <info>
    <locale>sma</locale>
    <title>Test speller</title>
    <description>A tiny test speller.</description>
    <producer>tests</producer>
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

fn olw(src: &str) -> Vec<u8> {
    stream_bytes(ImplementationType::HFST_OLW_TYPE, |out| {
        out.write_converted(&regex(src)).expect("written");
    })
}

fn source(src: &str, wanted: DhfstType) -> PackSource {
    resolve_source("t", SourceInput::Bytes(olw(src)), wanted, &mut |_| {}).expect("resolved")
}

fn index_xml() -> MetaSource {
    MetaSource::IndexXml {
        origin: "index.xml".into(),
        bytes: INDEX_XML.as_bytes().to_vec(),
    }
}

fn meta(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("meta.json is JSON")
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn dhfst_files_are_told_by_their_header() {
    assert!(is_dhfst(b"DHFST\x01\x01\0"));
    assert!(!is_dhfst(b"HFST\0\0\0\0"));
    assert!(!is_dhfst(b"DHF"));
    assert_eq!(
        dhfst_meta_ids(true, false),
        (Some("acceptor.default.dhfst"), None)
    );
    assert_eq!(
        dhfst_meta_ids(false, true),
        (None, Some("errmodel.default.dhfst"))
    );
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn index_xml_becomes_meta_json() {
    let json = build_meta_json(index_xml(), None, (None, None))
        .expect("built")
        .expect("some metadata");
    let value = meta(&json);
    assert_eq!(value["info"]["locale"], "sma");
    assert_eq!(value["acceptor"]["id"], "acceptor.default.thfst");
    assert_eq!(value["errmodel"]["id"], "errmodel.default.thfst");
    assert!(value.get("spellerConfig").is_none());

    let config = parse_speller_config("c.json", br#"{"beam": 2}"#).expect("an object");
    let ids = dhfst_meta_ids(true, false);
    let json = build_meta_json(index_xml(), Some(config), ids)
        .expect("built")
        .expect("some metadata");
    let value = meta(&json);
    assert_eq!(value["acceptor"]["id"], "acceptor.default.dhfst");
    assert_eq!(value["spellerConfig"]["beam"], 2);
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn verbatim_meta_json_is_kept_or_merged() {
    let verbatim = || MetaSource::Verbatim {
        origin: "meta.json".into(),
        bytes: b"{\"extra\": 1}".to_vec(),
    };
    let kept = build_meta_json(verbatim(), None, (None, None)).expect("built");
    assert_eq!(kept.as_deref(), Some(&b"{\"extra\": 1}"[..]));
    let config = parse_speller_config("c.json", b"{}").expect("an object");
    let merged = build_meta_json(verbatim(), Some(config), (None, None))
        .expect("built")
        .expect("some metadata");
    assert_eq!(meta(&merged)["extra"], 1);
    assert!(meta(&merged)["spellerConfig"].is_object());
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn speller_configs_must_be_objects_with_metadata() {
    let error = parse_speller_config("c.json", b"[1]")
        .err()
        .map(|e| e.to_string());
    assert_eq!(
        error,
        Some(
            "c.json is not a valid speller configuration: expected a JSON object, found an array"
                .into()
        )
    );
    let config = parse_speller_config("c.json", b"{}").expect("an object");
    let bare = build_meta_json(MetaSource::None, Some(config), (None, None));
    assert_eq!(bare, Err(BhfstError::ConfigWithoutMetadata));
}

fn member(archive: &[u8], path: &str) -> Vec<u8> {
    let header = ArchiveReader::parse_header(archive).expect("a header");
    let at = header.trailer.expect("a trailer").get() as usize;
    let meta = ArchiveReader::parse_metadata(&archive[at..], header.version).expect("metadata");
    let reader = ArchiveReader::new(header, meta, 0);
    let path = BoxPath::new(path).expect("a box path");
    let Some(Record::File(file)) = reader.find(&path) else {
        panic!("{path} is not a file of the archive");
    };
    let (offset, len) = reader.file_location(file);
    assert_eq!(offset % 8, 0, "{path} is 8-byte aligned");
    archive[offset as usize..(offset + len) as usize].to_vec()
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn packed_archive_holds_each_member() {
    let acceptor = source("{cat} | {dog}", DhfstType::Acceptor);
    let errmodel = source("[?:?]*", DhfstType::ErrorModel);
    let json = build_meta_json(index_xml(), None, (None, None)).expect("built");
    let mut heard = Vec::new();
    let archive = pack(json.as_deref(), &acceptor, &errmodel, &mut |m| {
        heard.push(m.to_string())
    })
    .expect("packed");
    assert_eq!(
        heard,
        [
            "Inserting acceptor.default.thfst...\n",
            "Inserting errmodel.default.thfst...\n",
            "Inserting meta.json...\n"
        ]
    );
    let PackSource::Thfst(members) = &acceptor else {
        panic!("the acceptor is THFST");
    };
    assert_eq!(
        member(&archive, "acceptor.default.thfst/index"),
        members.index
    );
    assert_eq!(meta_json_of("a.bhfst", &archive).ok(), json);
    let no_meta = pack(None, &acceptor, &errmodel, &mut |_| {}).expect("packed");
    assert_eq!(
        meta_json_of("a.bhfst", &no_meta)
            .err()
            .map(|e| e.to_string()),
        Some("a.bhfst contains no meta.json metadata".into())
    );
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn dhfst_of_the_wrong_type_is_refused() {
    let bad = resolve_source(
        "e.dhfst",
        SourceInput::Bytes(b"DHFST\x01\x01\0".to_vec()),
        DhfstType::Acceptor,
        &mut |_| {},
    );
    let Err(e) = bad else {
        panic!("an error model passed as an acceptor");
    };
    assert!(
        e.to_string()
            .starts_with("e.dhfst is a DHFST error model (type 1)"),
        "{e}"
    );
    let garbage = resolve_source(
        "x",
        SourceInput::Bytes(b"junk".to_vec()),
        DhfstType::Acceptor,
        &mut |_| {},
    );
    assert!(garbage.is_err());
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn zhfst_members_are_read_by_their_ids() {
    let acceptor = olw("{cat}");
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in [
        ("index.xml", INDEX_XML.as_bytes()),
        ("acceptor.default.hfst", &acceptor),
        ("errmodel.default.hfst", &acceptor),
    ] {
        zip.start_file(name, options).expect("a member");
        zip.write_all(bytes).expect("written");
    }
    let bytes = zip.finish().expect("a zip").into_inner();
    let zhfst = read_zhfst("s.zhfst", &bytes, &mut |_| {}).expect("read");
    assert_eq!(
        zhfst.acceptor,
        ("acceptor.default.hfst".to_string(), acceptor)
    );
    assert!(zhfst.speller_config.is_none());
    let missing = read_zhfst("s.zhfst", b"not a zip", &mut |_| {});
    assert!(missing.is_err());
}

fn zhfst_of(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in members {
        zip.start_file(*name, options).expect("a member");
        zip.write_all(bytes).expect("written");
    }
    zip.finish().expect("a zip").into_inner()
}

// [spec:hfst:sem:dhfst.bhfst-member+2/test]
// [spec:hfst:sem:thfst-backend.bhfst-tool/test]
// [spec:hfst:req:embed.tools-in-library/test]
// A ready THFST directory is packed as it is, a transducer stream is
// converted to THFST, and meta.json names a DHFST member by its file.
#[test]
fn files_pack_from_a_resolver() {
    let files = MemoryFiles::default();
    let errmodel = olw("[?:?]*");
    files.insert("e.hfst", errmodel.clone());
    files.insert("index.xml", INDEX_XML);
    for (member, bytes) in [("alphabet", "{}"), ("index", "ix"), ("transition", "tr")] {
        files.insert(format!("a.thfst/{member}"), bytes);
    }
    let speller = SpellerFiles {
        acceptor: "a.thfst".to_string(),
        errmodel: "e.hfst".to_string(),
        meta: MetaFile::IndexXml("index.xml".to_string()),
        speller_config: None,
    };
    let mut heard = Vec::new();
    let archive = pack_files(&speller, &files, &mut |m| heard.push(m.to_string())).expect("packed");
    assert_eq!(member(&archive, "acceptor.default.thfst/index"), b"ix");
    assert_eq!(
        heard[..3],
        [
            "Converting index.xml (index.xml) to meta.json...\n",
            "Using ready THFST directory a.thfst\n",
            "Reading e.hfst and converting to THFST...\n",
        ]
    );
    let json = meta(&meta_json_of("s.bhfst", &archive).expect("meta.json"));
    assert_eq!(json["acceptor"]["id"], "acceptor.default.thfst");

    files.insert("e.dhfst", b"DHFST\x01\x01\0".to_vec());
    let dhfst = SpellerFiles {
        errmodel: "e.dhfst".to_string(),
        ..speller.clone()
    };
    let refused = pack_files(&dhfst, &files, &mut |_| {}).expect_err("not loadable");
    assert!(
        refused.to_string().starts_with("e.dhfst is not a DHFST"),
        "{refused}"
    );

    let missing = SpellerFiles {
        acceptor: "nowhere.hfst".to_string(),
        ..speller
    };
    let refused = pack_files(&missing, &files, &mut |_| {}).expect_err("no such file");
    assert_eq!(
        refused.to_string(),
        "cannot open nowhere.hfst: not a transducer stream: file could not be opened"
    );
}

// [spec:hfst:sem:thfst-backend.speller-config/test]
// [spec:hfst:sem:thfst-backend.zhfst-input/test]
// [spec:hfst:req:embed.tools-in-library/test]
// A speller configuration named on its own wins over the zhfst's.
#[test]
fn an_explicit_speller_config_wins() {
    let acceptor = olw("{cat}");
    let files = MemoryFiles::default();
    files.insert(
        "s.zhfst",
        zhfst_of(&[
            ("index.xml", INDEX_XML.as_bytes()),
            ("acceptor.default.hfst", &acceptor),
            ("errmodel.default.hfst", &acceptor),
            ("speller-config.json", br#"{"beam": 2}"#),
        ]),
    );
    files.insert("c.json", br#"{"beam": 1}"#.to_vec());
    let beam = |config: Option<&str>| {
        let archive = pack_zhfst("s.zhfst", config, &files, &mut |_| {}).expect("packed");
        meta(&meta_json_of("s.bhfst", &archive).expect("meta.json"))["spellerConfig"]["beam"]
            .clone()
    };
    assert_eq!(beam(Some("c.json")), 1);
    assert_eq!(beam(None), 2);
    let unread = pack_zhfst("s.zhfst", Some("absent.json"), &files, &mut |_| {});
    assert!(unread.is_err_and(|e| e.to_string().starts_with("cannot read absent.json")));
}

// [spec:hfst:sem:thfst-backend.bhfst-tool/test]
// A directory without all three THFST members is not a source.
#[test]
fn a_directory_without_members_is_not_a_source() {
    let files = MemoryFiles::default();
    files.insert("p.thfst/index", b"ix".to_vec());
    let speller = SpellerFiles {
        acceptor: "p.thfst".to_string(),
        errmodel: "p.thfst".to_string(),
        meta: MetaFile::None,
        speller_config: None,
    };
    let refused = pack_files(&speller, &files, &mut |_| {}).expect_err("not a source");
    assert_eq!(
        refused.to_string(),
        "cannot open p.thfst: not a transducer stream: file could not be opened"
    );
}
