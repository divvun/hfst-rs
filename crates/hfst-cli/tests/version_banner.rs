//! Every tool is the one `hfst` binary reached under another name, so
//! `--version` must identify the product and the build the same way however
//! it was invoked. It used to print the invoked argv[0], so a tool run through
//! a symlink by path reported "Divvun /some/dir/hfst-strings2fst v0.1.0".

#![cfg(unix)]

use std::path::Path;
use std::process::Command;

/// The first non-empty line `--version` prints, from whichever stream.
fn first_version_line(mut command: Command) -> String {
    let out = command.arg("--version").output().expect("spawn hfst");
    assert!(out.status.success(), "--version must exit 0");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    text.lines()
        .find(|line| !line.is_empty())
        .expect("--version printed nothing")
        .to_string()
}

fn linked(dir: &Path, name: &str) {
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_hfst"), dir.join(name))
        .expect("symlink a tool name onto hfst");
}

// [spec:hfst:req:cli.version+1/test]
// [spec:hfst:sem:hfst-commandline.print-version-fn+1/test]
#[test]
fn every_entry_point_prints_the_same_identity_line() {
    let temp = tempfile::tempdir().expect("create a directory for the links");
    let dir = temp.path();
    linked(dir, "hfst-strings2fst");
    linked(dir, "hfst-compose");

    let umbrella = first_version_line(Command::new(env!("CARGO_BIN_EXE_hfst")));
    let expected = format!("Divvun HFST v{} (", env!("CARGO_PKG_VERSION"));
    assert!(
        umbrella.starts_with(&expected),
        "the identity line must name the product and version, got: {umbrella}"
    );

    let mut subcommand = Command::new(env!("CARGO_BIN_EXE_hfst"));
    subcommand.arg("strings2fst");
    let mut by_path = Command::new(dir.join("hfst-strings2fst"));
    by_path.current_dir(dir);
    let mut by_relative_path = Command::new("./hfst-compose");
    by_relative_path.current_dir(dir);

    for (how, command) in [
        ("hfst strings2fst", subcommand),
        ("an absolute symlink path", by_path),
        ("a relative symlink path", by_relative_path),
    ] {
        assert_eq!(
            first_version_line(command),
            umbrella,
            "--version through {how} must match the umbrella binary's"
        );
    }
}
