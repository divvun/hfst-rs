//! The tools print the diagnostics the compilers hand back. For each known
//! error, stderr must match its recorded fixture byte for byte.

use std::io::Write;
use std::process::{Command, Stdio};

/// Run `hfst ARGS` on `stdin` with stderr redirected, returning the exit code
/// and stderr without the timing line, whose seconds vary.
fn stderr_of(args: &[&str], stdin: &str) -> (Option<i32>, String) {
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
        .expect("piped stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst");
    let stderr = String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter(|l| !l.contains("hfst-time"))
        .map(|l| format!("{l}\n"))
        .collect();
    (out.status.code(), stderr)
}

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/diagnostics_stderr/{name}.stderr",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).expect("read the recorded stderr")
}

// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn known_errors_read_as_before() {
    let cases: [(&str, &[&str], &str); 5] = [
        (
            "regexp2fst",
            &["regexp2fst", "-S", "-o", "/dev/null"],
            "[a b\n",
        ),
        (
            "lexc",
            &["lexc", "-f", "openfst-tropical", "-o", "/dev/null"],
            "LEXICON Root\nabc # \n",
        ),
        (
            "twolc",
            &["twolc", "-o", "/dev/null"],
            "Alphabet a b ;\nRules\n\"r\" a:b => _ [ ;\n",
        ),
        (
            "pmatch2fst",
            &["pmatch2fst", "-o", "/dev/null"],
            "Define TOP {a} L( ;\n",
        ),
        ("xfst", &["xfst", "-p"], "regex a b ;\ndetrminize\n"),
    ];
    for (name, args, input) in cases {
        let (code, stderr) = stderr_of(args, input);
        assert_eq!(code, Some(1), "{name}: stderr:\n{stderr}");
        assert_eq!(stderr, fixture(name), "{name}");
    }
}

// A warning lands among the progress lines the compile logs, where it was
// produced, not after them.
// [spec:hfst:req:javascript.diagnostics-as-data/test]
#[test]
fn warnings_keep_their_place_among_log_lines() {
    let cases: [(&str, &[&str], &str); 3] = [
        (
            "twolc-warn",
            &["twolc", "-o", "/dev/null"],
            "Alphabet a b ;\nRules\n\"r\" a:b => _ c ;\n",
        ),
        (
            "lexc-warn",
            &["lexc", "-f", "openfst-tropical", "-o", "/dev/null"],
            "Definitions\nV = [a|e] ;\nLEXICON Root\nLexicon Foo\nab Foo ;\nLEXICON Foo\nc # ;\n",
        ),
        (
            "xfst-warn",
            &["xfst", "-p"],
            "set compose-flag-as-special ON\nhfst raw\nregex a ;\n",
        ),
    ];
    for (name, args, input) in cases {
        let (code, stderr) = stderr_of(args, input);
        assert_eq!(code, Some(0), "{name}: stderr:\n{stderr}");
        assert_eq!(stderr, fixture(name), "{name}");
    }
}
