//! String helpers ported from the lexer support in 'xre_utils.cc'.

use tracing::{error, warn};

use super::*;
use crate::string_manipulation::{parse_float_prefix, parse_int_prefix};

// ---- former xre_utils.cc string / lexer-support free helpers ----
//
// These are 1:1 ports of the pure 'char*'-style helpers that the original
// flex/bison lexer leaned on. The nfst parser no longer drives them, but they
// are faithful ports kept for completeness. C 'char*' buffers become Rust
// 'String'; C 'strtol'/'strtod' are mirrored by the shared
// 'string_manipulation::parse_int_prefix' / 'parse_float_prefix' scanners.

// [spec:hfst:def:xre-utils.hfst.xre.get-n-to-k-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.get-n-to-k-fn]
// xre_utils.cc:228. Parses the '{n,k}' / 'n,k' bounds of a repetition token.
fn get_n_to_k(s: &str) -> [i32; 2] {
    let b = s.as_bytes();
    let mut rv = [0i32; 2];
    if b.get(1).copied() == Some(b'{') {
        let (v0, endptr) = parse_int_prefix(b, 2);
        rv[0] = v0;
        let (v1, finalptr) = parse_int_prefix(b, endptr + 1);
        rv[1] = v1;
        assert!(b.get(finalptr).copied() == Some(b'}'));
    } else {
        let (v0, endptr) = parse_int_prefix(b, 1);
        rv[0] = v0;
        let (v1, finalptr) = parse_int_prefix(b, endptr + 1);
        rv[1] = v1;
        assert!(b.get(finalptr).copied().unwrap_or(0) == 0);
    }
    rv
}

// [spec:hfst:def:xre-utils.hfst.xre.strip-newline-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.strip-newline-fn]
// xre_utils.cc:268. Replaces every '\n'/'\r' byte with a nul, in place.
pub(super) fn strip_newline(s: &str) -> String {
    let mut b = s.as_bytes().to_vec();
    for byte in b.iter_mut() {
        if *byte == b'\n' || *byte == b'\r' {
            *byte = 0;
        }
    }
    String::from_utf8_lossy(&b).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.count-lines-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.count-lines-fn]
// xre_utils.cc:282. Advances the 'lr'/'cr' counters over a chunk of input.
// The former 'hfst::xre::cr'/'lr' file-scope counters are now function-local
// (this faithful port is never driven by the nfst parser), removing the
// thread-global mutable state.
fn count_lines(s: &str) {
    let cr = std::cell::Cell::new(0u32);
    let lr = std::cell::Cell::new(1u32);
    let b = s.as_bytes();
    let mut i: usize = 0;
    while i < b.len() && b[i] != 0 {
        if b[i] == b'\n' {
            lr.set(lr.get() + 1);
        } else if b[i] == b'\r' {
            i += 1;
            if i < b.len() && b[i] == b'\n' {
                cr.set(cr.get() + 1);
            } else {
                i -= 1;
            }
            lr.set(lr.get() + 1);
        }
        cr.set(cr.get() + 1);
        i += 1;
    }
}

// [spec:hfst:def:xre-utils.hfst.xre.strip-curly-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.strip-curly-fn]
// xre_utils.cc:312. Drops an enclosing pair of curly braces.
fn strip_curly(s: &str) -> String {
    let c = s.as_bytes();
    let mut stripped: Vec<u8> = vec![0u8; c.len() + 1];
    let mut i: usize = 0;
    let mut p: usize = 0;
    while p < c.len() && c[p] != 0 {
        let next_is_nul = p + 1 >= c.len() || c[p + 1] == 0;
        if (c[p] == b'{' && i == 0) || (c[p] == b'}' && next_is_nul) {
            if next_is_nul {
                break;
            } else {
                stripped[i] = c[p + 1];
                i += 1;
                p += 2;
            }
        } else {
            stripped[i] = c[p];
            i += 1;
            p += 1;
        }
    }
    stripped[i] = 0;
    String::from_utf8_lossy(&stripped[..i]).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.strip-percents-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.strip-percents-fn]
