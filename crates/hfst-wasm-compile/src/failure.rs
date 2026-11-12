//! A call into the compile module that failed, on its way to JavaScript: an
//! error the library or the glue returned, a compile that failed with the
//! diagnostics it made, or what the caller's files function threw.

use hfst_embed::compile::{CompileError, Diagnostic};
use hfst_wasm_shared::{GlueError, set};
use wasm_bindgen::JsValue;

use crate::diagnostics::diagnostics;

/// The code of a compile that failed, which only this module sends.
pub const COMPILE_FAILED: &str = "compile-failed";

#[derive(Debug)]
pub enum Failure {
    /// An error with one of the codes hfst-embed names.
    Glue(GlueError),
    /// A failure carrying the diagnostics of the compile it came from:
    /// `compile-failed` for a compile that failed, or another code for a
    /// compile whose result the call refused.
    Compile {
        code: &'static str,
        message: String,
        diagnostics: Vec<Diagnostic>,
    },
    /// What the caller's files function threw, rethrown as it was.
    Thrown(JsValue),
}

impl Failure {
    // [spec:hfst:req:javascript.compile-errors]
    /// The `compile-failed` error of a compile that failed, carrying every
    /// diagnostic it made.
    pub fn failed(failed: CompileError) -> Failure {
        Failure::Compile {
            code: COMPILE_FAILED,
            message: failed.error.into_message(),
            diagnostics: failed.diagnostics,
        }
    }

    /// The error `error` of a compile that made `diagnostics` before the
    /// call refused what it made.
    pub fn refused(error: hfst_embed::Error, diagnostics: Vec<Diagnostic>) -> Failure {
        Failure::Compile {
            code: error.code().name(),
            message: error.into_message(),
            diagnostics,
        }
    }
}

impl From<GlueError> for Failure {
    fn from(error: GlueError) -> Failure {
        Failure::Glue(error)
    }
}

impl From<hfst_embed::Error> for Failure {
    fn from(error: hfst_embed::Error) -> Failure {
        Failure::Glue(error.into())
    }
}

impl From<Failure> for JsValue {
    fn from(failure: Failure) -> JsValue {
        match failure {
            Failure::Glue(error) => error.into(),
            Failure::Thrown(thrown) => thrown,
            Failure::Compile {
                code,
                message,
                diagnostics: made,
            } => {
                let object = js_sys::Object::new();
                set(&object, "code", &JsValue::from_str(code));
                set(&object, "message", &JsValue::from_str(&message));
                set(&object, "diagnostics", &diagnostics(&made));
                object.into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrapper_documents_compile_failed() {
        let errors_ts = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/errors.ts"
        ))
        .expect("js/hfst/errors.ts is in the repository");
        assert!(errors_ts.contains(&format!("\"{COMPILE_FAILED}\"")));
    }
}
