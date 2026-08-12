//! Port of tools/src/hfst-info.cc — the "show or test HFST versions and
//! features" command-line tool. It reads no transducer streams; it parses
//! version/feature test options, then prints or tests the build's version and
//! features. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options).
//!
//! Deliberately NOT faithful in what it reports. Upstream answered `-a/-e/-m`
//! and `-f` from autoconf's config.h, and this port had those values frozen as
//! literals copied from a C++ 3.17.1 build — so it announced a version it is
//! not and backends it does not have. This tool's entire job is to be believed
//! by a configure script, so it answers from what this build actually is: the
//! crate version, and the backend table below.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into `run`. There are no `static mut` globals
//! and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, print_version, verbose_print,
    version_line,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;
use std::collections::BTreeSet;
use std::io::Write;

const PACKAGE_NAME: &str = "Divvun HFST";
const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

// CARGO_PKG_VERSION_{MAJOR,MINOR,PATCH} are pure digit runs — any pre-release
// tag lands in CARGO_PKG_VERSION_PRE — so a non-digit is a build-time failure
// rather than something to handle at runtime.
const fn version_component(s: &str) -> i64 {
    let b = s.as_bytes();
    let mut i = 0;
    let mut v: i64 = 0;
    while i < b.len() {
        assert!(b[i].is_ascii_digit(), "version component is not numeric");
        v = v * 10 + (b[i] - b'0') as i64;
        i += 1;
    }
    v
}

/// This build's version in the packed `major*10^8 + minor*10^4 + patch` form
/// that `-a/-e/-m` compare against — the same encoding `parse_version_string`
/// produces, so the operand and the subject are on one scale.
const HFST_LONGVERSION: i64 = version_component(env!("CARGO_PKG_VERSION_MAJOR")) * 10000 * 10000
    + version_component(env!("CARGO_PKG_VERSION_MINOR")) * 10000
    + version_component(env!("CARGO_PKG_VERSION_PATCH"));

/// One backend, as `-f` tests it and as the listing reports it.
struct Feature {
    label: &'static str,
    /// Every spelling `-f` accepts for it.
    names: &'static [&'static str],
    present: bool,
}

/// What this build has. The `-f` gate and the informational listing both read
/// this one table: the bug it replaces was the two answers disagreeing, with
/// `-f foma` failing while the listing said "foma supported".
const FEATURES: &[Feature] = &[
    Feature {
        label: "OpenFst (tropical)",
        names: &["openfst", "OPENFST", "HAVE_OPENFST"],
        present: true,
    },
    Feature {
        label: "foma",
        names: &["foma", "FOMA", "HAVE_FOMA"],
        present: cfg!(feature = "foma"),
    },
    Feature {
        label: "Unicode (ICU)",
        names: &["icu", "ICU", "USE_ICU_UNICODE"],
        present: true,
    },
    // Out of scope for this fork, and named here so asking for one gets a
    // refusal instead of the silence that reads as "old build, didn't say".
    Feature {
        label: "OpenFst (log)",
        names: &["openfst-log", "OPENFST_LOG", "HAVE_OPENFST_LOG"],
        present: false,
    },
    Feature {
        label: "SFST",
        names: &["sfst", "SFST", "HAVE_SFST"],
        present: false,
    },
    Feature {
        label: "xfsm",
        names: &["xfsm", "XFSM", "HAVE_XFSM"],
        present: false,
    },
];

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
        "MVER, EVER or UVER version vectors must be composed of one to three full stop separated runs of digits,\nand are compared against this build's own version, not against upstream HFST's.\nFEAT should be name of feature supported by HFST, such as openfst, foma or icu\n\n"
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
                "Requiring current version {} to be at least {}\n",
                HFST_LONGVERSION, options.min_version
            ),
        );
        if HFST_LONGVERSION < options.min_version {
            version_requirements_not_met(&common);
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
            version_requirements_not_met(&common);
        }
    }
    if options.max_version != -1 {
        verbose_print(
            &common,
            &format!(
                "Requiring current version {} to be at most {}\n",
                HFST_LONGVERSION, options.max_version
            ),
        );
        // Upstream tested `<` here, the same comparison as --atleast-version,
        // so --max-version rejected exactly the builds it was meant to accept.
        if HFST_LONGVERSION > options.max_version {
            version_requirements_not_met(&common);
        }
    }
    if let Some(features) = options.required_features.as_ref() {
        for f in features.iter() {
            match FEATURES
                .iter()
                .find(|feature| feature.names.contains(&f.as_str()))
            {
                Some(feature) => {
                    verbose_print(
                        &common,
                        &format!("Requiring {} support from library\n", feature.label),
                    );
                    if !feature.present {
                        error(
                            &common,
                            EXIT_FAILURE,
                            0,
                            &format!("Required {} support not present", feature.label),
                        );
                    }
                }
                None => error(
                    &common,
                    EXIT_FAILURE,
                    0,
                    &format!(
                        "Required {} support is unrecognised and therefore assumed to be missing",
                        f
                    ),
                ),
            }
        }
    }
    verbose_print(
        &common,
        &format!(
            "{}\nHFST packaging: {} {}\nHFST version: {}\nHFST long version: {}\n",
            version_line(&common.program_name),
            PACKAGE_NAME,
            PACKAGE_VERSION,
            PACKAGE_VERSION,
            HFST_LONGVERSION
        ),
    );
    for feature in FEATURES {
        verbose_print(
            &common,
            &format!(
                "{} {}\n",
                feature.label,
                if feature.present {
                    "supported"
                } else {
                    "not supported"
                }
            ),
        );
    }

    EXIT_SUCCESS
}

// A build script asking "is this HFST at least 3.17" has no truthful yes to
// receive, so it gets a no that says what this actually is instead of a bare
// refusal it would have to guess at.
fn version_requirements_not_met(common: &CommonOptions) {
    error(
        common,
        EXIT_FAILURE,
        0,
        &format!(
            "Version requirements not met: this is {} {} (long version {}), \
             an independent fork that does not carry an upstream HFST version",
            PACKAGE_NAME, PACKAGE_VERSION, HFST_LONGVERSION
        ),
    );
}
