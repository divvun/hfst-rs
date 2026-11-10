//! The failures both packages report, each with a code callers can test.

use std::fmt;

use hfst::error::ErrorKind;

/// What went wrong, as a code that does not change between releases. Each
/// code's name is the string both packages give it, and `js/hfst/errors.ts`
/// says what each means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    TypeMismatch,
    UnsupportedFormat,
    InvalidFormat,
    Io,
    NotAutomaton,
    Cyclic,
    InvalidArgument,
    Internal,
    Failed,
    /// A reader that takes one transducer got bytes holding more.
    TransducerCount,
}

impl Code {
    pub const ALL: [Code; 10] = [
        Code::TypeMismatch,
        Code::UnsupportedFormat,
        Code::InvalidFormat,
        Code::Io,
        Code::NotAutomaton,
        Code::Cyclic,
        Code::InvalidArgument,
        Code::Internal,
        Code::Failed,
        Code::TransducerCount,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Code::TypeMismatch => "type-mismatch",
            Code::UnsupportedFormat => "unsupported-format",
            Code::InvalidFormat => "invalid-format",
            Code::Io => "io",
            Code::NotAutomaton => "not-automaton",
            Code::Cyclic => "cyclic",
            Code::InvalidArgument => "invalid-argument",
            Code::Internal => "internal",
            Code::Failed => "failed",
            Code::TransducerCount => "transducer-count",
        }
    }
}

// [spec:hfst:req:javascript.errors+1]
// [spec:hfst:req:python.errors]
/// A failure, with its code and the message the caller reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    code: Code,
    message: String,
}

impl Error {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Error {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> Code {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_message(self) -> String {
        self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<hfst::error::Error> for Error {
    fn from(error: hfst::error::Error) -> Self {
        Error::new(code_of(&error.kind), error.to_string())
    }
}

/// The code for a library error. Every kind is named, so a kind the library
/// adds cannot fall through to a wrong code.
pub fn code_of(kind: &ErrorKind) -> Code {
    match kind {
        ErrorKind::HfstTransducerTypeMismatch
        | ErrorKind::TransducerTypeMismatch
        | ErrorKind::TransducerHasWrongType => Code::TypeMismatch,
        ErrorKind::ImplementationTypeNotAvailable(_) | ErrorKind::FileIsInGzFormat => {
            Code::UnsupportedFormat
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
        | ErrorKind::InvalidModelFile => Code::InvalidFormat,
        ErrorKind::StreamNotReadable
        | ErrorKind::StreamCannotBeWritten
        | ErrorKind::StreamIsClosed => Code::Io,
        ErrorKind::ContextTransducersAreNotAutomata
        | ErrorKind::TransducersAreNotAutomata
        | ErrorKind::TransducerIsNotAutomaton => Code::NotAutomaton,
        ErrorKind::TransducerIsCyclic => Code::Cyclic,
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
        | ErrorKind::EmptyMulticharSymbol => Code::InvalidArgument,
        ErrorKind::Fatal | ErrorKind::StateNotDefined => Code::Internal,
        ErrorKind::Hfst => Code::Failed,
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
            assert_eq!(code_of(&kind).name(), code, "{kind:?}");
        }
    }

    #[test]
    fn the_message_is_the_library_message() {
        let error = Error::from(hfst::error::Error::with_message(
            ErrorKind::Hfst,
            "no such state",
        ));
        assert_eq!(error.code(), Code::Failed);
        assert_eq!(error.message(), "HFST error: no such state");
    }

    #[test]
    fn every_code_has_its_own_name() {
        let mut names: Vec<&str> = Code::ALL.iter().map(|code| code.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Code::ALL.len());
    }
}
