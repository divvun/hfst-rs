use libc::{c_char, c_void};
use std::{ffi::CStr, path::Path};

extern "C" {
    fn hfst_tokenize(
        tokenizer: *const c_void,
        input_data: *const c_char,
        input_size: usize,
    ) -> *const c_char;
    fn hfst_make_tokenizer(tokenizer: *const u8, tokenizer_size: usize) -> *const c_void;
    fn hfst_tokenizer_free(ptr: *const c_void);
    fn hfst_free(ptr: *const c_void);
    fn hfst_transducer_free(ptr: *const c_void);
    fn hfst_transducer_new(analyzer_bytes: *const u8, analyzer_size: usize) -> *const c_void;
    fn hfst_transducer_lookup_tags(
        analyzer: *const c_void,
        input: *const c_char,
        input_size: usize,
        time_cutoff: f64,
        tags: *mut CVec,
        callback: extern "C" fn(tags: *mut CVec, it: *const u8, it_size: usize),
    );
}

#[repr(C)]
pub struct CVec {
    ptr: *mut c_void,
    len: usize,
    cap: usize,
}

impl CVec {
    pub fn new() -> CVec {
        let mut v = vec![];
        let cvec = CVec {
            ptr: v.as_mut_ptr(),
            len: v.len(),
            cap: v.capacity(),
        };
        std::mem::forget(v);
        cvec
    }

    pub unsafe fn inner(&mut self) -> Vec<String> {
        Vec::from_raw_parts(self.ptr as _, self.len, self.cap)
    }

    pub fn from(mut v: Vec<String>) -> CVec {
        let cvec = CVec {
            ptr: v.as_mut_ptr() as _,
            len: v.len(),
            cap: v.capacity(),
        };
        std::mem::forget(v);
        cvec
    }
}

impl Drop for CVec {
    fn drop(&mut self) {
        drop(unsafe { Vec::from_raw_parts(self.ptr, self.len, self.cap) });
    }
}

pub struct Transducer {
    ptr: *const c_void,
}

impl Transducer {
    pub fn new<P: AsRef<Path>>(path: P) -> Transducer {
        let buf = std::fs::read(path).unwrap();
        let ptr = unsafe { hfst_transducer_new(buf.as_ptr(), buf.len()) };
        Self { ptr }
    }

    pub fn lookup_tags(&self, input: &str) -> Vec<String> {
        let mut tags = CVec::new();

        extern "C" fn callback(tags: *mut CVec, it: *const u8, it_size: usize) {
            let slice = unsafe { std::slice::from_raw_parts(it, it_size) };
            let s = std::str::from_utf8(slice).unwrap();

            let mut vec = unsafe { tags.as_mut().unwrap().inner() };
            vec.push(s.to_string());
            unsafe {
                *tags = CVec::from(vec);
            }
        }

        unsafe {
            hfst_transducer_lookup_tags(
                self.ptr,
                input.as_ptr() as _,
                input.len(),
                10.0,
                &mut tags,
                callback,
            );
        }

        let mut tags = unsafe { tags.inner() };

        tags.sort();
        tags
    }
}

impl Drop for Transducer {
    fn drop(&mut self) {
        unsafe { hfst_transducer_free(self.ptr) };
    }
}

pub struct Tokenizer {
    ptr: *const c_void,
}

impl Drop for Tokenizer {
    fn drop(&mut self) {
        unsafe { hfst_tokenizer_free(self.ptr) };
    }
}

impl Tokenizer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let buf = std::fs::read(path).unwrap();
        let ptr = unsafe { hfst_make_tokenizer(buf.as_ptr() as _, buf.len()) };

        Ok(Self { ptr })
    }

    pub fn tokenize(&self, input: &str) -> Option<String> {
        let output = unsafe { hfst_tokenize(self.ptr, input.as_ptr() as _, input.len()) };

        if output.is_null() {
            return None;
        }

        let bytes = unsafe { CStr::from_ptr(output).to_bytes() };

        let out = String::from_utf8(bytes.to_vec()).unwrap();
        unsafe { hfst_free(output as _) };

        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let t = Tokenizer::new("tokeniser-gramcheck-gt-desc.pmhfst").unwrap();
        println!("Something: {:?}", t.tokenize("an ape sat in a car"));
    }
}
