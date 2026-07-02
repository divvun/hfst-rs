//! hfst-c -- the C API (libhfst_c) over the hfst facade.
//!
//! Wave-2 1:1 port of libhfst/src/c/libhfst_c.{cpp,h}: a thin extern "C" layer
//! whose arguments and return types are C-compatible (mostly void pointers cast
//! to the underlying facade objects). These are still ordinary Rust functions;
//! internally they use the ported hfst facade to do their work.
//!
//! Raw pointers and unsafe are expected here (the C++ original passes every
//! HfstTransducer / HfstInputStream / HfstOneLevelPaths across the boundary as a
//! void pointer). Strings handed back to the caller are allocated with
//! libc::malloc so the C caller can free() them, exactly as the C++ does.

#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]
#![allow(unsafe_op_in_unsafe_fn)]

use std::os::raw::{c_char, c_void};

use hfst::hfst_data_types::{HfstOneLevelPath, HfstOneLevelPaths};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;

// [spec:hfst:def:libhfst-c.result-iterator]
//
// The C++ struct holds two heap-allocated std::set iterators (begin, end). The
// port keeps the same two-void-pointer layout but materialises the path set into
// a heap Vec once: begin is a heap usize cursor, end is the heap Vec of paths.
// Iterator equality (done) becomes cursor >= len.
#[repr(C)]
pub struct ResultIterator {
    pub begin: *mut c_void,
    pub end: *mut c_void,
}

// [spec:hfst:def:libhfst-c.hfst-value]
#[repr(C)]
pub struct HfstValue {
    pub weight: f32,
    pub s: *mut c_char,
}

// Concatenate the symbol strings of one path into a fresh libc::malloc'd,
// NUL-terminated C string the caller is responsible for free()ing.
unsafe fn malloc_cstring(parts: &[String]) -> *mut c_char {
    let mut full = String::new();
    for p in parts {
        full.push_str(p);
    }
    let bytes = full.as_bytes();
    let len = bytes.len();
    let mem = libc::malloc(len + 1) as *mut u8;
    if !mem.is_null() {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), mem, len);
        *mem.add(len) = 0;
    }
    mem as *mut c_char
}

// [spec:hfst:def:libhfst-c.hfst-empty-transducer-fn]
// [spec:hfst:sem:libhfst-c.hfst-empty-transducer-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_empty_transducer() -> *mut c_void {
    let fsa = Box::new(HfstTransducer::new());
    Box::into_raw(fsa) as *mut c_void
}

// [spec:hfst:def:libhfst-c.hfst-input-stream-fn]
// [spec:hfst:sem:libhfst-c.hfst-input-stream-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_input_stream(filename: *const c_char) -> *mut c_void {
    // Rust cannot handle C++ exceptions, so the C++ wraps the constructor in a
    // catch-all; here new_filename does not unwind across the boundary.
    if filename.is_null() {
        return std::ptr::null_mut();
    }
    let name = unsafe { std::ffi::CStr::from_ptr(filename) }.to_string_lossy();
    let is = Box::new(HfstInputStream::new_filename(&name));
    Box::into_raw(is) as *mut c_void
}

// [spec:hfst:def:libhfst-c.hfst-input-stream-free-fn]
// [spec:hfst:sem:libhfst-c.hfst-input-stream-free-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_input_stream_free(input_stream: *mut c_void) {
    assert!(!input_stream.is_null());
    unsafe {
        drop(Box::from_raw(input_stream as *mut HfstInputStream));
    }
}

// [spec:hfst:def:libhfst-c.hfst-input-stream-close-fn]
// [spec:hfst:sem:libhfst-c.hfst-input-stream-close-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_input_stream_close(his: *mut c_void) {
    let inp = his as *mut HfstInputStream;
    unsafe {
        (*inp).close();
    }
}

// [spec:hfst:def:libhfst-c.hfst-input-stream-is-eof-fn]
// [spec:hfst:sem:libhfst-c.hfst-input-stream-is-eof-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_input_stream_is_eof(his: *mut c_void) -> bool {
    let inp = his as *mut HfstInputStream;
    unsafe { (*inp).is_eof() }
}

