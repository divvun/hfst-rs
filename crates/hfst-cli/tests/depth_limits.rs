//! The compiler tools fail with an error on input nested past the library's
//! depth limits (hfst::depth), which would overflow even the tools' large
//! stack: each exits 1 and names the limit, never killed by a signal.

use std::io::Write;
use std::process::{Command, Stdio};

use hfst::depth::{MAX_EXPRESSION_DEPTH, MAX_NESTING_DEPTH};

/// Run `hfst ARGS` with `input` as its standard input, returning its exit
/// code, the signal that killed it, if one did, and its stderr.
fn run(args: &[&str], input: &str) -> (Option<i32>, Option<i32>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .env_remove("HFST_OPTIONS")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hfst");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst");
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        out.status.signal()
    };
    #[cfg(not(unix))]
    let signal = None;
    (
        out.status.code(),
        signal,
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn assert_refused(args: &[&str], input: &str, limit: &str) {
    let (code, signal, stderr) = run(args, input);
    assert_eq!(signal, None, "hfst {args:?} was killed:\n{stderr}");
    assert_eq!(code, Some(1), "hfst {args:?}:\n{stderr}");
    assert!(
        stderr.contains(limit),
        "hfst {args:?} did not say {limit:?}:\n{stderr}"
    );
}

fn brackets(n: usize) -> String {
    format!("{}a{}", "[".repeat(n), "]".repeat(n))
}

const NESTED: &str = "brackets are nested more than";
const OPERATORS: &str = "operators are nested more than";

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch2fst_refuses_too_deep_scripts() {
    let args = ["pmatch2fst", "-o", "/dev/null"];
    let nested = brackets(MAX_NESTING_DEPTH + 1);
    assert_refused(&args, &format!("Define TOP {nested} ;\n"), NESTED);
    let chain = vec!["a"; MAX_EXPRESSION_DEPTH + 1].join(" | ");
    assert_refused(&args, &format!("Define TOP {chain} ;\n"), OPERATORS);
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn twolc_refuses_too_deep_rules() {
    let args = ["twolc", "-o", "/dev/null"];
    let rule = |regex: &str| format!("Alphabet a b ;\nRules\n\"r\"\na:b <=> {regex} _ ;\n");
    assert_refused(&args, &rule(&brackets(MAX_NESTING_DEPTH + 1)), NESTED);
    let operators = format!("{}a", "~".repeat(MAX_EXPRESSION_DEPTH + 1));
    assert_refused(&args, &rule(&operators), OPERATORS);
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn xfst_and_lexc_refuse_too_deep_regular_expressions() {
    let nested = brackets(MAX_NESTING_DEPTH + 1);
    assert_refused(&["xfst", "-p"], &format!("regex {nested} ;\n"), NESTED);
    assert_refused(
        &["lexc", "-o", "/dev/null"],
        &format!("LEXICON Root\n< {nested} > # ;\n"),
        NESTED,
    );
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn regexp2fst_refuses_operators_nested_too_deep() {
    let operators = format!("{}a ;\n", "~".repeat(MAX_EXPRESSION_DEPTH + 1));
    assert_refused(
        &["regexp2fst", "-S", "-o", "/dev/null"],
        &operators,
        OPERATORS,
    );
}
