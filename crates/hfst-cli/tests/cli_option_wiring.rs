//! Locks for option-plumbing defects that made valid invocations either
//! silently do the wrong thing or abort:
//!
//!  * `hfst tokenize --space-separated` / `-i` reached no case at all, so it
//!    enabled `--debug` and emitted the default segmenting format.
//!  * an optional-argument option given without `=value` used to inherit the
//!    previous option's argument (the getopt-era `optarg` leak), so e.g.
//!    `--colour` read the input filename as its WHEN.
//!  * `hfst optimized-lookup -q` / `-s` set only the verbosity field, which
//!    nothing reads, dropping the weight display that is the flag's whole
//!    observable effect.
//!
//! The getopt parser is gone; the unit tests below pin the same contract at
//! the clap layer, on the real shared groups ([`CommonArgs`] + [`UnaryIo`]):
//! a bare optional-argument option takes its default and never a neighbour's
//! value, because its value must be attached with '='.

use clap::Parser;
use hfst_cli::cli::{CommonArgs, UnaryIo};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// optional-argument values stay attached (the former optarg-leak lock)
// ---------------------------------------------------------------------------

/// A minimal tool: exactly the shared option groups and nothing else.
#[derive(clap::Parser)]
struct Probe {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,
}

fn probe(words: &[&str]) -> Probe {
    Probe::try_parse_from(words).expect("the probe invocation must parse")
}

/// `--colour` with no `=WHEN` means "always" — it must not read the previous
/// option's argument as its WHEN, which the getopt port once did.
#[test]
fn bare_colour_takes_default_not_previous_argument() {
    let args = probe(&["prog", "-i", "x.hfst", "--colour"]);
    assert_eq!(args.io.input.as_deref(), Some("x.hfst"));
    assert_eq!(
        args.common.colour.as_deref(),
        Some("always"),
        "--colour with no =WHEN must not inherit -i's argument"
    );

    // At the end of argv (nothing left to misread as its value) too.
    let args = probe(&["prog", "--input=x.hfst", "--colour"]);
    assert_eq!(args.io.input.as_deref(), Some("x.hfst"));
    assert_eq!(args.common.colour.as_deref(), Some("always"));
}

#[test]
fn colour_with_inline_value_keeps_it() {
    let args = probe(&["prog", "-i", "x.hfst", "--colour=never"]);
    assert_eq!(args.common.colour.as_deref(), Some("never"));
}

/// The value must be ATTACHED: a bare `--colour` ahead of an operand leaves
/// the operand alone, exactly as getopt's OPTIONAL_ARGUMENT never reached out
/// to the next argv word.
#[test]
fn bare_colour_never_swallows_the_next_word() {
    let args = probe(&["prog", "--colour", "y.hfst"]);
    assert_eq!(args.common.colour.as_deref(), Some("always"));
    assert_eq!(
        args.io.infiles,
        vec!["y.hfst".to_string()],
        "the word after a bare --colour is an operand, not its WHEN"
    );
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
/// `-i` work; this port has no short string, so both spellings ride on the one
/// clap arg.
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

/// A weighted optimized-lookup transducer mapping `a` to `b` at weight 0.5.
fn weighted_ol_transducer(dir: &Path) -> String {
    let algebraic = dir.join("w.hfst");
    let table = dir.join("w.hfstol");
    let (ok, _) = run(
        &["regexp2fst", "-o", algebraic.to_str().expect("utf-8 path")],
        b"a:b::0.5;\n",
    );
    assert!(ok, "regexp2fst failed to build the weighted transducer");
    let (ok, _) = run(
        &[
            "fst2fst",
            "-w",
            "-i",
            algebraic.to_str().expect("utf-8 path"),
            "-o",
            table.to_str().expect("utf-8 path"),
        ],
        b"",
    );
    assert!(ok, "fst2fst failed to convert to optimized-lookup");
    table.to_str().expect("utf-8 path").to_string()
}

/// Upstream's `-q`/`-s` case clears verbosity *and* sets displayWeights. The
/// port kept only the first half, and that field is read by nothing, so both
/// flags were inert. The expected bytes come from the C++ tool: streaming a
/// float is `%g`, so the weight is `0.5`, not hfst-lookup's `0.500000`.
#[test]
fn quiet_flags_turn_weights_on() {
    let dir = scratch("ol-quiet");
    let table = weighted_ol_transducer(&dir);

    let (ok, plain) = run(&["optimized-lookup", &table], b"a\n");
    assert!(ok, "hfst optimized-lookup exited non-zero");
    assert_eq!(plain, "a\tb\n\n", "the default output must carry no weight");

    for flag in ["-q", "--quiet", "-s", "--silent", "-w", "--show-weights"] {
        let (ok, out) = run(&["optimized-lookup", flag, &table], b"a\n");
        assert!(ok, "hfst optimized-lookup {flag} exited non-zero");
        assert_eq!(
            out, "a\tb\t0.5\n\n",
            "hfst optimized-lookup {flag} did not display the weight"
        );
    }
}

/// `-v` is the half of the verbosity pair that stays invisible: upstream
/// assigns verboseFlag and never tests it, so the output must not change.
#[test]
fn verbose_flag_changes_no_output() {
    let dir = scratch("ol-verbose");
    let table = weighted_ol_transducer(&dir);

    for flag in ["-v", "--verbose"] {
        let (ok, out) = run(&["optimized-lookup", flag, &table], b"a\n");
        assert!(ok, "hfst optimized-lookup {flag} exited non-zero");
        assert_eq!(out, "a\tb\n\n", "-v must not alter the analyses printed");
    }
}

/// `--pipe-mode` is a Windows console switch that does nothing here, but it is
/// part of the flag contract: every documented STREAM must still be accepted,
/// and an undocumented one must still fail.
#[test]
fn pipe_mode_accepts_documented_streams() {
    let dir = scratch("ol-pipe");
    let table = weighted_ol_transducer(&dir);

    for flag in [
        "-p",
        "--pipe-mode",
        "--pipe-mode=both",
        "--pipe-mode=input",
        "--pipe-mode=output",
    ] {
        let (ok, out) = run(&["optimized-lookup", flag, &table], b"a\n");
        assert!(ok, "hfst optimized-lookup {flag} exited non-zero");
        assert_eq!(out, "a\tb\n\n", "{flag} must not change the analyses");
    }

    let (ok, _) = run(
        &["optimized-lookup", "--pipe-mode=sideways", &table],
        b"a\n",
    );
    assert!(!ok, "an unrecognised --pipe-mode STREAM must fail");
}
