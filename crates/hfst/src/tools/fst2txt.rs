//! `hfst fst2txt`: a transducer written as AT&T, dot, Prolog or PC-KIMMO
//! text.

use std::io::Write;

use crate::backend::Backend;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_print_dot::print_dot_file;
use crate::hfst_print_pckimmo::print_pckimmo;
use crate::hfst_transducer::{AnyTransducer, HfstTransducer};

/// The text `hfst fst2txt` writes, as `--format` names it.
// [spec:hfst:def:hfst-fst2txt.fst-text-format]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextFormat {
    /// AT&T, the tab-separated format OpenFst also reads.
    #[default]
    Att,
    /// Graphviz.
    Dot,
    Pckimmo,
    Prolog,
}

impl TextFormat {
    // [spec:hfst:def:hfst-fst2txt.fst-text-format]
    // [spec:hfst:req:embed.tools-in-library]
    /// The format `--format NAME` chooses, or `None` for a name the tool
    /// refuses.
    pub fn from_name(name: &str) -> Option<TextFormat> {
        match name {
            "att" | "AT&T" | "openfst" | "OpenFst" => Some(TextFormat::Att),
            "dot" | "graphviz" | "GraphViz" => Some(TextFormat::Dot),
            "pckimmo" => Some(TextFormat::Pckimmo),
            "prolog" | "Prolog" => Some(TextFormat::Prolog),
            _ => None,
        }
    }
}

/// `hfst fst2txt`'s options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextOptions {
    pub format: TextFormat,
    /// `--print-weights`: weights in every type, foma's zeros included.
    pub print_weights: bool,
    /// `--do-not-print-weights`, which `print_weights` overrides.
    pub do_not_print_weights: bool,
    /// AT&T with symbol numbers rather than names: `--use-numbers`.
    pub use_numbers: bool,
}

impl TextOptions {
    /// Whether a transducer of type `ty` is written with its weights: by
    /// default every weighted type is, and foma, which has none, is not.
    pub fn weights_of(&self, ty: ImplementationType) -> bool {
        self.print_weights
            || !(self.do_not_print_weights
                || ty == ImplementationType::SFST_TYPE
                || ty == ImplementationType::FOMA_TYPE)
    }
}

/// What the tool tells its standard error when it names a Prolog network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextNote {
    /// The transducer had no name and is written as this one.
    Named(String),
    /// The transducer's name is replaced by this one.
    Renamed(String),
}

impl std::fmt::Display for TextNote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextNote::Named(name) => {
                write!(f, "Transducer has no name, giving it a name '{name}'...")
            }
            TextNote::Renamed(name) => write!(f, "Renaming transducer into '{name}'..."),
        }
    }
}

/// Why a transducer was not written as text.
#[derive(Debug)]
pub enum TextError {
    Prolog(crate::error::Error),
    Write(std::io::Error),
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextError::Prolog(e) => {
                write!(f, "Error encountered when writing in prolog format: {e}")
            }
            TextError::Write(e) => {
                write!(f, "Error encountered when writing in text format: {e}")
            }
        }
    }
}

impl std::error::Error for TextError {}

// [spec:hfst:sem:hfst-fst2txt.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
/// Write `t`, the `n`th transducer of its stream counting from 1, as
/// `hfst fst2txt` writes it: after a `--` line unless it is the first, and
/// in Prolog under the name `NO_NAME_n`, which `note` hears of.
pub fn write_text(
    t: &AnyTransducer,
    n: usize,
    options: &TextOptions,
    out: &mut dyn Write,
    note: &mut dyn FnMut(TextNote),
) -> Result<(), TextError> {
    match t {
        AnyTransducer::Tropical(t) => write_typed(t, n, options, out, note),
        AnyTransducer::OlW(t) => write_typed(t, n, options, out, note),
        AnyTransducer::OlU(t) => write_typed(t, n, options, out, note),
        AnyTransducer::Thfst(t) => write_typed(t, n, options, out, note),
        #[cfg(feature = "foma")]
        AnyTransducer::Foma(t) => write_typed(t, n, options, out, note),
    }
}

fn write_typed<B: Backend>(
    t: &HfstTransducer<B>,
    n: usize,
    options: &TextOptions,
    out: &mut dyn Write,
    note: &mut dyn FnMut(TextNote),
) -> Result<(), TextError> {
    if n > 1 {
        out.write_all(b"--\n").map_err(TextError::Write)?;
    }
    let weights = options.weights_of(t.get_type());
    let written = match options.format {
        TextFormat::Att if options.use_numbers => t.write_in_att_format_number(out, weights),
        TextFormat::Att => t.write_in_att_format_file(out, weights),
        TextFormat::Dot => out
            .write_all(b"// This graph generated with hfst-fst2txt\n")
            .and_then(|()| print_dot_file(out, t)),
        TextFormat::Pckimmo => print_pckimmo(out, t),
        TextFormat::Prolog => {
            let name = format!("NO_NAME_{n}");
            note(if t.get_name().is_empty() {
                TextNote::Named(name.clone())
            } else {
                TextNote::Renamed(name.clone())
            });
            return t
                .write_in_prolog_format(out, &name, weights)
                .map_err(TextError::Prolog);
        }
    };
    written.map_err(TextError::Write)
}
