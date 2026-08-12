//! Locks for two option-plumbing defects that made valid invocations either
//! silently do the wrong thing or abort:
//!
//!  * `hfst tokenize --space-separated` / `-i` reached no case at all, so it
//!    enabled `--debug` and emitted the default segmenting format.
//!  * an OPTIONAL_ARGUMENT option given without `=value` inherited the previous
//!    option's `optarg`, so e.g. `--colour` read the input filename as its WHEN.

use hfst_cli::hfst_commandline::GETOPT_COLOUR;
use hfst_cli::hfst_getopt::{GetOpt, Getopt, NO_ARGUMENT, OPTIONAL_ARGUMENT, REQUIRED_ARGUMENT};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// optarg lifetime (hfst-getopt)
// ---------------------------------------------------------------------------

fn long_options() -> Vec<GetOpt> {
    vec![
        GetOpt {
            name: "input",
            has_arg: REQUIRED_ARGUMENT,
            val: b'i' as i32,
        },
        GetOpt {
            name: "colour",
            has_arg: OPTIONAL_ARGUMENT,
            val: GETOPT_COLOUR,
        },
        GetOpt {
            name: "verbose",
            has_arg: NO_ARGUMENT,
            val: b'v' as i32,
        },
    ]
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_string()).collect()
}

#[test]
fn optional_argument_without_value_clears_optarg() {
    let long = long_options();
    let mut args = argv(&["prog", "-i", "x.hfst", "--colour"]);
    let mut opt = Getopt::new();

    assert_eq!(opt.getopt_long(&mut args, &long), b'i' as i32);
    assert_eq!(opt.optarg_opt().as_deref(), Some("x.hfst"));

    assert_eq!(opt.getopt_long(&mut args, &long), GETOPT_COLOUR);
    assert_eq!(
        opt.optarg_opt(),
        None,
        "--colour with no =WHEN must not inherit -i's argument"
    );
}

#[test]
fn optional_argument_with_inline_value_keeps_it() {
    let long = long_options();
    let mut args = argv(&["prog", "-i", "x.hfst", "--colour=never"]);
    let mut opt = Getopt::new();

    assert_eq!(opt.getopt_long(&mut args, &long), b'i' as i32);
    assert_eq!(opt.getopt_long(&mut args, &long), GETOPT_COLOUR);
    assert_eq!(opt.optarg_opt().as_deref(), Some("never"));
}

#[test]
fn no_argument_option_clears_optarg() {
    let long = long_options();
    let mut args = argv(&["prog", "-i", "x.hfst", "--verbose"]);
    let mut opt = Getopt::new();

    assert_eq!(opt.getopt_long(&mut args, &long), b'i' as i32);
    assert_eq!(opt.getopt_long(&mut args, &long), b'v' as i32);
    assert_eq!(opt.optarg_opt(), None);
}

/// A trailing OPTIONAL_ARGUMENT option (nothing left to misread as its value)
/// must not carry the previous argument either.
#[test]
fn optional_argument_at_end_of_argv_clears_optarg() {
    let long = long_options();
    let mut args = argv(&["prog", "--input=x.hfst", "--colour"]);
    let mut opt = Getopt::new();

    assert_eq!(opt.getopt_long(&mut args, &long), b'i' as i32);
    assert_eq!(opt.optarg_opt().as_deref(), Some("x.hfst"));
    assert_eq!(opt.getopt_long(&mut args, &long), GETOPT_COLOUR);
    assert_eq!(opt.optarg_opt(), None);
}

// ---------------------------------------------------------------------------
// end-to-end: the tools the leak and the unreachable flag actually broke
// ---------------------------------------------------------------------------

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hfst-option-wiring-{name}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Run `hfst <args>` with `stdin`, returning (success, stdout).
fn run(args: &[&str], stdin: &[u8]) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn hfst");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn pmatch_ruleset(dir: &Path) -> String {
    let script = dir.join("rules.pmscript");
    let ruleset = dir.join("rules.pmhfst");
    std::fs::write(&script, "Define TOP [{cat}|{dog}|{the}] ;\n").expect("write pmscript");
    let (ok, _) = run(
        &[
            "pmatch2fst",
            script.to_str().expect("utf-8 path"),
            "-o",
            ruleset.to_str().expect("utf-8 path"),
        ],
        b"",
    );
    assert!(ok, "pmatch2fst failed to build the tokenizer ruleset");
    ruleset.to_str().expect("utf-8 path").to_string()
}

/// Both spellings of the space-separated format must select it. The C++ long
/// table maps `--space-separated` to 'd' and only its short-option string makes
/// `-i` work; this port has no short string, so both spellings ride on `val`.
#[test]
fn tokenize_space_separated_is_reachable() {
    let dir = scratch("tokenize");
    let ruleset = pmatch_ruleset(&dir);

    let (ok_default, default_out) = run(&["tokenize", &ruleset], b"the cat\n");
    assert!(ok_default);
    assert_eq!(default_out, "the\ncat\n\n");

    for flag in ["--space-separated", "-i"] {
        let (ok, out) = run(&["tokenize", flag, &ruleset], b"the cat\n");
        assert!(ok, "hfst tokenize {flag} exited non-zero");
        assert_eq!(
            out, "the cat ",
            "hfst tokenize {flag} did not select the space-separated format"
        );
    }
}

/// `-d`/`--debug` keeps 'd' — moving `--space-separated` off it must not have
/// stolen the common debug flag.
#[test]
fn tokenize_debug_still_selects_the_default_format() {
    let dir = scratch("tokenize-debug");
    let ruleset = pmatch_ruleset(&dir);

    for flag in ["-d", "--debug"] {
        let (ok, out) = run(&["tokenize", flag, &ruleset], b"the cat\n");
        assert!(ok, "hfst tokenize {flag} exited non-zero");
        assert_eq!(out, "the\ncat\n\n");
    }
}

fn small_transducer(dir: &Path) -> String {
    let fst = dir.join("ab.hfst");
    let (ok, _) = run(
        &["regexp2fst", "-o", fst.to_str().expect("utf-8 path")],
        b"a:b;\n",
    );
    assert!(ok, "regexp2fst failed to build the test transducer");
    fst.to_str().expect("utf-8 path").to_string()
}

/// `--colour` and `-S` take an optional argument; supplied bare after an
/// argument-taking option they used to read that option's argument.
#[test]
fn bare_optional_argument_options_do_not_read_the_previous_argument() {
    let dir = scratch("optarg");
    let fst = small_transducer(&dir);

    let (ok, out) = run(&["lookup", "-i", &fst, "--colour"], b"a\n");
    assert!(ok, "hfst lookup --colour after -i FILE must not fail");
    assert!(out.contains('b'), "lookup produced no result: {out:?}");

    let (ok, out) = run(&["summarize", "-i", &fst, "-S"], b"");
    assert!(ok, "hfst summarize -S after -i FILE must not fail");
    assert!(out.contains("fst type"), "summarize printed nothing useful");
}
