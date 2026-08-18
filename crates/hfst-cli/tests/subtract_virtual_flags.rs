use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FLAG: &str = "@P.FEATURE.VALUE@";

fn hfst() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hfst"));
    command.env_remove("HFST_OPTIONS");
    command
}

fn build_operand(dir: &Path, output_path: &Path, format: Option<&str>, source: &str) -> Output {
    let source_path = PathBuf::from(output_path).with_extension("att");
    std::fs::write(&source_path, source).expect("write ATT fixture");

    let mut args = vec!["txt2fst"];
    if let Some(format) = format {
        args.extend(["-f", format]);
    }
    args.extend([
        "-o",
        output_path.to_str().expect("UTF-8 output path"),
        source_path.to_str().expect("UTF-8 source path"),
    ]);
    hfst()
        .current_dir(dir)
        .args(args)
        .output()
        .expect("build subtraction operand")
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
    let temp = tempfile::tempdir().expect("create subtract working directory");
    let dir = temp.path();
    let left = dir.join("left.hfst");
    let right = dir.join("right.hfst");
    let result = dir.join("result.hfst");
    let left_path = left.to_str().expect("UTF-8 left path");
    let right_path = right.to_str().expect("UTF-8 right path");
    let result_path = result.to_str().expect("UTF-8 result path");

    let left_source =
        format!("0\t1\t{FLAG}\t{FLAG}\n1\t2\ta\ta\n2\n0\t3\t{FLAG}\t{FLAG}\n3\t4\tb\tb\n4\n");
    let left_output = build_operand(dir, &left, format, &left_source);
    assert_success("build left subtraction operand", &left_output);

    let right_source = format!("0\t1\t{FLAG}\t{FLAG}\n1\t2\ta\ta\n2\n");
    let right_output = build_operand(dir, &right, format, &right_source);
    assert_success("build right subtraction operand", &right_output);

    let subtract = hfst()
        .current_dir(dir)
        .args(["subtract", "-F", "-o", result_path, left_path, right_path])
        .output()
        .expect("run hfst-subtract");
    assert_success("virtual flag subtraction", &subtract);
    assert!(
        !String::from_utf8_lossy(&subtract.stderr)
            .contains("materializing missing flag-diacritic self-loops eagerly"),
        "subtract selected eager harmonization: {}",
        String::from_utf8_lossy(&subtract.stderr)
    );

    let text = hfst()
        .current_dir(dir)
        .args(["fst2txt", result_path])
        .output()
        .expect("render subtraction result");
    assert_success("render subtraction result", &text);
    assert!(
        String::from_utf8_lossy(&text.stdout).contains("\tb\tb"),
        "subtraction lost the unmatched left branch: {}",
        String::from_utf8_lossy(&text.stdout)
    );
}

// [spec:hfst:req:virtual-flag-algebra.subtraction/test]
#[test]
fn subtract_routes_virtual_flags() {
    exercise(None);
    #[cfg(feature = "foma")]
    exercise(Some("foma"));
}
