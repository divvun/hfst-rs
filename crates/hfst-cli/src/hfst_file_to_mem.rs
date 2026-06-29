//! Faithful 1:1 port of tools/src/hfst-file-to-mem.cc — read a whole file (or
//! stdin) into a freshly malloc'd, NUL-terminated C string. Based on a function
//! in foma written by Mans Hulden.
//!
//! The io-foundation de-C-ism replaced the FILE* reading (fgetc/feof on stdin;
//! fopen/fseek/ftell/fread on a named file) with std::io reads; the result is
//! still a malloc'd, NUL-terminated C buffer the caller frees. The original
//! stdin path capped input at 1 MB (a fixed-buffer artefact); that arbitrary
//! cap is dropped — std reads the whole stream.

use libc::c_char;
use std::io::Read;

use crate::hfst_commandline::error;

const EXIT_FAILURE: i32 = 1;

// Copy `bytes` into a fresh malloc'd, NUL-terminated C buffer (the caller frees
// it, matching the original malloc'd return).
unsafe fn bytes_to_cmalloc(bytes: &[u8], what: &str) -> *mut c_char {
    let n = bytes.len();
    let buffer = unsafe { libc::malloc(n + 1) } as *mut c_char;
    if buffer.is_null() {
        error(
            EXIT_FAILURE,
            0,
            &format!("Error allocating memory to read file '{}'\n", what),
        );
        return std::ptr::null_mut();
    }
    unsafe {
        if n > 0 {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer as *mut u8, n);
        }
        *buffer.add(n) = 0;
    }
    buffer
}

// [spec:hfst:def:hfst-file-to-mem.hfst-stdin-to-mem-fn]
// [spec:hfst:sem:hfst-file-to-mem.hfst-stdin-to-mem-fn]
pub fn hfst_stdin_to_mem() -> *mut c_char {
    let mut bytes = Vec::new();
    if std::io::stdin().read_to_end(&mut bytes).is_err() {
        error(EXIT_FAILURE, 0, "Error reading file '<stdin>' to memory\n");
        return std::ptr::null_mut();
    }
    unsafe { bytes_to_cmalloc(&bytes, "<stdin>") }
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
        Ok(bytes) => unsafe { bytes_to_cmalloc(&bytes, filename) },
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
