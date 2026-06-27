//! Faithful 1:1 port of tools/src/hfst-getopt.cc — HFST's own 'getopt_long'
//! fallback (used where the system getopt is unavailable). The Rust tools use
//! this implementation directly. Quirks (argv reordering, strdup leaks, the
//! 'optopt = NULL' int assignments) are preserved bug-for-bug.

use libc::{c_char, c_int};

pub const NO_ARGUMENT: c_int = 0;
pub const REQUIRED_ARGUMENT: c_int = 1;
pub const OPTIONAL_ARGUMENT: c_int = 2;

// [spec:hfst:def:hfst-getopt.option]
#[repr(C)]
pub struct Option {
    pub name: *const c_char,
    pub has_arg: c_int,
    pub flag: *mut c_int,
    pub val: c_int,
}

// The C file-scope globals. 'optarg'/'optopt'/'optind' are read by every tool.
pub static mut OPTARG: *mut c_char = std::ptr::null_mut();
pub static mut OPTOPT: c_int = 0;
pub static mut OPTIND: c_int = 1;

pub static mut FREE_ARGUMENTS: Vec<*mut c_char> = Vec::new();
pub static mut OTHER_ARGUMENTS: Vec<*mut c_char> = Vec::new();

// The Vec globals are reached through 'addr_of_mut!' to avoid the edition-2024
// 'static_mut_refs' hard error (same convention as the library crate). The
// scalar/pointer globals are read/written by value, which needs no reference.
fn free_arguments() -> &'static mut Vec<*mut c_char> {
    unsafe { &mut *std::ptr::addr_of_mut!(FREE_ARGUMENTS) }
}
fn other_arguments() -> &'static mut Vec<*mut c_char> {
    unsafe { &mut *std::ptr::addr_of_mut!(OTHER_ARGUMENTS) }
}

// [spec:hfst:def:hfst-getopt.getopt-long-fn]
// [spec:hfst:sem:hfst-getopt.getopt-long-fn]
//
// 'longindex' is accepted to match the signature but, as in the C, never read.
pub unsafe fn getopt_long(
    argc: c_int,
    argv: *mut *mut c_char,
    _optstring: *const c_char,
    mut longopts: *const Option,
    _longindex: *mut c_int,
) -> c_int {
    unsafe {
        // check that there are more args
        if OPTIND > (argc - 1) {
            let mut i: u32 = 1;
            for it in other_arguments().iter() {
                *argv.offset(i as isize) = *it;
                i += 1;
            }
            OPTIND = i as c_int;
            for it in free_arguments().iter() {
                *argv.offset(i as isize) = *it;
                i += 1;
            }
            return -1;
        }

        // skip free arguments
        while *(*argv.offset(OPTIND as isize)) != b'-' as c_char {
            free_arguments().push(*argv.offset(OPTIND as isize));
            OPTIND += 1;
            if OPTIND > (argc - 1) {
                let mut i: u32 = 1;
                for it in other_arguments().iter() {
                    *argv.offset(i as isize) = *it;
                    i += 1;
                }
                OPTIND = i as c_int;
                for it in free_arguments().iter() {
                    *argv.offset(i as isize) = *it;
                    i += 1;
                }
                return -1;
            }
        }

        other_arguments().push(*argv.offset(OPTIND as isize));

        // strdup because we are possibly modifying the argument
        let arg0: *mut c_char = libc::strdup(*argv.offset(OPTIND as isize));
        let mut arg: *mut c_char = arg0; // free() should be called at the end...

        // skip initial '-' signs
        while *arg == b'-' as c_char {
            arg = arg.offset(1);
        }

        // empty arg string
        if *arg == b'\0' as c_char {
            OPTOPT = -2;
            return b'?' as c_int;
        }

        // whether arg is used in its short form: -f(=bar)
        let mut short_option = false;
        arg = arg.offset(1);
        if *arg == b'\0' as c_char || *arg == b'=' as c_char {
            short_option = true;
        }
        arg = arg.offset(-1);

        // whether option argument is given after an '=' sign (--foo=bar, -f=bar)
        let mut eq_used = false;
        let mut argptr: *mut c_char = arg; // points to the char after '=' if eq_used
        while *argptr != b'\0' as c_char {
            if *argptr == b'=' as c_char {
                *argptr = b'\0' as c_char; // change '=' into '\0' for easier strcmp
                argptr = argptr.offset(1);
                eq_used = true;
                break;
            }
            argptr = argptr.offset(1);
        }

        // Go through all possible option strings
        while (*longopts).name != std::ptr::null() {
            // match found, short or long format
            if libc::strcmp((*longopts).name, arg) == 0
                || (short_option && (*longopts).val == *arg as c_int)
            {
                OPTIND += 1;
                // no argument
                if (*longopts).has_arg == NO_ARGUMENT {
                    // argument given for an option that does not take one
                    if eq_used {
                        libc::fprintf(
                            stderr_file(),
                            c"warning: argument ignored for option '--%s'\n".as_ptr(),
                            (*longopts).name,
                        );
                    }
                    return (*longopts).val;
                }
                // required argument
                else if (*longopts).has_arg == REQUIRED_ARGUMENT
                    || (*longopts).has_arg == OPTIONAL_ARGUMENT
                {
                    if eq_used {
                        // we already have a pointer to the argument
                        OPTARG = libc::strdup(argptr);
                        argptr = argptr.offset(-1);
                        *argptr = b'=' as c_char; // change '\0' back to '='
                        return (*longopts).val;
                    }
                    // no more args, argument thus missing
                    if OPTIND > (argc - 1) {
                        if (*longopts).has_arg == REQUIRED_ARGUMENT {
                            OPTOPT = (*longopts).val;
                            return b':' as c_int;
                        } else {
                            OPTOPT = 0; // C: optopt = NULL;
                            return (*longopts).val;
                        }
                    }
                    // next arg is required argument (cannot be optional argument)
                    if (*longopts).has_arg == REQUIRED_ARGUMENT {
                        OPTARG = libc::strdup(*argv.offset(OPTIND as isize));
                        other_arguments().push(*argv.offset(OPTIND as isize));
                        OPTIND += 1;
                        return (*longopts).val;
                    } else {
                        OPTOPT = 0; // C: optopt = NULL;
                        return (*longopts).val;
                    }
                }
                // this should not happen
                else {
                    return 0;
                }
            }
            longopts = longopts.offset(1);
        }

        // no match found
        OPTIND += 1;
        OPTOPT = -2;
        if short_option {
            OPTOPT = *arg as c_int;
        }
        b'?' as c_int
    }
}

// 'stderr' as a FILE* for the faithful fprintf above.
fn stderr_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stderrp")]
        static mut stderr: *mut libc::FILE;
    }
    unsafe { stderr }
}
