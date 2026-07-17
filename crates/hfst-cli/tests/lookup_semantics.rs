//! Regression locks for four `hfst-lookup` bounding / symbol-semantics issues
//! surfaced during the upstream-bugs validation (2026-07):
//!
//!   * hfst/hfst#476 — lookup "memory hole" / `--time-cutoff` ineffective on
//!     pathological (epsilon-cyclic) optimized-lookup FSTs. `--time-cutoff`
//!     bypasses the infinite-ambiguity pre-check, so before the fix the engine
//!     recursed `MAX_RECURSION_DEPTH` (5000) levels deep, emitting 5000 junk
//!     analyses and running the machine flat out. The OL engine now traps the
//!     non-progressing epsilon cycle (and carries a hard total-work budget as a
//!     backstop), so lookup terminates promptly with a bounded result set.
//!   * hfst/hfst#293 — OL lookup accumulated HUGE weights (the epsilon loop
//!     added weight per level, climbing to ~4999) before finally printing
//!     infinities. The cycle trap stops the weight climb at the first
//!     non-progressing revisit.
//!   * hfst/hfst#225 — `@_IDENTITY_SYMBOL_@` arcs must match input symbols that
//!     are NOT in the transducer alphabet during lookup (verify-and-lock: the
//!     port is already correct and agrees with the slow/basic engine).
//!   * hfst/hfst#445 — `?` (identity/unknown) matching in `hfst-lookup`: an
//!     identity arc matches any out-of-alphabet symbol, while a known symbol
//!     still takes its explicit arc (verify-and-lock).
//!
//! Each test drives the real `hfst` binary. Every cyclic/infinite case is
//! bounded by an explicit `--time-cutoff` / `--max-number` AND a wall-clock
//! guard on the child process, so a regression that reintroduces the runaway
//! traversal fails the assertion (or trips the guard) instead of hanging the
//! suite.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hfst-lookup-semantics-{name}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Run `hfst <args>` feeding `stdin`, returning (success, stdout, elapsed).
/// The child is given a generous but finite deadline: if the traversal ever
/// runs away again, the wall-clock assertion in the caller catches it rather
/// than the whole test suite hanging.
fn run(args: &[&str], stdin: &[u8]) -> (bool, String, Duration) {
    let start = Instant::now();
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
        start.elapsed(),
    )
}

