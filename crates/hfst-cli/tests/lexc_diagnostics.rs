use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_lexc(args: &[&str], source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .env_remove("HFST_OPTIONS")
        .env("NO_COLOR", "1")
        .arg("lexc")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hfst lexc");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(source.as_bytes())
        .expect("write lexc source");
    child.wait_with_output().expect("wait for hfst lexc")
}

#[test]
fn unicode_prefix_does_not_shift_grapheme_report() {
    let source = concat!(
        "LEXICON Root\n",
        "组织机构 enddomain ;\n",
        "健康 enddomain ;\n",
        "भा enddomain ;\n",
        "LEXICON enddomain\n",
        "# ;\n",
    );
    let output = run_lexc(&["-f", "openfst-tropical", "-Werror"], source);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("<stdin>:4:1"), "{stderr}");
    assert!(
        stderr.contains("undeclared multi-code-point grapheme 'भा'"),
        "{stderr}"
    );
}

#[test]
fn implicit_literal_zero_is_info_under_werror() {
    let source = "Multichar_Symbols a b\nLEXICON Root\na%0:b # ;\n";
    let output = run_lexc(
        &[
            "-f",
            "openfst-tropical",
            "-A",
            "-Werror",
            "-Wmissing-alphabets",
        ],
        source,
    );

    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Info:"), "{stderr}");
    assert!(
        stderr.contains("Adding 0 to Alphabets [-Wmissing-alphabets]"),
        "{stderr}"
    );
    assert!(!stderr.contains("Error: Adding 0"), "{stderr}");
    assert!(!stderr.contains("Warning: Adding 0"), "{stderr}");
}
