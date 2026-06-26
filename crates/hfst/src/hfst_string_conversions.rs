//! Port of 'libhfst/src/hfst-string-conversions.{h,cc}'.
//!
//! The whole C++ file is '#ifdef WINDOWS': it works around the Windows console
//! code page so UTF-8 reaches 'WriteConsoleW'/'ReadConsoleW'. Its own header
//! documents that "on linux and mac, calls always fprintf directly". Rust's
//! stdin/stdout/stderr are UTF-8 on every platform, so the wide-char dance is
//! unnecessary; these are ported as cross-platform shims that capture the
//! intended observable behavior.

// [spec:hfst:def:hfst-string-conversions.hfst.hfst-fprintf-console-fn]
// [spec:hfst:sem:hfst-string-conversions.hfst.hfst-fprintf-console-fn]
// The C++ is variadic 'fprintf'; on non-Windows it is exactly 'vfprintf'. Here
// the caller does the formatting (Rust 'format!') and passes the finished
// string; we write it to the FILE* stream. Rust strings are UTF-8, so no
// codepage conversion is needed.
pub fn hfst_fprintf_console(stream: *mut libc::FILE, s: &str) -> i32 {
    let c = match std::ffi::CString::new(s) {
        Ok(c) => c,
        Err(_) => return -1,
    };
    unsafe { libc::fputs(c.as_ptr(), stream) }
}

// [spec:hfst:def:hfst-string-conversions.hfst.get-line-from-console-fn]
// [spec:hfst:sem:hfst-string-conversions.hfst.get-line-from-console-fn]
pub fn get_line_from_console(str: &mut String, _buffer_size: usize, keep_newline: bool) -> bool {
    use std::io::BufRead;
    let mut line = String::new();
    let n = std::io::stdin().lock().read_line(&mut line).unwrap_or(0);
    if n == 0 {
        return false; // EOF
    }
    *str = line;

    // control+Z (26) or control+D (4)
    if !str.is_empty() && (str.as_bytes()[0] == 26 || str.as_bytes()[0] == 4) {
        return false;
    }

    // Get rid of carriage returns and newlines.
    if str.is_empty() {
        return true;
    }

    // Get rid of carriage return before the newline.
    if str.len() > 1 && str.as_bytes()[str.len() - 2] == b'\r' {
        str.remove(str.len() - 2);
    }

    if str.as_bytes()[str.len() - 1] != b'\n' {
        return true;
    }

    if keep_newline {
        return true;
    }

    str.truncate(str.len() - 1);
    true
}

// [spec:hfst:def:hfst-string-conversions.hfst.set-console-cp-to-utf8-fn]
// [spec:hfst:sem:hfst-string-conversions.hfst.set-console-cp-to-utf8-fn]
pub fn set_console_cp_to_utf8() {
    // no-op: Rust console I/O is UTF-8 on every platform.
}