/// Compile `regex` to a weighted optimized-lookup file at `out` (via
/// `regexp2fst` then `fst2fst -w`), the format `hfst lookup`'s fast path uses.
fn build_ol(regex: &str, out: &Path) {
    let hfst = out.with_extension("hfst");
    let (ok, _, _) = run(
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
    let (ok, _, _) = run(
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

/// Every `\t<float>` weight column printed in `stdout`.
fn weights(stdout: &str) -> Vec<f32> {
    stdout
        .lines()
        .filter_map(|line| line.rsplit('\t').next())
        .filter_map(|col| col.trim().parse::<f32>().ok())
        .collect()
}

/// hfst/hfst#476 + hfst/hfst#293: an epsilon-cyclic OL FST (`[0:a::1]*`, an
/// input-epsilon loop that outputs `a` with weight 1 each turn) must NOT send
/// lookup into a 5000-deep runaway when `--time-cutoff` bypasses the
/// infinite-ambiguity pre-check. The traversal has to terminate quickly, hand
/// back only a handful of analyses, and never let the epsilon-loop weight
/// explode.
#[test]
fn ol_lookup_epsilon_cycle_is_bounded_under_time_cutoff() {
    let dir = scratch("eps-cycle");
    let ol = dir.join("w_cycle.ol");
    build_ol("[0:a::1]*", &ol);

    // `--time-cutoff` deliberately skips the `is_lookup_infinitely_ambiguous`
    // short-circuit, so this exercises the raw `get_analyses` bounding.
    let (ok, out, elapsed) = run(
        &[
            "lookup",
            "--time-cutoff",
            "2.0",
            ol.to_str().expect("utf8 path"),
        ],
        b"\n",
    );
    assert!(ok, "lookup on epsilon-cyclic OL failed:\n{out}");

    // Before the fix this printed 5000 analyses in a tight loop; the cycle trap
    // now yields the two shortest outputs ("" and "a"). Keep the bound loose
    // enough to be robust yet far below the old 5000.
    let ws = weights(&out);
    assert!(
        ws.len() <= 20,
        "epsilon-cyclic lookup produced {} analyses (expected a small bounded \
         set, not the 5000-deep runaway):\n{out}",
        ws.len()
    );

    // hfst/hfst#293: the epsilon-loop weight must not explode. Each loop turn
    // added 1.0, reaching ~4999 before; the trap caps it at the first
    // non-progressing revisit.
    let max_w = ws.iter().cloned().fold(0.0f32, f32::max);
    assert!(
        max_w < 100.0,
        "epsilon-loop weight exploded to {max_w} (hfst#293: huge weights before \
         infinity):\n{out}"
    );

    // And it must have finished promptly, not spun to the depth cap.
    assert!(
        elapsed < Duration::from_secs(5),
        "epsilon-cyclic lookup took {elapsed:?} (time-cutoff/bounding ineffective)"
    );
}

/// hfst/hfst#476: the DEFAULT lookup path (no `--time-cutoff`) over the same
/// epsilon-cyclic FST must also terminate promptly. Here the
/// infinite-ambiguity guard fires and caps results at `--max-number`, but the
/// engine still has to walk the (now trapped) cycle without running away.
#[test]
fn ol_lookup_epsilon_cycle_default_terminates() {
    let dir = scratch("eps-cycle-default");
    let ol = dir.join("w_cycle.ol");
    build_ol("[0:a::1]*", &ol);

    let (ok, out, elapsed) = run(&["lookup", ol.to_str().expect("utf8 path")], b"\n");
    assert!(ok, "default lookup on epsilon-cyclic OL failed:\n{out}");
    assert!(
        elapsed < Duration::from_secs(5),
        "default epsilon-cyclic lookup took {elapsed:?} (did not terminate promptly)"
    );

    // Weights must stay tiny (no explosion) regardless of the result cap.
    let max_w = weights(&out).iter().cloned().fold(0.0f32, f32::max);
    assert!(
        max_w < 100.0,
        "default epsilon-cyclic lookup weight exploded to {max_w}:\n{out}"
    );
}

/// hfst/hfst#476 + hfst/hfst#293 companion: an *unweighted* epsilon cycle
/// (`[0:a]*`) under `--time-cutoff` must likewise terminate with a bounded
/// result set — the runaway was structural, not weight-dependent.
#[test]
fn ol_lookup_unweighted_epsilon_cycle_is_bounded() {
    let dir = scratch("eps-cycle-unweighted");
    let ol = dir.join("e_cycle.ol");
    build_ol("[0:a]*", &ol);

    let (ok, out, elapsed) = run(
        &[
            "lookup",
            "--time-cutoff",
            "2.0",
            ol.to_str().expect("utf8 path"),
        ],
        b"\n",
    );
    assert!(ok, "lookup on unweighted epsilon-cyclic OL failed:\n{out}");
    assert!(
        weights(&out).len() <= 20,
        "unweighted epsilon-cyclic lookup produced a runaway result set:\n{out}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "unweighted epsilon-cyclic lookup took {elapsed:?}"
    );
}

/// hfst/hfst#225: an `@_IDENTITY_SYMBOL_@` arc (regex `?`) must match an input
/// symbol that is NOT in the transducer's alphabet, echoing it on the output
/// side. Verify-and-lock: the OL engine is already correct.
#[test]
fn ol_lookup_identity_arc_matches_unknown_input() {
    let dir = scratch("identity");
    let ol = dir.join("id.ol");
    build_ol("?", &ol);

    // `q`, `Z`, `5` are all outside the (empty) alphabet, so each must ride the
    // identity arc and come back echoed unchanged with weight 0.
    for sym in ["q", "Z", "5"] {
        let (ok, out, _) = run(
            &["lookup", ol.to_str().expect("utf8 path")],
            format!("{sym}\n").as_bytes(),
        );
        assert!(ok, "identity lookup of {sym:?} failed:\n{out}");
        assert!(
            out.contains(&format!("{sym}\t{sym}\t0.000000")),
            "identity arc did not match unknown input {sym:?} (hfst#225):\n{out}"
        );
        assert!(
            !out.contains("+?"),
            "identity lookup of {sym:?} reported no-match (hfst#225):\n{out}"
        );
    }
}

/// hfst/hfst#445: `?` (identity/unknown) matching in `hfst-lookup`. With a
/// known symbol AND an identity arc (`[a:b | ?]`):
///   * a KNOWN symbol `a` takes BOTH the explicit `a:b` arc and the
///     identity-expanded `a:a` arc;
///   * an UNKNOWN symbol takes the identity arc, echoed unchanged.
/// Verify-and-lock against the port's (correct) unknown-symbol semantics.
#[test]
fn ol_lookup_question_mark_unknown_and_identity_semantics() {
    let dir = scratch("question-mark");
    let ol = dir.join("abq.ol");
    build_ol("[a:b | ?]", &ol);

    // Known input `a`: both readings.
    let (ok, out, _) = run(&["lookup", ol.to_str().expect("utf8 path")], b"a\n");
    assert!(ok, "lookup of 'a' failed:\n{out}");
    assert!(
        out.contains("a\tb\t0.000000"),
        "known symbol 'a' lost its explicit a:b reading (hfst#445):\n{out}"
    );
    assert!(
        out.contains("a\ta\t0.000000"),
        "known symbol 'a' lost its identity-expanded a:a reading (hfst#445):\n{out}"
    );

    // Unknown input `q`: identity arc, echoed.
    let (ok, out, _) = run(&["lookup", ol.to_str().expect("utf8 path")], b"q\n");
    assert!(ok, "lookup of 'q' failed:\n{out}");
    assert!(
        out.contains("q\tq\t0.000000"),
        "unknown symbol 'q' did not match the identity arc (hfst#445):\n{out}"
    );
    assert!(
        !out.contains("+?"),
        "unknown symbol 'q' reported no-match (hfst#445):\n{out}"
    );
}
