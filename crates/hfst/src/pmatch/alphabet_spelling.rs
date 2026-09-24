//! Classifying a pmatch symbol by its spelling.

use super::*;

impl PmatchAlphabet {
    // ---- static string predicates ----

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-end-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-end-tag-fn]
    pub fn is_end_tag(symbol: &str) -> bool {
        symbol.find("@PMATCH_ENDTAG_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-capture-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-capture-tag-fn]
    pub fn is_capture_tag(symbol: &str) -> bool {
        symbol.find("@PMATCH_CAPTURE_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-captured-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-captured-tag-fn]
    pub fn is_captured_tag(symbol: &str) -> bool {
        symbol.find("@PMATCH_CAPTURED_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-insertion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-insertion-fn]
    pub fn is_insertion(symbol: &str) -> bool {
        symbol.find("@I.") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
    pub fn is_guard(symbol: &str) -> bool {
        symbol.find("@PMATCH_GUARD_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-list-fn]
    pub fn is_list(symbol: &str) -> bool {
        (symbol.find("@L.") == Some(0) || symbol.find("@X.") == Some(0))
            && symbol.rfind('@') == Some(symbol.len() - 1)
            && symbol.len() > 4
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-underscored-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-underscored-list-fn]
    pub fn is_underscored_list(symbol: &str) -> bool {
        (symbol.find("@L.") == Some(0) || symbol.find("@X.") == Some(0))
            && symbol.rfind("_@") == Some(symbol.len() - 2)
            && symbol.len() > 5
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-counter-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-counter-fn]
    pub fn is_counter(symbol: &str) -> bool {
        symbol.find("@PMATCH_COUNTER_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
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
            (symbol.find("@PMATCH") == Some(0) && symbol.as_bytes()[symbol.len() - 1] == b'@')
                || Self::is_list(symbol)
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-printable-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-printable-fn]
    pub fn is_printable(symbol: &str) -> bool {
        if symbol.len() < 3 {
            return true;
        }
        symbol.find('@') != Some(0) || symbol.as_bytes()[symbol.len() - 1] != b'@'
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-global-flag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-global-flag-fn]
    pub fn is_global_flag(symbol: &str) -> bool {
        (symbol.find("@P.") == Some(0) || symbol.find("@C.") == Some(0))
            && symbol.find("PMATCH_GLOBAL_") == Some(3)
            && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.name-from-insertion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.name-from-insertion-fn]
    pub fn name_from_insertion(symbol: &str) -> String {
        // C++ symbol.substr(sizeof("@I.") - 1, symbol.size() - (sizeof("@I.@") - 1)):
        // drop the leading "@I." (3 bytes) and the trailing "@", so "@I.Animal@"
        // yields "Animal". 'sizeof("@I.@")' is 5 in C (counts the NUL), so the
        // count is 'len - 4'; the earlier port mistranslated it as '"@I.@".len()
        // - 1' (== 3) and left the trailing "@" on the name, so RTN members never
        // matched their insertion symbol. [upstream hfst/hfst#354]
        symbol[("@I.".len())..(symbol.len() - 1)].to_string()
    }
}
