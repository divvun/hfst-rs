//! The optimized-lookup and pmatch readers index their tables with numbers
//! taken from the tables themselves, so a degenerate or hostile transducer used
//! to end the process instead of returning.
//!
//! Two shapes are covered:
//!
//!   * A transducer with no transitions at all — the empty language, which is
//!     what `compose_intersect` of an empty rule vector yields — converts to an
//!     index table padded for exactly one input symbol (epsilon), while its
//!     alphabet still carries `@_IDENTITY_SYMBOL_@` / `@_UNKNOWN_SYMBOL_@`. The
//!     lookup engine probes the index table at `state + identity_symbol` and
//!     walked off the end. It is a legitimate value, so it must answer: an
//!     empty result set, not an error.
//!
//!   * A plain optimized-lookup transducer handed to the pmatch runtime, which
//!     encodes the WHOLE alphabet as potential input and so probes the same
//!     table well past its input-symbol padding. Wrong input type: the archive
//!     reader says so, and the runtime itself no longer crashes on the probe.
//!
//! Plus the reader-level structural checks those fixes rest on: a header or a
//! table pair that does not hold together is now an error at the read boundary
//! rather than an out-of-bounds read somewhere in the traversal.

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::pmatch::PmatchContainer;
use hfst::transducer::{IStream, Transducer, WeightedTables};

// The tropical transition-data symbol coding lives in process-global statics;
// cargo runs each #[test] as a parallel thread in one process, so construction
// is serialized through a shared lock, matching the house style elsewhere.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn to_ol(basic: &HfstBasicTransducer) -> Transducer<WeightedTables> {
    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(basic, true, "", None)
        .expect("well within the optimized-lookup format limits")
}

/// A one-arc transducer whose output symbol is not an input symbol anywhere, so
/// the alphabet is numbered well past `input_symbol_count`.
fn one_arc_ol() -> Transducer<WeightedTables> {
    let mut basic = HfstBasicTransducer::new();
    {
        let coder = basic.coder_mut();
        let tr = HfstBasicTransition::new_symbols(1, "a".into(), "zzz".into(), 0.0, coder);
        basic.add_transition(0, &tr, true);
    }
    basic.set_final_weight(1, &0.0);
    to_ol(&basic)
}

/// Frame a payload as an HFST3 stream: `HFST\0`, the property-block length, a
/// NUL, then NUL-separated key/value pairs and the payload.
fn hfst3_archive(properties: &[(&str, &str)], payload: &[u8]) -> Vec<u8> {
    let mut block: Vec<u8> = Vec::new();
    for (k, v) in properties {
        block.extend_from_slice(k.as_bytes());
        block.push(0);
        block.extend_from_slice(v.as_bytes());
        block.push(0);
    }
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"HFST\0");
    out.extend_from_slice(&(block.len() as u16).to_ne_bytes());
    out.push(0);
    out.extend_from_slice(&block);
    out.extend_from_slice(payload);
    out
}

fn write_ol(t: &Transducer<WeightedTables>) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    t.write(&mut buf);
    buf
}

// ---------------------------------------------------------------------------
// The empty transducer is a value, not an error
// ---------------------------------------------------------------------------

// Before the fix this panicked in `TransitionTable::at` with "index out of
// bounds: the len is 2 but the index is 2": the index table holds one blank
// entry plus one symbol of padding, and the lookup probes it at
// `0 + @_IDENTITY_SYMBOL_@`.
#[test]
fn empty_transducer_lookup_returns_no_analyses() {
    let _guard = serialized();
    let mut ol = to_ol(&HfstBasicTransducer::new());
    assert!(
        ol.lookup_fd_str("foo", -1, 0.0).is_empty(),
        "the empty language accepts nothing, so lookup yields no analyses"
    );
    // Longer input walks further into the traversal; still no analyses, still
    // no panic.
    assert!(ol.lookup_fd_str("a longer probe", -1, 0.0).is_empty());
    assert!(ol.lookup_fd_str("", -1, 0.0).is_empty());
}

// The same shape reached through the pmatch runtime, whose encoder numbers the
// whole alphabet as potential input and so probes even further past the
// index-table padding.
#[test]
fn plain_optimized_lookup_transducer_in_pmatch_runtime_does_not_panic() {
    let _guard = serialized();
    let mut container =
        PmatchContainer::new_from_transducer(Box::new(one_arc_ol())).expect("container builds");
    // "zzz" is an output-only symbol: numbered above input_symbol_count, so the
    // probe lands outside the padding. This panicked in
    // `PmatchTransducer::make_transition_table_index`.
    for probe in ["a", "zzz", "azzz", "zzza"] {
        container.process(probe);
    }
}

// ---------------------------------------------------------------------------
// Wrong input type: a plain optimized-lookup transducer is not a pmatch archive
// ---------------------------------------------------------------------------

