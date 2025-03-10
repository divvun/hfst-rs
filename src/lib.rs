use libc::{c_char, c_void};
use std::{ffi::CStr, path::Path, sync::Arc};

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
        is_diacritic: bool,
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
    inner: Option<(*mut u8, usize)>,
}

unsafe impl Send for Transducer {}
unsafe impl Sync for Transducer {}

impl Drop for Transducer {
    fn drop(&mut self) {
        unsafe { hfst_transducer_free(self.ptr) };
        if let Some((ptr, len)) = self.inner.take() {
            drop(unsafe { Vec::from_raw_parts(ptr, len, len) });
        }
    }
}

impl Transducer {
    pub fn new<P: AsRef<Path>>(path: P) -> Transducer {
        let buf = std::fs::read(path).unwrap();
        Self::from_bytes(buf)
    }

    pub fn from_bytes(mut buf: Vec<u8>) -> Transducer {
        buf.shrink_to_fit();
        assert!(buf.len() == buf.capacity());
        let vec_ptr = buf.as_mut_ptr();
        let len = buf.len();
        std::mem::forget(buf);

        let ptr = unsafe { hfst_transducer_new(vec_ptr, len) };
        Self {
            ptr,
            inner: Some((vec_ptr, len)),
        }
    }

    pub unsafe fn from_ptr(ptr: *const u8, size: usize) -> Transducer {
        let ptr = unsafe { hfst_transducer_new(ptr, size) };
        Self { ptr, inner: None }
    }

    pub fn lookup_tags(&self, input: &str, is_diacritic: bool) -> Vec<String> {
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
                is_diacritic,
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

pub struct Tokenizer {
    ptr: Arc<*const c_void>,
}

unsafe impl Send for Tokenizer {}
unsafe impl Sync for Tokenizer {}

impl Drop for Tokenizer {
    fn drop(&mut self) {
        if let Some(x) = Arc::get_mut(&mut self.ptr) {
            unsafe { hfst_tokenizer_free(*x) };
        }
    }
}

impl Tokenizer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let buf = std::fs::read(path).unwrap();
        let ptr = unsafe { hfst_make_tokenizer(buf.as_ptr() as _, buf.len()) };

        Ok(Self { ptr: Arc::new(ptr) })
    }

    pub fn tokenize(&self, input: &str) -> Option<String> {
        let output = unsafe { hfst_tokenize(*self.ptr, input.as_ptr() as _, input.len()) };

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
