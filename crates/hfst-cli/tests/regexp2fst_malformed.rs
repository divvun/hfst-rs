//! Regression locks for upstream hfst#253 — "hfst-regexp2fst segfaults on
//! malformed input". In C++ a battery of malformed regular expressions could
//! segfault or abort the tool. The Rust port must NEVER panic/abort on
//! malformed regex input: it must produce a CLEAN diagnostic (a non-zero exit
//! plus an error message) and exit gracefully.
//!
//! Each test drives the real `hfst regexp2fst` binary over an input that either
//! used to crash or exercises a semantic-error path, and asserts the outcome is
//! a clean parse failure — exit code exactly `1`, never a Rust panic (`101`),
//! an abort/`SIGABRT` (`134`), a `SIGSEGV` (`139`) or a `SIGILL` — accompanied
//! by a diagnostic on stderr.
//!
//! The tool runs on a large-stack worker thread (hfst#287); a panic there still
//! surfaces as exit `101`, so `101` is treated as a FAILURE here, not success.

use std::io::Write;
use std::process::{Command, Stdio};

/// Outcome of one `hfst regexp2fst` run over a malformed input.
struct Outcome {
    /// The process exit code, if it exited normally (not killed by a signal).
    code: Option<i32>,
    /// The signal number, if the process was killed by one (Unix only).
    signal: Option<i32>,
    /// Everything the tool wrote to stderr.
    stderr: String,
}