#[test]
fn non_pmatch_archive_is_rejected_with_a_diagnostic() {
    let _guard = serialized();
    let archive = hfst3_archive(
        &[("name", "TOP"), ("type", "HFST_OLW"), ("version", "3.0")],
        &write_ol(&one_arc_ol()),
    );
    let mut cursor = std::io::Cursor::new(archive);
    let mut is = IStream::new(&mut cursor);
    // `PmatchContainer` has no `Debug`, so `Result::expect_err` is unavailable.
    let err = match PmatchContainer::new_from_stream(&mut is) {
        Ok(_) => panic!("a plain optimized-lookup transducer is not a pmatch archive"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("not a pmatch archive"),
        "unhelpful diagnostic: {err}"
    );
}

// ---------------------------------------------------------------------------
// Structural checks at the read boundary
// ---------------------------------------------------------------------------

fn read_ol(bytes: &[u8]) -> hfst::error::Result<Transducer<WeightedTables>> {
    let mut cursor = std::io::Cursor::new(bytes.to_vec());
    let mut is = IStream::new(&mut cursor);
    Transducer::<WeightedTables>::new_istream(&mut is)
}

/// `Transducer` has no `Debug`, so `Result::expect_err` is unavailable.
fn expect_read_error(bytes: &[u8], why: &str) -> hfst::error::Error {
    match read_ol(bytes) {
        Ok(_) => panic!("{why}"),
        Err(e) => e,
    }
}

#[test]
fn well_formed_optimized_lookup_bytes_still_load() {
    let _guard = serialized();
    let bytes = write_ol(&one_arc_ol());
    let mut back = match read_ol(&bytes) {
        Ok(t) => t,
        Err(e) => panic!("round-tripped bytes are valid: {e}"),
    };
    assert!(
        !back.lookup_fd_str("a", -1, 0.0).is_empty(),
        "the round-tripped transducer still maps its one arc"
    );
}

// The header's first u16 is the input-symbol count and the second the total
// symbol count. Claiming more input symbols than symbols sent the encoder off
// the end of the symbol table while building its tokenization trie.
#[test]
fn header_claiming_more_input_symbols_than_symbols_is_rejected() {
    let _guard = serialized();
    let mut bytes = write_ol(&one_arc_ol());
    let symbols = u16::from_le_bytes([bytes[2], bytes[3]]);
    bytes[0..2].copy_from_slice(&(symbols + 7).to_le_bytes());
    let err = expect_read_error(&bytes, "input symbols cannot outnumber symbols");
    assert!(
        err.to_string().contains("input symbols"),
        "unhelpful diagnostic: {err}"
    );
}

// A transition target outside both tables is what a truncated or spliced
// .hfstol looks like from the inside; it used to surface as an out-of-bounds
// panic in the middle of a lookup ("the len is 2 but the index is 897").
#[test]
fn out_of_range_transition_target_is_rejected() {
    let _guard = serialized();
    let transducer = one_arc_ol();
    let mut bytes = write_ol(&transducer);
    // The payload is header, alphabet, 6-byte index entries, 12-byte weighted
    // transition entries; each transition is (input u16, output u16, target
    // u32, weight f32). Point the first transition at an entry neither table
    // has.
    let transitions = alphabet_end(&bytes, transducer.get_header().symbol_count() as usize)
        + transducer.get_header().index_table_size() as usize * 6;
    bytes[transitions + 4..transitions + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    let err = expect_read_error(&bytes, "a target outside both tables is corruption");
    assert!(
        err.to_string().contains("corrupt"),
        "unhelpful diagnostic: {err}"
    );
}

/// The optimized-lookup header: two u16 counts, four u32 fields, then nine u32
/// boolean properties.
const HEADER_LEN: usize = 2 * 2 + 4 * 4 + 9 * 4;

/// Offset just past the alphabet's `symbols` NUL-terminated strings.
fn alphabet_end(bytes: &[u8], symbols: usize) -> usize {
    HEADER_LEN
        + bytes[HEADER_LEN..]
            .iter()
            .enumerate()
            .filter(|(_, b)| **b == 0)
            .map(|(i, _)| i)
            .nth(symbols - 1)
            .expect("the alphabet holds symbol_count NUL-terminated strings")
        + 1
}

// An alphabet entry that spells the empty string (two adjacent NUL separators)
// is trivially craftable and used to panic while the encoder built its trie
// over a buffer holding only the terminator.
#[test]
fn empty_symbol_string_in_the_alphabet_does_not_panic() {
    let _guard = serialized();
    let transducer = one_arc_ol();
    let symbols = transducer.get_header().symbol_count() as usize;
    let bytes = write_ol(&transducer);
    // Replace the whole alphabet with `symbols` empty strings, keeping every
    // other field (and hence the tables) intact.
    let mut spliced = bytes[..HEADER_LEN].to_vec();
    spliced.extend(std::iter::repeat_n(0u8, symbols));
    spliced.extend_from_slice(&bytes[alphabet_end(&bytes, symbols)..]);
    // Nameless symbols cannot be tokenized, so nothing matches — but the load
    // and the lookup both have to come back.
    let mut back = match read_ol(&spliced) {
        Ok(t) => t,
        Err(e) => panic!("an unnameable alphabet is still readable: {e}"),
    };
    assert!(back.lookup_fd_str("a", -1, 0.0).is_empty());
}
