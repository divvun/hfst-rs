#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-info.cc — the "show or test HFST
//! versions and features" command-line tool. It reads no transducer streams
//! (it only includes inc/globals-common.h); it parses version/feature test
//! options, then prints or tests the library's compiled-in version and
//! features. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options).

use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name, print_more_info,
    print_report_bugs, print_version, verbose_print,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;
use std::collections::BTreeSet;
use std::io::Write;

// Configured-build constants the C tool received from config.h. The reference
// build (config.log) sets HFST_LONGVERSION=300170001, PACKAGE_VERSION="3.17.1",
// PACKAGE_STRING="" and enables HAVE_SFST / HAVE_OPENFST / HAVE_OPENFST_LOG /
// HAVE_FOMA; HAVE_XFSM, HAVE_LEAN_SFST and USE_ICU_UNICODE are not defined.
const HFST_LONGVERSION: i64 = 300170001;
const PACKAGE_STRING: &str = "";
const PACKAGE_VERSION: &str = "3.17.1";
const HAVE_SFST: bool = true;
const HAVE_LEAN_SFST: bool = false;
const HAVE_FOMA: bool = true;
const HAVE_XFSM: bool = false;
const HAVE_OPENFST: bool = true;

// Tool-local state mirroring the C file-scope statics.
static mut MIN_VERSION: i64 = -1;
static mut EXACT_VERSION: i64 = -1;
static mut MAX_VERSION: i64 = -1;
// required_features collected as a set<string>; BTreeSet preserves the
// sorted-iteration order the C++ std::set used.
static mut REQUIRED_FEATURES: Option<BTreeSet<String>> = None;

// strtoul(s, &endptr, 10): parse a leading run of base-10 digits from 's',
// returning the parsed value and the unparsed remainder (the C 'endptr'). Like
// libc strtoul it accepts no digits (value 0, whole string remaining).
fn parse_u64_prefix(s: &str) -> (u64, &str) {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let val = s[..end].parse::<u64>().unwrap_or(0);
    (val, &s[end..])
}

// [spec:hfst:def:hfst-info.parse-version-string-fn]
// [spec:hfst:sem:hfst-info.parse-version-string-fn]
fn parse_version_string(s: &str) -> i64 {
    let (major, endptr) = parse_u64_prefix(s);
    let major = major as i64;
    if endptr.is_empty() {
        return major * 10000 * 10000;
    } else if !endptr.starts_with('.') {
        error(
            EXIT_FAILURE,
            0,
            &format!("cannot parse version string from {}", endptr),
        );
    }
    let s = &endptr[1..];
    let (minor, endptr) = parse_u64_prefix(s);
    let minor = minor as i64;
    if endptr.is_empty() {
        return (major * 10000 * 10000) + (minor * 10000);
    } else if !endptr.starts_with('.') {
        error(
            EXIT_FAILURE,
            0,
            &format!("cannot parse version string from {}", endptr),
        );
    }
    let s = &endptr[1..];
    let (patch, endptr) = parse_u64_prefix(s);
    let patch = patch as i64;
    if endptr.is_empty() {
        return (major * 10000 * 10000) + (minor * 10000) + patch;
    } else {
        error(
            EXIT_FAILURE,
            0,
            &format!("cannot parse version string from {}", endptr),
        );
    }
    -1
}