/// Feed `input` to `hfst regexp2fst <mode> -o /dev/null` and capture the result.
/// `mode` is `"-S"` (semicolon-separated) or `"-l"` (line-separated).
fn run_regexp2fst(mode: &str, input: &[u8]) -> Outcome {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(["regexp2fst", mode, "-o", "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hfst regexp2fst");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst regexp2fst");

    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        out.status.signal()
    };
    #[cfg(not(unix))]
    let signal = None;

    Outcome {
        code: out.status.code(),
        signal,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Assert that a malformed `input` produces a CLEAN diagnostic in both the
/// semicolon (`-S`) and line (`-l`) input modes: it is never killed by a signal
/// (no `SIGSEGV`/`SIGABRT`/`SIGILL`), never panics (exit `101`), exits with the
/// tool's ordinary error code `1`, and prints something to stderr.
fn assert_clean_rejection(input: &[u8]) {
    for mode in ["-S", "-l"] {
        let o = run_regexp2fst(mode, input);
        let label = format!("mode {mode}, input {input:?}");

        assert!(
            o.signal.is_none(),
            "{label}: killed by signal {:?} (segfault/abort) — must never happen",
            o.signal
        );
        assert_ne!(
            o.code,
            Some(101),
            "{label}: exited 101 (Rust panic) — malformed input must not panic\nstderr: {}",
            o.stderr
        );
        assert_eq!(
            o.code,
            Some(1),
            "{label}: expected a clean exit code 1, got {:?}\nstderr: {}",
            o.code,
            o.stderr
        );
        assert!(
            !o.stderr.trim().is_empty(),
            "{label}: exited 1 but emitted no diagnostic on stderr",
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Parse-level malformed input (unbalanced/dangling/incomplete syntax).
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn unbalanced_open_bracket() {
    assert_clean_rejection(b"[a\n");
}

#[test]
fn unbalanced_close_bracket() {
    assert_clean_rejection(b"a]\n");
}

#[test]
fn dangling_range_dash() {
    assert_clean_rejection(b"[a-\n");
}

#[test]
fn unbalanced_open_paren() {
    assert_clean_rejection(b"(a\n");
}

#[test]
fn dangling_union_right() {
    assert_clean_rejection(b"a |\n");
}

#[test]
fn dangling_union_left() {
    assert_clean_rejection(b"| a\n");
}

#[test]
fn dangling_intersect() {
    assert_clean_rejection(b"a &\n");
}

#[test]
fn lone_star() {
    assert_clean_rejection(b"*\n");
}

#[test]
fn leading_plus() {
    assert_clean_rejection(b"+ b\n");
}

#[test]
fn lone_backslash() {
    assert_clean_rejection(b"\\\n");
}

#[test]
fn unterminated_quote() {
    assert_clean_rejection(b"\"abc\n");
}

#[test]
fn unterminated_curly() {
    assert_clean_rejection(b"{abc\n");
}

#[test]
fn invalid_flag_diacritic() {
    assert_clean_rejection(b"@X.@\n");
}

#[test]
fn weight_syntax_error() {
    assert_clean_rejection(b"a::x\n");
}

#[test]
fn stray_right_arrow() {
    assert_clean_rejection(b"->\n");
}

#[test]
fn stray_double_arrow() {
    assert_clean_rejection(b"=>\n");
}

#[test]
fn dangling_pair_colon_upper() {
    assert_clean_rejection(b"a:\n");
}

#[test]
fn dangling_pair_colon_lower() {
    assert_clean_rejection(b":b\n");
}

#[test]
fn nested_unbalanced_brackets() {
    assert_clean_rejection(b"[[[a\n");
}

// ──────────────────────────────────────────────────────────────────────────
// Semantic-error paths that used to reach `panic_any(...)` in the evaluator
// (upstream hfst#253 crash sites). These parse into a valid AST but fail
// during transducer construction, and MUST propagate as a clean parse failure.
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn left_right_replace_arrow() {
    // `<->` reaches the `ReplaceArrow::LeftRight` arm, formerly a `panic_any`.
    assert_clean_rejection(b"a <-> b\n");
}

#[test]
fn replace_context_not_automaton() {
    // A transducer (`c:d`) as a replace context formerly hit
    // `panic_any("Contexts need to be automata")`.
    assert_clean_rejection(b"a -> b || c:d _ ;\n");
}

#[test]
fn replace_context_not_automaton_both_sides() {
    assert_clean_rejection(b"a -> b || c:d _ e:f ;\n");
}

#[test]
fn read_text_missing_file() {
    // `@txt"..."` on a missing file formerly hit `panic!("File cannot be opened.")`.
    assert_clean_rejection(b"@txt\"/nonexistent/hfst-253/nope.txt\"\n");
}

#[test]
fn read_spaced_missing_file() {
    assert_clean_rejection(b"@stxt\"/nonexistent/hfst-253/nope.txt\"\n");
}

#[test]
fn read_prolog_missing_file() {
    assert_clean_rejection(b"@pl\"/nonexistent/hfst-253/nope.pl\"\n");
}

#[test]
fn read_regex_missing_file() {
    assert_clean_rejection(b"@re\"/nonexistent/hfst-253/nope.re\"\n");
}

// ──────────────────────────────────────────────────────────────────────────
// Pathologically deep nesting (parser + evaluator recursion). Without a depth
// guard this overflowed even the worker thread's large stack and aborted the
// process with SIGABRT.
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn deep_unbalanced_open_brackets() {
    let input = "[".repeat(100_000);
    assert_clean_rejection(input.as_bytes());
}

#[test]
fn deep_balanced_bracket_nesting() {
    let mut input = "[".repeat(50_000);
    input.push('a');
    input.push_str(&"]".repeat(50_000));
    input.push('\n');
    assert_clean_rejection(input.as_bytes());
}

#[test]
fn deep_balanced_paren_nesting() {
    let mut input = "(".repeat(50_000);
    input.push('a');
    input.push_str(&")".repeat(50_000));
    input.push('\n');
    assert_clean_rejection(input.as_bytes());
}

// ──────────────────────────────────────────────────────────────────────────
// Byte-level hostility: NUL bytes and non-UTF-8 input.
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn embedded_nul_byte() {
    assert_clean_rejection(b"a\x00b\n");
}

#[test]
fn invalid_utf8_bytes() {
    assert_clean_rejection(b"\xff\xfe\n");
}

#[test]
fn invalid_utf8_inside_expression() {
    assert_clean_rejection(b"a\xffb\n");
}

#[test]
fn invalid_utf8_after_open_bracket() {
    assert_clean_rejection(b"[\x80\n");
}

// ──────────────────────────────────────────────────────────────────────────
// Positive control: a well-formed expression, including deep-but-legal nesting,
// still compiles successfully (the depth guard must not reject real grammars).
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn valid_expression_still_compiles() {
    for mode in ["-S", "-l"] {
        let o = run_regexp2fst(mode, b"a:b\n");
        assert!(
            o.signal.is_none(),
            "mode {mode}: valid input killed by signal {:?}",
            o.signal
        );
        assert_eq!(
            o.code,
            Some(0),
            "mode {mode}: valid input should compile cleanly (exit 0), got {:?}\nstderr: {}",
            o.code,
            o.stderr
        );
    }
}

#[test]
fn valid_deep_but_legal_nesting_compiles() {
    // Well below the depth guard's ceiling: a real grammar can nest this far.
    let mut input = "[".repeat(3_000);
    input.push('a');
    input.push_str(&"]".repeat(3_000));
    input.push('\n');
    let o = run_regexp2fst("-S", input.as_bytes());
    assert!(
        o.signal.is_none(),
        "deep-but-legal input killed by signal {:?}",
        o.signal
    );
    assert_eq!(
        o.code,
        Some(0),
        "deep-but-legal input should compile cleanly (exit 0), got {:?}\nstderr: {}",
        o.code,
        o.stderr
    );
}
