//! Regression locks for crashes found during the upstream-bugs validation
//! (2026-07). Each test drives the real `hfst` binary over an input that used
//! to abort the process, and asserts a clean, correct result instead.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hfst-crash-regress-{name}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Run `hfst <args>` with `stdin`, returning (success, stdout).
fn run(args: &[&str], stdin: &[u8]) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn hfst");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// Compile `regex` to a weighted optimized-lookup file at `out`.
fn build_ol(regex: &str, out: &Path) {
    let hfst = out.with_extension("hfst");
    let (ok, _) = run(
        &[
            "regexp2fst",
            "-S",
            "-f",
            "openfst-tropical",
            "-o",
            hfst.to_str().expect("utf8 path"),
        ],
        regex.as_bytes(),
    );
    assert!(ok, "regexp2fst failed for {regex}");
    let (ok, _) = run(
        &[
            "fst2fst",
            "-w",
            "-i",
            hfst.to_str().expect("utf8 path"),
            "-o",
            out.to_str().expect("utf8 path"),
        ],
        b"",
    );
    assert!(ok, "fst2fst -w failed for {regex}");
}

/// hfst/hfst#395 companion (port-found): `hfst lookup` over a stream holding
/// two concatenated optimized-lookup transducers must union them, not panic
/// (was: index out of bounds in `find_loop_index` when the input tape carries a
/// symbol from the other member's alphabet).
#[test]
fn lookup_multi_transducer_ol_archive_unions_without_panic() {
    let dir = scratch("olunion");
    let a = dir.join("a.ol");
    let b = dir.join("b.ol");
    let both = dir.join("both.ol");
    build_ol("{cat}:{CAT}", &a);
    build_ol("{dog}:{DOG}", &b);

    let mut merged = std::fs::read(&a).expect("read a.ol");
    merged.extend(std::fs::read(&b).expect("read b.ol"));
    std::fs::write(&both, &merged).expect("write both.ol");

    let (ok, out) = run(
        &["lookup", both.to_str().expect("utf8 path")],
        b"cat\ndog\n",
    );
    assert!(
        ok,
        "hfst lookup on a two-transducer OL archive crashed/failed"
    );
    assert!(out.contains("CAT"), "first member not matched:\n{out}");
    assert!(out.contains("DOG"), "second member not matched:\n{out}");
}

/// hfst/hfst#354: a pmatch grammar that inserts an RTN sub-transducer via
/// `Ins(...)` must match — recursing into the RTN and returning to its caller —
/// rather than crashing or silently failing. Exercises three formerly-broken
/// layers at once: the shared archive alphabet's `input_symbol_count` (was
/// under-padded, an out-of-bounds index lookup), `name_from_insertion` (kept a
/// stray trailing `@` so the RTN was filed under the wrong name), and RTN
/// re-entrancy (the caller is suspended on the Rust stack when the RTN returns).
/// The `{the } ... EndTag` wrapping means the caller has a non-initial frame at
/// the insertion point, so the return must restore the caller's own state.
#[test]
fn pmatch_ins_rtn_matches_and_returns_to_caller() {
    let dir = scratch("insrtn");
    let src = dir.join("g.pmatch");
    let out = dir.join("g.pmhfst");
    std::fs::write(
        &src,
        "Define Animal [{cat} | {dog}] ;\nDefine TOP {the } Ins(Animal) EndTag(np) ;\n",
    )
    .expect("write grammar");
    let (ok, _) = run(
        &[
            "pmatch2fst",
            "-i",
            src.to_str().expect("utf8 path"),
            "-o",
            out.to_str().expect("utf8 path"),
        ],
        b"",
    );
    assert!(ok, "pmatch2fst failed to compile the Ins/RTN grammar");
    let (ok, o) = run(&["pmatch", out.to_str().expect("utf8 path")], b"the cat\n");
    assert!(ok, "hfst pmatch crashed on an Ins/RTN archive");
    assert!(
        o.contains("<np>the cat</np>"),
        "RTN insertion did not match/return correctly:\n{o}"
    );
}

/// hfst/hfst#287: a deeply-nested regular expression must compile, not overflow
/// the stack. ~2000 nested brackets is far past the ~250 that aborted the
/// default 8 MiB main-thread stack before the big-stack worker thread.
#[test]
fn regexp2fst_deep_nesting_does_not_overflow_stack() {
    let dir = scratch("deepregex");
    let out = dir.join("deep.hfst");
    let regex = format!("{}a{} ;", "[".repeat(2000), "]".repeat(2000));
    let (ok, _) = run(
        &["regexp2fst", "-S", "-o", out.to_str().expect("utf8 path")],
        regex.as_bytes(),
    );
    assert!(
        ok,
        "deeply-nested regexp2fst should compile, not overflow the stack"
    );
}
