//! The plain objects and arrays the glue hands the wrapper, built a property
//! at a time. Each has the shape `js/hfst/compile-glue.ts` declares.

use hfst_wasm_shared::set;
use wasm_bindgen::JsValue;

/// A plain object under construction.
pub struct Object(js_sys::Object);

impl Object {
    pub fn new() -> Object {
        Object(js_sys::Object::new())
    }

    pub fn with(self, key: &str, value: impl Into<JsValue>) -> Object {
        set(&self.0, key, &value.into());
        self
    }

    pub fn done(self) -> JsValue {
        self.0.into()
    }
}

pub fn array(items: impl IntoIterator<Item = JsValue>) -> JsValue {
    let array = js_sys::Array::new();
    for item in items {
        array.push(&item);
    }
    array.into()
}

pub fn strings<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> JsValue {
    let array = js_sys::Array::new();
    for item in items {
        array.push(&JsValue::from_str(item.as_ref()));
    }
    array.into()
}

/// An object whose properties are `entries`, each an own data property, so
/// a name such as `__proto__` is a key like any other.
pub fn record(entries: impl IntoIterator<Item = (String, JsValue)>) -> JsValue {
    let pairs = entries
        .into_iter()
        .map(|(key, value)| JsValue::from(js_sys::Array::of2(&JsValue::from_str(&key), &value)))
        .collect::<js_sys::Array>();
    js_sys::Object::from_entries(&pairs)
        .expect("an array of key and value pairs makes an object")
        .into()
}
