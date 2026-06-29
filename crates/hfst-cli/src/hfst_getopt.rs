//! Rust-native reimplementation of HFST's `getopt_long` fallback (was a 1:1 port
//! of tools/src/hfst-getopt.cc). It works directly on the program's `Vec<String>`
//! arguments: the option tables, the `OPTARG`/`OPTOPT`/`OPTIND` module state, and
//! the argument-permuting behaviour (non-option arguments shuffled to the tail of
//! `args`, `OPTIND` left pointing at the first of them) are preserved so each
//! tool's `while getopt_long(...) != -1` loop is unchanged.

pub const NO_ARGUMENT: i32 = 0;
pub const REQUIRED_ARGUMENT: i32 = 1;
pub const OPTIONAL_ARGUMENT: i32 = 2;

// [spec:hfst:def:hfst-getopt.option]
pub struct GetOpt {
    pub name: &'static str,
    pub has_arg: i32,
    pub val: i32,
}

// The file-scope getopt state, read by every tool after each call. `OPTIND` is a
// 1-based index into `args` (args[0] is the program name); `OPTARG` carries the
// option argument of the last returned option.
pub static mut OPTARG: Option<String> = None;
pub static mut OPTOPT: i32 = 0;
pub static mut OPTIND: usize = 1;

/// The option argument of the last returned option, or None if it took none.
pub fn optarg_opt() -> Option<String> {
    unsafe { (*std::ptr::addr_of!(OPTARG)).clone() }
}

/// The option argument as an owned String (empty when there was none).
pub fn optarg() -> String {
    optarg_opt().unwrap_or_default()
}

// Accumulators for the permutation: option tokens (+ their separate-word values)
// versus the free (non-option) arguments. Reached via addr_of_mut! to stay clear
// of the edition-2024 static_mut_refs error.
static mut FREE_ARGUMENTS: Vec<String> = Vec::new();
static mut OTHER_ARGUMENTS: Vec<String> = Vec::new();

fn free_arguments() -> &'static mut Vec<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(FREE_ARGUMENTS) }
}
fn other_arguments() -> &'static mut Vec<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(OTHER_ARGUMENTS) }
}

// Rebuild `args` as [program-name, ...options, ...free] and leave OPTIND pointing
// at the first free argument; the end-of-options return.
fn finish(args: &mut Vec<String>) -> i32 {
    let program = args.first().cloned().unwrap_or_default();
    let mut rebuilt = Vec::with_capacity(args.len());
    rebuilt.push(program);
    rebuilt.append(other_arguments());
    let optind = rebuilt.len();
    rebuilt.append(free_arguments());
    *args = rebuilt;
    unsafe {
        OPTIND = optind;
    }
    -1
}

// [spec:hfst:def:hfst-getopt.getopt-long-fn]
// [spec:hfst:sem:hfst-getopt.getopt-long-fn]
pub fn getopt_long(args: &mut Vec<String>, longopts: &[GetOpt]) -> i32 {
    let argc = args.len();
    unsafe {
        // skip free arguments (anything not beginning with '-')
        loop {
            if OPTIND >= argc {
                return finish(args);
            }
            if args[OPTIND].as_bytes().first() != Some(&b'-') {
                free_arguments().push(args[OPTIND].clone());
                OPTIND += 1;
            } else {
                break;
            }
        }

        other_arguments().push(args[OPTIND].clone());

        // work on a copy since we are possibly splitting the argument at '='
        let token = args[OPTIND].clone();
        // skip initial '-' signs
        let stripped = token.trim_start_matches('-');

        // empty arg string (e.g. "-" or "--")
        if stripped.is_empty() {
            OPTOPT = -2;
            return b'?' as i32;
        }

        // split name from an inline value at '=' (--foo=bar, -f=bar)
        let (name, eq_value) = match stripped.find('=') {
            Some(eq) => (&stripped[..eq], Some(stripped[eq + 1..].to_string())),
            None => (stripped, None),
        };
        let eq_used = eq_value.is_some();
        // short form: the option name is a single character (-f / -f=bar)
        let short_option = name.chars().count() == 1;
        let first_char = name
            .as_bytes()
            .first()
            .copied()
            .map(|b| b as i32)
            .unwrap_or(0);

        // Go through all possible option strings
        for opt in longopts {
            if opt.name == name || (short_option && opt.val == first_char) {
                OPTIND += 1;
                if opt.has_arg == NO_ARGUMENT {
                    if eq_used {
                        eprint!("warning: argument ignored for option '--{}'\n", opt.name);
                    }
                    return opt.val;
                } else if opt.has_arg == REQUIRED_ARGUMENT || opt.has_arg == OPTIONAL_ARGUMENT {
                    if let Some(value) = eq_value {
                        OPTARG = Some(value);
                        return opt.val;
                    }
                    // no inline value: the next word is the argument
                    if OPTIND >= argc {
                        if opt.has_arg == REQUIRED_ARGUMENT {
                            OPTOPT = opt.val;
                            return b':' as i32;
                        } else {
                            OPTOPT = 0;
                            return opt.val;
                        }
                    }
                    if opt.has_arg == REQUIRED_ARGUMENT {
                        OPTARG = Some(args[OPTIND].clone());
                        other_arguments().push(args[OPTIND].clone());
                        OPTIND += 1;
                        return opt.val;
                    } else {
                        OPTOPT = 0;
                        return opt.val;
                    }
                } else {
                    // this should not happen
                    return 0;
                }
            }
        }

        // no match found
        OPTIND += 1;
        OPTOPT = if short_option { first_char } else { -2 };
        b'?' as i32
    }
}
