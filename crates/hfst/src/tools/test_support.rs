//! What the tools' tests share: transducers from regular expressions, and
//! transducer streams in memory.

use crate::file_resolver::{FileResolver, MemoryFiles};
use crate::hfst_data_types::ImplementationType;
use crate::hfst_input_stream::HfstInputStream;
use crate::hfst_output_stream::HfstOutputStream;
use crate::hfst_transducer::{AnyTransducer, HfstTransducer};
use crate::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

pub(super) type Tropical = HfstTransducer<StdVectorFst>;

pub(super) fn regex(src: &str) -> Tropical {
    XreCompiler::<StdVectorFst>::new()
        .compile(src)
        .unwrap_or_else(|| panic!("{src} compiles"))
}

/// The bytes of a stream of type `ty` that `write` writes.
pub(super) fn stream_bytes(
    ty: ImplementationType,
    write: impl FnOnce(&mut HfstOutputStream),
) -> Vec<u8> {
    let files = MemoryFiles::default();
    let out = files.create("stream").expect("a file in memory");
    let mut stream = HfstOutputStream::from_writer(out, ty, true).expect("a stream");
    write(&mut stream);
    stream.close();
    files.get("stream").expect("the stream was written")
}

/// The transducers of the stream `bytes`.
pub(super) fn read_all(bytes: &[u8]) -> Vec<AnyTransducer> {
    let mut cursor = std::io::Cursor::new(bytes.to_vec());
    let mut stream = HfstInputStream::read_from(&mut cursor).expect("a transducer stream");
    let mut read = Vec::new();
    while stream.is_good() {
        read.push(stream.read().expect("a transducer"));
    }
    read
}

/// Whether `t` is a tropical transducer with the language of `src`.
pub(super) fn is_language(t: &AnyTransducer, src: &str) -> bool {
    match t {
        AnyTransducer::Tropical(t) => t.compare(&regex(src), true).expect("tropical"),
        AnyTransducer::OlW(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_) => false,
        #[cfg(feature = "foma")]
        AnyTransducer::Foma(_) => false,
    }
}

/// Each path of `t` as `input:output weight`, sorted.
pub(super) fn paths_of(t: &Tropical) -> Vec<String> {
    let mut paths = crate::hfst_data_types::HfstTwoLevelPaths::new();
    t.extract_paths(&mut paths, -1, -1).expect("paths");
    let side = |pairs: &crate::hfst_data_types::StringPairVector, output: bool| -> String {
        pairs
            .iter()
            .map(|(i, o)| if output { o } else { i })
            .filter(|s| !crate::hfst_symbol_defs::is_epsilon(s))
            .map(|s| s.as_str())
            .collect()
    };
    let mut shown: Vec<String> = paths
        .iter()
        .map(|p| {
            format!(
                "{}:{} {}",
                side(&p.second, false),
                side(&p.second, true),
                p.first
            )
        })
        .collect();
    shown.sort();
    shown
}
