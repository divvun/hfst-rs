//! String helpers ported from the lexer support in 'xre_utils.cc'.

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
    t.substitute_symbol_substitutions(&substitutions)?;
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
