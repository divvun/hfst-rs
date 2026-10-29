//! DHFST files, divvunspell's speller formats, loaded from bytes: an acceptor
//! to look words up in, or an error model to describe.

use hfst::dhfst::DhfstReader;
use hfst::dhfst_acceptor::{AcceptorInfo, AcceptorReader};
use hfst::dhfst_header::{DhfstType, MAGIC, read_type};
use wasm_bindgen::prelude::*;

use crate::error::GlueError;
use crate::stream;

const INVALID: &str = "invalid-format";

/// A DHFST acceptor, type 2, read and validated as divvunspell reads one.
/// The wrapper's `DhfstAcceptor` class in `js/hfst/dhfst.ts` owns one.
#[wasm_bindgen]
pub struct DhfstAcceptor {
    reader: AcceptorReader<'static>,
}

#[wasm_bindgen]
impl DhfstAcceptor {
    pub fn version(&self) -> u8 {
        self.reader.version()
    }

    pub fn symbols(&self) -> Vec<String> {
        self.reader.symbol_names().to_vec()
    }

    pub fn meta(&self) -> Option<String> {
        self.reader.meta().map(str::to_owned)
    }

    /// How the file stores its arcs, as a plain object with the fields of
    /// the wrapper's `AcceptorInfo`.
    pub fn info(&self) -> JsValue {
        info_to_js(self.reader.info())
    }

    // [spec:hfst:req:javascript.dhfst+1]
    /// The least weight of a path that accepts `word`, as divvunspell's
    /// speller reads the word, or nothing when no path does.
    pub fn lookup(&self, word: &str) -> Option<f32> {
        self.reader.lookup(word)
    }
}

/// A DHFST error model, type 1: what its header and sections say, read once
/// the whole file has been validated. Nothing in the package searches an
/// error model, so its bytes are not kept.
#[wasm_bindgen]
pub struct DhfstErrorModel {
    version: u8,
    state_count: u32,
    symbols: Vec<String>,
    meta: Option<String>,
}

#[wasm_bindgen]
impl DhfstErrorModel {
    pub fn version(&self) -> u8 {
        self.version
    }

    #[wasm_bindgen(js_name = stateCount)]
    pub fn state_count(&self) -> u32 {
        self.state_count
    }

    pub fn symbols(&self) -> Vec<String> {
        self.symbols.clone()
    }

    pub fn meta(&self) -> Option<String> {
        self.meta.clone()
    }
}

/// A loaded DHFST file of either type.
enum Model {
    Acceptor(Box<DhfstAcceptor>),
    ErrorModel(DhfstErrorModel),
}

// [spec:hfst:req:javascript.dhfst+1]
// [spec:hfst:req:javascript.load-bytes+2]
/// The DHFST file in `bytes`, as a plain object: `type`, `acceptor` or
/// `error-model`, and `model`, the object of that type. The bytes are
/// validated as divvunspell validates them on load.
#[wasm_bindgen(js_name = readDhfst)]
pub fn read_dhfst(bytes: &[u8]) -> Result<JsValue, GlueError> {
    let (kind, model): (&str, JsValue) = match read(bytes)? {
        Model::Acceptor(acceptor) => ("acceptor", (*acceptor).into()),
        Model::ErrorModel(error_model) => ("error-model", error_model.into()),
    };
    let object = js_sys::Object::new();
    set(&object, "type", &JsValue::from_str(kind));
    set(&object, "model", &model);
    Ok(object.into())
}

fn read(bytes: &[u8]) -> Result<Model, GlueError> {
    if !bytes.starts_with(MAGIC) {
        return Err(not_dhfst(bytes));
    }
    let invalid = |error: hfst::error::Error| GlueError::new(INVALID, error.to_string());
    match read_type(bytes).map_err(invalid)? {
        DhfstType::Acceptor => {
            let reader = AcceptorReader::parse_owned(bytes.to_vec()).map_err(invalid)?;
            Ok(Model::Acceptor(Box::new(DhfstAcceptor { reader })))
        }
        DhfstType::ErrorModel => {
            let reader = DhfstReader::parse(bytes).map_err(invalid)?;
            Ok(Model::ErrorModel(DhfstErrorModel {
                version: reader.version(),
                state_count: reader.state_count(),
                symbols: reader.symbol_names().to_vec(),
                meta: reader.meta().map(str::to_owned),
            }))
        }
    }
}

