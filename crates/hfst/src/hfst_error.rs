//! The HFST error type — the `Result`-based replacement for the C++ exception
//! hierarchy. Every former `Hfst*Exception` becomes a variant of [`HfstErrorKind`]
//! with the `Exception` suffix dropped; fallible functions return [`HfstResult`]
//! and propagate with `?` instead of `panic_any` / `catch_unwind`.
//!
//! Construction is via [`HfstError::new`] / [`HfstError::with_message`] or the
//! [`hfst_bail!`]/[`hfst_err!`] macros. The optional `message` carries the text
//! the old `HFST_THROW_MESSAGE` attached at a throw site.

use thiserror::Error;

use crate::hfst_data_types::ImplementationType;

/// The library's standard `Result`, carrying an [`HfstError`] on failure.
pub type HfstResult<T> = Result<T, HfstError>;

/// An error raised by the HFST library: a [`HfstErrorKind`] plus the optional
/// contextual message a throw site supplied.
#[derive(Clone, Debug)]
pub struct HfstError {
    /// The error condition.
    pub kind: HfstErrorKind,
    /// Optional context (the old `HFST_THROW_MESSAGE` text), if any.
    pub message: Option<String>,
}

impl HfstError {
    /// An error with no extra context.
    pub fn new(kind: HfstErrorKind) -> Self {
        HfstError {
            kind,
            message: None,
        }
    }

    /// An error carrying a contextual message.
    pub fn with_message(kind: HfstErrorKind, message: impl Into<String>) -> Self {
        HfstError {
            kind,
            message: Some(message.into()),
        }
    }
}

impl From<HfstErrorKind> for HfstError {
    fn from(kind: HfstErrorKind) -> Self {
        HfstError::new(kind)
    }
}

impl std::fmt::Display for HfstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(message) = &self.message {
            write!(f, ": {message}")?;
        }
        Ok(())
    }
}

impl std::error::Error for HfstError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}

/// Every distinct error condition the library can raise — one variant per former
/// `Hfst*Exception`, with the `Exception` suffix dropped.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum HfstErrorKind {
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

/// Build an [`HfstError`] without returning it. `hfst_err!(Kind)`,
/// `hfst_err!(Kind, message)`, or `hfst_err!(Kind(data))`.
#[macro_export]
macro_rules! hfst_err {
    ($kind:ident ( $($arg:expr),* $(,)? )) => {
        $crate::hfst_error::HfstError::new($crate::hfst_error::HfstErrorKind::$kind($($arg),*))
    };
    ($kind:ident, $msg:expr) => {
        $crate::hfst_error::HfstError::with_message($crate::hfst_error::HfstErrorKind::$kind, $msg)
    };
    ($kind:ident) => {
        $crate::hfst_error::HfstError::new($crate::hfst_error::HfstErrorKind::$kind)
    };
}

/// `return Err(...)` an [`HfstError`]. Same argument forms as [`hfst_err!`].
#[macro_export]
macro_rules! hfst_bail {
    ($($tt:tt)*) => {
        return ::core::result::Result::Err($crate::hfst_err!($($tt)*))
    };
}
