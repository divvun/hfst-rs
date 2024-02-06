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
