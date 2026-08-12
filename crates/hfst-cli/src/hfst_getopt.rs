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
    // The unscanned tail of a clustered short-option token ('q' while '-wq' is
    // being taken apart); glibc's `nextchar`.
    cluster: Option<String>,
}

impl Default for Getopt {
    fn default() -> Getopt {
        Getopt {
            optarg: None,
            optopt: 0,
            optind: 1,
            free_arguments: Vec::new(),
            other_arguments: Vec::new(),
            cluster: None,
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

    // Park the rest of a short-option token for the next getopt_long call.
    fn park_cluster(&mut self, tail: &str) {
        self.cluster = if tail.is_empty() {
            None
        } else {
            Some(tail.to_string())
        };
    }

    /// Take the next short option off a clustered token, `rest` being what is
    /// left of it. The letter is resolved through the same `val` scan of the
    /// long table that a lone `-f` uses, so the two spellings can never drift.
    fn scan_cluster(&mut self, args: &mut Vec<String>, longopts: &[GetOpt], rest: &str) -> i32 {
        let argc = args.len();
        let mut letters = rest.chars();
        let Some(letter) = letters.next() else {
            // Unreachable: an exhausted cluster is dropped rather than parked,
            // and the caller only enters with two characters or more.
            return self.finish(args);
        };
        let tail = letters.as_str();
        let val = letter as i32;

        let Some(opt) = longopts.iter().find(|opt| opt.val == val) else {
            // An unknown letter is reported as itself, and scanning resumes
            // after it: '-wZq' names 'Z', not the whole token.
            self.park_cluster(tail);
            self.optopt = val;
            return b'?' as i32;
        };

        if opt.has_arg == NO_ARGUMENT {
            self.park_cluster(tail);
            return opt.val;
        }
        if opt.has_arg != REQUIRED_ARGUMENT && opt.has_arg != OPTIONAL_ARGUMENT {
            // this should not happen
            return 0;
        }
        // Whatever is left of the token is the argument, so '-n2' and '-wn2'
        // both give 'n' the value "2" and no cluster survives it.
        if !tail.is_empty() {
            self.optarg = Some(tail.to_string());
            return opt.val;
        }
        // Only a required argument reaches out to the next argv word; an
        // optional one has to be attached, so a bare '-p' leaves the operand
        // that follows it alone.
        if opt.has_arg == OPTIONAL_ARGUMENT {
            self.optopt = 0;
            return opt.val;
        }
        if self.optind >= argc {
            self.optopt = opt.val;
            return b':' as i32;
        }
        self.optarg = Some(args[self.optind].clone());
        self.other_arguments.push(args[self.optind].clone());
        self.optind += 1;
        opt.val
    }

    // [spec:hfst:def:hfst-getopt.getopt-long-fn]
    // [spec:hfst:sem:hfst-getopt.getopt-long-fn]
    pub fn getopt_long(&mut self, args: &mut Vec<String>, longopts: &[GetOpt]) -> i32 {
        // glibc clears optarg on entry to every getopt_long call. Without that,
        // an OPTIONAL_ARGUMENT option given with no '=value' (--colour,
        // hfst-summarize -S, --pipe-mode) would see the *previous* option's
        // argument through optarg_opt() and misread it as its own.
        self.optarg = None;
        // A half-scanned token outranks the argv cursor: its remaining letters
        // are not argv elements, so the end-of-argv test below would drop them
        // when the cluster is the last word ('hfst-optimized-lookup FILE -wq').
        if let Some(rest) = self.cluster.take() {
            return self.scan_cluster(args, longopts, &rest);
        }
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

        // One leading dash and more than a letter after it is GNU's packed
        // short-option token: either options clustered together ('-wq' is
        // '-w -q') or an option with its argument attached ('-Wall' is
        // '-W all'). The system getopt_long the C tools link against does
        // this, so Giella invocations depend on it; the shipped fallback this
        // module ports did neither. The scan runs over `stripped` rather than
        // `name` so an '=' stays verbatim in an attached argument, as GNU
        // keeps it.
        if token.as_bytes().first() == Some(&b'-')
            && token.as_bytes().get(1) != Some(&b'-')
            && name.chars().count() > 1
        {
            self.optind += 1;
            return self.scan_cluster(args, longopts, stripped);
        }

        // no match found
        self.optind += 1;
        self.optopt = if short_option { first_char } else { -2 };
        b'?' as i32
    }
}
