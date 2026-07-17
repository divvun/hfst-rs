//! Faithful 1:1 port of tools/src/hfst-info.cc — the "show or test HFST
//! versions and features" command-line tool. It reads no transducer streams
//! (it only includes inc/globals-common.h); it parses version/feature test
//! options, then prints or tests the library's compiled-in version and
//! features. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into `run`. There are no `static mut` globals
//! and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, print_version, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};

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

/// hfst-info's own options (the former tool-specific `static mut`s).
struct Options {
    min_version: i64,
    exact_version: i64,
    max_version: i64,
    // required_features collected as a set<string>; BTreeSet preserves the
    // sorted-iteration order the C++ std::set used.
    required_features: Option<BTreeSet<String>>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            min_version: -1,
            exact_version: -1,
            max_version: -1,
            required_features: None,
        }
    }
}

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
fn parse_version_string(common: &CommonOptions, s: &str) -> i64 {
    let (major, endptr) = parse_u64_prefix(s);
    let major = major as i64;
    if endptr.is_empty() {
        return major * 10000 * 10000;
    } else if !endptr.starts_with('.') {
        error(
            common,
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
            common,
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
            common,
            EXIT_FAILURE,
            0,
            &format!("cannot parse version string from {}", endptr),
        );
    }
    -1
}

// [spec:hfst:def:hfst-info.print-usage-fn]
// [spec:hfst:sem:hfst-info.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nshow or test HFST versions and features\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Test features:\n  -a, --atleast-version=MVER   require at least MVER version of HFST\n  -e, --exact-version=EVER     require exactly EVER version of HFST\n  -m, --max-version=UVER       require at most UVER version of HFST\n  -f, --requirefeature=FEAT    require named FEAT support from HFST\n"
    );
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "MVER, EVER or UVER version vectors must be composed of one to three full stop separated runs of digits.\nFEAT should be name of feature supported by HFST, such as SFST, foma or openfst\n\n"
    );
}

// [spec:hfst:def:hfst-info.parse-options-fn]
// [spec:hfst:sem:hfst-info.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }
        // The C switch handles only a/e/m/f/h/V; every other accepted option
        // (the common -v/-q/-s/-d/-o/--colour) falls through with no action.
        if c == b'a' as i32 {
            options.min_version = parse_version_string(&common, &opt.optarg());
        } else if c == b'e' as i32 {
            options.exact_version = parse_version_string(&common, &opt.optarg());
        } else if c == b'm' as i32 {
            options.max_version = parse_version_string(&common, &opt.optarg());
        } else if c == b'f' as i32 {
            options
                .required_features
                .get_or_insert_with(BTreeSet::new)
                .insert(opt.optarg());
        } else if c == b'h' as i32 {
            print_usage(&common);
            return Err(EXIT_SUCCESS);
        } else if c == b'V' as i32 {
            print_version(&common);
            return Err(EXIT_SUCCESS);
        }
    }
    let feature_count = options.required_features.as_ref().map_or(0, |s| s.len());
    if (options.min_version == -1)
        && (options.max_version == -1)
        && (options.exact_version == -1)
        && (feature_count == 0)
        && (!common.verbose)
    {
        common.verbose = true;
        verbose_print(&common, "No tests selected; printing known data\n");
    }
    Ok((common, options))
}

// [spec:hfst:def:hfst-info.main-fn]
// [spec:hfst:sem:hfst-info.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstInfo");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    if options.min_version != -1 {
        verbose_print(
            &common,
            &format!(
                "Requiring current version {} to be greater than {}\n",
                HFST_LONGVERSION, options.min_version
            ),
        );
        if HFST_LONGVERSION < options.min_version {
            error(&common, EXIT_FAILURE, 0, "Version requirements not met");
        }
    }
    if options.exact_version != -1 {
        verbose_print(
            &common,
            &format!(
                "Requiring current version {} to be exactly {}\n",
                HFST_LONGVERSION, options.exact_version
            ),
        );
        if HFST_LONGVERSION != options.exact_version {
            error(&common, EXIT_FAILURE, 0, "Version requirements not met");
        }
    }
    if options.max_version != -1 {
        verbose_print(
            &common,
            &format!(
                "Requiring current version {} to be greater than {}\n",
                HFST_LONGVERSION, options.max_version
            ),
        );
        if HFST_LONGVERSION < options.max_version {
            error(&common, EXIT_FAILURE, 0, "Version requirements not met");
        }
    }
    if let Some(features) = options.required_features.as_ref() {
        for f in features.iter() {
            if (f == "sfst") || (f == "SFST") || (f == "HAVE_SFST") {
                verbose_print(&common, "Requiring SFST support from library");
                if !HAVE_SFST {
                    if !HAVE_LEAN_SFST {
                        error(
                            &common,
                            EXIT_FAILURE,
                            0,
                            "Required SFST support not present",
                        );
                    } else {
                        error(
                            &common,
                            EXIT_FAILURE,
                            0,
                            "Required SFST support present only in limited form",
                        );
                    }
                }
            } else if (f == "foma") || (f == "FOMA") || (f == "HAVE_FOMA") {
                verbose_print(&common, "Requiring foma support from library");
                if HAVE_FOMA {
                    error(
                        &common,
                        EXIT_FAILURE,
                        0,
                        "Required foma support not present",
                    );
                }
            } else if (f == "xfsm") || (f == "XFSM") || (f == "HAVE_XFSM") {
                verbose_print(&common, "Requiring xfsm support from library");
                if HAVE_XFSM {
                    error(
                        &common,
                        EXIT_FAILURE,
                        0,
                        "Required xfsm support not present",
                    );
                }
            } else if (f == "openfst") || (f == "OPENFST") || (f == "HAVE_OPENFST") {
                verbose_print(&common, "Requiring OpenFst support from library");
                if HAVE_OPENFST {
                    error(
                        &common,
                        EXIT_FAILURE,
                        0,
                        "Required OpenFst support not present",
                    );
                }
            } else if (f == "icu") || (f == "USE_ICU_UNICODE") {
                verbose_print(&common, "Requiring Unicode parsed by ICU");
            } else {
                error(
                    &common,
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
    verbose_print(
        &common,
        &format!(
            "HFST info version: {}\nHFST packaging: {}\nHFST version: {}\nHFST long version: {}\n",
            common.hfst_tool_version, PACKAGE_STRING, PACKAGE_VERSION, HFST_LONGVERSION
        ),
    );
    if HAVE_OPENFST {
        verbose_print(&common, "OpenFst supported\n");
    }
    if HAVE_SFST {
        verbose_print(&common, "SFST supported\n");
    } else if HAVE_LEAN_SFST {
        verbose_print(&common, "SFST limitedly supported\n");
    }
    if HAVE_FOMA {
        verbose_print(&common, "foma supported\n");
    }
    if HAVE_XFSM {
        verbose_print(&common, "xfsm supported\n");
    }
    verbose_print(&common, "Unicode support: ICU\n");

    EXIT_SUCCESS
}
