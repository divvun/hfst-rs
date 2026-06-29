//! Golden smoke test for `hfst-optimized-lookup`.
//!
//! `tests/fixtures/lookup.hfstol` is a committed optimized-lookup transducer
//! accepting {cat, cats, dog, dogs} (built with `hfst-strings2fst -j |
//! hfst-fst2fst -O`); `tests/fixtures/lookup.golden` is the tool's output for a
//! fixed query set. This is the regression oracle for `librarify.ol-reuse`:
//! gutting the tool to call the library OL engine (`crate::transducer`) must
//! preserve this exact output. The tool engine and the library engine were
//! verified to agree on the analyses (only the per-tool Xerox formatting is
//! tool-specific), so byte-equality here is the right contract.

use std::io::Write;
use std::process::{Command, Stdio};

const QUERIES: &[u8] = b"cat\ncats\ndog\ndogs\nxyz\n";

#[test]
fn optimized_lookup_matches_golden() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let fixture = format!("{dir}/tests/fixtures/lookup.hfstol");
    let golden = include_str!("fixtures/lookup.golden");

    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst-optimized-lookup"))
        .arg(&fixture)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn hfst-optimized-lookup");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(QUERIES)
        .expect("write queries to stdin");

    let out = child
        .wait_with_output()
        .expect("wait for hfst-optimized-lookup");
    assert!(out.status.success(), "tool exited with {:?}", out.status);

    let got = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        got, golden,
        "hfst-optimized-lookup output drifted from the golden oracle"
    );
}
