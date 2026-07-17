// Tests for the on-disk optimized-lookup (OL) format limits and endianness.
//
// Two upstream issues are covered here:
//
//   * hfst/hfst#123 (on-disk width ceilings): the OL table-size fields are u32.
//     Narrowing a `usize` table length into that field must return a clean,
//     propagated error instead of silently wrapping or panicking. The checked
//     helper `ol_table_size` (mirroring `ol_symbol_number` for the u16 symbol
//     ceiling) is exercised directly at its boundaries — the overflow path is
//     unreachable on real data, so a unit test on the helper is the only way to
//     reach it.
//
//   * hfst/hfst#328 (OL endianness): the OL read/write path is deliberately
//     LITTLE-ENDIAN (diverging from the C++'s native-endian `reinterpret_cast`)
//     so the format is portable and deterministic. On this little-endian host
//     that is byte-identical to the old native path, which these tests confirm
//     by pinning the serialized header bytes AND by a write -> read -> write
//     round-trip stability check.
//
// The tropical transition-data symbol coding used by `HfstBasicTransducer`
// lives in process-global statics guarded by their own mutexes; cargo runs each
// #[test] as a parallel thread in one process, so construction is serialized
// through a shared lock, matching the house style in test_thfst.rs.

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::transducer::{IStream, Transducer, WeightedTables, ol_table_size};

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Build a tiny weighted transducer mapping "cat" -> "cats" (weight 0.5) and
/// "dog" -> "dogs" (weight 1.25), then convert it to a weighted OL backend.
fn build_ol() -> Transducer<WeightedTables> {
    let mut basic = HfstBasicTransducer::new();

    // "cat" -> "cats": c:c a:a t:t, then an epsilon arc emitting the final "s".
    // Symbols go straight in as single graphemes; the conversion collects them.
    let add = |basic: &mut HfstBasicTransducer, from, to, i: &str, o: &str, w: f32| {
        let coder = basic.coder_mut();
        let tr = HfstBasicTransition::new_symbols(to, i.into(), o.into(), w, coder);
        basic.add_transition(from, &tr, true);
    };

    // cat -> cats
    add(&mut basic, 0, 1, "c", "c", 0.0);
    add(&mut basic, 1, 2, "a", "a", 0.0);
    add(&mut basic, 2, 3, "t", "ts", 0.5);
    basic.set_final_weight(3, &0.0);

    // dog -> dogs
    add(&mut basic, 0, 4, "d", "d", 0.0);
    add(&mut basic, 4, 5, "o", "o", 0.0);
    add(&mut basic, 5, 6, "g", "gs", 1.25);
    basic.set_final_weight(6, &0.0);

    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(&basic, true, "", None)
        .expect("small transducer is well within the OL format limits")
}

/// Serialize a weighted OL transducer to bytes.
fn write_ol(t: &Transducer<WeightedTables>) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    t.write(&mut buf);
    buf
}

/// Read a weighted OL transducer back from bytes (payload only, no HFST3
/// header, matching `Transducer::write`).
fn read_ol(bytes: &[u8]) -> Transducer<WeightedTables> {
    let mut cursor = std::io::Cursor::new(bytes.to_vec());
    let mut is = IStream::new(&mut cursor);
    Transducer::<WeightedTables>::new_istream(&mut is).expect("round-tripped OL bytes are valid")
}

// ---- hfst/hfst#123: the checked u32 table-size conversion helper ----

#[test]
fn ol_table_size_accepts_zero() {
    assert_eq!(ol_table_size(0).expect("0 fits in u32"), 0u32);
}

#[test]
fn ol_table_size_accepts_small_value() {
    assert_eq!(ol_table_size(12_345).expect("12345 fits in u32"), 12_345u32);
}

#[test]
fn ol_table_size_accepts_u32_max() {
    let max = u32::MAX as usize;
    assert_eq!(ol_table_size(max).expect("u32::MAX fits in u32"), u32::MAX);
}

