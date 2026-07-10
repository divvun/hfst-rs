// Seam-level tests for the THFST backend citizen (WBS node `thfst.seam`).
//
// This node makes `ImplementationType::THFST_TYPE` constructible and
// convertible: the `ThfstTransducer` newtype, its `Backend`/`LookupBackend`
// delegation, the O(1) OLW<->THFST table moves, and the format-conversion arm.
// Disk I/O (read_dir/write_dir) lands in the next node (`thfst.io`), so these
// tests never touch the filesystem — they exercise the in-memory seam only.
//
// The compatibility contract (`docs/spec/port/back-ends/thfst/thfst-backend.md`)
// is that THFST is the weighted optimized-lookup engine under a distinct stream
// tag: converting to THFST and looking up must yield exactly the analyses and
// weights the OLW conversion of the same source yields, and the OLW<->THFST
// moves must preserve both the lookup relation and the facade metadata.
//
// The tropical transition-data symbol coding lives in process-global statics
// guarded by their own mutexes; concurrent callers can race and throw
// HfstFatalException. cargo runs every #[test] as a parallel thread in one
// process, so we serialize construction through a shared lock, matching the
// house style in test_transducer_functions.rs.

use hfst::backend::{Backend, LookupBackend};
use hfst::backend_thfst::ThfstTransducer;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::HfstOneLevelPaths;
use hfst::hfst_data_types::ImplementationType::{HFST_OLW_TYPE, THFST_TYPE};
use hfst::hfst_data_types::StringVector;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::HfstTransducer;
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Build the classic weighted "animals" transducer as tropical: each animal
/// maps its singular to its plural with a final weight, disjuncted and
/// minimized. Mirrors test_transducer_functions.rs's animal block.
fn build_animals() -> Result<HfstTransducer<StdVectorFst>, hfst::error::Error> {
    let tok = HfstTokenizer::new();
    let mut cat = HfstTransducer::<StdVectorFst>::new_tokenized_pair("cat", "cats", &tok)?;
    cat.set_final_weights(3.0, false)?;
    let mut dog = HfstTransducer::<StdVectorFst>::new_tokenized_pair("dog", "dogs", &tok)?;
    dog.set_final_weights(2.5, false)?;
    let mut mouse = HfstTransducer::<StdVectorFst>::new_tokenized_pair("mouse", "mice", &tok)?;
    mouse.set_final_weights(1.7, false)?;
    let mut hippo1 =
        HfstTransducer::<StdVectorFst>::new_tokenized_pair("hippopotamus", "hippopotami", &tok)?;
    hippo1.set_final_weights(1.2, false)?;
    let mut hippo2 =
        HfstTransducer::<StdVectorFst>::new_tokenized_pair("hippopotamus", "hippopotamuses", &tok)?;
    hippo2.set_final_weights(1.4, false)?;

    let mut animals = HfstTransducer::<StdVectorFst>::new();
    animals.disjunct(&cat, true)?;
    animals.disjunct(&dog, true)?;
    animals.disjunct(&mouse, true)?;
    animals.disjunct(&hippo1, true)?;
    animals.disjunct(&hippo2, true)?;
    animals.minimize()?;
    Ok(animals)
}

/// Collect a lookup as a set of (output-string, weight) pairs, so two lookups
/// can be compared irrespective of iteration order and float formatting.
fn as_pairs(results: &HfstOneLevelPaths) -> std::collections::BTreeSet<(String, u32)> {
    results
        .iter()
        .map(|p| {
            let out: String = p.second.iter().map(|s| s.as_str()).collect();
            // Quantize the weight so tiny float noise does not split equal
            // analyses; 1000ths is finer than the test weights' spacing.
            (out, (p.first * 1000.0).round() as u32)
        })
        .collect()
}

fn tok_one_level(tok: &HfstTokenizer, s: &str) -> StringVector {
    tok.tokenize_one_level(s, false)
}

