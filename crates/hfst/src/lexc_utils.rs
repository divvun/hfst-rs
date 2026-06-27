//! Port of the genuinely-standalone string helpers from
//! 'libhfst/src/parsers/lexc-utils.{h,cc}'.
//!
//! The bulk of lexc-utils.cc is either (a) already inlined into 'lexc.rs' (the
//! RECODE-LEXC encoders, 'replace_zero', 'should_colourise', 'find_med_alingment',
//! the 'error_at_current_token' / 'warning_at_current_token' reporting, and the
//! 'lexc_'-singleton-driven 'strip_percents'), or (b) here.
//!
//! What lives here are the pure C-string helpers ('count_newlines', 'strstrip',
//! 'strdup_nonconst_part') plus the Flex/Bison position-tracking bookkeeping
//! ('set_infile_name', 'token_reset_positions', 'token_update_positions',
//! 'strdup_token_positions', 'strdup_token_part'). The latter mutate the
//! generated-lexer globals ('hlexclloc' / 'hlexctext' / 'hlexclineno' /
//! 'hlexcfilename'); since the nfst-driven AST walk has no generated lexer,
//! those globals are modelled as module-level thread-local state below.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(unused_imports)]

use std::cell::{Cell, RefCell};

use crate::hfst_data_types::size_t_to_int;

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

// ==========================================================================
// Flex/Bison generated-lexer globals used by the position-tracking helpers
// below. In the C++ these are flex externs ('hlexclineno', 'hlexctext',
// 'hlexclloc') plus a file-static ('hlexcfilename'); the AST-walk port has no
// generated lexer, so they are modelled here as module-level thread-local
// state. 'None' stands in for the C++ NULL 'char *'.
// ==========================================================================

// Mirror of the Flex/Bison 'YYLTYPE' source-location struct.
#[derive(Clone, Copy)]
struct Yyltype {
    first_line: i32,
    first_column: i32,
    last_line: i32,
    last_column: i32,
}

thread_local! {
    static HLEXCLINENO: Cell<i32> = const { Cell::new(0) };
    static HLEXCFILENAME: RefCell<Option<String>> = const { RefCell::new(None) };
    static HLEXCTEXT: RefCell<Option<String>> = const { RefCell::new(None) };
    static HLEXCLLOC: Cell<Yyltype> = const {
        Cell::new(Yyltype {
            first_line: 0,
            first_column: 0,
            last_line: 0,
            last_column: 0,
        })
    };
}

// [spec:hfst:def:lexc-utils.hfst.lexc.set-infile-name-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.set-infile-name-fn]
fn set_infile_name(s: &str) {
    // 'free(hlexcfilename); hlexcfilename = strdup(s);'
    HLEXCFILENAME.with(|f| *f.borrow_mut() = Some(s.to_string()));
}

// [spec:hfst:def:lexc-utils.hfst.lexc.token-reset-positions-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.token-reset-positions-fn]
fn token_reset_positions() {
    HLEXCLLOC.with(|l| {
        let mut loc = l.get();
        loc.first_line = 1;
        loc.last_line = 1;
        loc.first_column = 1;
        loc.last_column = 1;
        l.set(loc);
    });
    HLEXCLINENO.with(|n| n.set(1));
    // 'if (hlexcfilename != 0) free(hlexcfilename); hlexcfilename = 0;'
    HLEXCFILENAME.with(|f| *f.borrow_mut() = None);
}

// [spec:hfst:def:lexc-utils.hfst.lexc.token-update-positions-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.token-update-positions-fn]
fn token_update_positions(token: &str) {
    let token_length = cstrlen(token);
    let newlines = size_t_to_int(count_newlines(token));
    HLEXCLLOC.with(|l| {
        let mut loc = l.get();
        loc.first_line = loc.last_line;
        loc.last_line = loc.first_line + newlines;
        // FIXME: columns equal bytes not characters
        loc.first_column = loc.last_column + 1;
        if 0 == newlines {
            loc.last_column = loc.first_column + size_t_to_int(token_length);
        } else {
            let bytes = token.as_bytes();
            let token_end = cstrlen(token); // strrchr(token, '\0') - token
            // strrchr(token, '\n') - token: index of the last newline (newlines
            // > 0 guarantees one exists).
            let token_last_line_start = bytes[..token_end]
                .iter()
                .rposition(|&b| b == b'\n')
                .unwrap();
            loc.last_column = (token_end as i32 - token_last_line_start as i32) - 1;
        }
        l.set(loc);
    });
}

// [spec:hfst:def:lexc-utils.hfst.lexc.strdup-token-positions-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.strdup-token-positions-fn]
// N.B. reason for this error format is automagic support by vim/emacs/jedit:
// it must be 'filename:lineno:colno-lineno:colno: stuff'
// (c.f. http://www.gnu.org/prep/standards/standards.html#Errors).
fn strdup_token_positions() -> String {
    let hlexcfilename = HLEXCFILENAME.with(|f| f.borrow().clone().unwrap_or_default());
    let loc = HLEXCLLOC.with(|l| l.get());
    // linenumbers and columns
    if (loc.first_line == loc.last_line) && (loc.first_column == (loc.last_column - 1)) {
        // filename, line and column
        format!("{}:{}.{}", hlexcfilename, loc.first_line, loc.first_column)
    } else if loc.first_line == loc.last_line {
        // filename, line, column to column
        format!(
            "{}:{}.{}-{}",
            hlexcfilename, loc.first_line, loc.first_column, loc.last_column
        )
    } else {
        // filename, line-column to line-column
        format!(
            "{}:{}.{}-{}.{}",
            hlexcfilename, loc.first_line, loc.first_column, loc.last_line, loc.last_column
        )
    }
}

// [spec:hfst:def:lexc-utils.hfst.lexc.strdup-token-part-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.strdup-token-part-fn]
fn strdup_token_part() -> String {
    let hlexctext = HLEXCTEXT.with(|t| t.borrow().clone().unwrap_or_default());
    let bytes = hlexctext.as_bytes();
    let len = cstrlen(&hlexctext);
    let maybelbr = bytes[..len].iter().position(|&b| b == b'\n'); // strchr -> first '\n'
    if let Some(nl) = maybelbr {
        let beforelbr = String::from_utf8_lossy(&bytes[..nl]).into_owned();
        format!("[near: `{}\\n']", beforelbr)
    } else if len < 80 {
        format!("[near: `{}']", hlexctext)
    } else {
        format!("[near: `{:>30}...' (truncated)]", hlexctext)
    }
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