// [spec:hfst:def:hfst-info.print-usage-fn]
// [spec:hfst:sem:hfst-info.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nshow or test HFST versions and features\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Test features:\n  -a, --atleast-version=MVER   require at least MVER version of HFST\n  -e, --exact-version=EVER     require exactly EVER version of HFST\n  -m, --max-version=UVER       require at most UVER version of HFST\n  -f, --requirefeature=FEAT    require named FEAT support from HFST\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "MVER, EVER or UVER version vectors must be composed of one to three full stop separated runs of digits.\nFEAT should be name of feature supported by HFST, such as SFST, foma or openfst\n\n"
    );
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-info.parse-options-fn]
// [spec:hfst:sem:hfst-info.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.push(getopt::GetOpt {
                name: "atleast-version",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'a' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "exact-version",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'e' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "max-version",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'm' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "require-feature",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'f' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }
            // The C switch handles only a/e/m/f/h/V; every other accepted option
            // (the common -v/-q/-s/-d/-o/--colour) falls through with no action.
            if c == b'a' as i32 {
                MIN_VERSION = parse_version_string(&getopt::optarg());
            } else if c == b'e' as i32 {
                EXACT_VERSION = parse_version_string(&getopt::optarg());
            } else if c == b'm' as i32 {
                MAX_VERSION = parse_version_string(&getopt::optarg());
            } else if c == b'f' as i32 {
                REQUIRED_FEATURES
                    .get_or_insert_with(BTreeSet::new)
                    .insert(getopt::optarg());
            } else if c == b'h' as i32 {
                print_usage();
                return EXIT_SUCCESS;
            } else if c == b'V' as i32 {
                print_version();
                return EXIT_SUCCESS;
            }
        }
        let feature_count = REQUIRED_FEATURES.as_ref().map_or(0, |s| s.len());
        if (MIN_VERSION == -1)
            && (MAX_VERSION == -1)
            && (EXACT_VERSION == -1)
            && (feature_count == 0)
            && (!globals::VERBOSE)
        {
            globals::VERBOSE = true;
            verbose_print("No tests selected; printing known data\n");
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-info.main-fn]
// [spec:hfst:sem:hfst-info.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstInfo");
        parse_options(&mut args);
        if MIN_VERSION != -1 {
            verbose_print(&format!(
                "Requiring current version {} to be greater than {}\n",
                HFST_LONGVERSION, MIN_VERSION
            ));
            if HFST_LONGVERSION < MIN_VERSION {
                error(EXIT_FAILURE, 0, "Version requirements not met");
            }
        }
        if EXACT_VERSION != -1 {
            verbose_print(&format!(
                "Requiring current version {} to be exactly {}\n",
                HFST_LONGVERSION, EXACT_VERSION
            ));
            if HFST_LONGVERSION != EXACT_VERSION {
                error(EXIT_FAILURE, 0, "Version requirements not met");
            }
        }
        if MAX_VERSION != -1 {
            verbose_print(&format!(
                "Requiring current version {} to be greater than {}\n",
                HFST_LONGVERSION, MAX_VERSION
            ));
            if HFST_LONGVERSION < MAX_VERSION {
                error(EXIT_FAILURE, 0, "Version requirements not met");
            }
        }
        if let Some(features) = REQUIRED_FEATURES.as_ref() {
            for f in features.iter() {
                if (f == "sfst") || (f == "SFST") || (f == "HAVE_SFST") {
                    verbose_print("Requiring SFST support from library");
                    if !HAVE_SFST {
                        if !HAVE_LEAN_SFST {
                            error(EXIT_FAILURE, 0, "Required SFST support not present");
                        } else {
                            error(
                                EXIT_FAILURE,
                                0,
                                "Required SFST support present only in limited form",
                            );
                        }
                    }
                } else if (f == "foma") || (f == "FOMA") || (f == "HAVE_FOMA") {
                    verbose_print("Requiring foma support from library");
                    if HAVE_FOMA {
                        error(EXIT_FAILURE, 0, "Required foma support not present");
                    }
                } else if (f == "xfsm") || (f == "XFSM") || (f == "HAVE_XFSM") {
                    verbose_print("Requiring xfsm support from library");
                    if HAVE_XFSM {
                        error(EXIT_FAILURE, 0, "Required xfsm support not present");
                    }
                } else if (f == "openfst") || (f == "OPENFST") || (f == "HAVE_OPENFST") {
                    verbose_print("Requiring OpenFst support from library");
                    if HAVE_OPENFST {
                        error(EXIT_FAILURE, 0, "Required OpenFst support not present");
                    }
                } else if (f == "icu") || (f == "USE_ICU_UNICODE") {
                    verbose_print("Requiring Unicode parsed by ICU");
                } else {
                    error(
                        EXIT_FAILURE,
                        0,
                        &format!(
                            "Required {} support is unrecognised and therefore assumed to be missing",
                            f
                        ),
                    );
                }
            }
        }
        verbose_print(&format!(
            "HFST info version: {}\nHFST packaging: {}\nHFST version: {}\nHFST long version: {}\n",
            globals::hfst_tool_version(),
            PACKAGE_STRING,
            PACKAGE_VERSION,
            HFST_LONGVERSION
        ));
        if HAVE_OPENFST {
            verbose_print("OpenFst supported\n");
        }
        if HAVE_SFST {
            verbose_print("SFST supported\n");
        } else if HAVE_LEAN_SFST {
            verbose_print("SFST limitedly supported\n");
        }
        if HAVE_FOMA {
            verbose_print("foma supported\n");
        }
        if HAVE_XFSM {
            verbose_print("xfsm supported\n");
        }
        verbose_print("Unicode support: ICU\n");

        EXIT_SUCCESS
    }
}
