use libc::c_char;
use std::ffi::CStr;

extern "C" {
    fn hfst_tokenize(
        input_data: *const c_char,
        input_size: usize,
        tokenizer: *const c_char,
        tokenizer_size: usize,
    ) -> *const c_char;
    fn hfst_free_cstr(c_str: *const c_char);
}

pub fn run(input: &str, tokenizer: &str) -> String {
    let output = unsafe {
        hfst_tokenize(
            input.as_ptr() as _,
            input.len(),
            tokenizer.as_ptr() as _,
            tokenizer.len(),
        )
    };
    let bytes = unsafe { CStr::from_ptr(output).to_bytes() };

    let out = String::from_utf8(bytes.to_vec()).unwrap();
    unsafe { hfst_free_cstr(output) };

    out
}

#[cfg(test)]
mod tests {
    use crate::run;

    #[test]
    fn test() {
        println!(
            "Something: {}",
            run("an ape sat in a car", "tokeniser-gramcheck-gt-desc.pmhfst")
        );
    }
}
