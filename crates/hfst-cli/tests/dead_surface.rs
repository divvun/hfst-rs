// Options with no implementation behind them are not declared, so they are
// rejected as unknown instead of being accepted only to fail
// (docs/spec/xfst-commands.md).
use std::process::Command;

fn stderr_of(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(args)
        .output()
        .expect("run hfst");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// [spec:hfst:req:xfst-cmd.no-dead-surface/test]
#[test]
fn grep_rejects_options_it_never_had() {
    for flag in [
        "-I",
        "-r",
        "-E",
        "-P",
        "--include=x",
        "--binary-files=text",
        "-u",
    ] {
        let (ok, err) = stderr_of(&["grep", flag, "a"]);
        assert!(!ok, "{flag} was accepted");
        assert!(err.contains("Unknown option"), "{flag}: {err}");
    }
}

// [spec:hfst:req:xfst-cmd.no-dead-surface/test]
#[test]
fn fst2fst_has_no_xfsm_switch() {
    let (ok, err) = stderr_of(&["fst2fst", "-x"]);
    assert!(!ok);
    assert!(err.contains("Unknown option"), "{err}");
}

// [spec:hfst:sem:xre-utils.hfst.xre.check-multichar-symbol-fn/test]
// In verbose mode lexc warns about a multichar symbol a regex entry uses but
// the Multichar_Symbols section never declared.
#[test]
fn lexc_warns_about_undeclared_multichar_symbols() {
    let dir = std::env::temp_dir().join(format!("hfst-dead-surface-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let src = dir.join("mc.lexc");
    std::fs::write(
        &src,
        "Multichar_Symbols\n+N\n\nLEXICON Root\n< a \"+N\" > # ;\n< b \"+X\" > # ;\n",
    )
    .expect("write lexc");
    let out = dir.join("mc.hfst");
    let (ok, err) = stderr_of(&[
        "lexc",
        "-v",
        src.to_str().expect("utf8"),
        "-o",
        out.to_str().expect("utf8"),
    ]);
    assert!(ok, "{err}");
    assert!(
        err.contains("multichar symbol '+X' used but not defined"),
        "{err}"
    );
    assert!(!err.contains("'+N' used but not defined"), "{err}");
}
