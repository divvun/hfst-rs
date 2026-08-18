use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const FLAG: &str = "@P.FEATURE.VALUE@";

fn hfst() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hfst"));
    command.env_remove("HFST_OPTIONS");
    command
}

fn run_with_stdin(dir: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = hfst()
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hfst");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input)
        .expect("write hfst stdin");
    child.wait_with_output().expect("wait for hfst")
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn exercise(format: Option<&str>) {
    let temp = tempfile::tempdir().expect("create conjunct working directory");
    let dir = temp.path();
    let left = dir.join("left.hfst");
    let right = dir.join("right.hfst");
    let result = dir.join("result.hfst");
    let left_path = left.to_str().expect("UTF-8 left path");
    let right_path = right.to_str().expect("UTF-8 right path");
    let result_path = result.to_str().expect("UTF-8 result path");

    let mut left_args = vec!["txt2fst"];
    if let Some(format) = format {
        left_args.extend(["-f", format]);
    }
    left_args.extend(["-o", left_path]);
    let left_output = run_with_stdin(
        dir,
        &left_args,
        format!("0\t1\t{FLAG}\t{FLAG}\n1\t2\ta\ta\n2\n").as_bytes(),
    );
    assert_success("build left intersection operand", &left_output);

    let mut right_args = vec!["txt2fst"];
    if let Some(format) = format {
        right_args.extend(["-f", format]);
    }
    right_args.extend(["-o", right_path]);
    let right_output = run_with_stdin(dir, &right_args, b"0\t1\ta\ta\n1\n");
    assert_success("build right intersection operand", &right_output);

    let conjunct = hfst()
        .current_dir(dir)
        .args(["conjunct", "-F", "-o", result_path, left_path, right_path])
        .output()
        .expect("run hfst-conjunct");
    assert_success("virtual flag conjunction", &conjunct);
    assert!(
        !String::from_utf8_lossy(&conjunct.stderr)
            .contains("materializing missing flag-diacritic self-loops eagerly"),
        "conjunct selected eager harmonization: {}",
        String::from_utf8_lossy(&conjunct.stderr)
    );

    let text = hfst()
        .current_dir(dir)
        .args(["fst2txt", result_path])
        .output()
        .expect("render conjunction result");
    assert_success("render conjunction result", &text);
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(
        text.contains(FLAG),
        "missing virtual flag transition: {text}"
    );
    assert!(
        text.contains("\ta\ta"),
        "missing ordinary transition: {text}"
    );
}

// [spec:hfst:req:virtual-flag-algebra.intersection/test]
#[test]
fn conjunct_routes_virtual_flags() {
    exercise(None);
    #[cfg(feature = "foma")]
    exercise(Some("foma"));
}
