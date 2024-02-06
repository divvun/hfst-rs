use std::ffi::CStr;

extern "C" {
    fn hfst_tokenize(input_data: *const u8) -> *const i8;
}

pub fn run(input: &str) -> String {
    let output = unsafe { hfst_tokenize(input.as_ptr()) };
    let bytes = unsafe { CStr::from_ptr(output).to_bytes() };
    
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::run;

    #[test]
    fn test() {
        let input = "test";
        println!("Something: {}", run(input));
    }
}
