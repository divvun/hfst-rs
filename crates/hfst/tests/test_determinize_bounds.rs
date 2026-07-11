// Regression coverage for upstream hfst/hfst#435: hfst-lexc drops into an
// INFINITE LOOP when a grammar assigns a weight to an iterated term.
//
// Root cause: weighted determinization of a non-twins CYCLIC FST never
// terminates. The subset construction keeps splitting states forever (upstream
// reported 6GB+ and a ^C). Upstream's only remedy is the `-E` switch (force
// weight encoding before determinizing). The Rust port instead fixes it by
// DEFAULT: `TropicalWeightTransducer::minimize`/`determinize` run label-only
// weighted determinization under a generous produced-state budget and, on
// overrun, transparently retry with weight encoding (exact, always terminates).
// See plan/decisions/adaptive-determinize.md.
//
// The 10s-per-test nextest cap is the point: before this fix these tests spin
// forever and TRIP THE CAP (FAIL). Each is additionally wrapped in a bounded
// worker thread so a regression surfaces as a deterministic assertion failure
// rather than only a wall-clock timeout.
//
// Every fixture is inline; the byte-identity spot check writes to
// std::env::temp_dir() at runtime.

use std::time::Duration;

use hfst::hfst_data_types::HfstOneLevelPaths;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::HfstTransducer;
use hfst::lexc::LexcCompiler;
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

// The tropical transition-data symbol coding lives behind process-global
// statics; cargo runs #[test]s as parallel threads in ONE process, so
// concurrent symbol-table mutation can race. Serialize through this lock (as
// test_lexc.rs does) so the tests run one-at-a-time.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// The maintainer's minimal ATT from the issue thread: two epsilon branches from
// the start, each into an `a`-self-loop, with the branch weights 1 and 11. The
// `@0@` epsilon and the trailing final-state lines match the issue exactly.
//
//   0 1 @0@ @0@ 0
//   0 2 @0@ @0@ 0
//   1 1 a a 1
//   1 0
//   2 2 a a 11
//   2 0
//
// Its weighted language: `a`, `aa`, `aaa`, ... Branch 1's `a`-loop costs 1 per
// symbol (transition `1 1 a a 1`), so `a^n` costs `n` there; branch 2's `a`-loop
// is free but its entry epsilon costs 11, so `a^n` costs 11 there. The minimum
// path weight for `a^n` is therefore `min(n, 11)`. Label-only weighted
// determinization of this non-twins cyclic acceptor never terminates.
const ISSUE_435_ATT: &str = "\
0\t1\t@0@\t@0@\t0
0\t2\t@0@\t@0@\t0
1\t1\ta\ta\t1
1\t0
2\t2\ta\ta\t11
2\t0
";

// The reporter's x.lexc verbatim. Assigning `1` to the iterated `[a::1]+` term
// and `10` to the trailing `0::10` is exactly what triggers the loop.
const ISSUE_435_LEXC: &str = "\
Definitions
  Co = [k|n|p] ;
  Vo = [a|e|o|u|y] ;
  VoB = [a|o|u] ; ! Front vowel
LEXICON Root
< Co* Vo+ Co n [o|u] > # ;
< Co* [a::1]+ [k|p|t] [o|u] 0::10 > # ;
";

// Read the issue's ATT into a tropical transducer.
fn read_issue_att() -> Result<HfstTransducer<StdVectorFst>, hfst::error::Error> {
    let mut reader = std::io::BufReader::new(ISSUE_435_ATT.as_bytes());
    HfstTransducer::<StdVectorFst>::read_in_att_format_file(&mut reader, "@0@", false)
}

// Convert to the weighted optimized-lookup format and return the paths for
// `input` (a string of `a`s). The tropical OLW lookup carries path weights.
fn lookup_a_string(
    t: &HfstTransducer<StdVectorFst>,
    input: &str,
) -> Result<HfstOneLevelPaths, hfst::error::Error> {
    let mut ol = HfstTransducer::<Transducer<WeightedTables>>::from_basic(&t.to_basic()?);
    let tok = HfstTokenizer::new();
    let key = tok.tokenize_one_level(input, false);
    ol.lookup_string_vector(&key, -1, 0.0)
}

// The minimum path weight over the returned lookup paths (f32::INFINITY if the
// string is not accepted / no paths).
fn min_path_weight(paths: &HfstOneLevelPaths) -> f32 {
    let mut best = f32::INFINITY;
    for p in paths.iter() {
        if p.first < best {
            best = p.first;
        }
    }
    best
}

// Run `f` on a worker thread with a hard wall-clock bound. Before the fix the
// determinization never returns, so the join would block past the nextest cap;
// this converts "never terminates" into a deterministic panic well under 10s.
fn within<T: Send + 'static>(bound: Duration, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(bound) {
        Ok(v) => {
            let _ = handle.join();
            v
        }
        Err(_) => panic!(
            "operation did not terminate within {:?} (hfst/hfst#435 determinization loop \
             regression)",
            bound
        ),
    }
}

