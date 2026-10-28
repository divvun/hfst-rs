// Named files through a caller-supplied resolver: xfst commands, xre file
// references and pmatch includes and file references read the files they
// name from an in-memory resolver, and xfst writes land in it
// (docs/spec/javascript.md).
use std::sync::Arc;

use hfst::backend_thfst::ThfstTransducer;
use hfst::file_resolver::{FileResolver, MemoryFiles};
use hfst::hfst_data_types::ImplementationType::{THFST_TYPE, TROPICAL_OPENFST_TYPE};
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::transducer::{Transducer, WeightedTables};
use hfst::xfst_compiler::XfstCompiler;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

fn xfst(files: &MemoryFiles) -> XfstCompiler<StdVectorFst> {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_file_resolver(Arc::new(files.clone()));
    c
}

fn text(files: &MemoryFiles, name: &str) -> String {
    let bytes = files
        .get(name)
        .unwrap_or_else(|| panic!("{name} was not written"));
    String::from_utf8(bytes).expect("utf8 output")
}

fn regex(src: &str) -> HfstTransducer<StdVectorFst> {
    XreCompiler::<StdVectorFst>::new()
        .compile(src)
        .unwrap_or_else(|| panic!("{src} compiles"))
}

// The bytes of a binary transducer file holding `src` named `name`.
fn binary(src: &str, name: &str) -> Vec<u8> {
    let mut t = regex(src);
    t.set_name(name);
    let scratch = MemoryFiles::default();
    let out = scratch.create("t.hfst").expect("create in memory");
    let mut stream = HfstOutputStream::from_writer(out, TROPICAL_OPENFST_TYPE, true)
        .expect("open a stream on the writer");
    stream.write(&mut t).expect("write transducer");
    stream.close();
    scratch.get("t.hfst").expect("the transducer was written")
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// A sourced script and the word list it reads both come from the resolver,
// and the words written out land in it.
#[test]
fn xfst_source_reads_nested_files_through_the_resolver() {
    let files = MemoryFiles::default();
    files.insert("main.xfst", "source inner.xfst\n");
    files.insert("inner.xfst", "read text words.txt\n");
    files.insert("words.txt", "cat\ndog\n");
    let mut c = xfst(&files);
    c.parse("source main.xfst\nwrite text out.txt\n")
        .expect("the scripts run");
    assert_eq!(text(&files, "out.txt"), "cat\ndog\n");
}

// [spec:hfst:req:javascript.named-file-resolver/test]
#[test]
fn xfst_loads_stack_and_definitions_from_the_resolver() {
    let files = MemoryFiles::default();
    files.insert("nets.hfst", binary("{cat}", "Cat"));
    let mut c = xfst(&files);
    c.parse("load stack nets.hfst\nload defined nets.hfst\nwrite text out.txt\n")
        .expect("the networks load");
    assert_eq!(c.get_stack().len(), 1);
    assert!(c.definitions.contains_key("Cat"));
    assert_eq!(text(&files, "out.txt"), "cat\n");
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// The binary saves write into the resolver, and a later session loads what
// they wrote.
#[test]
fn xfst_saves_round_trip_through_the_resolver() {
    let files = MemoryFiles::default();
    xfst(&files)
        .parse("regex {cat} ;\nregex {dog} ;\nsave stack s.hfst\ndefine Pet {cow} ;\nsave defined d.hfst\n")
        .expect("the networks are saved");
    let mut c = xfst(&files);
    c.parse("load stack s.hfst\nload defined d.hfst\nwrite text out.txt\n")
        .expect("the saved networks load");
    assert_eq!(c.get_stack().len(), 2);
    assert!(c.definitions.contains_key("Pet"));
    assert_eq!(text(&files, "out.txt"), "dog\n");
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// The filesystem resolver loads a .thfst transducer, which is a directory,
// from its path.
#[test]
fn xfst_loads_a_thfst_directory_from_the_filesystem() {
    let dir = std::env::temp_dir().join(format!("hfst-file-resolver-{}.thfst", std::process::id()));
    let dir_name = dir.to_str().expect("utf8 temp path").to_string();
    let basic = regex("{cat}").to_basic().expect("basic transducer");
    let mut thfst = HfstTransducer::<ThfstTransducer>::new_from_basic(&basic).expect("thfst");
    let mut out =
        HfstOutputStream::new_filename(&dir_name, THFST_TYPE, true).expect("thfst stream");
    out.write(&mut thfst).expect("write thfst");
    out.close();
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let loaded = c.parse(&format!("load stack {dir_name}\n"));
    std::fs::remove_dir_all(&dir).expect("remove the thfst directory");
    loaded.expect("the directory loads");
    assert_eq!(c.get_stack().len(), 1);
    let top = c.net(c.get_stack()[0]);
    assert!(top.compare(&regex("{cat}"), true).expect("compare"));
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// A resolver with no filesystem paths cannot supply a directory, and the
// error names it and says it is one.
#[test]
fn a_pathless_directory_fails_naming_it() {
    let files = MemoryFiles::default();
    for member in ["alphabet", "index", "transition"] {
        files.insert(format!("x.thfst/{member}"), Vec::new());
    }
    for script in ["load stack x.thfst\n", "regex @bin\"x.thfst\" ;\n"] {
        let err = xfst(&files)
            .parse(script)
            .expect_err("a directory cannot be read as bytes");
        let message = err.to_string();
        assert!(message.contains("x.thfst"), "{script:?}: {message}");
        assert!(message.contains("directory"), "{script:?}: {message}");
    }
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    compiler.set_file_resolver(Arc::new(files.clone()));
    let Err(err) = compiler.compile("Define TOP @bin\"x.thfst\" ;\n") else {
        panic!("a directory compiled as a binary reference");
    };
    let message = err.to_string();
    assert!(
        message.contains("x.thfst") && message.contains("directory"),
        "{message}"
    );
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// 'read lexc' reads the lexc file through the resolver, and so does the
// regex file reference inside it.
#[test]
fn xfst_read_lexc_reads_its_regex_files_too() {
    let files = MemoryFiles::default();
    files.insert("x.lexc", "LEXICON Root\n< @txt\"w.txt\" > # ;\n");
    files.insert("w.txt", "dog\n");
    let mut c = xfst(&files);
    c.parse("read lexc x.lexc\nwrite text out.txt\n")
        .expect("the lexc file compiles");
    assert_eq!(text(&files, "out.txt"), "dog\n");
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// A text file an xfst command writes can be read back by another command,
// and an appending redirect adds to it.
#[test]
fn xfst_writes_land_in_the_resolver() {
    let files = MemoryFiles::default();
    let mut c = xfst(&files);
    c.parse(
        "regex {ab} ;\nwrite prolog p.pl\nclear stack\nread prolog p.pl\n\
         print words > out.txt\nprint words >> out.txt\n",
    )
    .expect("the script runs");
    assert!(text(&files, "p.pl").contains("network("));
    let out = text(&files, "out.txt");
    assert_eq!(out.matches("ab").count(), 2, "{out}");
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// Every kind of xre file reference reads through the resolver, including a
// regex file that itself names a word list.
#[test]
fn xre_file_references_read_through_the_resolver() {
    let files = MemoryFiles::default();
    files.insert("w.txt", "cat\n");
    files.insert("s.txt", "c a t\n");
    files.insert("r.re", "@txt\"w.txt\"");
    files.insert("b.hfst", binary("{cat}", "cat"));
    xfst(&files)
        .parse("regex {cat} ;\nwrite prolog p.pl\n")
        .expect("the prolog file is written");
    let expected = regex("{cat}");
    let mut c = XreCompiler::<StdVectorFst>::new();
    c.set_file_resolver(Arc::new(files.clone()));
    for src in [
        "@txt\"w.txt\"",
        "@stxt\"s.txt\"",
        "@pl\"p.pl\"",
        "@re\"r.re\"",
        "@bin\"b.hfst\"",
    ] {
        let t = c.compile(src).unwrap_or_else(|| panic!("{src} compiles"));
        assert!(t.compare(&expected, true).expect("compare"), "{src}");
    }
}

// Compile `src` with `files` and match `input` against its TOP.
fn pmatch(files: &MemoryFiles, src: &str, input: &str) -> Result<String, hfst::error::Error> {
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    compiler.set_file_resolver(Arc::new(files.clone()));
    let defs = compiler.compile(src)?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    let top = HfstTransducer::<Transducer<WeightedTables>>::new_from_basic(&top.to_basic()?)?;
    let mut container = PmatchContainer::new_from_hfst_transducers(vec![top])?;
    Ok(container.do_match(input, 0.0, 0.0))
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// An include inside an include reads through the same resolver.
#[test]
fn pmatch_includes_nest_through_the_resolver() -> Result<(), hfst::error::Error> {
    let files = MemoryFiles::default();
    files.insert(
        "a.pmatch",
        "@include\"b.pmatch\"\nDefine Animal Pet | {cow} ;\n",
    );
    files.insert("b.pmatch", "Define Pet {cat} | {dog} ;\n");
    let src = "@include\"a.pmatch\"\nDefine TOP Animal EndTag(animal) ;\n";
    let out = pmatch(&files, src, "cat cow")?;
    assert_eq!(out, "<animal>cat</animal> <animal>cow</animal>");
    Ok(())
}

// [spec:hfst:req:javascript.named-file-resolver/test]
#[test]
fn pmatch_file_references_read_through_the_resolver() -> Result<(), hfst::error::Error> {
    let files = MemoryFiles::default();
    files.insert("b.hfst", binary("{cat}", "cat"));
    files.insert("w.txt", "dog\n");
    files.insert("x.lexc", "LEXICON Root\ncow # ;\n");
    files.insert("r.re", "{pig}");
    let src = "Define TOP [@bin\"b.hfst\" | @txt\"w.txt\" | @lexc\"x.lexc\" | @re\"r.re\"] \
               EndTag(animal) ;\n";
    let out = pmatch(&files, src, "cat dog cow pig")?;
    assert_eq!(
        out,
        "<animal>cat</animal> <animal>dog</animal> <animal>cow</animal> <animal>pig</animal>"
    );
    Ok(())
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// A name the resolver cannot supply fails the command, and the error names
// the file.
#[test]
fn a_missing_name_fails_xfst_naming_it() {
    let files = MemoryFiles::default();
    files.insert("main.xfst", "source missing-inner.xfst\n");
    for (script, name) in [
        ("source missing.xfst\n", "missing.xfst"),
        ("source main.xfst\n", "missing-inner.xfst"),
        ("load stack missing.hfst\n", "missing.hfst"),
        ("load defined missing.hfst\n", "missing.hfst"),
        ("read lexc missing.lexc\n", "missing.lexc"),
        ("read text missing.txt\n", "missing.txt"),
        ("read att missing.att\n", "missing.att"),
        ("read prolog missing.pl\n", "missing.pl"),
        ("regex @txt\"missing.txt\" ;\n", "missing.txt"),
    ] {
        let err = xfst(&files)
            .parse(script)
            .expect_err("a missing file fails");
        assert!(
            err.to_string().contains(name),
            "{script:?} did not name {name}: {err}"
        );
    }
}

// [spec:hfst:req:javascript.named-file-resolver/test]
#[test]
fn a_missing_name_fails_pmatch_naming_it() {
    let files = MemoryFiles::default();
    files.insert("a.pmatch", "@include\"missing-inner.pmatch\"\n");
    for (src, name) in [
        (
            "@include\"missing.pmatch\"\nDefine TOP {a} ;\n",
            "missing.pmatch",
        ),
        (
            "@include\"a.pmatch\"\nDefine TOP {a} ;\n",
            "missing-inner.pmatch",
        ),
        ("Define TOP @bin\"missing.hfst\" ;\n", "missing.hfst"),
        ("Define TOP @txt\"missing.txt\" ;\n", "missing.txt"),
        ("Define TOP @lexc\"missing.lexc\" ;\n", "missing.lexc"),
        ("Define TOP @re\"missing.re\" ;\n", "missing.re"),
        ("Define TOP @pl\"missing.pl\" ;\n", "missing.pl"),
    ] {
        let mut compiler = PmatchCompiler::<StdVectorFst>::new();
        compiler.set_file_resolver(Arc::new(files.clone()));
        let Err(err) = compiler.compile(src) else {
            panic!("{src:?} compiled without {name}");
        };
        assert!(
            err.to_string().contains(name),
            "{src:?} did not name {name}: {err}"
        );
    }
}

// [spec:hfst:req:javascript.named-file-resolver/test]
// A file that includes itself fails at the nesting limit.
#[test]
fn a_self_include_fails() {
    let files = MemoryFiles::default();
    files.insert("loop.pmatch", "@include\"loop.pmatch\"\n");
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    compiler.set_file_resolver(Arc::new(files.clone()));
    let Err(err) = compiler.compile("@include\"loop.pmatch\"\nDefine TOP {a} ;\n") else {
        panic!("a self-include compiled");
    };
    assert!(err.to_string().contains("loop.pmatch"), "{err}");
}
