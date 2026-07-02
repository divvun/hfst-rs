//! Port of 'libhfst/src/parsers/string_src/string_manipulation.{cc,h}' — generic
//! string helpers used by the twolc/lexc parsers (escaping, whitespace
//! normalisation, quoting, integer parsing and the 'StringVector' token splitter).
//!
//! @author Miikka Silfverberg
//!
//! 'replace_substr' itself already lives (credited) in 'twolc.rs'; it is reused
//! here. 'unescape_name' likewise is credited in 'twolc.rs'.

#![allow(dead_code)]

use crate::twolc::replace_substr;

// [spec:hfst:def:string-manipulation.new-string-fn]
// [spec:hfst:sem:string-manipulation.new-string-fn]
pub fn new_string(lgth: usize) -> String {
    "\0".repeat(lgth)
}

// [spec:hfst:def:string-manipulation.remove-sign-fn]
// [spec:hfst:sem:string-manipulation.remove-sign-fn]
pub fn remove_sign_char(str: &str, sign: char) -> String {
    replace_substr(str, &sign.to_string(), "")
}

pub fn remove_sign_str(str: &str, sign: &str) -> String {
    replace_substr(str, sign, "")
}

// [spec:hfst:def:string-manipulation.unescape-fn]
// [spec:hfst:sem:string-manipulation.unescape-fn]
pub fn unescape(str: &str) -> crate::error::Result<String> {
    if str.contains('\n') {
        crate::bail!(FaultyStringInput, format!("{}: {}", "unescape", str));
    }
    // Change "%%" to "\n", remove all remaining %'s
    // and change all '\n's to '%'.
    Ok(replace_substr(
        &remove_sign_char(&replace_substr(str, "%%", "\n"), '%'),
        "\n",
        "%",
    ))
}

// [spec:hfst:def:string-manipulation.strcmp-unescaped-fn]
// [spec:hfst:sem:string-manipulation.strcmp-unescaped-fn]
pub fn compare_unescaped(str1: &str, str2: &str) -> crate::error::Result<i32> {
    // Remove all escapes from str1 and str2 and
    // compare them.
    let str1_copy = unescape(str1)?;
    let str2_copy = unescape(str2)?;
    Ok(match str1_copy.cmp(&str2_copy) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    })
}

// [spec:hfst:def:string-manipulation.remove-white-space-fn]
// [spec:hfst:sem:string-manipulation.remove-white-space-fn]
pub fn remove_white_space(str: &str) -> crate::error::Result<String> {
    if str.contains('\n') {
        crate::bail!(
            FaultyStringInput,
            format!("{}: {}", "remove_white_space", str)
        );
    }
    let mut str = replace_substr(
        &remove_sign_char(&replace_substr(str, "% ", "\n"), ' '),
        "\n",
        "__HFST_TWOLC_SPACE",
    );
    str = replace_substr(
        &remove_sign_char(&replace_substr(&str, "%\t", "\n"), '\t'),
        "\n",
        "__HFST_TWOLC_TAB",
    );
    str = replace_substr(
        &remove_sign_char(&replace_substr(&str, "%\r", "\n"), '\r'),
        "\n",
        "__HFST_TWOLC_CR",
    );
    str = replace_substr(
        &remove_sign_str(
            &replace_substr(&str, "%__HFST_TWOLC_\\n", "\n"),
            "__HFST_TWOLC_\\n",
        ),
        "\n",
        "__HFST_TWOLC_\\n",
    );
    Ok(str)
}

// [spec:hfst:def:string-manipulation.unescape-and-remove-white-space-fn]
// [spec:hfst:sem:string-manipulation.unescape-and-remove-white-space-fn]
pub fn unescape_and_remove_white_space(str: &str) -> crate::error::Result<String> {
    unescape(&remove_white_space(str)?)
}

// [spec:hfst:def:string-manipulation.unquote-fn]
// [spec:hfst:sem:string-manipulation.unquote-fn]
pub fn unquote(str: &str) -> crate::error::Result<String> {
    let bytes = str.as_bytes();
    if str.len() < 2 || bytes[0] != b'"' || bytes[str.len() - 1] != b'"' {
        crate::bail!(FaultyStringInput, format!("{}: {}", "unquote", str));
    }
    // Return the substring of str spanning from the
    // second to the next to final character.
    Ok(str[1..str.len() - 1].to_string())
}

