// Exercises the C API (libhfst_c) lookup + result-iterator path: build a small
// acceptor with the facade, hand its raw pointer to hfst_lookup, then walk the
// ResultIterator the way a C caller would.
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;

use hfst::hfst_data_types::ImplementationType::{HFST_OLW_TYPE, TROPICAL_OPENFST_TYPE};
use hfst::hfst_transducer::HfstTransducer;
use hfst_c::{
    hfst_lookup, hfst_lookup_iterator, hfst_lookup_iterator_done, hfst_lookup_iterator_free,
    hfst_lookup_iterator_next, hfst_lookup_iterator_value,
};

fn main() {
    // Build an acceptor for "a" via the facade, convert it to optimized-lookup
    // form (lookup requires HFST_OL/OLW, exactly as in C++), and leak it as a
    // raw pointer the way a C caller would hold an opaque transducer handle.
    let mut fsa = HfstTransducer::new_symbol("a", TROPICAL_OPENFST_TYPE);
    fsa.convert(HFST_OLW_TYPE, String::new());
    let t = Box::into_raw(Box::new(fsa));

    let input = CString::new("a").unwrap();
    let holps = hfst_lookup(t as *mut c_void, input.as_ptr());
    assert!(!holps.is_null(), "hfst_lookup returned null");

    let it = hfst_lookup_iterator(holps);
    let mut count = 0;
    let mut last = String::new();
    while !hfst_lookup_iterator_done(it) {
        let mut s: *mut c_char = ptr::null_mut();
        let mut w: f32 = 0.0;
        hfst_lookup_iterator_value(it, &mut s as *mut *mut c_char, &mut w as *mut f32);
        assert!(!s.is_null(), "iterator value string is null");
        last = unsafe { std::ffi::CStr::from_ptr(s) }
            .to_string_lossy()
            .into_owned();
        unsafe { libc::free(s as *mut c_void) };
        count += 1;
        hfst_lookup_iterator_next(it);
    }
    hfst_lookup_iterator_free(it);

    assert!(count >= 1, "expected at least one lookup result for 'a'");
    println!("C API OK: hfst_lookup('a') -> {count} result(s), last = {last:?}");
}
