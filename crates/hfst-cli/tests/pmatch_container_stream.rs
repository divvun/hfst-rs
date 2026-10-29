//! The tools read a pmatch container as a stream of transducers, and read
//! optimized lookup written with no HFST3 header. A container's definitions
//! are written with an empty alphabet and TOP's symbol numbering, so the
//! stream reader has to give each of them TOP's alphabet.

mod common;

use common::run;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hfst-pmatch-container-{name}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn path(p: &Path) -> &str {
    p.to_str().expect("utf8 path")
}

fn container(dir: &Path) -> PathBuf {
    let source = dir.join("animals.pmatch");
    std::fs::write(
        &source,
        "define Animal {cat} | {dog} ;\n\
         define TOP Ins(Animal) EndTag(Animal) | {mouse} EndTag(Small) ;\n",
    )
    .expect("write grammar");
    let out = dir.join("animals.pmhfst");
    let (ok, _) = run(&["pmatch2fst", path(&source), "-o", path(&out)], b"");
    assert!(ok, "pmatch2fst failed");
    out
}

// [spec:hfst:req:javascript.load-bytes+1/test]
#[test]
fn head_reads_every_transducer_of_a_pmatch_container() {
    let dir = scratch("head");
    let archive = container(&dir);
    let copy = dir.join("copy.pmhfst");
    let (ok, _) = run(
        &["head", "-n", "100", path(&archive), "-o", path(&copy)],
        b"",
    );
    assert!(ok, "hfst head failed on a pmatch2fst container");
    let (ok, properties) = run(&["edit-metadata", "-p", path(&copy)], b"");
    assert!(ok, "edit-metadata failed on the copy");
    let names: Vec<&str> = properties
        .lines()
        .filter_map(|line| line.strip_prefix("name: "))
        .collect();
    assert_eq!(names, ["TOP", "Animal"]);
}

// [spec:hfst:req:javascript.load-bytes+1/test]
#[test]
fn a_container_definition_prints_as_its_own_transducer() {
    let dir = scratch("fst2txt");
    let archive = container(&dir);
    let (ok, text) = run(&["fst2txt", path(&archive)], b"");
    assert!(ok, "fst2txt failed on a pmatch2fst container");
    let definition = text.split("--\n").nth(1).expect("a second transducer");
    for arc in [
        "\tc\tc\t", "\ta\ta\t", "\tt\tt\t", "\td\td\t", "\to\to\t", "\tg\tg\t",
    ] {
        assert!(definition.contains(arc), "Animal has {arc:?}: {definition}");
    }
}

// [spec:hfst:req:javascript.load-bytes+1/test]
#[test]
fn headerless_optimized_lookup_reads_like_headered() {
    let dir = scratch("headerless");
    let tropical = dir.join("words.hfst");
    let (ok, _) = run(
        &["regexp2fst", "-o", path(&tropical)],
        b"{cat}:{dog}::0.5 | {mouse}\n",
    );
    assert!(ok, "regexp2fst failed");
    for format in ["-w", "-O"] {
        let headered = dir.join(format!("headered{format}.hfst"));
        let bare = dir.join(format!("bare{format}.hfstol"));
        let (ok, _) = run(
            &["fst2fst", format, path(&tropical), "-o", path(&headered)],
            b"",
        );
        assert!(ok, "fst2fst {format} failed");
        let (ok, _) = run(
            &["fst2fst", "-b", format, path(&tropical), "-o", path(&bare)],
            b"",
        );
        assert!(ok, "fst2fst -b {format} failed");
        let (ok, from_headered) = run(&["fst2txt", path(&headered)], b"");
        assert!(ok);
        let (ok, from_bare) = run(&["fst2txt", path(&bare)], b"");
        assert!(ok, "fst2txt failed on headerless {format}");
        assert_eq!(from_bare, from_headered, "{format}");
    }
}
