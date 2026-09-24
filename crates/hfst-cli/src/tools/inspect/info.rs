//! Port of tools/src/hfst-info.cc — the "show or test HFST versions and
//! features" command-line tool. It reads no transducer streams; it parses
//! version/feature test options, then prints or tests the build's version and
//! features. Option handling is clap 4 derive through [`crate::cli`].
//!
//! Deliberately NOT faithful in what it reports. Upstream answered `-a/-e/-m`
//! and `-f` from autoconf's config.h, and this port had those values frozen as
//! literals copied from a C++ 3.17.1 build — so it announced a version it is
//! not and backends it does not have. This tool's entire job is to be believed
//! by a configure script, so it answers from what this build actually is: the
//! crate version, the upstream interface-compatibility version, and the
//! backend table below.
//!
//! Version tests speak two namespaces. Existing build systems (every Giella
//! language repo) gate on upstream HFST versions (`--atleast-version=3.16.0`),
//! which this build satisfies through [`HFST_COMPAT_VERSION`] — the upstream
//! release whose tool interface it provides. The fork's own version answers
//! too, so `-a` keeps working against Divvun HFST versions once those are what
//! scripts ask about. A requirement is met if either version meets it;
//! identity reporting (`-V`, the listing) never claims to BE upstream HFST.
//!
//! Idiomatic option handling: the tool's state lives in a tool-local
//! [`Options`] built from the parsed [`Args`] and threaded into `run`. The
//! shared `-v/-q/-o/…` options are accepted and discarded, as the C's
//! switch did — it never chained the common cases.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print, version_line};

const EXIT_FAILURE: i32 = 1;
use std::collections::BTreeSet;

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

/// The upstream HFST release whose command-line interface this build provides:
/// the C++ oracle the port is validated against (Giella lang builds produce
/// equivalent artifacts). Configure scripts across the Giella ecosystem gate on
/// `--atleast-version=3.16.0` in this namespace; without a compat answer no
/// language repo can configure against this toolchain.
const HFST_COMPAT_VERSION: &str = "3.17.1";
const HFST_COMPAT_LONGVERSION: i64 = 3 * 10000 * 10000 + 17 * 10000 + 1;

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

/// hfst-info's command line.
//
// This tool's switch handles only its own version/feature options plus
// help and version: '-v/-q/-s/-d/-o/--colour' are accepted and discarded,
// and no output file is resolved, which is why the report goes to stdout.
// [`ToolArgs::applies_common_options`] carries that.
// [spec:hfst:def:hfst-info.parse-options-fn]
// [spec:hfst:sem:hfst-info.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "show or test HFST versions and features",
    after_help = "MVER, EVER or UVER version vectors must be composed of one to three full stop separated runs of digits.
A requirement is met if either this build's own version or the upstream HFST version it is interface-compatible with (3.17.1) meets it.
FEAT should be name of feature supported by HFST, such as openfst, foma or icu"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    /// Require at least MVER version of HFST
    #[arg(short = 'a', long = "atleast-version", value_name = "MVER")]
    atleast_version: Option<String>,

    /// Require exactly EVER version of HFST
    #[arg(short = 'e', long = "exact-version", value_name = "EVER")]
    exact_version: Option<String>,

    /// Require at most UVER version of HFST
    #[arg(short = 'm', long = "max-version", value_name = "UVER")]
    max_version: Option<String>,

    /// Require named FEAT support from HFST
    #[arg(
        short = 'f',
        long = "require-feature",
        value_name = "FEAT",
        action = clap::ArgAction::Append
    )]
    require_feature: Vec<String>,

    /// Accepted and ignored, as the C's leftover free arguments were
    #[arg(value_name = "INFILE", num_args = 0..)]
    infiles: Vec<String>,
}

impl Args {
    fn options(&self, common: &CommonOptions) -> Options {
        let version = |v: &Option<String>| match v {
            Some(s) => parse_version_string(common, s),
            None => -1,
        };
        let required_features = if self.require_feature.is_empty() {
            None
        } else {
            Some(
                self.require_feature
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
            )
        };
        Options {
            min_version: version(&self.atleast_version),
            exact_version: version(&self.exact_version),
            max_version: version(&self.max_version),
            required_features,
        }
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, _opts: &mut CommonOptions) {}

    fn applies_common_options(&self) -> bool {
        false
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // parse_version_string exits on a malformed vector; the C did that
        // inside its getopt loop.
        self.options(opts);
        Ok(())
    }
}

// [spec:hfst:def:hfst-info.main-fn]
// [spec:hfst:sem:hfst-info.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstInfo");
    let (mut common, args) = cli::parse::<Args>(common, args)?;
    let options = args.options(&common);
    let _ = &args.infiles;
    // With no test selected the tool reports everything, so it turns
    // verbosity on itself.
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
    version_gate(&common, options.min_version, "at least", |v, req| v < req);
    version_gate(&common, options.exact_version, "exactly", |v, req| v != req);
    // Upstream tested `<` for --max-version, the same comparison as
    // --atleast-version, so it rejected exactly the builds it was meant to
    // accept.
    version_gate(&common, options.max_version, "at most", |v, req| v > req);
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
            "{}\nHFST packaging: {} {}\nHFST version: {}\nHFST long version: {}\nCompatible with upstream HFST: {} (long version {})\n",
            version_line(&common.program_name),
            PACKAGE_NAME,
            PACKAGE_VERSION,
            PACKAGE_VERSION,
            HFST_LONGVERSION,
            HFST_COMPAT_VERSION,
            HFST_COMPAT_LONGVERSION
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

    Ok(())
}

/// One `-a/-e/-m` test: `requirement` is -1 when the option was not given, and
/// `fails` is the failing comparison for one version against it. The gate
/// passes if either the fork's own version or the upstream interface-compat
/// version satisfies it — the two namespaces scripts ask in, and a requirement
/// met in either one is genuinely met.
fn version_gate(
    common: &CommonOptions,
    requirement: i64,
    relation: &str,
    fails: impl Fn(i64, i64) -> bool,
) {
    if requirement == -1 {
        return;
    }
    verbose_print(
        common,
        &format!(
            "Requiring current version {} (upstream-compatible {}) to be {} {}\n",
            HFST_LONGVERSION, HFST_COMPAT_LONGVERSION, relation, requirement
        ),
    );
    if fails(HFST_LONGVERSION, requirement) && fails(HFST_COMPAT_LONGVERSION, requirement) {
        version_requirements_not_met(common);
    }
}

// The refusal names both identities so a build script's log says what was
// actually asked of what, instead of a bare no it would have to guess at.
fn version_requirements_not_met(common: &CommonOptions) {
    error(
        common,
        EXIT_FAILURE,
        0,
        &format!(
            "Version requirements not met: this is {} {} (long version {}), \
         interface-compatible with upstream HFST {} (long version {})",
            PACKAGE_NAME,
            PACKAGE_VERSION,
            HFST_LONGVERSION,
            HFST_COMPAT_VERSION,
            HFST_COMPAT_LONGVERSION
        ),
    );
}
