//! Format names as the tools' `--format` options read them, the description
//! the tools print for each type, and weights as the tools read them from
//! text.

use crate::hfst_data_types::ImplementationType;

/// A format name read from text: the implementation type it names, and for a
/// name that fits more than one type, the one guessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatName {
    pub implementation: ImplementationType,
    /// The full name of the guessed format when the name was ambiguous.
    pub guessed: Option<&'static str>,
}

impl FormatName {
    fn exact(implementation: ImplementationType) -> Option<Self> {
        Some(Self {
            implementation,
            guessed: None,
        })
    }

    fn guess(implementation: ImplementationType, guessed: &'static str) -> Option<Self> {
        Some(Self {
            implementation,
            guessed: Some(guessed),
        })
    }

    /// The warning a tool prints for `name` when it was ambiguous.
    pub fn warning(&self, name: &str) -> Option<String> {
        self.guessed
            .map(|guess| format!("Ambiguous format name {}, guessing {}", name, guess))
    }
}

// [spec:hfst:def:hfst-commandline.hfst-parse-format-name-fn]
// [spec:hfst:sem:hfst-commandline.hfst-parse-format-name-fn]
// [spec:hfst:req:embed.tools-in-library]
/// The format `name` names, case-insensitively, or `None` for a name that
/// names none.
pub fn parse_format_name(name: &str) -> Option<FormatName> {
    use ImplementationType::*;
    match name.to_ascii_lowercase().as_str() {
        "sfst" => FormatName::exact(SFST_TYPE),
        "openfst-tropical" | "ofst-tropical" => FormatName::exact(TROPICAL_OPENFST_TYPE),
        "openfst" | "ofst" => FormatName::guess(TROPICAL_OPENFST_TYPE, "openfst-tropical"),
        "foma" => FormatName::exact(FOMA_TYPE),
        "xfsm" => FormatName::exact(XFSM_TYPE),
        "optimized-lookup-unweighted" | "olu" => FormatName::exact(HFST_OL_TYPE),
        "optimized-lookup-weighted" | "olw" => FormatName::exact(HFST_OLW_TYPE),
        "thfst" => FormatName::exact(THFST_TYPE),
        "optimized-lookup" | "ol" => FormatName::guess(HFST_OLW_TYPE, "optimized-lookup-weighted"),
        _ => None,
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strformat-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strformat-fn]
// [spec:hfst:req:embed.tools-in-library]
pub fn format_description(format: ImplementationType) -> &'static str {
    match format {
        ImplementationType::SFST_TYPE => "SFST (1.4 compatible)",
        ImplementationType::TROPICAL_OPENFST_TYPE => "OpenFST, std arc, tropical semiring",
        ImplementationType::FOMA_TYPE => "foma",
        ImplementationType::XFSM_TYPE => "xfsm",
        ImplementationType::HFST_OL_TYPE => "Hfst's lookup optimized, unweighted",
        ImplementationType::HFST_OLW_TYPE => "Hfst's lookup optimized, weighted",
        ImplementationType::THFST_TYPE => {
            "thfst (divvunspell optimized-lookup, weighted, directory format)"
        }
        ImplementationType::HFST2_TYPE => "Hfst 2 legacy (deprecated)",
        ImplementationType::ERROR_TYPE | ImplementationType::UNSPECIFIED_TYPE => {
            "ERROR (not a HFST supported transducer)"
        }
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strtoweight-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtoweight-fn]
// [spec:hfst:req:embed.tools-in-library]
/// The weight `text` spells, read as the C++ tools' `strtod` read it: leading
/// whitespace is skipped, the empty string is 0, and anything left after the
/// number is not a weight. `Err` carries the text after the skipped
/// whitespace, which is what the tools name when they refuse it.
pub fn parse_weight(text: &str) -> Result<f32, &str> {
    // An empty field means "no weight", not an error: Giella's speller rules
    // pipe through `cut -f1-2`, so a stray second tab hands over exactly this.
    if text.is_empty() {
        return Ok(0.0);
    }
    let text = text.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    text.parse::<f64>().map(|w| w as f32).map_err(|_| text)
}
