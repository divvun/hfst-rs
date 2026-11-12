//! Errors on their way to JavaScript.

use hfst_embed::Code;
use wasm_bindgen::JsValue;

// [spec:hfst:req:javascript.errors+1]
/// An error for the wrapper: a stable code and the message. Every export that
/// can fail returns it, and it reaches JavaScript as a plain object with
/// `code` and `message`, which the wrapper rethrows as an `HfstError`.
#[derive(Debug)]
pub struct GlueError {
    code: &'static str,
    message: String,
}

impl GlueError {
    /// An error with one of the codes `js/hfst/errors.ts` lists.
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        GlueError {
            code: code.name(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<hfst_embed::Error> for GlueError {
    fn from(error: hfst_embed::Error) -> Self {
        GlueError::new(error.code(), error.into_message())
    }
}

impl From<hfst::error::Error> for GlueError {
    fn from(error: hfst::error::Error) -> Self {
        hfst_embed::Error::from(error).into()
    }
}

impl From<GlueError> for JsValue {
    fn from(error: GlueError) -> Self {
        let object = js_sys::Object::new();
        set(&object, "code", &JsValue::from_str(error.code));
        set(&object, "message", &JsValue::from_str(&error.message));
        object.into()
    }
}

pub fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(object, &JsValue::from_str(key), value)
        .expect("a fresh plain object takes any property");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrapper_documents_every_code() {
        let errors_ts = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/errors.ts"
        ))
        .expect("js/hfst/errors.ts is in the repository");
        for code in Code::ALL {
            assert!(
                errors_ts.contains(&format!("\"{}\"", code.name())),
                "js/hfst/errors.ts lists {}",
                code.name()
            );
        }
    }

    #[test]
    fn a_library_error_keeps_its_code_and_message() {
        let error = GlueError::from(hfst::error::Error::with_message(
            hfst::error::ErrorKind::Hfst,
            "no such state",
        ));
        assert_eq!(error.code, "failed");
        assert_eq!(error.message, "HFST error: no such state");
    }
}
