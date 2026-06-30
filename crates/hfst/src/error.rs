//! The HFST error type — the `Result`-based replacement for the C++ exception
//! hierarchy. Every former `Hfst*Exception` becomes a variant of [`ErrorKind`]
//! with the `Exception` suffix dropped; fallible functions return [`Result`]
//! and propagate with `?` instead of `panic_any` / `catch_unwind`.
//!
//! Construction is via [`Error::new`] / [`Error::with_message`] or the
//! [`bail!`]/[`err!`] macros. The optional `message` carries the text the old
//! `HFST_THROW_MESSAGE` attached at a throw site.

use thiserror::Error as ThisError;

use crate::hfst_data_types::ImplementationType;

/// The library's standard `Result`, carrying an [`Error`] on failure.
pub type Result<T> = core::result::Result<T, Error>;

/// An error raised by the HFST library: an [`ErrorKind`] plus the optional
/// contextual message a throw site supplied.
#[derive(Clone, Debug)]
pub struct Error {
    /// The error condition.
    pub kind: ErrorKind,
    /// Optional context (the old `HFST_THROW_MESSAGE` text), if any.
    pub message: Option<String>,
}

impl Error {
    /// An error with no extra context.
    pub fn new(kind: ErrorKind) -> Self {
        Error {
            kind,
            message: None,
        }
    }

    /// An error carrying a contextual message.
    pub fn with_message(kind: ErrorKind, message: impl Into<String>) -> Self {
        Error {
            kind,
            message: Some(message.into()),
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error::new(kind)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(message) = &self.message {
            write!(f, ": {message}")?;
        }
        Ok(())
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&self.kind)
    }
}

/// Every distinct error condition the library can raise — one variant per former
/// `Hfst*Exception`, with the `Exception` suffix dropped.
#[derive(Clone, Debug, PartialEq, ThisError)]
pub enum ErrorKind {
    #[error("transducer type mismatch")]
    HfstTransducerTypeMismatch,
    #[error("implementation type {0:?} is not available")]
    ImplementationTypeNotAvailable(ImplementationType),
    #[error("function not implemented")]
    FunctionNotImplemented,
    #[error("stream is not readable")]
    StreamNotReadable,
    #[error("stream cannot be written")]
    StreamCannotBeWritten,
    #[error("stream is closed")]
    StreamIsClosed,
    #[error("end of stream")]
    EndOfStream,
    #[error("transducer is cyclic")]
    TransducerIsCyclic,
    #[error("not a transducer stream")]
    NotTransducerStream,
    #[error("file is in gzip format")]
    FileIsInGzFormat,
    #[error("not valid AT&T format")]
    NotValidAttFormat,
    #[error("not valid Prolog format")]
    NotValidPrologFormat,
    #[error("not valid lexc format")]
    NotValidLexcFormat,
    #[error("state is not final")]
    StateIsNotFinal,
    #[error("context transducers are not automata")]
    ContextTransducersAreNotAutomata,
    #[error("transducers are not automata")]
    TransducersAreNotAutomata,
    #[error("transducer is not an automaton")]
    TransducerIsNotAutomaton,
    #[error("state index out of bounds")]
    StateIndexOutOfBounds,
    #[error("transducer header error")]
    TransducerHeader,
    #[error("missing OpenFst input symbol table")]
    MissingOpenFstInputSymbolTable,
    #[error("transducer type mismatch")]
    TransducerTypeMismatch,
    #[error("empty set of contexts")]
    EmptySetOfContexts,
    #[error("specified type required")]
    SpecifiedTypeRequired,
    #[error("fatal error")]
    Fatal,
    #[error("transducer has wrong type")]
    TransducerHasWrongType,
    #[error("incorrect UTF-8 coding")]
    IncorrectUtf8Coding,
    #[error("empty string")]
    EmptyString,
    #[error("symbol not found")]
    SymbolNotFound,
    #[error("metadata error")]
    Metadata,
    #[error("flag diacritics are not identities")]
    FlagDiacriticsAreNotIdentities,
}

/// Build an [`Error`] without returning it. `err!(Kind)`, `err!(Kind, message)`,
/// or `err!(Kind(data))`.
#[macro_export]
macro_rules! err {
    ($kind:ident ( $($arg:expr),* $(,)? )) => {
        $crate::error::Error::new($crate::error::ErrorKind::$kind($($arg),*))
    };
    ($kind:ident, $msg:expr) => {
        $crate::error::Error::with_message($crate::error::ErrorKind::$kind, $msg)
    };
    ($kind:ident) => {
        $crate::error::Error::new($crate::error::ErrorKind::$kind)
    };
}

/// `return Err(...)` an [`Error`]. Same argument forms as [`err!`].
#[macro_export]
macro_rules! bail {
    ($($tt:tt)*) => {
        return ::core::result::Result::Err($crate::err!($($tt)*))
    };
}
