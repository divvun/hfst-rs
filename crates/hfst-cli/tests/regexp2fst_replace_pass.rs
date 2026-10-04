//! `hfst regexp2fst --replace-pass`: one pass per expression, and refusals
//! that exit non-zero and point at the file's own line.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use hfst::hfst_input_stream::HfstInputStream;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hfst-regexp2fst-replace-pass-{name}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Run `hfst regexp2fst ARGS` on `input`, returning (exit code, stderr).
fn regexp2fst(args: &[&str], input: &str) -> (Option<i32>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .arg("regexp2fst")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hfst regexp2fst");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst regexp2fst");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn transducer_count(path: &std::path::Path) -> usize {
    let mut stream =
        HfstInputStream::new_filename(path.to_str().expect("utf8 path")).expect("open output");
    let mut n = 0;
    while !stream.is_eof() {
        stream.read().expect("read a pass");
        n += 1;
    }
    n
}

// [spec:hfst:req:xre-replace-pass.cli/test]
#[test]
fn each_semicolon_separated_expression_is_one_pass() {
    let out = scratch("two").join("passes.hfst");
    let input =
        "! hand strings\n[ {ab} (->) x::1 , b (->) 0::2 ] ;\n\n[ {ba} (->) y::3 || _ .#. ] ;\n";
    let (code, stderr) = regexp2fst(
        &[
            "-S",
            "--replace-pass",
            "-o",
            out.to_str().expect("utf8 path"),
        ],
        input,
    );
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(transducer_count(&out), 2);
}

// [spec:hfst:req:xre-replace-pass.cli/test]
// [spec:hfst:req:xre-replace-pass.refusals/test]
#[test]
fn refusal_names_the_rule_and_file_line() {
    let out = scratch("refused").join("passes.hfst");
    let input = "[ a (->) b ] ;\n\n! second pass\n[ {xy} (->) z::1 ,\n  c -> d ,,\n  e (->) f || g _ .#. ] ;\n";
    let (code, stderr) = regexp2fst(
        &[
            "-S",
            "--replace-pass",
            "-o",
            out.to_str().expect("utf8 path"),
        ],
        input,
    );
    assert_eq!(code, Some(1), "{stderr}");
    assert!(stderr.contains("mapping 2 of rule 1 uses '->'"), "{stderr}");
    assert!(
        stderr.contains(":5:3"),
        "position of 'c -> d' missing: {stderr}"
    );
    assert!(
        stderr.contains("rule 2 has a context other than the pass boundary"),
        "{stderr}"
    );
    assert!(
        stderr.contains(":6:15"),
        "position of 'g' missing: {stderr}"
    );
}
