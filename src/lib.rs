use libc::{c_char, c_void};
use std::{ffi::CStr, path::Path, sync::Arc};

unsafe extern "C" {
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

#[repr(transparent)]
pub struct CVec {
    vec: Vec<String>,
}

impl CVec {
    pub fn new() -> CVec {
        CVec { vec: Vec::new() }
    }

    pub fn push(&mut self, s: String) {
        self.vec.push(s);
    }

    pub fn into_inner(self) -> Vec<String> {
        self.vec
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
        // println!("Loading transducer from {:?}", path.as_ref());
        let buf = std::fs::read(path).unwrap();
        Self::from_bytes(buf)
    }

    pub fn from_bytes(mut buf: Vec<u8>) -> Transducer {
        // println!("Loading transducer from bytes");
        buf.shrink_to_fit();
        assert!(buf.len() == buf.capacity());
        let vec_ptr = buf.as_mut_ptr();
        let len = buf.len();
        std::mem::forget(buf);

        // println!("Creating transducer");
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
        // println!("Looking up tags: {:?}", input);
        let mut tags = CVec::new();

        extern "C" fn callback(tags: *mut CVec, it: *const u8, it_size: usize) {
            let slice = unsafe { std::slice::from_raw_parts(it, it_size) };
            let s = std::str::from_utf8(slice).unwrap();
            unsafe { tags.as_mut().unwrap().push(s.to_string()) };
        }

        // println!("Looking up tags: {:?}", input);
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

        let tags = tags.into_inner();
        // println!("Tags: {:?}", tags);

        // tags.sort();
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
        // println!("Something: {:?}", t.tokenize("an ape sat in a car"));
    }
}
