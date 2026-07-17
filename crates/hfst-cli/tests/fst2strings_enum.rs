//! Regression locks for the `hfst fst2strings` path-enumeration / random-path
//! family (upstream hfst/hfst#170, #327, #490, #222, #444, #441). Each test
//! drives the real `hfst` binary. The C++ tool crashed/aborted or produced wrong
//! output on these inputs; the port must instead be memory-safe AND produce
//! correct, bounded output.
//!
//! Every cyclic/infinite case is bounded (`-n`, `-c`, or a fixed-length FST) so
//! no test can hang the nextest 10s-per-test cap.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hfst-fst2strings-enum-{name}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Run `hfst <args>` feeding `stdin`, returning (success, stdout).
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

/// Compile a regex to a (default tropical) transducer file.
fn build_regex(regex: &str, out: &std::path::Path) {
    let (ok, _) = run(
        &["regexp2fst", "-S", "-o", out.to_str().expect("utf8 path")],
        regex.as_bytes(),
    );
    assert!(ok, "regexp2fst failed for {regex}");
}

/// Compile an ATT description to a transducer file (precise control over final
/// weights, which regex cannot express on the final STATE).
fn build_att(att: &str, out: &std::path::Path) {
    let (ok, _) = run(
        &["txt2fst", "-o", out.to_str().expect("utf8 path")],
        att.as_bytes(),
    );
    assert!(ok, "txt2fst failed for ATT:\n{att}");
}

/// hfst/hfst#170: `fst2strings` must PRINT the strings a transducer recognises,
/// not silently emit nothing. The empty-string language (a lone final start
/// state) must yield exactly one empty line; a single-state self-loop
/// (`[a|b|c]*`, a start-is-final state that loops on each symbol) must, at cycle
/// depth 1, yield the empty path plus every single symbol. (The C++ segfault that
/// produced no output is void in Rust — here we lock in that real output
/// appears.) Letters, not digits: `0` is the epsilon symbol in HFST regex.
#[test]
fn prints_recognised_strings_not_nothing() {
    let dir = scratch("prints");
    // Empty-string acceptor: state 0 is final with no arcs.
    let eps = dir.join("eps.hfst");
    build_att("0\t0.0\n", &eps);
    let (ok, out) = run(&["fst2strings", eps.to_str().expect("p")], b"");
    assert!(ok, "fst2strings crashed on the empty-string acceptor");
    assert_eq!(
        out, "\n",
        "empty-string acceptor must print one empty line, got {out:?}"
    );

    // Single-state self-loop: at cycle depth 1, the empty path and each symbol.
    let star = dir.join("star.hfst");
    build_regex("[a|b|c]*", &star);
    let (ok, out) = run(&["fst2strings", "-c", "1", star.to_str().expect("p")], b"");
    assert!(ok, "fst2strings crashed on the single-state self-loop FST");
    let mut lines: Vec<&str> = out.lines().collect();
    lines.sort_unstable();
    assert_eq!(
        lines,
        vec!["", "a", "b", "c"],
        "self-loop FST at -c 1 must print the empty path and every symbol, got:\n{out}"
    );
}

/// hfst/hfst#327 & #222: `-n` (max-strings) must BOUND the enumeration of an
/// otherwise-infinite transducer — the callback streams results and stops the
/// depth-first search as soon as the cap is hit, so the work is bounded rather
/// than accumulating an unbounded set (the C++ resource/format ceiling that
/// aborted long enumerations). The infinite `[a|b|c]+` FST must terminate
/// promptly with exactly `-n` lines.
#[test]
fn max_strings_bounds_infinite_enumeration() {
    let dir = scratch("bound");
    let inf = dir.join("inf.hfst");
    build_regex("[a|b|c]+", &inf);

    let (ok, out) = run(&["fst2strings", "-n", "5", inf.to_str().expect("p")], b"");
    assert!(ok, "fst2strings -n on an infinite FST did not succeed");
    let n = out.lines().count();
    assert_eq!(n, 5, "-n 5 must print exactly 5 strings, got {n}:\n{out}");
}

/// hfst/hfst#222: enumeration STREAMS through the callback — memory does not grow
/// with the number of strings printed. A fixed-length 6-symbol combinatoric FST
/// (`[a|b|c|d]^6`) has exactly 4^6 = 4096 paths; asking for a large `-n` prints
/// all of them and terminates (bounded length keeps the test well under the time
/// cap), demonstrating the tool does not accumulate/overflow on long
/// enumerations but streams path by path.
#[test]
fn long_enumeration_streams_all_paths() {
    let dir = scratch("stream");
    let combo = dir.join("combo.hfst");
    build_regex("[a|b|c|d]^6", &combo);

    let (ok, out) = run(
        &["fst2strings", "-n", "1000000", combo.to_str().expect("p")],
        b"",
    );
    assert!(ok, "fst2strings did not stream the 6-symbol enumeration");
    let n = out.lines().count();
    assert_eq!(
        n, 4096,
        "the fixed-length 6-symbol FST has 4^6 = 4096 paths; all must stream out, got {n}"
    );
}

