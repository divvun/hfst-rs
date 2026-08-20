//! `hfst info` is the tool a configure script believes. Its answers were
//! copied from a C++ 3.17.1 config.h and its `-f` tests were polarity-inverted,
//! so it claimed a version this project does not have, advertised SFST (out of
//! scope, deleted) and rejected `-f foma` / `-f openfst` — the two backends it
//! actually ships. These lock the exit codes, the identity/compatibility split
//! (the listing reports Divvun HFST plus an explicit upstream-compat line, and
//! never claims to BE upstream HFST), and the version gates that the Giella
//! configure scripts depend on.

use std::process::Command;

/// Run `hfst info ARGS...`, returning (exit code, stdout, stderr).
fn info(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .arg("info")
        .args(args)
        .output()
        .expect("spawn hfst info");
    (
        out.status.code().expect("hfst info exited via a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The informational listing (no tests selected turns verbose on).
fn listing() -> String {
    let (code, stdout, _) = info(&[]);
    assert_eq!(code, 0, "the bare listing must succeed");
    stdout
}

#[test]
fn require_openfst_succeeds() {
    let (code, _, stderr) = info(&["-f", "openfst"]);
    assert_eq!(
        code, 0,
        "OpenFst is the primary backend; -f openfst must pass. stderr: {stderr}"
    );
}

#[test]
fn require_foma_matches_the_build() {
    let (code, _, stderr) = info(&["-f", "foma"]);
    let expected = if cfg!(feature = "foma") { 0 } else { 1 };
    assert_eq!(
        code, expected,
        "-f foma must reflect the foma feature. stderr: {stderr}"
    );
}

#[test]
fn require_sfst_fails() {
    let (code, _, stderr) = info(&["-f", "sfst"]);
    assert_eq!(code, 1, "SFST is out of scope; -f sfst must fail");
    assert!(
        stderr.contains("SFST support not present"),
        "the refusal must name SFST, got: {stderr}"
    );
}

#[test]
fn require_xfsm_fails() {
    let (code, _, _) = info(&["-f", "xfsm"]);
    assert_eq!(code, 1, "xfsm is out of scope; -f xfsm must fail");
}

#[test]
fn unknown_feature_fails() {
    let (code, _, stderr) = info(&["-f", "no-such-backend"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("unrecognised"), "got: {stderr}");
}

#[test]
fn listing_reports_identity_and_upstream_compat() {
    let listing = listing();
    assert!(
        listing.contains(env!("CARGO_PKG_VERSION")),
        "the listing must report this crate's version:\n{listing}"
    );
    assert!(
        !listing.contains("HFST version: 3.17"),
        "the listing must not claim to BE upstream HFST:\n{listing}"
    );
    assert!(
        listing.contains("Compatible with upstream HFST: 3.17.1"),
        "the listing must state the upstream compat version:\n{listing}"
    );
}

/// The Giella ecosystem's configure gate: every language repo requires
/// `hfst-info --atleast-version=3.16.0`. This build provides the 3.17.1 tool
/// interface, so these must pass or no Giella repo configures against it.
#[test]
fn version_gate_accepts_the_compatible_upstream_version() {
    for (opt, ver) in [
        ("-a", "3.16.0"),
        ("-a", "3.17"),
        ("-a", "3.17.1"),
        ("-e", "3.17.1"),
        ("-m", "3.17.1"),
        ("-m", "4"),
    ] {
        let (code, _, stderr) = info(&[opt, ver]);
        assert_eq!(
            code, 0,
            "{opt} {ver} must pass via the upstream compat version. stderr: {stderr}"
        );
    }
}

#[test]
fn version_gate_refuses_beyond_the_compatible_version() {
    for (opt, ver) in [("-a", "3.17.2"), ("-a", "3.18"), ("-e", "3.16.0")] {
        let (code, _, stderr) = info(&[opt, ver]);
        assert_eq!(
            code, 1,
            "{opt} {ver} must fail: neither this build nor its compat version is {ver}"
        );
        assert!(
            stderr.contains("Version requirements not met"),
            "got: {stderr}"
        );
        assert!(
            stderr.contains("interface-compatible with upstream HFST 3.17.1"),
            "the refusal must name the compat version so the no is actionable, got: {stderr}"
        );
    }
}

#[test]
fn version_gate_accepts_this_version() {
    let v = env!("CARGO_PKG_VERSION");
    for opt in ["-a", "-e", "-m"] {
        let (code, _, stderr) = info(&[opt, v]);
        assert_eq!(code, 0, "{opt} {v} must pass. stderr: {stderr}");
    }
}

/// Upstream's `--max-version` used the `--atleast-version` comparison, so it
/// accepted only the builds it was supposed to reject. Neither the fork
/// version nor the compat version is ≤ 0.0.1, so this still rejects.
#[test]
fn max_version_rejects_a_newer_build() {
    let (code, _, _) = info(&["-m", "0.0.1"]);
    assert_eq!(code, 1, "-m 0.0.1 must reject a 0.1.0 build");
}

/// The `-f` gate and the listing read one table; if they ever drift apart
/// again, one of these pairs disagrees.
#[test]
fn listing_agrees_with_the_feature_gate() {
    let listing = listing();
    for (name, label) in [
        ("openfst", "OpenFst (tropical)"),
        ("foma", "foma"),
        ("icu", "Unicode (ICU)"),
        ("sfst", "SFST"),
        ("xfsm", "xfsm"),
        ("openfst-log", "OpenFst (log)"),
    ] {
        let says_yes = listing.lines().any(|l| l == format!("{label} supported"));
        let says_no = listing
            .lines()
            .any(|l| l == format!("{label} not supported"));
        assert!(
            says_yes ^ says_no,
            "the listing says nothing definite about {label}:\n{listing}"
        );
        let (code, _, _) = info(&["-f", name]);
        assert_eq!(
            code == 0,
            says_yes,
            "-f {name} and the listing disagree about {label}"
        );
    }
}
