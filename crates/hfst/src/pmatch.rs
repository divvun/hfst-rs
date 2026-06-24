//! Partial port of `libhfst/src/implementations/optimized-lookup/pmatch.{h,cc}`
//! (namespace `hfst_ol`).
//!
//! Only `PmatchAlphabet`'s special-symbol predicates are ported so far, because
//! `HarmonizeUnknownAndIdentitySymbols::remove_flags` depends on
//! `PmatchAlphabet::is_special`. The rest of pmatch belongs to the
//! optimized-lookup layer and is ported there.

/// `class PmatchAlphabet` — here a unit struct exposing only the static special
/// symbol predicates; instance state arrives with the full OL port.
pub struct PmatchAlphabet;

impl PmatchAlphabet {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-insertion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-insertion-fn]
    pub fn is_insertion(symbol: &str) -> bool {
        symbol.starts_with("@I.") && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-list-fn]
    pub fn is_list(symbol: &str) -> bool {
        (symbol.starts_with("@L.") || symbol.starts_with("@X."))
            && symbol.rfind('@') == Some(symbol.len() - 1)
            && symbol.len() > 4
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-special-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-special-fn]
    pub fn is_special(symbol: &str) -> bool {
        if symbol.len() < 3 {
            return false;
        }
        if symbol == "@PMATCH_INPUT_MARK@" || symbol == "@PMATCH_BACKTRACK@" {
            // is_special symbols can't be referred to in pmatch scripts
            return false;
        }
        if Self::is_insertion(symbol)
            || symbol == "@BOUNDARY@"
            || symbol == "@UNICODE_ALPHA@"
            || symbol == "@UNICODE_UPPERALPHA@"
            || symbol == "@UNICODE_LOWERALPHA@"
            || symbol == "@UNICODE_WHITESPACE@"
        {
            true
        } else {
            (symbol.starts_with("@PMATCH") && symbol.as_bytes()[symbol.len() - 1] == b'@')
                || Self::is_list(symbol)
        }
    }
}