/// hfst/hfst#490: `-c 0` (follow cycles zero times) must TERMINATE on a cyclic
/// transducer, not loop forever / explode. On a single-state self-loop
/// (`[a|b|c]*`, start-is-final) the only zero-cycle path is the empty string, so
/// `-c 0` yields exactly one empty line; bumping to `-c 1` then follows the loop
/// once and adds the three single symbols — proving the cycle bound is honoured
/// and finite (never hangs).
#[test]
fn max_cycles_zero_terminates_with_bounded_semantics() {
    let dir = scratch("cyc0");
    let star = dir.join("star.hfst");
    build_regex("[a|b|c]*", &star);

    let (ok, out) = run(&["fst2strings", "-c", "0", star.to_str().expect("p")], b"");
    assert!(
        ok,
        "fst2strings -c 0 did not terminate cleanly on a cyclic FST"
    );
    assert_eq!(
        out, "\n",
        "-c 0 on a single-state self-loop must print only the empty path, got {out:?}"
    );

    let (ok, out) = run(&["fst2strings", "-c", "1", star.to_str().expect("p")], b"");
    assert!(ok, "fst2strings -c 1 did not terminate on a cyclic FST");
    assert_eq!(
        out.lines().count(),
        4,
        "-c 1 must follow the loop once (empty path + three symbols), got:\n{out}"
    );
}

/// hfst/hfst#444: `-r` (random paths) must PRODUCE accepting paths on a
/// guesser/cyclic transducer whose accepting state hides behind a rare deep
/// suffix — the C++ blind random walk almost always failed to reach a final
/// state and returned nothing. `[?+ x y z]` accepts any non-empty string ending
/// in `xyz`; steering the walk toward final states must make every request
/// yield paths, all of which end in the `xyz` suffix. Run several times to guard
/// against a lucky seed masking a regression.
#[test]
fn random_paths_reach_deep_final_on_guesser() {
    let dir = scratch("rand_guesser");
    let g = dir.join("g.hfst");
    build_regex("[?+ x y z]", &g);

    for attempt in 0..5 {
        let (ok, out) = run(&["fst2strings", "-r", "5", g.to_str().expect("p")], b"");
        assert!(ok, "fst2strings -r crashed on the guesser FST");
        let lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
        assert!(
            !lines.is_empty(),
            "attempt {attempt}: -r produced NO random paths on a non-empty guesser FST"
        );
        for l in &lines {
            assert!(
                l.ends_with("xyz"),
                "attempt {attempt}: random path {l:?} is not an accepting path (must end in xyz)"
            );
        }
    }
}

/// hfst/hfst#441: `-r -w` must report the correct weight, including the
/// FINAL-STATE weight, on random paths. The C++ truncation branch (when a walk
/// overshoots a final state and is rolled back) omitted `+= final_weight`, so a
/// path ending at a weighted final state was printed with the wrong weight
/// (typically 0). Here the only accepting path is `a`, ending at a final state
/// whose final weight is 2.5, and the sole continuation is a dead end that forces
/// the truncation branch — every random path must carry weight 2.5, never 0.
#[test]
fn random_weight_keeps_final_state_weight() {
    let dir = scratch("rand_weight");
    // 0 --a--> 1(final, fw=2.5) --b--> 2(dead end, non-final).
    let w = dir.join("w.hfst");
    build_att("0\t1\ta\ta\t0.0\n1\t2\tb\tb\t0.0\n1\t2.5\n", &w);

    // Sanity: full enumeration agrees the only path is `a` with weight 2.5.
    let (ok, out) = run(&["fst2strings", "-w", w.to_str().expect("p")], b"");
    assert!(ok, "fst2strings -w failed on the weighted-final FST");
    assert_eq!(
        out.trim_end(),
        "a\t2.5",
        "unexpected full enumeration: {out:?}"
    );

    for attempt in 0..8 {
        let (ok, out) = run(
            &["fst2strings", "-r", "5", "-w", w.to_str().expect("p")],
            b"",
        );
        assert!(ok, "fst2strings -r -w crashed on the weighted-final FST");
        for l in out.lines().filter(|l| !l.is_empty()) {
            assert_eq!(
                l, "a\t2.5",
                "attempt {attempt}: random path dropped the final-state weight: {l:?}"
            );
        }
    }
}
