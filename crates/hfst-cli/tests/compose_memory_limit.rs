use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn hfst() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hfst"));
    command
        .env_remove("HFST_OPTIONS")
        .env_remove("HFST_COMPOSE_MEMORY_LIMIT");
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

fn build_missing_flag_operands(dir: &Path) -> (String, String) {
    let left = dir.join("left.hfst");
    let right = dir.join("right.hfst");
    let left_path = left.to_str().expect("UTF-8 temporary path").to_string();
    let right_path = right.to_str().expect("UTF-8 temporary path").to_string();

    // The left output carries a flag absent from the right alphabet, followed
    // by an ordinary label the right consumes. `-F` must therefore supply the
    // right-hand f:f self-loop; without it the composition has no accepting
    // path.
    let left_output = run_with_stdin(
        dir,
        &["txt2fst", "-o", &left_path],
        b"0\t1\ta\t@P.FEATURE.VALUE@\n1\t2\tb\tb\n2\n",
    );
    assert_success("build left operand", &left_output);
    let right_output = run_with_stdin(dir, &["txt2fst", "-o", &right_path], b"0\t1\tb\tc\n1\n");
    assert_success("build right operand", &right_output);

    (left_path, right_path)
}

#[cfg(feature = "foma")]
fn build_foma_operands(dir: &Path) -> (String, String) {
    let left = dir.join("left-foma.hfst");
    let right = dir.join("right-foma.hfst");
    let left_path = left.to_str().expect("UTF-8 temporary path").to_string();
    let right_path = right.to_str().expect("UTF-8 temporary path").to_string();

    let left_output = run_with_stdin(
        dir,
        &["txt2fst", "-f", "foma", "-o", &left_path],
        b"0\t1\ta\t@P.FEATURE.VALUE@\n1\t2\tb\tb\n2\n",
    );
    assert_success("build Foma left operand", &left_output);
    let right_output = run_with_stdin(
        dir,
        &["txt2fst", "-f", "foma", "-o", &right_path],
        b"0\t1\tb\tc\n1\n",
    );
    assert_success("build Foma right operand", &right_output);

    (left_path, right_path)
}

fn assert_semantically_equal(dir: &Path, first: &str, second: &str) {
    let output = hfst()
        .current_dir(dir)
        .args(["compare", first, second])
        .output()
        .expect("run hfst compare");
    assert_success("semantic comparison", &output);
}

fn is_compose_scratch_artifact(name: &str) -> bool {
    name.ends_with(".scratch")
        && (name.starts_with(".hfst-compose.")
            || name.starts_with(".rustfst-compose-state-table.")
            || name.starts_with(".foma-compose."))
}

fn scratch_artifacts(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .expect("read compose working directory")
        .map(|entry| {
            entry
                .expect("read compose working-directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| is_compose_scratch_artifact(name))
        .collect()
}

#[test]
fn scratch_detection_covers_all_compose_stores() {
    assert!(is_compose_scratch_artifact(".hfst-compose.123.456.scratch"));
    assert!(is_compose_scratch_artifact(
        ".rustfst-compose-state-table.random.scratch"
    ));
    assert!(is_compose_scratch_artifact(
        ".foma-compose.123.random.scratch"
    ));
    assert!(!is_compose_scratch_artifact("ordinary.scratch"));
    assert!(!is_compose_scratch_artifact(
        ".rustfst-compose-state-table.random"
    ));
}

#[test]
fn compose_help_documents_memory_allowance_policy() {
    let output = hfst()
        .args(["compose", "--help"])
        .output()
        .expect("run hfst compose --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--memory-limit=SIZE"), "{stdout}");
    assert!(stdout.contains("50% of available RAM"), "{stdout}");
    assert!(stdout.contains("OpenFst tropical"), "{stdout}");
    assert!(stdout.contains("Foma compose state"), "{stdout}");
    assert!(stdout.contains("HFST_COMPOSE_MEMORY_LIMIT"), "{stdout}");
}

#[test]
fn compose_intersect_help_documents_memory_policy() {
    let output = hfst()
        .args(["compose-intersect", "--help"])
        .output()
        .expect("run hfst compose-intersect --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--memory-limit=SIZE"), "{stdout}");
    assert!(stdout.contains("50% of available RAM"), "{stdout}");
    assert!(stdout.contains("one-rule"), "{stdout}");
    assert!(stdout.contains("HFST_COMPOSE_MEMORY_LIMIT"), "{stdout}");
}

#[test]
fn invalid_cli_precedes_input_open() {
    let output = hfst()
        .args(["compose", "--memory-limit=1.5GiB", "missing-a", "missing-b"])
        .output()
        .expect("run hfst compose with invalid memory allowance");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid value for --memory-limit"),
        "{stderr}"
    );
    assert!(stderr.contains("1.5GiB"), "{stderr}");
    assert!(
        !stderr.contains("missing-a"),
        "inputs were opened: {stderr}"
    );
}

