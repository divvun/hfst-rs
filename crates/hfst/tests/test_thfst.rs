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

use hfst::backend::Backend;
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
