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

#[test]
fn set_centre_naming_no_pair_fails_at_rule() {
    let source = concat!(
        "Alphabet a b c a:b ;\n",
        "Sets\n",
        "Cns = a b ;\n",
        "Rules\n",
        "\"R1\"\n",
        "Cns:0 <=> _ c ;\n",
    );
    let output = run_twolc(&[], source);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(output.stdout.is_empty(), "no archive for a failed grammar");
    assert!(stderr.contains("The pair set Cns:0 is empty."), "{stderr}");
    assert!(stderr.contains(":5:1"), "{stderr}");
}

// [spec:hfst:sem:string-manipulation.unescape-fn/test]
#[test]
fn rule_names_are_stored_unescaped() {
    let source = concat!(
        "Alphabet a b %{p1%}:0 ;\n",
        "Rules\n",
        "\"%{p1%}:0 100%%\"\n",
        "%{p1%}:0 <=> _ a ;\n",
    );
    let output = run_twolc(&[], source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let archive = String::from_utf8_lossy(&output.stdout);
    assert!(archive.contains("\"{p1}:0 100%\""), "{archive}");
    assert!(!archive.contains("%{p1%}"), "{archive}");
}
