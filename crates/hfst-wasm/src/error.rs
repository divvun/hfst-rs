//! Errors on their way to JavaScript.

use hfst::error::{Error, ErrorKind};
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
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        GlueError {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
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
        object.into()
    }
}

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(object, &JsValue::from_str(key), value)
        .expect("a fresh plain object takes any property");
}

/// The wrapper's code for a library error. `js/hfst/errors.ts` documents
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
    }

    #[test]
    fn the_wrapper_documents_every_code() {
        let errors_ts = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/errors.ts"
        ))
        .expect("js/hfst/errors.ts is in the repository");
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
        ];
        for code in codes {
            assert!(
                errors_ts.contains(&format!("\"{code}\"")),
                "js/hfst/errors.ts lists {code}"
            );
        }
    }
}
