//! DHFST files, divvunspell's speller formats, loaded from bytes: an acceptor
//! to look words up in, or an error model to describe.

use hfst_embed::dhfst::{AcceptorInfo, AcceptorReader, Dhfst, ErrorModel};
use wasm_bindgen::prelude::*;

use crate::error::GlueError;

/// A DHFST acceptor, type 2, read and validated as divvunspell reads one.
/// The wrapper's `DhfstAcceptor` class in `js/hfst/dhfst.ts` owns one.
#[wasm_bindgen]
pub struct DhfstAcceptor {
    reader: Box<AcceptorReader<'static>>,
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

    // [spec:hfst:req:javascript.dhfst+2]
    /// The least weight of a path that accepts `word`, as divvunspell's
    /// speller reads the word, or nothing when no path does.
    pub fn lookup(&self, word: &str) -> Option<f32> {
        self.reader.lookup(word)
    }
}

/// A DHFST error model, type 1: what its header and sections say.
#[wasm_bindgen]
pub struct DhfstErrorModel {
    model: ErrorModel,
}

#[wasm_bindgen]
impl DhfstErrorModel {
    pub fn version(&self) -> u8 {
        self.model.version
    }

    #[wasm_bindgen(js_name = stateCount)]
    pub fn state_count(&self) -> u32 {
        self.model.state_count
    }

    pub fn symbols(&self) -> Vec<String> {
        self.model.symbols.clone()
    }

    pub fn meta(&self) -> Option<String> {
        self.model.meta.clone()
    }
}

// [spec:hfst:req:javascript.dhfst+2]
// [spec:hfst:req:javascript.load-bytes+2]
/// The DHFST file in `bytes`, as a plain object: `type`, `acceptor` or
/// `error-model`, and `model`, the object of that type.
#[wasm_bindgen(js_name = readDhfst)]
pub fn read_dhfst(bytes: &[u8]) -> Result<JsValue, GlueError> {
    let (kind, model): (&str, JsValue) =
        match hfst_embed::dhfst::read_dhfst(bytes, &crate::PACKAGE)? {
            Dhfst::Acceptor(reader) => ("acceptor", DhfstAcceptor { reader }.into()),
            Dhfst::ErrorModel(model) => ("error-model", DhfstErrorModel { model }.into()),
        };
    let object = js_sys::Object::new();
    set(&object, "type", &JsValue::from_str(kind));
    set(&object, "model", &model);
    Ok(object.into())
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