#[test]
fn invalid_env_precedes_input_open() {
    let output = hfst()
        .env("HFST_COMPOSE_MEMORY_LIMIT", "1.5GiB")
        .args(["compose", "missing-a", "missing-b"])
        .output()
        .expect("run hfst compose with invalid environment allowance");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("HFST_COMPOSE_MEMORY_LIMIT"), "{stderr}");
    assert!(stderr.contains("1.5GiB"), "{stderr}");
    assert!(
        !stderr.contains("missing-a"),
        "inputs were opened: {stderr}"
    );
}

#[test]
fn missing_flag_spill_parity_and_cleanup() {
    let temp = tempfile::tempdir().expect("create compose working directory");
    let dir = temp.path();
    let (left, right) = build_missing_flag_operands(dir);
    let spilled = dir.join("spilled.hfst");
    let memory = dir.join("memory.hfst");
    let spilled_path = spilled.to_str().expect("UTF-8 temporary path");
    let memory_path = memory.to_str().expect("UTF-8 temporary path");

    let forced = hfst()
        .current_dir(dir)
        .args([
            "compose",
            "-F",
            "--memory-limit=0",
            "-o",
            spilled_path,
            &left,
            &right,
        ])
        .output()
        .expect("run forced-spill compose");
    assert_success("forced-spill compose", &forced);
    assert!(
        scratch_artifacts(dir).is_empty(),
        "scratch survived successful compose: {:?}",
        scratch_artifacts(dir)
    );

    let generous = hfst()
        .current_dir(dir)
        .args([
            "compose",
            "-F",
            "--memory-limit=1GiB",
            "-o",
            memory_path,
            &left,
            &right,
        ])
        .output()
        .expect("run in-memory compose");
    assert_success("in-memory compose", &generous);
    assert_semantically_equal(dir, spilled_path, memory_path);

    let text = hfst()
        .current_dir(dir)
        .args(["fst2txt", spilled_path])
        .output()
        .expect("render forced-spill result");
    assert_success("render forced-spill result", &text);
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("@P.FEATURE.VALUE@"), "{text}");
    assert!(text.contains("\tb\tc"), "{text}");
}

#[test]
fn compose_intersect_spill_parity_and_cleanup() {
    let temp = tempfile::tempdir().expect("create compose-intersect working directory");
    let dir = temp.path();
    let (lexicon, rule) = build_missing_flag_operands(dir);
    let spilled = dir.join("ci-spilled.hfst");
    let memory = dir.join("ci-memory.hfst");
    let spilled_path = spilled.to_str().expect("UTF-8 temporary path");
    let memory_path = memory.to_str().expect("UTF-8 temporary path");

    for (limit, output_path) in [("0", spilled_path), ("1GiB", memory_path)] {
        let memory_limit = format!("--memory-limit={limit}");
        let output = hfst()
            .current_dir(dir)
            .args([
                "compose-intersect",
                memory_limit.as_str(),
                "-o",
                output_path,
                &lexicon,
                &rule,
            ])
            .output()
            .expect("run bounded compose-intersect");
        assert_success("bounded compose-intersect", &output);
    }

    assert_semantically_equal(dir, spilled_path, memory_path);
    let text = hfst()
        .current_dir(dir)
        .args(["fst2txt", spilled_path])
        .output()
        .expect("render compose-intersect result");
    assert_success("render compose-intersect result", &text);
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("@P.FEATURE.VALUE@"), "{text}");
    assert!(text.contains("\tb\tc"), "{text}");
    assert!(
        scratch_artifacts(dir).is_empty(),
        "compose-intersect left scratch behind: {:?}",
        scratch_artifacts(dir)
    );
}

#[test]
fn compose_intersect_rejects_limited_fast_mode() {
    let temp = tempfile::tempdir().expect("create compose-intersect working directory");
    let dir = temp.path();
    let (lexicon, rule) = build_missing_flag_operands(dir);
    let output_path = dir.join("unsupported-fast.hfst");

    let output = hfst()
        .current_dir(dir)
        .args([
            "compose-intersect",
            "--fast",
            "--memory-limit=0",
            "-o",
            output_path.to_str().expect("UTF-8 temporary path"),
            &lexicon,
            &rule,
        ])
        .output()
        .expect("run explicitly limited fast compose-intersect");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("without --fast"), "{stderr}");
    assert!(scratch_artifacts(dir).is_empty());
}

