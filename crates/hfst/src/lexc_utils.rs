//! Port of the genuinely-standalone string helpers from
//! 'libhfst/src/parsers/lexc-utils.{h,cc}'.
//!
//! The bulk of lexc-utils.cc is either (a) already inlined into 'lexc.rs' (the
//! RECODE-LEXC encoders, 'replace_zero', 'should_colourise', 'find_med_alingment',
//! and the 'error_at_current_token' / 'warning_at_current_token' reporting), or
//! (b) Flex/Bison bookkeeping coupled to the generated-lexer globals
//! ('hlexclloc' / 'hlexctext' / 'hlexclineno' / 'hlexcfilename') and the 'lexc_'
//! compiler singleton -- which the nfst-driven AST walk replaces, so those are
//! out of scope (see the 'Deferred' note at the top of 'lexc.rs').
//!
//! What remains here are the three pure, flex-global-free helpers that operate
//! on plain C strings: 'count_newlines', 'strstrip' and 'strdup_nonconst_part'.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(unused_imports)]

// C 'isspace' for the "C" locale: HT, LF, VT, FF, CR and space. (Rust's
// 'u8::is_ascii_whitespace' deliberately omits VT/0x0B, so it is not a match.)
fn isspace(c: u8) -> bool {
    matches!(c, b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b' ')
}

// 'strlen' on a Rust string: the byte length, but stopping at the first NUL the
// way the C 'char *' helpers do.
fn cstrlen(s: &str) -> usize {
    match s.as_bytes().iter().position(|&b| b == b'\0') {
        Some(n) => n,
        None => s.len(),
    }
}

// [spec:hfst:def:lexc-utils.hfst.lexc.count-newlines-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.count-newlines-fn]
fn count_newlines(linestring: &str) -> usize {
    let bytes = linestring.as_bytes();
    let mut linecount: usize = 0;
    let mut i = 0;
    while i < bytes.len() && bytes[i] != b'\0' {
        if bytes[i] == b'\n' {
            linecount += 1;
        }
        i += 1;
    }
    linecount
}

// [spec:hfst:def:lexc-utils.hfst.lexc.strstrip-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.strstrip-fn]
fn strstrip(s: &str) -> String {
    let bytes = s.as_bytes();

    // empty string is a special case
    if bytes.is_empty() || bytes[0] == b'\0' {
        return String::new();
    }

    // skip leading whitespace
    let mut i = 0;
    while i < bytes.len() && isspace(bytes[i]) {
        i += 1;
    }

    // copy the remaining bytes up to the first NUL
    let mut rv: Vec<u8> = Vec::new();
    while i < bytes.len() && bytes[i] != b'\0' {
        rv.push(bytes[i]);
        i += 1;
    }

    // trim trailing whitespace
    while let Some(&last) = rv.last() {
        if isspace(last) {
            rv.pop();
        } else {
            break;
        }
    }

    String::from_utf8(rv).unwrap()
}

// [spec:hfst:def:lexc-utils.hfst.lexc.strdup-nonconst-part-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.strdup-nonconst-part-fn]
fn strdup_nonconst_part(
    token: &str,
    prefix: Option<&str>,
    suffix: Option<&str>,
    strip: bool,
) -> String {
    let token_len = cstrlen(token);
    let prefix_len = prefix.map_or(0, cstrlen);
    let suffix_len = suffix.map_or(0, cstrlen);
    let varpart_len = token_len - prefix_len - suffix_len;
    debug_assert!(varpart_len <= token_len);

    let token_bytes = token.as_bytes();

    if prefix.is_none() {
        debug_assert!(token_bytes.starts_with(b""));
    } else {
        debug_assert!(token_bytes.starts_with(prefix.unwrap().as_bytes()));
    }

    if suffix.is_none() {
        debug_assert!(token_bytes[prefix_len + varpart_len..].starts_with(b""));
    } else {
        debug_assert!(
            token_bytes[prefix_len + varpart_len..].starts_with(suffix.unwrap().as_bytes())
        );
    }

    let part = &token_bytes[prefix_len..prefix_len + varpart_len];
    let mut token_part = String::from_utf8_lossy(part).into_owned();
    if strip {
        token_part = strstrip(&token_part);
    }
    token_part
}