// The shared leading-prefix number scanners (the C 'strtol'/'strtod' shape):
// skip leading whitespace, parse the longest valid number prefix, ignore
// trailing garbage, and return the parsed value plus the index of the first
// unconsumed byte (the C 'endptr'). On no conversion: (0, start).

pub fn parse_int_prefix(b: &[u8], start: usize) -> (i32, usize) {
    let mut i = start;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let mut neg = false;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        neg = b[i] == b'-';
        i += 1;
    }
    let digits_start = i;
    let mut val: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        val = val * 10 + i64::from(b[i] - b'0');
        i += 1;
    }
    if i == digits_start {
        return (0, start);
    }
    let v = if neg { -val } else { val } as i32;
    (v, i)
}

pub fn parse_float_prefix(b: &[u8], start: usize) -> (f64, usize) {
    let mut i = start;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let num_start = i;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let save = i;
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        if i < b.len() && b[i].is_ascii_digit() {
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            i = save;
        }
    }
    if i <= num_start {
        return (0.0, start);
    }
    match std::str::from_utf8(&b[num_start..i])
        .ok()
        .and_then(|x| x.parse::<f64>().ok())
    {
        Some(v) => (v, i),
        None => (0.0, start),
    }
}

/// The leading-prefix float value of `s` (0.0 if none) — the C `atof` shape.
pub fn parse_float_prefix_str(s: &str) -> f64 {
    parse_float_prefix(s.as_bytes(), 0).0
}

// [spec:hfst:def:string-manipulation.str2int-fn]
// [spec:hfst:sem:string-manipulation.str2int-fn]
pub fn str2int(str: &str) -> crate::error::Result<i32> {
    // Mirror 'std::istringstream in(str); in >> number;': a leading-prefix
    // integer parse; fail (no digits) -> FaultyStringInput.
    let (number, endptr) = parse_int_prefix(str.as_bytes(), 0);
    if endptr == 0 {
        crate::bail!(FaultyStringInput, format!("{}: {}", "str2int", str));
    }
    Ok(number)
}

// [spec:hfst:def:string-manipulation.print-kill-symbol-fn]
// [spec:hfst:sem:string-manipulation.print-kill-symbol-fn]
pub fn print_kill_symbol() {
    println!();
    println!("__HFST_TWOLC_DIE");
}

// @brief Container for strings.
// [spec:hfst:def:string-manipulation.string-vector]
#[derive(Clone, Debug, Default)]
pub struct StringVector {
    pub vec: Vec<String>,
}

impl std::ops::Deref for StringVector {
    type Target = Vec<String>;
    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}

impl std::ops::DerefMut for StringVector {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vec
    }
}

impl StringVector {
    // @brief Initialize empty.
    pub fn new() -> Self {
        StringVector { vec: Vec::new() }
    }

    // @brief Split @a s to tokens at spaces and store the tokens in @a this.
    // [spec:hfst:def:string-manipulation.string-vector.string-vector-fn]
    // [spec:hfst:sem:string-manipulation.string-vector.string-vector-fn]
    pub fn new_string(s: &str) -> Self {
        let mut vec: Vec<String> = Vec::new();
        let mut start_pos = 0;
        while let Some(rel) = s[start_pos..].find(' ') {
            let space_pos = start_pos + rel;
            vec.push(s[start_pos..space_pos].to_string());
            start_pos = space_pos + 1;
        }
        vec.push(s[start_pos..].to_string());
        StringVector { vec }
    }

    // @brief Add the values in @a another at the end of @a this.
    pub fn add_values(&mut self, another: &StringVector) -> &mut Self {
        self.vec.extend(another.vec.iter().cloned());
        self
    }
}

// @brief Regular string comparison.
// [spec:hfst:def:string-manipulation.str-cmp]
pub struct StrCmp;

impl StrCmp {
    // [spec:hfst:def:string-manipulation.str-cmp.operator-fn]
    // [spec:hfst:sem:string-manipulation.str-cmp.operator-fn]
    pub fn operator_call(str1: &str, str2: &str) -> bool {
        str1 < str2
    }
}

// @brief String comparison of unescaped strings.
// [spec:hfst:def:string-manipulation.relaxed-str-cmp]
pub struct RelaxedStrCmp;

impl RelaxedStrCmp {
    // [spec:hfst:def:string-manipulation.relaxed-str-cmp.operator-fn]
    // [spec:hfst:sem:string-manipulation.relaxed-str-cmp.operator-fn]
    pub fn operator_call(str1: &str, str2: &str) -> crate::error::Result<bool> {
        Ok(compare_unescaped(str1, str2)? < 0)
    }
}
