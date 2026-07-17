//! Rust-native reimplementation of HFST's `getopt_long` fallback (was a 1:1 port
//! of tools/src/hfst-getopt.cc). The parser state — `optarg`/`optopt`/`optind`
//! and the argument-permuting accumulators — lives in a [`Getopt`] value that a
//! tool's `parse_options` owns and threads through the getopt loop and the
//! `crate::inc` case handlers (the idiomatic replacement for the former
//! file-scope `static mut` globals). The argument-permuting behaviour
//! (non-option arguments shuffled to the tail of `args`, `optind` left pointing
//! at the first of them) is preserved so each tool's `while
//! getopt_long(...) != -1` loop is unchanged.

pub const NO_ARGUMENT: i32 = 0;
pub const REQUIRED_ARGUMENT: i32 = 1;
pub const OPTIONAL_ARGUMENT: i32 = 2;

// [spec:hfst:def:hfst-getopt.option]
pub struct GetOpt {
    pub name: &'static str,
    pub has_arg: i32,
    pub val: i32,
}

/// The getopt parser state, read by a tool after each `getopt_long` call.
/// `optind` is a 1-based index into `args` (args[0] is the program name);
/// `optarg` carries the option argument of the last returned option.
pub struct Getopt {
    pub optarg: Option<String>,
    pub optopt: i32,
    pub optind: usize,
    // Accumulators for the permutation: option tokens (+ their separate-word
    // values) versus the free (non-option) arguments.
    free_arguments: Vec<String>,
    other_arguments: Vec<String>,
}

impl Default for Getopt {
    fn default() -> Getopt {
        Getopt {
            optarg: None,
            optopt: 0,
            optind: 1,
            free_arguments: Vec::new(),
            other_arguments: Vec::new(),
        }
    }
}

impl Getopt {
    pub fn new() -> Getopt {
        Getopt::default()
    }

    /// The option argument of the last returned option, or None if it took none.
    pub fn optarg_opt(&self) -> Option<String> {
        self.optarg.clone()
    }

    /// The option argument as an owned String (empty when there was none).
    pub fn optarg(&self) -> String {
        self.optarg.clone().unwrap_or_default()
    }

    // Rebuild `args` as [program-name, ...options, ...free] and leave `optind`
    // pointing at the first free argument; the end-of-options return.
    fn finish(&mut self, args: &mut Vec<String>) -> i32 {
        let program = args.first().cloned().unwrap_or_default();
        let mut rebuilt = Vec::with_capacity(args.len());
        rebuilt.push(program);
        rebuilt.append(&mut self.other_arguments);
        let optind = rebuilt.len();
        rebuilt.append(&mut self.free_arguments);
        *args = rebuilt;
        self.optind = optind;
        -1
    }

    // [spec:hfst:def:hfst-getopt.getopt-long-fn]
    // [spec:hfst:sem:hfst-getopt.getopt-long-fn]
    pub fn getopt_long(&mut self, args: &mut Vec<String>, longopts: &[GetOpt]) -> i32 {
        let argc = args.len();
        // skip free arguments: anything not beginning with '-', plus the
        // getopt specials — a lone "-" is an operand (conventionally stdin),
        // and "--" terminates option parsing with the rest as operands.
        loop {
            if self.optind >= argc {
                return self.finish(args);
            }
            if args[self.optind] == "--" {
                self.optind += 1;
                while self.optind < argc {
                    self.free_arguments.push(args[self.optind].clone());
                    self.optind += 1;
                }
                return self.finish(args);
            }
            if args[self.optind].as_bytes().first() != Some(&b'-') || args[self.optind] == "-" {
                self.free_arguments.push(args[self.optind].clone());
                self.optind += 1;
            } else {
                break;
            }
        }

        self.other_arguments.push(args[self.optind].clone());

        // work on a copy since we are possibly splitting the argument at '='
        let token = args[self.optind].clone();
        // skip initial '-' signs
        let stripped = token.trim_start_matches('-');

        // empty arg string of dashes beyond the specials (e.g. "---")
        if stripped.is_empty() {
            self.optopt = -2;
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
                self.optind += 1;
                if opt.has_arg == NO_ARGUMENT {
                    if eq_used {
                        eprintln!("warning: argument ignored for option '--{}'", opt.name);
                    }
                    return opt.val;
                } else if opt.has_arg == REQUIRED_ARGUMENT || opt.has_arg == OPTIONAL_ARGUMENT {
                    if let Some(value) = eq_value {
                        self.optarg = Some(value);
                        return opt.val;
                    }
                    // no inline value: the next word is the argument
                    if self.optind >= argc {
                        if opt.has_arg == REQUIRED_ARGUMENT {
                            self.optopt = opt.val;
                            return b':' as i32;
                        } else {
                            self.optopt = 0;
                            return opt.val;
                        }
                    }
                    if opt.has_arg == REQUIRED_ARGUMENT {
                        self.optarg = Some(args[self.optind].clone());
                        self.other_arguments.push(args[self.optind].clone());
                        self.optind += 1;
                        return opt.val;
                    } else {
                        self.optopt = 0;
                        return opt.val;
                    }
                } else {
                    // this should not happen
                    return 0;
                }
            }
        }

        // GNU short-option-with-attached-argument: '-Wall' is '-W' with the
        // argument 'all' when the token has exactly one leading dash and 'W'
        // is an argument-taking option. (The system getopt_long the C tools
        // normally used did this; the shipped fallback this module ports did
        // not, so Giella invocations like 'hfst-lexc -Wall' relied on it.)
        if token.as_bytes().first() == Some(&b'-')
            && token.as_bytes().get(1) != Some(&b'-')
            && name.chars().count() > 1
        {
            for opt in longopts {
                if opt.val == first_char && opt.has_arg != NO_ARGUMENT {
                    self.optind += 1;
                    // everything after the option letter, including any '='
                    // (GNU keeps it verbatim in optarg for attached args)
                    self.optarg = Some(stripped[1..].to_string());
                    return opt.val;
                }
            }
        }

        // no match found
        self.optind += 1;
        self.optopt = if short_option { first_char } else { -2 };
        b'?' as i32
    }
}
