//! Errors on their way to JavaScript.

use hfst::error::{Error, ErrorKind};
use wasm_bindgen::JsValue;

// [spec:hfst:req:javascript.errors]
/// An error for the wrapper: a stable code, the message, and for a failed
/// compile its diagnostics. Every export that can fail returns it, and it
/// reaches JavaScript as a plain object with `code`, `message` and, for a
/// compile, `diagnostics`. The wrapper rethrows that object as an `HfstError`,
/// or as an `HfstCompileError` when it has diagnostics.
#[derive(Debug)]
pub struct GlueError {
    code: &'static str,
    message: String,
    diagnostics: Option<Vec<JsValue>>,
}

impl GlueError {
    /// An error with one of the codes `js/shared/errors.ts` lists.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        GlueError {
            code,
            message: message.into(),
            diagnostics: None,
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    /// A failed compile, carrying every diagnostic the compiler produced.
    pub fn compile(message: impl Into<String>, diagnostics: Vec<JsValue>) -> Self {
        GlueError {
            code: "compile-error",
            message: message.into(),
            diagnostics: Some(diagnostics),
        }
    }
}

impl From<Error> for GlueError {
    fn from(error: Error) -> Self {
        GlueError::new(code_of(&error.kind), error.to_string())
    }
}

impl From<GlueError> for JsValue {
    fn from(error: GlueError) -> Self {
        let object = js_sys::Object::new();
        set(&object, "code", &JsValue::from_str(error.code));
        set(&object, "message", &JsValue::from_str(&error.message));
        if let Some(diagnostics) = error.diagnostics {
            let diagnostics: js_sys::Array = diagnostics.into_iter().collect();
            set(&object, "diagnostics", &diagnostics);
        }
        object.into()
    }
}

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(object, &JsValue::from_str(key), value)
        .expect("a fresh plain object takes any property");
}

/// The wrapper's code for a library error. `js/shared/errors.ts` documents
/// each code beside `HfstErrorCode`, and a test here checks that it does.
pub fn code_of(kind: &ErrorKind) -> &'static str {
    match kind {
        ErrorKind::HfstTransducerTypeMismatch
        | ErrorKind::TransducerTypeMismatch
        | ErrorKind::TransducerHasWrongType => "type-mismatch",
        ErrorKind::ImplementationTypeNotAvailable(_) | ErrorKind::FileIsInGzFormat => {
            "unsupported-format"
        }
        ErrorKind::NotTransducerStream
        | ErrorKind::EmptyTransducerStream
        | ErrorKind::EndOfStream
        | ErrorKind::TransducerHeader
        | ErrorKind::NotValidAttFormat
        | ErrorKind::NotValidPrologFormat
        | ErrorKind::NotValidLexcFormat
        | ErrorKind::MissingOpenFstInputSymbolTable
        | ErrorKind::Metadata
        | ErrorKind::InvalidModelFile => "invalid-format",
        ErrorKind::StreamNotReadable
        | ErrorKind::StreamCannotBeWritten
        | ErrorKind::StreamIsClosed => "io",
        ErrorKind::ContextTransducersAreNotAutomata
        | ErrorKind::TransducersAreNotAutomata
        | ErrorKind::TransducerIsNotAutomaton => "not-automaton",
        ErrorKind::TransducerIsCyclic => "cyclic",
        ErrorKind::StateIsNotFinal
        | ErrorKind::StateIndexOutOfBounds
        | ErrorKind::EmptySetOfContexts
        | ErrorKind::SpecifiedTypeRequired
        | ErrorKind::IncorrectUtf8Coding
        | ErrorKind::EmptyString
        | ErrorKind::SymbolNotFound
        | ErrorKind::FlagDiacriticsAreNotIdentities
        | ErrorKind::AlphabetTranslation
        | ErrorKind::EmptySymbolPairSet
        | ErrorKind::UndefinedSymbolPairsFound
        | ErrorKind::UnequalSetSize
        | ErrorKind::FaultyStringInput
        | ErrorKind::UnescapedColsFound
        | ErrorKind::EmptyMulticharSymbol => "invalid-argument",
        ErrorKind::Fatal | ErrorKind::StateNotDefined => "internal",
        ErrorKind::Hfst => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::hfst_data_types::ImplementationType;

    #[test]
    fn library_errors_map_to_their_codes() {
        let cases = [
            (ErrorKind::TransducerTypeMismatch, "type-mismatch"),
            (
                ErrorKind::ImplementationTypeNotAvailable(ImplementationType::FOMA_TYPE),
                "unsupported-format",
            ),
            (ErrorKind::NotTransducerStream, "invalid-format"),
            (ErrorKind::StreamNotReadable, "io"),
            (ErrorKind::TransducerIsNotAutomaton, "not-automaton"),
            (ErrorKind::TransducerIsCyclic, "cyclic"),
            (ErrorKind::SymbolNotFound, "invalid-argument"),
            (ErrorKind::Fatal, "internal"),
            (ErrorKind::Hfst, "failed"),
        ];
        for (kind, code) in cases {
            assert_eq!(code_of(&kind), code, "{kind:?}");
        }
    }

    #[test]
    fn the_message_is_the_library_message() {
        let error = GlueError::from(Error::with_message(ErrorKind::Hfst, "no such state"));
        assert_eq!(error.code, "failed");
        assert_eq!(error.message, "HFST error: no such state");
        assert!(error.diagnostics.is_none());
    }

    #[test]
    fn the_wrapper_documents_every_code() {
        let errors_ts = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/shared/errors.ts"
        ))
        .expect("js/shared/errors.ts is in the repository");
        let codes = [
            "type-mismatch",
            "unsupported-format",
            "invalid-format",
            "io",
            "not-automaton",
            "cyclic",
            "invalid-argument",
            "internal",
            "failed",
            "compile-error",
        ];
        for code in codes {
            assert!(
                errors_ts.contains(&format!("\"{code}\"")),
                "js/shared/errors.ts lists {code}"
            );
        }
        assert_eq!(GlueError::compile("x", Vec::new()).code, "compile-error");
    }
}