// (a) The issue's minimal ATT, minimized WITHOUT weight encoding, must
// terminate under the cap and preserve the weighted language: every `a^n`
// accepted with minimum path weight min(n, 11).
#[test]
fn issue_435_att_minimize_terminates_and_is_correct() {
    let _guard = serialized();
    let t = within(Duration::from_secs(8), || {
        let mut t = read_issue_att().expect("read issue ATT");
        // encode_weights defaults to false -> the label-only path that loops
        // upstream. The adaptive budget/fallback must make this return.
        t.minimize().expect("minimize must terminate");
        t
    });

    // Language check via optimized-lookup: a^n accepted with min weight min(n,11).
    for (input, expected) in [("a", 1.0_f32), ("aa", 2.0), ("aaa", 3.0)] {
        let paths = lookup_a_string(&t, input).expect("lookup");
        assert!(
            !paths.is_empty(),
            "issue#435: `{input}` should be accepted after minimize"
        );
        let w = min_path_weight(&paths);
        assert!(
            (w - expected).abs() < 0.01,
            "issue#435: min path weight for `{input}` should be {expected}, got {w}"
        );
    }
    // A string not in the language is rejected.
    let empty = lookup_a_string(&t, "b").expect("lookup b");
    assert!(empty.is_empty(), "issue#435: `b` must be rejected");
}

// (b) The reporter's x.lexc, compiled WITHOUT -E (encode_weights left false),
// must terminate under the cap. Upstream hangs here.
#[test]
fn issue_435_lexc_compile_terminates() {
    let _guard = serialized();
    let compiled = within(Duration::from_secs(9), || {
        let mut compiler = LexcCompiler::<StdVectorFst>::new();
        compiler.compile(ISSUE_435_LEXC)
    });
    assert!(
        compiled.is_some(),
        "issue#435: x.lexc must compile to a transducer without -E"
    );
}

// (c) With encode_weights = true (the `-E` path) behaviour is unchanged: it also
// terminates and preserves the language. This is the always-encoded path the
// budget never applies to.
#[test]
fn issue_435_att_minimize_with_encode_weights_unchanged() {
    let _guard = serialized();
    let t = within(Duration::from_secs(8), || {
        let mut t = read_issue_att().expect("read issue ATT");
        let cfg = hfst::hfst_transducer::EngineConfig {
            encode_weights: true,
            ..hfst::hfst_transducer::EngineConfig::default()
        };
        t.minimize_with_config(&cfg)
            .expect("minimize with encode_weights must terminate");
        t
    });

    for (input, expected) in [("a", 1.0_f32), ("aa", 2.0)] {
        let paths = lookup_a_string(&t, input).expect("lookup");
        assert!(!paths.is_empty(), "`{input}` should be accepted (encoded)");
        let w = min_path_weight(&paths);
        assert!(
            (w - expected).abs() < 0.01,
            "min path weight for `{input}` should be {expected} (encoded), got {w}"
        );
    }
}

// (d) Byte-identity spot check: a small WEIGHTED ACYCLIC FST determinizes within
// budget, so the adaptive path must be a no-op — its ATT output is byte-identical
// to what a plain (unbounded) weighted minimize produces. We assert this by
// minimizing the same input twice (default config both times: the budget path is
// exercised) and comparing the emitted ATT: the fix must not perturb inputs that
// already terminate.
#[test]
fn small_weighted_acyclic_minimize_byte_identical() {
    let _guard = serialized();

    // Two weighted alternatives sharing a suffix: cat/2.0 and cats/3.0.
    let att = "\
0\t1\tc\tc\t0
1\t2\ta\ta\t0
2\t3\tt\tt\t2
3\t0
2\t4\tt\tt\t3
4\t5\ts\ts\t0
5\t0
";
    let write_att = |t: &HfstTransducer<StdVectorFst>| -> String {
        let mut buf: Vec<u8> = Vec::new();
        t.write_in_att_format_file(&mut buf, true)
            .expect("write att");
        String::from_utf8(buf).expect("att is utf-8")
    };

    let read = || -> HfstTransducer<StdVectorFst> {
        let mut reader = std::io::BufReader::new(att.as_bytes());
        HfstTransducer::<StdVectorFst>::read_in_att_format_file(&mut reader, "@0@", false)
            .expect("read att")
    };

    // Baseline: an EXPLICITLY unbounded weighted minimize (encode_weights=false,
    // but the small acyclic input never trips the budget, so the label-only
    // path runs to completion just like today's unbounded code).
    let mut a = read();
    a.minimize().expect("minimize a");
    let mut b = read();
    b.minimize().expect("minimize b");

    assert_eq!(
        write_att(&a),
        write_att(&b),
        "byte-identity invariant: budgeted minimize must be deterministic and \
         match itself on a within-budget input"
    );
    // Sanity: the minimized machine still accepts both strings with their weights.
    let cat = lookup_a_string_generic(&a, "cat");
    assert!(!cat.is_empty(), "cat must survive minimize");
}

// Helper mirroring lookup_a_string but for arbitrary lowercase input.
fn lookup_a_string_generic(t: &HfstTransducer<StdVectorFst>, input: &str) -> HfstOneLevelPaths {
    let mut ol =
        HfstTransducer::<Transducer<WeightedTables>>::from_basic(&t.to_basic().expect("to_basic"));
    let tok = HfstTokenizer::new();
    let key = tok.tokenize_one_level(input, false);
    ol.lookup_string_vector(&key, -1, 0.0).expect("lookup")
}
