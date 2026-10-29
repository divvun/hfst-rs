// Optimized-lookup streams as `HfstInputStream` reads them: a pmatch
// container, whose definitions borrow the alphabet of the transducer before
// them, and payloads written with no HFST3 header.

use hfst::file_resolver::{FileResolver, MemoryFiles};
use hfst::hfst_data_types::ImplementationType::{
    self, FOMA_TYPE, HFST_OL_TYPE, HFST_OLW_TYPE, TROPICAL_OPENFST_TYPE,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::AnyTransducer;
use hfst::pmatch_compiler::{PmatchCompiler, write_archive};
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

// The tropical transition-data symbol coding lives in process-global statics;
// cargo runs each #[test] as a thread of one process, so compiling is
// serialized here as it is in the other suites.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

const GRAMMAR: &str = "define Animal {cat} | {dog} ;\n\
                       define TOP Ins(Animal) EndTag(Animal) | {mouse} EndTag(Small) ;\n";

fn regex(src: &str, name: &str) -> AnyTransducer {
    let mut t = XreCompiler::<StdVectorFst>::new()
        .compile(src)
        .unwrap_or_else(|| panic!("{src} compiles"));
    t.set_name(name);
    AnyTransducer::Tropical(t)
}

fn written(ty: ImplementationType, hfst_format: bool, transducers: Vec<AnyTransducer>) -> Vec<u8> {
    let files = MemoryFiles::default();
    let out = files.create("t").expect("create in memory");
    let mut stream =
        HfstOutputStream::from_writer(out, ty, hfst_format).expect("open a stream on the writer");
    for t in transducers {
        let mut t = t.convert(ty, "").expect("convert to the stream type");
        t.write(&mut stream).expect("write transducer");
    }
    stream.close();
    files.get("t").expect("the stream was written")
}

fn read_all(bytes: &[u8]) -> hfst::error::Result<Vec<AnyTransducer>> {
    let mut cursor = bytes;
    let mut stream = HfstInputStream::read_from(&mut cursor)?;
    let mut all = Vec::new();
    while stream.is_good() {
        all.push(stream.read()?);
    }
    Ok(all)
}

fn container() -> Vec<u8> {
    let mut definitions = PmatchCompiler::<StdVectorFst>::new()
        .compile(GRAMMAR)
        .expect("the grammar compiles");
    let files = MemoryFiles::default();
    let out = files.create("c").expect("create in memory");
    let mut stream =
        HfstOutputStream::from_writer(out, HFST_OLW_TYPE, true).expect("open a stream");
    let wrote = write_archive(&mut definitions, &mut stream, false, &mut std::io::sink())
        .expect("the archive is written");
    assert!(wrote, "the grammar has a TOP");
    stream.close();
    files.get("c").expect("the container was written")
}

fn outputs(t: &AnyTransducer, input: &str) -> Vec<String> {
    let AnyTransducer::OlW(t) = t else {
        panic!("a pmatch container holds weighted optimized lookup");
    };
    t.lookup_fd_string(input, -1, 0.0)
        .expect("lookup runs")
        .iter()
        .map(|path| path.second.iter().map(|s| s.to_string()).collect())
        .collect()
}

// [spec:hfst:req:javascript.load-bytes+2/test]
// The definitions of a container are written with an empty alphabet; they
// read as transducers of their own with TOP's alphabet, in stream order.
#[test]
fn a_pmatch_container_reads_as_its_transducers() {
    let _guard = serialized();
    let members = read_all(&container()).expect("the container reads");
    let names: Vec<String> = members.iter().map(AnyTransducer::get_name).collect();
    assert_eq!(names, ["TOP", "Animal"]);
    assert!(members.iter().all(|t| t.get_type() == HFST_OLW_TYPE));
    assert_eq!(outputs(&members[1], "cat"), ["cat"]);
    assert_eq!(outputs(&members[1], "dog"), ["dog"]);
    assert!(outputs(&members[1], "mouse").is_empty());
}

// [spec:hfst:req:javascript.load-bytes+2/test]
// Each transducer carries the properties of its own header: TOP's pmatch
// settings do not leak onto the definition after it.
#[test]
fn each_member_has_only_its_own_header_properties() {
    let _guard = serialized();
    let members = read_all(&container()).expect("the container reads");
    assert_eq!(members[0].get_property("max-recursion"), "5000");
    assert_eq!(members[1].get_property("max-recursion"), "");
    assert!(!members[1].get_properties().contains_key("max-recursion"));
}

// [spec:hfst:req:javascript.load-bytes+2/test]
// What `hfst fst2fst -b` writes: payloads with no HFST3 header, one after
// another, weighted and unweighted.
#[test]
fn headerless_optimized_lookup_reads_in_stream_order() {
    let _guard = serialized();
    for ty in [HFST_OLW_TYPE, HFST_OL_TYPE] {
        let bytes = written(
            ty,
            false,
            vec![regex("{cat}", "first"), regex("{dog} | {cow}", "second")],
        );
        assert!(!bytes.starts_with(b"HFST"), "{ty:?} has no header");
        let mut cursor = &bytes[..];
        let mut stream = HfstInputStream::read_from(&mut cursor).expect("the stream opens");
        assert_eq!(stream.get_type(), ty);
        assert!(!stream.is_hfst_header_included());
        let mut members = Vec::new();
        while stream.is_good() {
            members.push(stream.read().expect("each member reads"));
        }
        assert_eq!(members.len(), 2, "{ty:?}");
        assert!(
            members
                .iter()
                .all(|t| t.get_type() == ty && t.get_name().is_empty())
        );
        let alphabet_size =
            |t: &AnyTransducer| t.to_basic().expect("to basic").get_alphabet().len();
        assert!(
            alphabet_size(&members[0]) < alphabet_size(&members[1]),
            "{ty:?} keeps stream order"
        );
    }
}

// [spec:hfst:req:javascript.load-bytes+2/test]
// A stream that starts with a headerless payload and goes on with a headered
// one of the same type reads both, as the type of each member is all the
// stream checks.
#[test]
fn headered_and_headerless_members_of_one_type_mix() {
    let _guard = serialized();
    let mut bytes = written(HFST_OLW_TYPE, false, vec![regex("{cat}", "bare")]);
    bytes.extend(written(HFST_OLW_TYPE, true, vec![regex("{dog}", "named")]));
    let members = read_all(&bytes).expect("the stream reads");
    let names: Vec<String> = members.iter().map(AnyTransducer::get_name).collect();
    assert_eq!(names, ["", "named"]);
}

// `converted` builds the same transducer `convert` does and leaves its source
// as it was, whatever the pair of types.
#[test]
fn converted_matches_convert_and_keeps_its_source() {
    let _guard = serialized();
    let mut types = vec![TROPICAL_OPENFST_TYPE, HFST_OLW_TYPE, HFST_OL_TYPE];
    if cfg!(feature = "foma") {
        types.push(FOMA_TYPE);
    }
    for &from in &types {
        for &to in &types {
            let source = regex("{cat}:{dog}::0.5 | {mouse}", "pair")
                .convert(from, "")
                .expect("source type");
            let before = written(from, true, vec![source.copy().expect("copy")]);
            let kept = source.converted(to, "").expect("converted");
            assert_eq!(
                written(from, true, vec![source.copy().expect("copy")]),
                before,
                "{from:?} -> {to:?} leaves the source"
            );
            let moved = source.convert(to, "").expect("convert");
            assert_eq!(kept.get_type(), to);
            assert_eq!(kept.get_name(), "pair");
            assert_eq!(
                written(to, true, vec![kept]),
                written(to, true, vec![moved]),
                "{from:?} -> {to:?}"
            );
        }
    }
}

// [spec:hfst:req:javascript.load-bytes+2/test]
// A container member read on its own has no TOP to borrow from, so it does not
// load as a transducer.
#[test]
fn a_definition_without_its_container_is_rejected() {
    let _guard = serialized();
    let bytes = container();
    let definition = bytes
        .windows(5)
        .skip(1)
        .position(|w| w == b"HFST\0")
        .map(|i| i + 1)
        .expect("a second member");
    assert!(read_all(&bytes[definition..]).is_err());
}