#[test]
// [spec:hfst:req:virtual-flag-algebra.special-compose/test]
fn flag_epsilon_spill_parity_and_cleanup() {
    let temp = tempfile::tempdir().expect("create compose working directory");
    let dir = temp.path();
    let (left, right) = build_missing_flag_operands(dir);
    let spilled = dir.join("special-spilled.hfst");
    let memory = dir.join("special-memory.hfst");
    let spilled_path = spilled.to_str().expect("UTF-8 temporary path");
    let memory_path = memory.to_str().expect("UTF-8 temporary path");

    for (limit, output_path, label) in [
        ("0", spilled_path, "forced-spill special compose"),
        ("1GiB", memory_path, "in-memory special compose"),
    ] {
        let memory_limit = format!("--memory-limit={limit}");
        let output = hfst()
            .current_dir(dir)
            .args([
                "compose",
                "-F",
                "--xfst=flag-is-epsilon",
                memory_limit.as_str(),
                "-o",
                output_path,
                &left,
                &right,
            ])
            .output()
            .expect("run flag-as-epsilon compose");
        assert_success(label, &output);
    }

    assert_semantically_equal(dir, spilled_path, memory_path);
    let text = hfst()
        .current_dir(dir)
        .args(["fst2txt", spilled_path])
        .output()
        .expect("render special-mode result");
    assert_success("render special-mode result", &text);
    assert!(
        !text.stdout.is_empty(),
        "special-mode composition unexpectedly produced an empty transducer"
    );
    assert!(
        scratch_artifacts(dir).is_empty(),
        "scratch survived special-mode compose: {:?}",
        scratch_artifacts(dir)
    );
}

#[cfg(feature = "foma")]
#[test]
// [spec:hfst:req:foma-transducer.hfst.implementations.foma-transducer.resource-controlled-compose/test]
// [spec:hfst:req:virtual-flag-algebra.special-compose/test]
fn foma_limits_preserve_flag_compose() {
    let temp = tempfile::tempdir().expect("create compose working directory");
    let dir = temp.path();
    let (left, right) = build_foma_operands(dir);

    let spilled = dir.join("spilled-foma.hfst");
    let memory = dir.join("memory-foma.hfst");
    let spilled_path = spilled.to_str().expect("UTF-8 temporary path");
    let memory_path = memory.to_str().expect("UTF-8 temporary path");

    let forced = hfst()
        .current_dir(dir)
        .args([
            "compose",
            "-F",
            "--memory-limit=0",
            "-o",
            spilled_path,
            &left,
            &right,
        ])
        .output()
        .expect("run forced-spill Foma compose");
    assert_success("forced-spill Foma compose", &forced);

    let generous = hfst()
        .current_dir(dir)
        .env("HFST_COMPOSE_MEMORY_LIMIT", "1GiB")
        .args(["compose", "-F", "-o", memory_path, &left, &right])
        .output()
        .expect("run in-memory Foma compose");
    assert_success("in-memory Foma compose", &generous);
    assert_semantically_equal(dir, spilled_path, memory_path);

    let text = hfst()
        .current_dir(dir)
        .args(["fst2txt", spilled_path])
        .output()
        .expect("render forced-spill Foma result");
    assert_success("render forced-spill Foma result", &text);
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("@P.FEATURE.VALUE@"), "{text}");
    assert!(text.contains("\tb\tc"), "{text}");

    let special_spilled = dir.join("special-spilled-foma.hfst");
    let special_memory = dir.join("special-memory-foma.hfst");
    let special_spilled_path = special_spilled.to_str().expect("UTF-8 temporary path");
    let special_memory_path = special_memory.to_str().expect("UTF-8 temporary path");
    for (limit, output_path) in [("0", special_spilled_path), ("1GiB", special_memory_path)] {
        let memory_limit = format!("--memory-limit={limit}");
        let output = hfst()
            .current_dir(dir)
            .args([
                "compose",
                "-F",
                "--xfst=flag-is-epsilon",
                memory_limit.as_str(),
                "-o",
                output_path,
                &left,
                &right,
            ])
            .output()
            .expect("run Foma special-mode composition");
        assert_success("Foma special-mode composition", &output);
    }
    assert_semantically_equal(dir, special_spilled_path, special_memory_path);

    assert!(
        scratch_artifacts(dir).is_empty(),
        "Foma composition left scratch behind: {:?}",
        scratch_artifacts(dir)
    );
}
