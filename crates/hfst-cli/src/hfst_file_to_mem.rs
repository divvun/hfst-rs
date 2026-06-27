//! Faithful 1:1 port of tools/src/hfst-file-to-mem.cc — read a whole file (or
//! stdin) into a freshly malloc'd, NUL-terminated C string. Based on a function
//! in foma written by Mans Hulden. The raw libc FILE*/malloc machinery and the
//! 'error(EXIT_FAILURE, ...)'  failure paths are preserved bug-for-bug.

use libc::c_char;

use crate::hfst_commandline::error;

const EXIT_FAILURE: i32 = 1;

// [spec:hfst:def:hfst-file-to-mem.hfst-stdin-to-mem-fn]
// [spec:hfst:sem:hfst-file-to-mem.hfst-stdin-to-mem-fn]
pub fn hfst_stdin_to_mem() -> *mut c_char {
    unsafe {
        let maxbytes: usize = 1000000;
        let mut numbytes: usize = 0;
        // fix: slow
        let buffer = libc::malloc(maxbytes * std::mem::size_of::<c_char>()) as *mut c_char;
        if buffer.is_null() {
            error(
                EXIT_FAILURE,
                0,
                "Error allocating memory to read file '<stdin>'\n",
            );
            return std::ptr::null_mut();
        }

        loop {
            *(buffer.add(numbytes)) = libc::fgetc(stdin_file()) as c_char;
            if libc::feof(stdin_file()) != 0 {
                *(buffer.add(numbytes)) = b'\0' as c_char;
                break;
            }
            numbytes += 1;
            if numbytes >= maxbytes {
                error(
                    EXIT_FAILURE,
                    0,
                    "Error reading file '<stdin>' to memory, not enough memory\n",
                );
                return std::ptr::null_mut();
            }
        }
        buffer
    }
}

// Based on a function in foma written by Mans Hulden.
// Read the file 'filename' to memory and return a pointer to it.
// Filename "<stdin>" uses stdin for reading.
// Returns NULL if file cannot be opened or read or memory cannot be allocated.

// [spec:hfst:def:hfst-file-to-mem.hfst-file-to-mem-fn]
// [spec:hfst:sem:hfst-file-to-mem.hfst-file-to-mem-fn]
pub fn hfst_file_to_mem(filename: &str) -> *mut c_char {
    unsafe {
        if filename == "<stdin>" {
            return hfst_stdin_to_mem();
        }

        let c_filename = std::ffi::CString::new(filename).unwrap();
        let mode = std::ffi::CString::new("rb").unwrap();
        let infile = libc::fopen(c_filename.as_ptr(), mode.as_ptr());
        if infile.is_null() {
            error(
                EXIT_FAILURE,
                0,
                &format!("Error opening file '{}'\n", filename),
            );
            return std::ptr::null_mut();
        }
        libc::fseek(infile, 0, libc::SEEK_END);
        let numbytes: usize = libc::ftell(infile) as usize;
        libc::fseek(infile, 0, libc::SEEK_SET);

        let buffer = libc::malloc((numbytes + 1) * std::mem::size_of::<c_char>()) as *mut c_char;
        if buffer.is_null() {
            error(
                EXIT_FAILURE,
                0,
                &format!("Error allocating memory to read file '{}'\n", filename),
            );
            return std::ptr::null_mut();
        }
        if libc::fread(
            buffer as *mut libc::c_void,
            std::mem::size_of::<c_char>(),
            numbytes,
            infile,
        ) != numbytes
        {
            error(
                EXIT_FAILURE,
                0,
                &format!("Error reading file '{}' to memory\n", filename),
            );
            return std::ptr::null_mut();
        }
        libc::fclose(infile);
        *(buffer.add(numbytes)) = b'\0' as c_char;
        buffer
    }
}

// C reads the global 'stdin' macro directly. Same 'extern static mut stdin'
// shape as the rest of the crate (see 'hfst_getopt.rs' / 'globals.rs').
fn stdin_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stdinp")]
        static mut stdin: *mut libc::FILE;
    }
    unsafe { stdin }
}
