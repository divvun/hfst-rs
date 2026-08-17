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
        b"0\t1\ta\tb\n1\n",
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
        && (name.starts_with(".hfst-compose.") || name.starts_with(".rustfst-compose-state-table."))
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
fn scratch_detection_covers_materializer_and_pair_interner() {
    assert!(is_compose_scratch_artifact(".hfst-compose.123.456.scratch"));
    assert!(is_compose_scratch_artifact(
        ".rustfst-compose-state-table.random.scratch"
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
    assert!(
        stdout.contains("not supported for Foma composition"),
        "{stdout}"
    );
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
fn foma_auto_silent_explicit_limits_rejected() {
    let temp = tempfile::tempdir().expect("create compose working directory");
    let dir = temp.path();
    let (left, right) = build_foma_operands(dir);

    let automatic_path = dir.join("automatic-foma.hfst");
    let automatic = hfst()
        .current_dir(dir)
        .args([
            "compose",
            "-o",
            automatic_path.to_str().expect("UTF-8 temporary path"),
            &left,
            &right,
        ])
        .output()
        .expect("run automatic Foma compose");
    assert_success("automatic Foma compose", &automatic);
    let automatic_stderr = String::from_utf8_lossy(&automatic.stderr);
    assert!(
        !automatic_stderr.contains("memory allowance")
            && !automatic_stderr.contains("bounded spilling"),
        "automatic Foma compose emitted a memory-policy message: {automatic_stderr}"
    );

    let cli_path = dir.join("cli-foma.hfst");
    let explicit_cli = hfst()
        .current_dir(dir)
        .args([
            "compose",
            "--memory-limit=0",
            "-o",
            cli_path.to_str().expect("UTF-8 temporary path"),
            &left,
            &right,
        ])
        .output()
        .expect("run explicit-limit Foma compose");
    assert!(!explicit_cli.status.success());
    let cli_stderr = String::from_utf8_lossy(&explicit_cli.stderr);
    assert!(cli_stderr.contains("--memory-limit"), "{cli_stderr}");
    assert!(cli_stderr.contains("OpenFst tropical"), "{cli_stderr}");

    let env_path = dir.join("env-foma.hfst");
    let explicit_env = hfst()
        .current_dir(dir)
        .env("HFST_COMPOSE_MEMORY_LIMIT", "0")
        .args([
            "compose",
            "-o",
            env_path.to_str().expect("UTF-8 temporary path"),
            &left,
            &right,
        ])
        .output()
        .expect("run environment-limit Foma compose");
    assert!(!explicit_env.status.success());
    let env_stderr = String::from_utf8_lossy(&explicit_env.stderr);
    assert!(
        env_stderr.contains("HFST_COMPOSE_MEMORY_LIMIT"),
        "{env_stderr}"
    );
    assert!(env_stderr.contains("OpenFst tropical"), "{env_stderr}");
    assert!(
        scratch_artifacts(dir).is_empty(),
        "Foma policy checks left compose scratch behind: {:?}",
        scratch_artifacts(dir)
    );
}