#[test]
fn ol_table_size_rejects_beyond_u32() {
    // usize is 64-bit on every target this crate builds for, so u32::MAX + 1 is
    // representable as a usize and must be rejected with a clean error (never a
    // silent wrap to 0 and never a panic).
    let over = (u32::MAX as usize)
        .checked_add(1)
        .expect("64-bit usize can hold u32::MAX + 1");
    let err = ol_table_size(over).expect_err("a table larger than u32::MAX must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("optimized-lookup") && msg.contains("table"),
        "expected a clear OL table-size diagnostic, got: {msg}"
    );
}

// ---- hfst/hfst#328: little-endian OL serialization ----

#[test]
fn ol_header_is_little_endian() {
    let _guard = serialized();
    let t = build_ol();
    let bytes = write_ol(&t);

    // The OL header begins with number_of_input_symbols (u16) then
    // number_of_symbols (u16), both little-endian. Decode them from the bytes
    // and confirm they match the in-memory header, which pins the on-disk byte
    // order regardless of host endianness.
    let input_symbols = u16::from_le_bytes([bytes[0], bytes[1]]);
    let symbols = u16::from_le_bytes([bytes[2], bytes[3]]);

    assert_eq!(
        input_symbols,
        t.get_header().input_symbol_count(),
        "number_of_input_symbols must be stored little-endian"
    );
    assert_eq!(
        symbols,
        t.get_header().symbol_count(),
        "number_of_symbols must be stored little-endian"
    );

    // The next four bytes are the index-table size (u32) little-endian.
    let index_table_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(
        index_table_size,
        t.get_header().index_table_size(),
        "size_of_transition_index_table must be stored little-endian"
    );
}

#[test]
fn ol_round_trip_is_lossless() {
    let _guard = serialized();
    let original = build_ol();
    let bytes = write_ol(&original);

    let mut reloaded = read_ol(&bytes);

    // The header fields survive the round-trip identically.
    assert_eq!(
        reloaded.get_header().symbol_count(),
        original.get_header().symbol_count()
    );
    assert_eq!(
        reloaded.get_header().input_symbol_count(),
        original.get_header().input_symbol_count()
    );
    assert_eq!(
        reloaded.get_header().index_table_size(),
        original.get_header().index_table_size()
    );
    assert_eq!(
        reloaded.get_header().target_table_size(),
        original.get_header().target_table_size()
    );

    // And so does the lookup relation: "cat" -> "cats" @ 0.5, "dog" -> "dogs"
    // @ 1.25.
    let cat = reloaded.lookup_fd_str("cat", -1, 0.0);
    let outputs: Vec<(String, f32)> = cat
        .iter()
        .map(|p| {
            (
                p.second.iter().map(|s| s.as_str()).collect::<String>(),
                p.first,
            )
        })
        .collect();
    assert_eq!(outputs, vec![("cats".to_string(), 0.5f32)]);

    let dog = reloaded.lookup_fd_str("dog", -1, 0.0);
    let dog_outputs: Vec<(String, f32)> = dog
        .iter()
        .map(|p| {
            (
                p.second.iter().map(|s| s.as_str()).collect::<String>(),
                p.first,
            )
        })
        .collect();
    assert_eq!(dog_outputs, vec![("dogs".to_string(), 1.25f32)]);
}

#[test]
fn ol_write_read_write_is_byte_identical() {
    let _guard = serialized();
    let original = build_ol();
    let first = write_ol(&original);

    // Reading the bytes back and re-serializing must reproduce them exactly:
    // the little-endian read and write paths are mutual inverses, so the format
    // is deterministic (a fixed-point of write . read).
    let reloaded = read_ol(&first);
    let second = write_ol(&reloaded);

    assert_eq!(
        first, second,
        "OL serialization must be a byte-exact fixed point of write . read"
    );
}
