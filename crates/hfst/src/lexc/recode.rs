//! The lexc-utils symbol recoders: joiner, flag, regex and definition
//! markers, and the percent and zero escapes.

use super::*;

// ==========================================================================
// lexc-utils.cc — module-scope constants (ported from lexc-utils.h).
// Declared here (the lexc-utils helper body) rather than in the skeleton to
// avoid duplicate-definition collisions.
// ==========================================================================

const LEXC_JOINER_START: &str = "$_LEXC_JOINER.";
const LEXC_JOINER_END: &str = "_$";
const LEXC_FLAG_LEFT_START: &str = "$R.LEXNAME.";
const LEXC_FLAG_RIGHT_START: &str = "$P.LEXNAME.";
const LEXC_FLAG_END: &str = "$";
const LEXC_DFN_START: &str = "@_LEXC_DEFINITION.";
const LEXC_DFN_END: &str = "_@";
const REG_EX_START: &str = "$_REG.";
const REG_EX_END: &str = "_$";

// ==========================================================================
// lexc-utils.cc — RECODE LEXC STYLE free helpers.
//
// The C++ encoders mutate-and-return a 'std::string&'; here they are pure
// 'fn(&str, …) -> String'. These module-scope helpers are shared with the
// 'compileLexical' body (which calls 'joiner_encode' / 'flag_joiner_encode' /
// 'should_colourise'); they are owned by this registration/lexc-utils body.
// ==========================================================================

/// Port of 'hfst::lexc::stripPercents'.
fn strip_percents_str(s: &str) -> String {
    let stripped = s.replace("%%", "@PERCENT@");
    let stripped = stripped.replace('%', "");
    stripped.replace("@PERCENT@", "%")
}

/// Port of 'hfst::lexc::addPercents'.
fn add_percents(s: &str) -> String {
    let added = s.replace('%', "%%");
    let added = added.replace('<', "%<");
    added.replace('>', "%>")
}

/// Port of 'hfst::lexc::flagJoinerEncode'.
pub(super) fn flag_joiner_encode(s: &str, left: bool) -> String {
    if left {
        format!("{}{}{}", LEXC_FLAG_LEFT_START, s, LEXC_FLAG_END)
    } else {
        format!("{}{}{}", LEXC_FLAG_RIGHT_START, s, LEXC_FLAG_END)
    }
}

/// Port of 'hfst::lexc::joinerEncode'.
pub(super) fn joiner_encode(s: &str) -> String {
    format!("{}{}{}", LEXC_JOINER_START, s, LEXC_JOINER_END)
}

/// Port of 'hfst::lexc::joinerDecode'.
fn joiner_decode(s: &str) -> String {
    let j_start = LEXC_JOINER_START.len();
    let j_end = LEXC_JOINER_END.len();
    s[j_start..s.len() - j_end].to_string()
}

/// Port of 'hfst::lexc::regExpresionEncode' (suffix is 'LEXC_JOINER_END', as in
/// the C++).
pub(super) fn reg_expresion_encode(s: &str) -> String {
    format!("{}{}{}", REG_EX_START, s, LEXC_JOINER_END)
}

/// Port of 'hfst::lexc::regExpresionDecode'.
fn reg_expresion_decode(s: &str) -> String {
    let j_start = REG_EX_START.len();
    let j_end = LEXC_JOINER_END.len();
    s[j_start..s.len() - j_end].to_string()
}

/// Port of 'hfst::lexc::xreDefinitionEncode'.
fn xre_definition_encode(s: &str) -> String {
    format!("{}{}{}", LEXC_DFN_START, s, LEXC_DFN_END)
}

// replaces the first '@ZERO@' with "0" in a string
// [spec:hfst:def:lexc-utils.hfst.lexc.replace-zero-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.replace-zero-fn]
pub(super) fn replace_zero(s: &str) -> String {
    if let Some(start_pos) = s.find("@ZERO@") {
        let mut str = s.to_string();
        str.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
        str
    } else {
        s.to_string()
    }
}

// [spec:hfst:def:lexc-utils.hfst.lexc.should-colourise-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.should-colourise-fn]
// The identical 'should_colourise' static is duplicated in LexcCompiler.cc; the
// single Rust helper ports both copies.
// [spec:hfst:def:lexc-compiler.should-colourise-fn]
// [spec:hfst:sem:lexc-compiler.should-colourise-fn]
fn should_colourise() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}