/// The error for bytes that do not start as DHFST does. A transducer the
/// package reads is named, with the call that reads it; a format the package
/// does not read is turned down by name, as the transducer readers turn it
/// down.
fn not_dhfst(bytes: &[u8]) -> GlueError {
    let neither = || {
        GlueError::new(
            INVALID,
            "the bytes are not DHFST: they do not begin with \"DHFST\", nor with a transducer stream",
        )
    };
    if bytes.is_empty() {
        return GlueError::new(
            INVALID,
            "the bytes are empty, and a DHFST file begins with an eight-byte header",
        );
    }
    match stream::member(bytes) {
        Ok(member) if member.format.is_read() => GlueError::new(
            INVALID,
            format!(
                "the bytes are not DHFST: they hold an {} transducer, which readTransducers reads",
                member.format.name()
            ),
        ),
        Ok(member) => stream::unsupported(&format!(
            "a transducer in the {} format",
            member.format.name()
        )),
        Err(error) if error.code() == INVALID => neither(),
        Err(error) => error,
    }
}

/// `info` as the plain object the wrapper's `AcceptorInfo` declares. The
/// arc counts are below 2^53, so a JavaScript number holds them exactly.
fn info_to_js(info: AcceptorInfo) -> JsValue {
    let object = js_sys::Object::new();
    let numbers = [
        ("ids", f64::from(info.ids)),
        ("slots", f64::from(info.slots)),
        ("usedSlots", f64::from(info.used_slots)),
        ("checkBytes", info.check_bytes as f64),
        ("labelMax", f64::from(info.label_max)),
        ("recordBytes", info.record_bytes as f64),
        ("listRecords", f64::from(info.list_records)),
        ("arcs", info.arcs as f64),
        ("freeArcs", info.free_arcs as f64),
        ("freePairs", f64::from(info.free_pairs)),
        ("weights", f64::from(info.weights)),
        ("finals", f64::from(info.finals)),
    ];
    for (key, value) in numbers {
        set(&object, key, &JsValue::from_f64(value));
    }
    set(&object, "distances", &JsValue::from_bool(info.distances));
    object.into()
}

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(object, &JsValue::from_str(key), value)
        .expect("a fresh plain object takes any property");
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::dhfst_acceptor_writer::{AcceptorOptions, AcceptorSource};
    use hfst::dhfst_source::{SourceArc, SourceModel, SourceState};
    use hfst::dhfst_writer::WriteOptions;
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_transducer::HfstTransducer;

    /// An acceptor of `talo` (0.5) and `talon` (1.5), with a flag diacritic.
    fn acceptor() -> Vec<u8> {
        let att = "0\t1\t@P.CASE.NOM@\t@P.CASE.NOM@\t0\n1\t2\tt\tt\t0\n2\t3\ta\ta\t0\n\
                   3\t4\tl\tl\t0\n4\t5\to\to\t0.5\n5\t6\t@R.CASE.NOM@\t@R.CASE.NOM@\t0\n\
                   6\t0\n6\t7\tn\tn\t1\n7\t0\n";
        let mut lines = 0u32;
        let net =
            HfstBasicTransducer::read_in_att_format(&mut att.as_bytes(), "@0@", &mut lines, false)
                .expect("the AT&T text parses");
        let olw = HfstTransducer::<hfst_openfst::StdVectorFst>::new_from_basic(&net)
            .expect("a transducer")
            .to_ol(true, "")
            .expect("weighted optimized lookup");
        let source = AcceptorSource::new(&olw).expect("an acceptor source");
        let options = AcceptorOptions {
            threads: 1,
            source_name: "talo".into(),
        };
        hfst::dhfst_acceptor_writer::write(&source, &options)
            .expect("the acceptor writes")
            .bytes
    }

    /// An error model of two states that turns `a` into `b`.
    fn error_model() -> Vec<u8> {
        let symbols = ["@_EPSILON_SYMBOL_@", "a", "b"].map(String::from).to_vec();
        let arc = |input, output, target, weight| SourceArc {
            input,
            output,
            target,
            weight,
        };
        let states = vec![
            SourceState {
                final_weight: Some(0.0),
                arcs: vec![arc(1, 1, 0, 0.0), arc(1, 2, 1, 1.0)],
            },
            SourceState {
                final_weight: Some(0.0),
                arcs: vec![arc(1, 1, 1, 0.0)],
            },
        ];
        let model = SourceModel::new(symbols, states).expect("a model");
        let options = WriteOptions {
            threads: 1,
            source_name: "a-to-b".into(),
            ..WriteOptions::default()
        };
        hfst::dhfst_writer::write(&model, &options)
            .expect("the error model writes")
            .bytes
    }

    fn code(result: Result<Model, GlueError>) -> &'static str {
        match result {
            Ok(Model::Acceptor(_)) => "acceptor",
            Ok(Model::ErrorModel(_)) => "error-model",
            Err(error) => error.code(),
        }
    }

    // [spec:hfst:req:javascript.dhfst+1/test]
    #[test]
    fn an_acceptor_loads_and_looks_words_up() {
        let Ok(Model::Acceptor(acceptor)) = read(&acceptor()) else {
            panic!("the acceptor does not load as one");
        };
        assert_eq!(acceptor.version(), 1);
        assert_eq!(acceptor.symbols()[0], "@_EPSILON_SYMBOL_@");
        assert!(
            acceptor
                .meta()
                .is_some_and(|meta| meta.contains("\"talo\""))
        );
        assert_eq!(acceptor.lookup("talo"), Some(0.5));
        assert_eq!(acceptor.lookup("talon"), Some(1.5));
        assert_eq!(acceptor.lookup("tal"), None);
    }

    // [spec:hfst:req:javascript.dhfst+1/test]
    #[test]
    fn an_error_model_loads_with_its_metadata() {
        let Ok(Model::ErrorModel(model)) = read(&error_model()) else {
            panic!("the error model does not load as one");
        };
        assert_eq!(model.version(), 1);
        assert_eq!(model.state_count(), 2);
        assert_eq!(model.symbols(), ["@_EPSILON_SYMBOL_@", "a", "b"]);
        assert!(model.meta().is_some_and(|meta| meta.contains("\"a-to-b\"")));
    }

    // [spec:hfst:req:javascript.dhfst+1/test]
    // [spec:hfst:req:javascript.load-bytes+2/test]
    #[test]
    fn invalid_or_foreign_bytes_are_refused() {
        let acceptor = acceptor();
        let mut as_error_model = acceptor.clone();
        as_error_model[5] = 1;
        let mut unknown_type = acceptor.clone();
        unknown_type[5] = 9;
        let cases: [(Vec<u8>, &str); 9] = [
            (acceptor[..acceptor.len() - 9].to_vec(), INVALID),
            (acceptor[..6].to_vec(), INVALID),
            (as_error_model, INVALID),
            (unknown_type, INVALID),
            (error_model()[..40].to_vec(), INVALID),
            (Vec::new(), INVALID),
            (b"not a speller".to_vec(), INVALID),
            (b"##foma-net 1.0##\n".to_vec(), "unsupported-format"),
            (vec![0x1f, 0x8b, 0x08, 0, 0], "unsupported-format"),
        ];
        for (bytes, expected) in cases {
            assert_eq!(code(read(&bytes)), expected, "{bytes:?}");
        }
    }
}
