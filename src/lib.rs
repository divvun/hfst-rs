use std::ffi::CStr;

extern "C" {
    fn hfst_tokenize(
        input_data: *const u8,
        input_size: usize,
        tokenizer: *const u8,
        tokenizer_size: usize,
    ) -> *const i8;
}

pub fn run(input: &str, tokenizer: &str) -> String {
    let output = unsafe {
        hfst_tokenize(
            input.as_ptr(),
            input.len(),
            tokenizer.as_ptr(),
            tokenizer.len(),
        )
    };
    let bytes = unsafe { CStr::from_ptr(output).to_bytes() };

    String::from_utf8(bytes.to_vec()).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::run;

    #[test]
    fn test() {
        println!(
            "Something: {}",
            run("an ape sat in a car", "gramcheck.pmhfst")
        );
    }
}
