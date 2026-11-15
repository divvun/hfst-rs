//! The error every call of the glue throws, which the Swift layer turns into
//! `HFSTError`, and the diagnostics a compile's error carries.

// [spec:hfst:req:swift.errors]
/// A failure the library reported, with the code `@divvun/hfst` gives it,
/// the library's message and, from a compile, every diagnostic it made.
#[derive(Debug, uniffi::Error)]
pub enum GlueError {
    Failed {
        code: String,
        message: String,
        diagnostics: Vec<Diagnostic>,
    },
}

impl std::fmt::Display for GlueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GlueError::Failed { message, .. } => f.write_str(message),
        }
    }
}

impl From<hfst_embed::Error> for GlueError {
    fn from(error: hfst_embed::Error) -> Self {
        GlueError::Failed {
            code: error.code().name().to_owned(),
            message: error.into_message(),
            diagnostics: Vec::new(),
        }
    }
}

pub fn invalid(message: impl Into<String>) -> GlueError {
    hfst_embed::Error::new(hfst_embed::Code::InvalidArgument, message).into()
}

/// A span of a source in UTF-8 bytes, end exclusive.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Span {
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct DiagnosticLabel {
    pub span: Span,
    pub message: String,
}

// [spec:hfst:req:swift.errors]
/// Something a compile said about its source. `source` is the position,
/// among the sources the caller gave the compile, of the one it is about,
/// so the Swift layer can make its spans ranges of the caller's string;
/// `None` for a file the compile read.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub source_name: String,
    pub source: Option<u32>,
    pub span: Option<Span>,
    pub labels: Vec<DiagnosticLabel>,
    pub notes: Vec<String>,
    pub help: Option<String>,
    pub rendered: String,
}
