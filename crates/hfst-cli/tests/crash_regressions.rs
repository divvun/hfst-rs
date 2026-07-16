//! Regression locks for crashes and correctness defects found during the
//! upstream-bugs validation (2026-07). Each test drives the real `hfst` binary
//! over an input that used to abort the process or produce wrong output, and
//! asserts a clean, correct result instead.

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

/// Like `run`, but also captures stderr (for diagnostics that go to the log).
fn run_captured(args: &[&str], stdin: &[u8]) -> (bool, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
        String::from_utf8_lossy(&out.stderr).into_owned(),
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

/// hfst/hfst#354 (root): an ambiguous token whose analyses converge on a shared
/// state must yield ALL readings in `tokenise -g`, not just the first. The i399
/// epsilon-cycle guard was a *global* per-attempt visited-set, which also pruned
/// convergent-but-distinct plain-epsilon branches (e.g. `cat+N` and `cat+V`
/// meeting at the same final state) — dropping every reading but one ("missing
/// wordforms"). The guard is now DFS-path-scoped, so cycles still terminate while
/// convergent analyses survive. Verified against `hfst lookup` (both readings).
#[test]
fn tokenise_emits_all_convergent_analyses() {
    let dir = scratch("ambig");
    let src = dir.join("g.pmatch");
    let out = dir.join("g.pmhfst");
    std::fs::write(
        &src,
        "Define TOP [ {cat}:{cat+N} | {cat}:{cat+V} | {dog}:{dog+N} ] EndTag(w) ;\n",
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
    assert!(ok, "pmatch2fst failed to compile the ambiguous grammar");
    let (ok, o) = run(
        &["tokenise", "-g", out.to_str().expect("utf8 path")],
        b"cat\n",
    );
    assert!(ok, "hfst tokenise failed on the ambiguous archive");
    assert!(
        o.contains("cat+N") && o.contains("cat+V"),
        "tokenise -g dropped a convergent analysis (missing wordforms):\n{o}"
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

/// Port-found (during the #354/RTN work): `pmatch2fst --flatten` on a grammar
/// whose `Ins(X)` target is inlined as the entire body of another definition
/// used to panic in `Rc::get_mut` ("freshly built node is uniquely owned") —
/// the inlined node is a *shared* definition Rc. It must compile and still match.
#[test]
fn pmatch2fst_flatten_shared_ins_does_not_panic() {
    let dir = scratch("flatten_ins");
    let src = dir.join("g.pmatch");
    let out = dir.join("g.pmhfst");
    // A and B both inline C; TOP inserts both. Under --flatten this shares C's
    // node across A, B, and TOP.
    std::fs::write(
        &src,
        "Define C [{red}] ;\nDefine A Ins(C) ;\nDefine B Ins(C) ;\nDefine TOP Ins(A) Ins(B) EndTag(w) ;\n",
    )
    .expect("write grammar");
    let (ok, _) = run(
        &[
            "pmatch2fst",
            "--flatten",
            "-i",
            src.to_str().expect("utf8 path"),
            "-o",
            out.to_str().expect("utf8 path"),
        ],
        b"",
    );
    assert!(ok, "pmatch2fst --flatten panicked on a shared inlined Ins");
    let (ok, o) = run(&["pmatch", out.to_str().expect("utf8 path")], b"redred\n");
    assert!(ok, "hfst pmatch failed on the flattened archive");
    assert!(
        o.contains("<w>redred</w>"),
        "flattened shared-Ins grammar did not match:\n{o}"
    );
}

/// Port-found (during the #354/RTN work): using a reserved predefined-acceptor
/// name (`Alpha`, `Whitespace`, ...) as a `Define` target is a parse error. The
/// compiler used to swallow it and emit only "Empty ruleset, nothing to write";
/// the real cause must now reach the user.
#[test]
fn pmatch2fst_reserved_name_redefinition_reports_parse_error() {
    let dir = scratch("reserved_redef");
    let src = dir.join("g.pmatch");
    std::fs::write(
        &src,
        "Define Alpha [a|b|c] ;\nDefine TOP Ins(Alpha) EndTag(w) ;\n",
    )
    .expect("write grammar");
    let (_ok, _out, err) = run_captured(
        &[
            "pmatch2fst",
            "-i",
            src.to_str().expect("utf8 path"),
            "-o",
            dir.join("g.pmhfst").to_str().expect("utf8 path"),
        ],
        b"",
    );
    assert!(
        err.contains("syntax error") && err.contains("Define"),
        "reserved-name redefinition should surface a parse error, got stderr:\n{err}"
    );
}