// xre_utils.cc:347. Removes '%' escape prefixes.
fn strip_percents(s: &str) -> String {
    let c = s.as_bytes();
    let mut stripped: Vec<u8> = vec![0u8; c.len() + 1];
    let mut i: usize = 0;
    let mut p: usize = 0;
    while p < c.len() && c[p] != 0 {
        if c[p] == b'%' {
            if p + 1 >= c.len() || c[p + 1] == 0 {
                break;
            } else {
                stripped[i] = c[p + 1];
                i += 1;
                p += 2;
            }
        } else {
            stripped[i] = c[p];
            i += 1;
            p += 1;
        }
    }
    stripped[i] = 0;
    String::from_utf8_lossy(&stripped[..i]).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.add-percents-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.add-percents-fn]
// xre_utils.cc:381. Prefixes '%' before xfst special characters.
fn add_percents(s: &str) -> String {
    let b = s.as_bytes();
    let mut ns: Vec<u8> = Vec::with_capacity(b.len() * 2 + 1);
    for &ch in b {
        if matches!(
            ch,
            b'@' | b'-'
                | b' '
                | b'|'
                | b'!'
                | b':'
                | b';'
                | b'0'
                | b'\\'
                | b'&'
                | b'?'
                | b'$'
                | b'+'
                | b'*'
                | b'/'
                | b'_'
                | b'('
                | b')'
                | b'{'
                | b'}'
                | b'['
                | b']'
        ) {
            ns.push(b'%');
        }
        ns.push(ch);
    }
    String::from_utf8_lossy(&ns).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.get-quoted-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.get-quoted-fn]
// xre_utils.cc:408. Returns the substring between the first and last '"'.
fn get_quoted(s: &str) -> String {
    let b = s.as_bytes();
    let first = b
        .iter()
        .position(|&c| c == b'"')
        .expect("quoted token contains an opening quote");
    let qstart = first + 1;
    let qend = b
        .iter()
        .rposition(|&c| c == b'"')
        .expect("quoted token contains a closing quote");
    let len = qend - qstart;
    String::from_utf8_lossy(&b[qstart..qstart + len]).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.parse-quoted-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.parse-quoted-fn]
// xre_utils.cc:420. Unescapes a quoted string, writing its utf8 length to
// 'length'. 'throw' becomes 'panic_any'; the deferred error stream is replaced
// by stderr writes. The octal escape preserves the C bug of writing a nul
// WITHOUT advancing the write pointer (so nothing is appended).
fn parse_quoted(s: &str, length: &mut u32) -> String {
    let quoted = get_quoted(s);
    let qb = quoted.as_bytes();
    let mut rv: Vec<u8> = Vec::with_capacity(qb.len() + 1);
    let mut p: usize = 0;
    while p < qb.len() && qb[p] != 0 {
        let cur = qb[p];
        if cur == b'\n' || cur == b'\r' {
            std::panic::panic_any(
                "Unescaped newline characters found inside quoted string.".to_string(),
            );
        } else if cur != b'\\' {
            rv.push(cur);
            p += 1;
        } else {
            let nxt = qb.get(p + 1).copied().unwrap_or(0);
            match nxt {
                b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' => {
                    error!(
                        "XRE unimplemented: parse octal escape in {}",
                        String::from_utf8_lossy(&qb[p..])
                    );
                    p += 5;
                }
                b'a' => {
                    rv.push(0x07);
                    p += 2;
                }
                b'b' => {
                    rv.push(0x08);
                    p += 2;
                }
                b'f' => {
                    rv.push(0x0c);
                    p += 2;
                }
                b'n' => {
                    rv.push(b'\n');
                    p += 2;
                }
                b'r' => {
                    rv.push(b'\r');
                    p += 2;
                }
                b't' => {
                    rv.push(b'\t');
                    p += 2;
                }
                b'u' => {
                    error!(
                        "Unimplemented: parse unicode escapes in {}",
                        String::from_utf8_lossy(&qb[p..])
                    );
                    rv.push(0);
                    p += 6;
                }
                b'v' => {
                    rv.push(0x0b);
                    p += 2;
                }
                b'x' => {
                    // NB: the C source uses base 10 here (a bug); preserved.
                    let (i, endp) = parse_int_prefix(qb, p + 2);
                    if 0 < i && i <= 127 {
                        rv.push(i as u8);
                    } else {
                        error!("XRE unimplemented: parse \\x{}", i);
                        rv.push(0);
                    }
                    assert!(endp != p);
                    p = endp;
                }
                0 => {
                    warn!("End of line after \\ escape");
                    rv.push(0);
                    p += 1;
                }
                other => {
                    rv.push(other);
                    p += 2;
                }
            }
        }
    }
    // C builds a 'std::string' from the buffer, which truncates at the first
    // interior nul left by a '\u'/'\x'/end-of-line escape.
    let end = rv.iter().position(|&c| c == 0).unwrap_or(rv.len());
    let result = String::from_utf8_lossy(&rv[..end]).into_owned();
    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-and-calculate-length-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-and-calculate-length-fn]
    //
    // A Rust 'String' is always valid UTF-8, so the check cannot fail; the
    // length is the UTF-16 code-unit count, as 'u_strFromUTF8' measured.
    *length = result.encode_utf16().count() as u32;
    result
}

