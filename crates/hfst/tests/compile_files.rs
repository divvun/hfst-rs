//! What a compile reads comes through the caller's file resolver, and an
//! xfst script writes only through its resolver and its output sink:
//! `write definition NAME`, the one binary save that takes no file name,
//! writes to the session output, not the process's standard output.

use std::sync::Arc;

use hfst::file_resolver::{FileResolver, MemoryFiles};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::xfst_compiler::XfstCompiler;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

fn regex(src: &str) -> HfstTransducer<StdVectorFst> {
    XreCompiler::<StdVectorFst>::new()
        .compile(src)
        .unwrap_or_else(|| panic!("{src} compiles"))
}

/// The transducers in a binary stream, with their names.
fn read_all(bytes: Vec<u8>) -> Vec<(String, HfstTransducer<StdVectorFst>)> {
    let mut cursor = std::io::Cursor::new(bytes);
    let mut stream = HfstInputStream::read_from(&mut cursor).expect("a transducer stream");
    let mut read = Vec::new();
    while stream.is_good() {
        let t = stream.read().expect("a transducer");
        read.push((t.get_name(), t.into_typed().expect("tropical")));
    }
    read
}

fn script_output(script: &str) -> Vec<u8> {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(script).expect("the script runs");
    c.take_output()
}

// [spec:hfst:req:embed.compile-files/test]
#[test]
fn unnamed_definition_save_joins_session_output() {
    let saved = read_all(script_output(
        "set name-nets ON\ndefine V [a|e] ;\nwrite definition V\n",
    ));
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].0, "V");
    assert!(saved[0].1.compare(&regex("a|e"), true).expect("tropical"));
}

// [spec:hfst:req:embed.compile-files/test]
#[cfg(unix)]
#[test]
fn system_command_prints_into_session_output() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("system echo from-the-shell\n")
        .expect("the script runs");
    let printed = String::from_utf8(c.take_output()).expect("utf8");
    assert!(printed.contains("from-the-shell"), "{printed:?}");
}

// [spec:hfst:req:embed.compile-files/test]
#[test]
fn read_lexc_reads_through_the_resolver() {
    let files = MemoryFiles::default();
    files.insert("animals.lexc", "LEXICON Root\n< @txt\"w.txt\" > # ;\n");
    files.insert("w.txt", "dog\n");
    let files: Arc<dyn FileResolver> = Arc::new(files);
    let mut diagnostics = Vec::new();
    let t = HfstTransducer::<StdVectorFst>::read_lexc(
        "animals.lexc",
        false,
        Arc::clone(&files),
        &mut diagnostics,
    )
    .expect("the lexc file compiles");
    assert!(t.compare(&regex("{dog}"), true).expect("tropical"));

    let missing =
        HfstTransducer::<StdVectorFst>::read_lexc("missing.lexc", false, files, &mut diagnostics);
    let Err(e) = missing else {
        panic!("a file the resolver lacks compiled");
    };
    assert!(e.to_string().contains("missing.lexc"), "{e}");
}