// [spec:hfst:sem:thfst-backend.thfst-transducer/test]
// [spec:hfst:sem:thfst-backend.olw-moves/test]
#[test]
fn thfst_conversion_matches_olw_lookup() {
    let _guard = serialized();
    let animals = build_animals().expect("build animals");

    // Convert the same source two ways: straight to OLW, and to THFST (via the
    // weighted OL tables + the O(1) move). THFST must carry the THFST tag.
    let olw: HfstTransducer<Transducer<WeightedTables>> =
        animals.to_ol(true, "").expect("to_ol(weighted)");
    assert_eq!(olw.get_type(), HFST_OLW_TYPE);

    let thfst = animals
        .to_ol(true, "")
        .expect("to_ol(weighted)")
        .into_thfst();
    assert_eq!(thfst.get_type(), THFST_TYPE, "THFST carries the THFST tag");
    // Backend const agrees with the facade tag.
    assert_eq!(
        <hfst::backend_thfst::ThfstTransducer as Backend>::TYPE,
        THFST_TYPE
    );

    // Lookup happens on the weighted engine either way. THFST recovers that
    // engine by an O(1) move (`into_olw`), so the analyses and weights must be
    // identical to the direct OLW conversion.
    let tok = HfstTokenizer::new();
    let mut olw = olw;
    let mut thfst_as_olw = thfst.into_olw();

    for word in ["cat", "dog", "mouse", "hippopotamus"] {
        let sv = tok_one_level(&tok, word);
        let via_olw = olw.lookup_string_vector(&sv, -1, 0.0).expect("olw lookup");
        let via_thfst = thfst_as_olw
            .lookup_string_vector(&sv, -1, 0.0)
            .expect("thfst lookup");
        assert_eq!(
            as_pairs(&via_olw),
            as_pairs(&via_thfst),
            "THFST lookup of {word:?} matches OLW lookup"
        );
    }

    // Concrete spot-check: hippopotamus has two weighted plurals.
    let hippo = tok_one_level(&tok, "hippopotamus");
    let res = thfst_as_olw
        .lookup_string_vector(&hippo, -1, 0.0)
        .expect("thfst hippo lookup");
    let pairs = as_pairs(&res);
    assert_eq!(pairs.len(), 2, "hippopotamus has two plurals");
    assert!(pairs.contains(&("hippopotami".to_string(), 1200)));
    assert!(pairs.contains(&("hippopotamuses".to_string(), 1400)));
}

// [spec:hfst:sem:thfst-backend.olw-moves/test]
#[test]
fn olw_thfst_olw_round_trip_preserves_lookup_and_name() {
    let _guard = serialized();
    let animals = build_animals().expect("build animals");

    let mut olw = animals.to_ol(true, "").expect("to_ol(weighted)");
    olw.set_name("animals-net");

    // Reference lookups before any moves.
    let tok = HfstTokenizer::new();
    let cat = tok_one_level(&tok, "cat");
    let mouse = tok_one_level(&tok, "mouse");
    let cat_ref = as_pairs(&olw.lookup_string_vector(&cat, -1, 0.0).expect("cat ref"));
    let mouse_ref = as_pairs(
        &olw.lookup_string_vector(&mouse, -1, 0.0)
            .expect("mouse ref"),
    );

    // OLW -> THFST -> OLW: the moves are O(1) retags that preserve the facade
    // metadata (name) and the lookup relation.
    let thfst = olw.into_thfst();
    assert_eq!(thfst.get_type(), THFST_TYPE);
    assert_eq!(thfst.get_name(), "animals-net", "name survives OLW->THFST");

    let mut back = thfst.into_olw();
    assert_eq!(back.get_type(), HFST_OLW_TYPE);
    assert_eq!(back.get_name(), "animals-net", "name survives THFST->OLW");

    assert_eq!(
        cat_ref,
        as_pairs(&back.lookup_string_vector(&cat, -1, 0.0).expect("cat after")),
        "cat lookup survives the round trip"
    );
    assert_eq!(
        mouse_ref,
        as_pairs(
            &back
                .lookup_string_vector(&mouse, -1, 0.0)
                .expect("mouse after")
        ),
        "mouse lookup survives the round trip"
    );
}

// -----------------------------------------------------------------------------
// thfst.io — the on-disk directory format (write_dir / read_dir).
// -----------------------------------------------------------------------------

/// A unique, per-test scratch directory under the OS temp dir, so parallel
/// tests never collide. The caller is responsible for the `.thfst` leaf name.
fn unique_tmp(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("hfst-thfst-{tag}-{pid}-{n}"))
}