// [spec:hfst:def:libhfst-c.hfst-input-stream-is-bad-fn]
// [spec:hfst:sem:libhfst-c.hfst-input-stream-is-bad-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_input_stream_is_bad(his: *mut c_void) -> bool {
    let inp = his as *mut HfstInputStream;
    unsafe { (*inp).is_bad() }
}

// [spec:hfst:def:libhfst-c.hfst-transducer-from-stream-fn]
// [spec:hfst:sem:libhfst-c.hfst-transducer-from-stream-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_transducer_from_stream(his: *mut c_void) -> *mut c_void {
    // C++: new HfstTransducer(*inp). Reading a transducer from a
    // HfstInputStream depends on the binary HFST stream reader, which is
    // deferred in the ported facade, so we cannot construct one here yet.
    let _inp = his as *mut HfstInputStream;
    std::ptr::null_mut()
}

// [spec:hfst:def:libhfst-c.hfst-lookup-fn]
// [spec:hfst:sem:libhfst-c.hfst-lookup-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_lookup(this: *mut c_void, s: *const c_char) -> *mut c_void {
    let fsa = this as *mut HfstTransducer;
    let input = unsafe { std::ffi::CStr::from_ptr(s) }.to_string_lossy();
    let rv = unsafe { (*fsa).lookup_string(&input, -1, 0.0) };
    rv as *mut c_void
}

// [spec:hfst:def:libhfst-c.hfst-lookup-results-fn]
// [spec:hfst:sem:libhfst-c.hfst-lookup-results-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_lookup_results(
    holps: *mut c_void,
    results: *mut *mut c_char,
    weights: *mut f32,
) -> usize {
    let v = holps as *mut HfstOneLevelPaths;
    let mut i: usize = 0;
    unsafe {
        for it in (*v).iter() {
            // anders: where will this malloc be free()'d? (preserved from C++)
            let result = malloc_cstring(&it.second);
            *results.add(i) = result;
            *weights.add(i) = it.first;
            i += 1;
        }
    }
    i
}

// Create an iterator over the paths in holps, put it on the heap, and return the
// pointer to it.
// [spec:hfst:def:libhfst-c.hfst-lookup-iterator-fn]
// [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_lookup_iterator(holps: *mut c_void) -> *mut ResultIterator {
    let v = holps as *mut HfstOneLevelPaths;
    let paths: Vec<HfstOneLevelPath> = unsafe { (*v).iter().cloned().collect() };
    let begin = Box::into_raw(Box::new(0usize)) as *mut c_void;
    let end = Box::into_raw(Box::new(paths)) as *mut c_void;
    Box::into_raw(Box::new(ResultIterator { begin, end }))
}

// [spec:hfst:def:libhfst-c.hfst-lookup-iterator-done-fn]
// [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-done-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_lookup_iterator_done(it: *mut ResultIterator) -> bool {
    unsafe {
        let cursor = (*it).begin as *mut usize;
        let paths = (*it).end as *mut Vec<HfstOneLevelPath>;
        *cursor >= (*paths).len()
    }
}

// [spec:hfst:def:libhfst-c.hfst-lookup-iterator-free-fn]
// [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-free-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_lookup_iterator_free(it: *mut ResultIterator) {
    unsafe {
        drop(Box::from_raw((*it).begin as *mut usize));
        drop(Box::from_raw((*it).end as *mut Vec<HfstOneLevelPath>));
        drop(Box::from_raw(it));
    }
}

// [spec:hfst:def:libhfst-c.hfst-lookup-iterator-value-fn]
// [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-value-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_lookup_iterator_value(
    it: *mut ResultIterator,
    s: *mut *mut c_char,
    weight: *mut f32,
) {
    unsafe {
        let cursor = *((*it).begin as *mut usize);
        let paths = &*((*it).end as *mut Vec<HfstOneLevelPath>);
        let pair = &paths[cursor];
        // the float can just be copied in
        *weight = pair.first;
        // the caller is responsible for free()ing this returned string
        *s = malloc_cstring(&pair.second);
    }
}

// [spec:hfst:def:libhfst-c.hfst-lookup-iterator-next-fn]
// [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-next-fn]
#[unsafe(no_mangle)]
pub extern "C" fn hfst_lookup_iterator_next(it: *mut ResultIterator) {
    unsafe {
        let cursor = (*it).begin as *mut usize;
        *cursor += 1;
    }
}
