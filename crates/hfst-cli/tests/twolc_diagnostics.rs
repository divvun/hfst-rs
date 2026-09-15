use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_twolc(args: &[&str], source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .env_remove("HFST_OPTIONS")
        .arg("twolc")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hfst twolc");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(source.as_bytes())
        .expect("write twolc source");
    child.wait_with_output().expect("wait for hfst twolc")
}

const FAO_MINIMAL: &str = concat!(
    "!! # Faroese æ comment\n",
    "Alphabet a b ;\n",
    "Sets\n",
    "Vow = a ;\n",
    "Rules\n",
    "\"R1\"\n",
    "a:b <=> vow _ # ;\n",
);

// [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.complete-alphabet-fn+1/test]
#[test]
fn undeclared_symbol_warns_at_context_and_compiles() {
    let output = run_twolc(&[], FAO_MINIMAL);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        output.stdout.starts_with(b"HFST"),
        "expected a rule archive"
    );
    assert!(stderr.contains("Warning:"), "{stderr}");
    assert!(stderr.contains("Symbol 'vow' is not declared"), "{stderr}");
    assert!(stderr.contains(":7:9"), "{stderr}");
    assert!(!stderr.contains(":1:1"), "{stderr}");
    assert!(!stderr.contains("Symbol '#'"), "{stderr}");
}

// [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.complete-alphabet-fn+1/test]
#[test]
fn silent_completion_writes_same_archive() {
    let normal = run_twolc(&[], FAO_MINIMAL);
    let silent = run_twolc(&["-q"], FAO_MINIMAL);
    assert!(normal.status.success());
    assert!(silent.status.success());
    assert_eq!(normal.stdout, silent.stdout);
    let stderr = String::from_utf8_lossy(&silent.stderr);
    assert!(!stderr.contains("is not declared"), "{stderr}");
}
