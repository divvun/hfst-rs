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

const SET_CENTRE: &str = concat!(
    "Alphabet a b c a:b ;\n",
    "Sets\n",
    "Cns = a b ;\n",
    "Rules\n",
    "\"R1\"\n",
    "Cns:0 <=> _ c ;\n",
);

const EMPTY_CONTEXT: &str = concat!(
    "Alphabet a b c a:b ;\n",
    "Sets\n",
    "Cns = a b ;\n",
    "Rules\n",
    "\"R1\"\n",
    "a:b <=> _ Cns:c ;\n",
);

#[test]
fn set_centre_declares_its_pairs_with_a_warning() {
    let output = run_twolc(&[], SET_CENTRE);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        output.stdout.starts_with(b"HFST"),
        "expected a rule archive"
    );
    assert!(
        stderr.contains("The rule centre Cns:0 declares 2 pairs the Alphabet does not list."),
        "{stderr}"
    );
    // Anchored at the centre pair, not the rule name above it.
    assert!(stderr.contains(":6:1"), "{stderr}");
    assert!(
        stderr.contains("List them in the Alphabet to silence this warning: a:0 b:0"),
        "{stderr}"
    );
}

#[test]
fn empty_context_set_points_at_pair_and_set() {
    let output = run_twolc(&[], EMPTY_CONTEXT);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(output.stdout.is_empty(), "no archive for a failed grammar");
    assert!(stderr.contains("The pair set Cns:c is empty."), "{stderr}");
    assert!(stderr.contains(":6:11"), "{stderr}");
    assert!(
        stderr.contains("Cns is defined here, with 2 symbols"),
        "{stderr}"
    );
    assert!(
        stderr.contains("No symbol in Cns (2 symbols) is declared with c as its lower side."),
        "{stderr}"
    );
    assert!(
        stderr.contains("Declare the pairs it should cover in the Alphabet: a:c b:c"),
        "{stderr}"
    );
    // A context was an error in C++ too, so no note about it being dropped.
    assert!(!stderr.contains("C++ hfst-twolc dropped"), "{stderr}");
}

#[test]
fn impossible_set_centre_pairs_are_an_error() {
    let source = concat!(
        "Alphabet a b @P.x.on@ ;\n",
        "Diacritics @P.x.on@ ;\n",
        "Sets\n",
        "Dia = @P.x.on@ ;\n",
        "Rules\n",
        "\"R1\"\n",
        "Dia:a <=> _ b ;\n",
    );
    let output = run_twolc(&[], source);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("The pair set Dia:a is empty."), "{stderr}");
    assert!(stderr.contains("None of its pairs can exist"), "{stderr}");
    assert!(
        stderr.contains("C++ hfst-twolc dropped a rule like this"),
        "{stderr}"
    );
}

#[test]
fn silent_mode_hides_warnings_but_not_errors() {
    let warned = run_twolc(&["-q"], SET_CENTRE);
    assert!(warned.status.success());
    let stderr = String::from_utf8_lossy(&warned.stderr);
    assert!(!stderr.contains("declares 2 pairs"), "{stderr}");

    let failed = run_twolc(&["-q"], EMPTY_CONTEXT);
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(!failed.status.success());
    assert!(stderr.contains("The pair set Cns:c is empty."), "{stderr}");
}

#[test]
fn redirected_diagnostics_carry_no_colour_codes() {
    let output = run_twolc(&[], EMPTY_CONTEXT);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("is empty"), "{stderr}");
    assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
}

#[test]
fn every_bad_rule_is_reported_in_one_run() {
    let source = concat!(
        "Alphabet a b c a:b ;\n",
        "Sets\n",
        "Cns = a b ;\n",
        "Rules\n",
        "\"first\"\n",
        "a:b <=> _ Cns:c ;\n",
        "\"second\"\n",
        "a:b <=> Cns:0 _ ;\n",
    );
    let output = run_twolc(&[], source);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("The pair set Cns:c is empty."), "{stderr}");
    assert!(stderr.contains("The pair set Cns:0 is empty."), "{stderr}");
    assert!(stderr.contains(":8:9"), "{stderr}");
}

#[test]
fn undeclared_symbol_suggests_the_set_it_resembles() {
    let output = run_twolc(&[], FAO_MINIMAL);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("Did you mean the set Vow?"), "{stderr}");
}

#[test]
fn diacritic_pair_warning_labels_each_side() {
    let source = concat!(
        "Alphabet a b @P.x.on@ ;\n",
        "Diacritics @P.x.on@ ;\n",
        "Sets\n",
        "X = a ;\n",
        "Rules\n",
        "\"R1\"\n",
        "a:b <=> _ @P.x.on@:X ;\n",
    );
    let output = run_twolc(&[], source);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("@P.x.on@ is declared a diacritic"),
        "{stderr}"
    );
    assert!(stderr.contains("so this side is ignored"), "{stderr}");
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