// If 'str' is of form "@_<foo>_@", insert pair ("@_<foo>_@", "<foo>") into
// 'substitutions'.
// [spec:hfst:def:xre-utils.hfst.xre.insert-angle-bracket-substitutions-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.insert-angle-bracket-substitutions-fn]
// xre_utils.cc:553.
fn insert_angle_bracket_substitutions(
    str: &str,
    substitutions: &mut crate::hfst_symbol_defs::HfstSymbolSubstitutions,
) {
    if str.len() < 6 {
        return;
    }
    let b = str.as_bytes();
    if &b[0..3] == b"@_<" && &b[b.len() - 3..] == b">_@" {
        let substituting_str = &str[2..str.len() - 2];
        substitutions.insert(Symbol::new(str), Symbol::new(substituting_str));
    }
}

// [spec:hfst:def:xre-utils.hfst.xre.escape-enclosing-angle-brackets-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.escape-enclosing-angle-brackets-fn]
// xre_utils.cc:569. Wraps a '<...>' symbol as "@_<...>_@".
fn escape_enclosing_angle_brackets(s: &str) -> String {
    let b = s.as_bytes();
    if b.is_empty() || b[0] != b'<' {
        return s.to_string();
    }
    let i = b.len() - 1;
    if b[i] != b'>' {
        return s.to_string();
    }
    format!("@_{}_@", s)
}

// [spec:hfst:def:xre-utils.hfst.xre.unescape-enclosing-angle-brackets-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.unescape-enclosing-angle-brackets-fn]
// xre_utils.cc:591. Reverses the "@_<...>_@" wrapping for every alphabet symbol.
fn unescape_enclosing_angle_brackets<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
) -> crate::error::Result<()> {
    let mut substitutions: crate::hfst_symbol_defs::HfstSymbolSubstitutions =
        crate::hfst_symbol_defs::HfstSymbolSubstitutions::new();
    let alpha = t.get_alphabet()?;
    for it in alpha.iter() {
        insert_angle_bracket_substitutions(it, &mut substitutions);
    }
    if substitutions.is_empty() {
        return Ok(());
    }
    t.substitute_substitutions(&substitutions)?;
    t.optimize_with_config(&crate::hfst_transducer::EngineConfig::default())?;
    Ok(())
}

// [spec:hfst:def:xre-utils.hfst.xre.get-weight-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.get-weight-fn]
// xre_utils.cc:610. Parses a trailing weight, skipping leading ' '/'\t'/';'.
fn get_weight(s: &str) -> f64 {
    let b = s.as_bytes();
    let mut weightstart: usize = 0;
    while weightstart < b.len()
        && b[weightstart] != 0
        && (b[weightstart] == b' ' || b[weightstart] == b'\t' || b[weightstart] == b';')
    {
        weightstart += 1;
    }
    let (val, endp) = parse_float_prefix(b, weightstart);
    assert!(endp != weightstart);
    val
}

// [spec:hfst:def:xre-utils.should-colourise-fn]
// [spec:hfst:sem:xre-utils.should-colourise-fn]
// xre_utils.cc:95. 'isatty(1)' -> stdout is a terminal.
fn should_colourise() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}
