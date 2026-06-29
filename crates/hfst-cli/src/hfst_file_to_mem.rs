//! Faithful 1:1 port of tools/src/hfst-file-to-mem.cc — read a whole file (or
//! stdin) into a freshly malloc'd, NUL-terminated C string. Based on a function
//! in foma written by Mans Hulden.
//!
//! The io-foundation de-C-ism replaced the FILE* reading (fgetc/feof on stdin;
//! fopen/fseek/ftell/fread on a named file) with std::io reads; the result is
//! still a malloc'd, NUL-terminated C buffer the caller frees. The original
//! stdin path capped input at 1 MB (a fixed-buffer artefact); that arbitrary
//! cap is dropped — std reads the whole stream.

use core::ffi::c_char;
use std::io::Read;

use crate::hfst_commandline::error;

const EXIT_FAILURE: i32 = 1;

// Copy `bytes` into a fresh owned, NUL-terminated C buffer (the caller reclaims
// it with hfst_free, matching the original malloc'd return). The buffer is a
// `CString` (`into_raw`), so the data is taken up to the first NUL — the C-string
// view a `char*` consumer would see — keeping the alloc/free path on the Rust
// allocator with no libc.
fn bytes_to_cmalloc(bytes: &[u8]) -> *mut c_char {
    let cs = match std::ffi::CString::new(bytes) {
        Ok(cs) => cs,
        Err(e) => {
            let pos = e.nul_position();
            std::ffi::CString::new(&bytes[..pos]).unwrap()
        }
    };
    cs.into_raw()
}

// [spec:hfst:def:hfst-file-to-mem.hfst-stdin-to-mem-fn]
// [spec:hfst:sem:hfst-file-to-mem.hfst-stdin-to-mem-fn]
pub fn hfst_stdin_to_mem() -> *mut c_char {
    let mut bytes = Vec::new();
    if std::io::stdin().read_to_end(&mut bytes).is_err() {
        error(EXIT_FAILURE, 0, "Error reading file '<stdin>' to memory\n");
        return std::ptr::null_mut();
    }
    bytes_to_cmalloc(&bytes)
}

// Based on a function in foma written by Mans Hulden.
// Read the file 'filename' to memory and return a pointer to it.
// Filename "<stdin>" uses stdin for reading.
// Returns NULL if file cannot be opened or read or memory cannot be allocated.

// [spec:hfst:def:hfst-file-to-mem.hfst-file-to-mem-fn]
// [spec:hfst:sem:hfst-file-to-mem.hfst-file-to-mem-fn]
pub fn hfst_file_to_mem(filename: &str) -> *mut c_char {
    if filename == "<stdin>" {
        return hfst_stdin_to_mem();
    }
    match std::fs::read(filename) {
        Ok(bytes) => bytes_to_cmalloc(&bytes),
        Err(_) => {
            error(
                EXIT_FAILURE,
                0,
                &format!("Error opening file '{}'\n", filename),
            );
            std::ptr::null_mut()
        }
    }
}
