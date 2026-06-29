//! Port of 'libhfst/src/parsers/string_src/string_manipulation.{cc,h}' — generic
//! string helpers used by the twolc/lexc parsers (escaping, whitespace
//! normalisation, quoting, integer parsing and the 'StringVector' token splitter).
//!
//! @author Miikka Silfverberg
//!
//! 'replace_substr' itself already lives (credited) in 'twolc.rs'; it is reused
//! here. 'unescape_name' likewise is credited in 'twolc.rs'.

#![allow(dead_code)]
#![allow(non_camel_case_types)]

use std::os::raw::c_char;

use crate::twolc::replace_substr;

// @brief Thrown when a string manipulation function receives incorrect
// string input.
// [spec:hfst:def:string-manipulation.faulty-string-input]
pub struct FaultyStringInput {
    // @var Name of the function which threw this instance.
    pub function: String,

    // @var The incorrect input @function received.
    pub input: String,
}

impl FaultyStringInput {
    // [spec:hfst:def:string-manipulation.faulty-string-input.faulty-string-input-fn]
    // [spec:hfst:sem:string-manipulation.faulty-string-input.faulty-string-input-fn]
    pub fn new(function: &str, input: &str) -> Self {
        FaultyStringInput {
            function: function.to_string(),
            input: input.to_string(),
        }
    }
}

// [spec:hfst:def:string-manipulation.new-string-fn]
// [spec:hfst:sem:string-manipulation.new-string-fn]
pub fn new_string(lgth: usize) -> String {
    "\0".repeat(lgth)
}

// [spec:hfst:def:string-manipulation.string-copy-fn]
// [spec:hfst:sem:string-manipulation.string-copy-fn]
pub unsafe fn string_copy(str: *const c_char) -> *mut c_char {
    // strdup over the Rust allocator: an owned copy reclaimed with CString::from_raw.
    unsafe { std::ffi::CStr::from_ptr(str) }
        .to_owned()
        .into_raw()
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
pub fn unescape(str: &str) -> String {
    if str.contains('\n') {
        std::panic::panic_any(FaultyStringInput::new("unescape", str));
    }
    // Change "%%" to "\n", remove all remaining %'s
    // and change all '\n's to '%'.
    replace_substr(
        &remove_sign_char(&replace_substr(str, "%%", "\n"), '%'),
        "\n",
        "%",
    )
}

// [spec:hfst:def:string-manipulation.strcmp-unescaped-fn]
// [spec:hfst:sem:string-manipulation.strcmp-unescaped-fn]
pub fn strcmp_unescaped(str1: &str, str2: &str) -> i32 {
    // Remove all escapes from str1 and str2 and
    // compare them.
    let str1_copy = unescape(str1);
    let str2_copy = unescape(str2);
    match str1_copy.cmp(&str2_copy) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

// [spec:hfst:def:string-manipulation.remove-white-space-fn]
// [spec:hfst:sem:string-manipulation.remove-white-space-fn]
pub fn remove_white_space(str: &str) -> String {
    if str.contains('\n') {
        std::panic::panic_any(FaultyStringInput::new("remove_white_space", str));
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
    str
}

// [spec:hfst:def:string-manipulation.unescape-and-remove-white-space-fn]
// [spec:hfst:sem:string-manipulation.unescape-and-remove-white-space-fn]
pub fn unescape_and_remove_white_space(str: &str) -> String {
    unescape(&remove_white_space(str))
}

// [spec:hfst:def:string-manipulation.unquote-fn]
// [spec:hfst:sem:string-manipulation.unquote-fn]
pub fn unquote(str: &str) -> String {
    let bytes = str.as_bytes();
    if str.len() < 2 || bytes[0] != b'"' || bytes[str.len() - 1] != b'"' {
        std::panic::panic_any(FaultyStringInput::new("unquote", str));
    }
    // Return the substring of str spanning from the
    // second to the next to final character.
    str[1..str.len() - 1].to_string()
}

// [spec:hfst:def:string-manipulation.str2int-fn]
// [spec:hfst:sem:string-manipulation.str2int-fn]
pub fn str2int(str: &str) -> i32 {
    // Mirror 'std::istringstream in(str); in >> number;': skip leading
    // whitespace, read an optional sign, then the run of decimal digits;
    // fail (no digits) -> FaultyStringInput.
    let bytes = str.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] as char).is_whitespace() {
        i += 1;
    }
    let mut neg = false;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        neg = bytes[i] == b'-';
        i += 1;
    }
    let digit_start = i;
    let mut value: i64 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        value = value * 10 + (bytes[i] - b'0') as i64;
        i += 1;
    }
    if i == digit_start {
        std::panic::panic_any(FaultyStringInput::new("str2int", str));
    }
    let number = if neg { -value } else { value };
    number as i32
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
pub struct str_cmp;

impl str_cmp {
    // [spec:hfst:def:string-manipulation.str-cmp.operator-fn]
    // [spec:hfst:sem:string-manipulation.str-cmp.operator-fn]
    pub fn operator_call(str1: &str, str2: &str) -> bool {
        str1 < str2
    }
}

// @brief String comparison of unescaped strings.
// [spec:hfst:def:string-manipulation.relaxed-str-cmp]
pub struct relaxed_str_cmp;

impl relaxed_str_cmp {
    // [spec:hfst:def:string-manipulation.relaxed-str-cmp.operator-fn]
    // [spec:hfst:sem:string-manipulation.relaxed-str-cmp.operator-fn]
    pub fn operator_call(str1: &str, str2: &str) -> bool {
        strcmp_unescaped(str1, str2) < 0
    }
}