/// Read all bytes of a file, panicking with context on failure.
fn slurp(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

/// The animals transducer, converted straight to a `ThfstTransducer` backend
/// (via the shared basic-transducer -> weighted-OL path). Returned alongside a
/// weighted-OL facade of the SAME source for pre-write reference lookups.
fn animals_thfst() -> (ThfstTransducer, HfstTransducer<Transducer<WeightedTables>>) {
    let animals = build_animals().expect("build animals");
    let basic = animals.get_basic_transducer().expect("basic");
    let thfst = ThfstTransducer::from_basic(&basic).expect("thfst from_basic");
    let olw = animals.to_ol(true, "").expect("to_ol(weighted)");
    (thfst, olw)
}

// [spec:hfst:sem:thfst-backend.write-dir-fn/test]
// [spec:hfst:sem:thfst-backend.read-dir-fn/test]
#[test]
fn thfst_roundtrip_lookup_parity() {
    let _guard = serialized();
    let (thfst, mut olw) = animals_thfst();

    // Reference lookups BEFORE writing to disk.
    let tok = HfstTokenizer::new();
    let words = ["cat", "dog", "mouse", "hippopotamus"];
    let mut refs = Vec::new();
    for word in words {
        let sv = tok_one_level(&tok, word);
        refs.push(as_pairs(
            &olw.lookup_string_vector(&sv, -1, 0.0).expect("olw ref"),
        ));
    }

    // Write, then read back into a fresh THFST transducer.
    let dir = unique_tmp("roundtrip").join("animals.thfst");
    thfst.write_dir(&dir).expect("write_dir");
    let mut reread = ThfstTransducer::read_dir(&dir).expect("read_dir");

    // Lookup on the re-read engine must match the pre-write references exactly.
    for (word, want) in words.iter().zip(refs.iter()) {
        let sv = tok_one_level(&tok, word);
        let got = as_pairs(&LookupBackend::lookup_fd_strvec(&mut reread, &sv, -1, 0.0));
        assert_eq!(
            *want, got,
            "re-read THFST lookup of {word:?} matches pre-write"
        );
    }

    // Stability: writing the re-read transducer to a SECOND dir yields
    // byte-identical index + transition files.
    let dir2 = unique_tmp("roundtrip2").join("animals.thfst");
    reread.write_dir(&dir2).expect("write_dir 2");
    assert_eq!(
        slurp(&dir.join("index")),
        slurp(&dir2.join("index")),
        "index bytes stable across write/read/write"
    );
    assert_eq!(
        slurp(&dir.join("transition")),
        slurp(&dir2.join("transition")),
        "transition bytes stable across write/read/write"
    );

    let _ = std::fs::remove_dir_all(dir.parent().expect("parent"));
    let _ = std::fs::remove_dir_all(dir2.parent().expect("parent"));
}

/// Build the minimal single-arc transducer `a:b` with final weight 1.5.
fn build_ab_weighted() -> HfstBasicTransducer {
    let mut t = HfstBasicTransducer::new();
    t.add_state(1);
    t.set_final_weight(1, &1.5);
    let arc = HfstBasicTransition::new_symbols(1, "a".into(), "b".into(), 0.0, t.coder_mut());
    t.add_transition(0, &arc, true);
    t
}

// [spec:hfst:sem:thfst-backend.index-record/test]
// [spec:hfst:sem:thfst-backend.transition-record/test]
#[test]
fn thfst_golden_bytes() {
    let _guard = serialized();
    let basic = build_ab_weighted();

    // Two `from_basic` paths share the same conversion, so the OLW engine's
    // tables and the THFST files are computed from identical data. We derive
    // the expected bytes from the OLW accessors (locking the LE encoding,
    // padding, and raw-u32 copy), not from magic constants.
    let olw = <Transducer<WeightedTables> as Backend>::from_basic(&basic).expect("olw");
    let thfst = ThfstTransducer::from_basic(&basic).expect("thfst");

    let dir = unique_tmp("golden").join("ab.thfst");
    thfst.write_dir(&dir).expect("write_dir");

    // Expected `index`: 8-byte LE { u16 input, u16 0, u32 raw stored target }.
    let mut expected_index = Vec::new();
    for i in 0..olw.get_header().index_table_size() {
        let input = olw.get_index_input(i);
        let raw = olw.get_index_target(i);
        expected_index.extend_from_slice(&input.to_le_bytes());
        expected_index.extend_from_slice(&0u16.to_le_bytes());
        expected_index.extend_from_slice(&raw.to_le_bytes());
    }
    assert_eq!(expected_index.len() % 8, 0, "index len is a multiple of 8");
    assert_eq!(
        slurp(&dir.join("index")),
        expected_index,
        "index bytes match the LE record layout"
    );

    // Expected `transition`: 12-byte LE { u16 in, u16 out, u32 target, f32 wt }.
    let mut expected_transition = Vec::new();
    for i in 0..olw.get_header().target_table_size() {
        let input = olw.get_transition_input(i);
        let output = olw.get_transition_output(i);
        let target = olw.get_transition_target(i);
        let weight = olw.get_transition_weight(i);
        expected_transition.extend_from_slice(&input.to_le_bytes());
        expected_transition.extend_from_slice(&output.to_le_bytes());
        expected_transition.extend_from_slice(&target.to_le_bytes());
        expected_transition.extend_from_slice(&weight.to_bits().to_le_bytes());
    }
    assert_eq!(
        expected_transition.len() % 12,
        0,
        "transition len is a multiple of 12"
    );
    assert_eq!(
        slurp(&dir.join("transition")),
        expected_transition,
        "transition bytes match the LE record layout"
    );

    // The final weight 1.5 must survive as f32 bits somewhere in the tables:
    // confirm it appears verbatim in one of the two files (index carries it in
    // final index slots, transition in the weight field).
    let idx_bytes = slurp(&dir.join("index"));
    let trans_bytes = slurp(&dir.join("transition"));
    let w_le = 1.5f32.to_bits().to_le_bytes();
    let found =
        idx_bytes.windows(4).any(|w| w == w_le) || trans_bytes.windows(4).any(|w| w == w_le);
    assert!(found, "final weight 1.5's f32 bits appear in the tables");

    let _ = std::fs::remove_dir_all(dir.parent().expect("parent"));
}

/// Build a transducer whose alphabet holds the flags `@U.CASE.UP@`,
/// `@U.CASE.LOW@`, `@D.NEED@`, epsilon (implicit at symbol 0), and a plain
/// multichar symbol `+Noun`. The arcs need not be meaningful — only the derived
/// alphabet matters.
fn build_flag_alphabet() -> HfstBasicTransducer {
    let mut t = HfstBasicTransducer::new();
    t.add_state(1);
    t.add_state(2);
    t.add_state(3);
    t.add_state(4);
    t.set_final_weight(4, &0.0);
    // Chain: @U.CASE.UP@ -> @U.CASE.LOW@ -> @D.NEED@ -> +Noun.
    let a = HfstBasicTransition::new_symbols(
        1,
        "@U.CASE.UP@".into(),
        "@U.CASE.UP@".into(),
        0.0,
        t.coder_mut(),
    );
    t.add_transition(0, &a, true);
    let b = HfstBasicTransition::new_symbols(
        2,
        "@U.CASE.LOW@".into(),
        "@U.CASE.LOW@".into(),
        0.0,
        t.coder_mut(),
    );
    t.add_transition(1, &b, true);
    let c = HfstBasicTransition::new_symbols(
        3,
        "@D.NEED@".into(),
        "@D.NEED@".into(),
        0.0,
        t.coder_mut(),
    );
    t.add_transition(2, &c, true);
    let d = HfstBasicTransition::new_symbols(4, "+Noun".into(), "cat".into(), 0.0, t.coder_mut());
    t.add_transition(3, &d, true);
    t
}

// [spec:hfst:sem:thfst-backend.alphabet-json/test]
#[test]
fn thfst_alphabet_numbering() {
    use hfst::thfst_io::{ThfstFlagOperator, build_alphabet_json};

    let _guard = serialized();
    let basic = build_flag_alphabet();
    let olw = <Transducer<WeightedTables> as Backend>::from_basic(&basic).expect("olw");
    let alpha = build_alphabet_json(&olw).expect("alphabet json");

    // Epsilon lives at key_table[0] and serializes as "" (divvunspell's epsilon
    // slot). initial_symbol_count == N == key_table length.
    assert_eq!(alpha.key_table[0], "", "epsilon slot is the empty string");
    assert_eq!(
        alpha.initial_symbol_count as usize,
        alpha.key_table.len(),
        "initial_symbol_count == N == key_table length"
    );

    // Flags and specials never appear in string_to_symbol; the plain multichar
    // symbol does.
    assert!(
        !alpha.string_to_symbol.contains_key("@U.CASE.UP@"),
        "flags are absent from string_to_symbol"
    );
    assert!(
        !alpha.string_to_symbol.contains_key("@D.NEED@"),
        "flags are absent from string_to_symbol"
    );
    assert!(
        !alpha.string_to_symbol.contains_key(""),
        "epsilon is absent from string_to_symbol"
    );
    assert!(
        alpha.string_to_symbol.contains_key("+Noun"),
        "the plain multichar symbol is in string_to_symbol"
    );

    // The three flags produce three operations entries. Feature/value numbers
    // are 0-based first-encounter over shared buckets — divvunspell's
    // `feature_bucket`/`value_bucket` scheme (alphabet.rs lines 92-100, 151-154),
    // NOT the house FdTable numbering. The order that drives first-encounter is
    // the OLW symbol-table order, which places flag diacritics in a sorted
    // BTreeSet: `@D.NEED@` < `@U.CASE.LOW@` < `@U.CASE.UP@`. Hence NEED's feature
    // (encountered first) is 0 and CASE's is 1.
    let ops: Vec<_> = alpha.operations.values().collect();
    assert_eq!(ops.len(), 3, "three flag symbols -> three operations");

    let case_up = alpha
        .operations
        .values()
        .find(|o| o.operation == ThfstFlagOperator::Unification && o.value == 2)
        .expect("U.CASE.UP present (value 2)");
    let case_low = alpha
        .operations
        .values()
        .find(|o| o.operation == ThfstFlagOperator::Unification && o.value == 1)
        .expect("U.CASE.LOW present (value 1)");
    let need = alpha
        .operations
        .values()
        .find(|o| o.operation == ThfstFlagOperator::Disallow)
        .expect("D.NEED present");

    // NEED is the first flag feature (sorted before CASE) -> feature 0; both
    // CASE flags share feature 1.
    assert_eq!(need.feature, 0, "NEED is the first feature -> 0");
    assert_eq!(case_low.feature, 1, "CASE -> 1");
    assert_eq!(case_up.feature, 1, "CASE reused -> 1");

    // Value numbering per divvunspell's shared value bucket: epsilon (symbol 0)
    // inserts "" -> value 0 (alphabet.rs lines 151-154). `@D.NEED@`'s empty
    // value reuses the "" bucket -> 0. Then (sorted order) `@U.CASE.LOW@`'s
    // "LOW" is new -> 1, and `@U.CASE.UP@`'s "UP" is new -> 2.
    assert_eq!(need.value, 0, "D.NEED's empty value reuses \"\"=0");
    assert_eq!(case_low.value, 1, "LOW is value 1");
    assert_eq!(case_up.value, 2, "UP is value 2");

    // flag_state_size = distinct feature count (CASE, NEED) = 2.
    assert_eq!(alpha.flag_state_size, 2, "two distinct features");

    // length = Σ over the original symbol strings of (byte length + 1).
    let expected_length: usize = olw.get_symbol_table().iter().map(|s| s.len() + 1).sum();
    assert_eq!(alpha.length, expected_length, "length == Σ(byte_len + 1)");
}

// [spec:hfst:sem:thfst-backend.read-dir-fn/test]
#[test]
fn thfst_reader_rejects() {
    let _guard = serialized();
    let basic = build_ab_weighted();
    let thfst = ThfstTransducer::from_basic(&basic).expect("thfst");

    // (a) A directory missing the `transition` file is rejected.
    let dir_a = unique_tmp("reject-missing").join("ab.thfst");
    thfst.write_dir(&dir_a).expect("write_dir");
    std::fs::remove_file(dir_a.join("transition")).expect("remove transition");
    let err_a = ThfstTransducer::read_dir(&dir_a)
        .err()
        .expect("missing transition rejected");
    assert_eq!(
        err_a.kind,
        hfst::error::ErrorKind::NotTransducerStream,
        "missing member -> NotTransducerStream"
    );

    // (b) An `index` file whose length is not a multiple of 8 is rejected.
    let dir_b = unique_tmp("reject-trunc").join("ab.thfst");
    thfst.write_dir(&dir_b).expect("write_dir");
    {
        // Append a stray byte so len % 8 != 0.
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(dir_b.join("index"))
            .expect("open index");
        f.write_all(&[0u8]).expect("append stray byte");
    }
    let err_b = ThfstTransducer::read_dir(&dir_b)
        .err()
        .expect("bad index length rejected");
    assert_eq!(
        err_b.kind,
        hfst::error::ErrorKind::NotTransducerStream,
        "index len % 8 != 0 -> NotTransducerStream"
    );

    let _ = std::fs::remove_dir_all(dir_a.parent().expect("parent"));
    let _ = std::fs::remove_dir_all(dir_b.parent().expect("parent"));
}
