//! THFST on-disk (de)serializers — the `X.thfst/` directory format that
//! divvunspell's `thfst-tools` produces and its `src/transducer/thfst/` reader
//! consumes. These functions have no C++ HFST ancestor: they are authored
//! greenfield against `docs/spec/port/back-ends/thfst/thfst-backend.md`, and
//! their contract is byte/semantics-compatibility with divvunspell.
//!
//! The three member files are `alphabet` (JSON), `index` and `transition`
//! (flat little-endian record arrays). This module builds the alphabet JSON by
//! replicating divvunspell's `TransducerAlphabetParser` from the transducer's
//! symbol STRINGS (never the engine's internal `FdTable` numbers — the two
//! numbering schemes disagree), and writes/reads the tables with explicit
//! little-endian byte order (the in-tree OLW writer is native-endian, so it
//! cannot be reused here).

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::hfst_data_types::Symbol;
use crate::transducer::{
    SymbolNumber, SymbolTable, Transducer, TransducerAlphabet, TransducerHeader, TransducerTable,
    TransducerTables, TransitionW, TransitionWIndex, WeightedTables,
};

// -----------------------------------------------------------------------------
// The alphabet JSON — the serde shape of divvunspell's `TransducerAlphabet`.
// -----------------------------------------------------------------------------

/// The flag-diacritic operator, serialized as divvunspell's variant names
/// (`PositiveSet`, `NegativeSet`, `Require`, `Disallow`, `Clear`,
/// `Unification`). The single operator char at index 1 of a flag key
/// (`@<op>.FEATURE.VALUE@`) maps to one of these.
// [spec:hfst:def:thfst-backend.alphabet-json]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThfstFlagOperator {
    PositiveSet,
    NegativeSet,
    Require,
    Disallow,
    Clear,
    Unification,
}

impl ThfstFlagOperator {
    /// Map a flag operator char to its variant, mirroring divvunspell's hard
    /// error on any char outside `P N R D C U`.
    fn from_char(c: char) -> Option<Self> {
        match c {
            'P' => Some(ThfstFlagOperator::PositiveSet),
            'N' => Some(ThfstFlagOperator::NegativeSet),
            'R' => Some(ThfstFlagOperator::Require),
            'D' => Some(ThfstFlagOperator::Disallow),
            'C' => Some(ThfstFlagOperator::Clear),
            'U' => Some(ThfstFlagOperator::Unification),
            _ => None,
        }
    }
}

/// One flag-diacritic operation entry in the alphabet's `operations` map:
/// divvunspell's `FlagDiacriticOperation` (`operation`, `feature: u16`,
/// `value: i16`). The value is `i16` because divvunspell's `ValueNumber` is a
/// transparent `i16`.
// [spec:hfst:def:thfst-backend.alphabet-json]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThfstFlagOp {
    pub operation: ThfstFlagOperator,
    pub feature: u16,
    pub value: i16,
}

/// The `alphabet` JSON object — the serde shape of divvunspell's
/// `TransducerAlphabet`, fields in the exact order divvunspell declares them.
/// `key_table`/`string_to_symbol` carry `Symbol` (`SmolStr`), which is exactly
/// what divvunspell's `TransducerAlphabet` uses; `SmolStr` serializes as a
/// plain string so the JSON text is byte-identical.
// [spec:hfst:def:thfst-backend.alphabet-json]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThfstAlphabet {
    /// Symbol i's string; epsilon and unhandled specials are stored as "".
    pub key_table: Vec<Symbol>,
    /// The header symbol count N (divvunspell: `initial_symbol_count`).
    pub initial_symbol_count: u16,
    /// Count of distinct flag features.
    pub flag_state_size: u16,
    /// Σ over the original symbol strings of (byte length + 1).
    pub length: usize,
    /// Plain symbol string → symbol number (no flags/specials).
    pub string_to_symbol: BTreeMap<Symbol, u16>,
    /// Symbol number → flag operation (serde stringifies the integer key).
    pub operations: BTreeMap<u16, ThfstFlagOp>,
    pub identity_symbol: Option<u16>,
    pub unknown_symbol: Option<u16>,
}

/// Build the alphabet JSON from a weighted optimized-lookup engine by
/// replicating divvunspell's `TransducerAlphabetParser::parse_inner` over the
/// symbol STRINGS. The scan is 0..N where N = the header symbol count; the
/// symbol table length must equal N.
// [spec:hfst:def:thfst-backend.alphabet-json]
// [spec:hfst:sem:thfst-backend.alphabet-json]
pub fn build_alphabet_json(t: &Transducer<WeightedTables>) -> crate::error::Result<ThfstAlphabet> {
    let n: SymbolNumber = t.get_header().symbol_count();
    let symbols: &SymbolTable = t.get_symbol_table();
    if symbols.len() != n as usize {
        crate::bail!(
            Hfst,
            format!(
                "THFST alphabet: symbol table length {} != header symbol count {n}",
                symbols.len()
            )
        );
    }

    let mut key_table: Vec<Symbol> = Vec::with_capacity(symbols.len());
    let mut string_to_symbol: BTreeMap<Symbol, u16> = BTreeMap::new();
    let mut operations: BTreeMap<u16, ThfstFlagOp> = BTreeMap::new();
    let mut identity_symbol: Option<u16> = None;
    let mut unknown_symbol: Option<u16> = None;

    // Shared first-encounter buckets, exactly as divvunspell's parser holds
    // them: `feature_bucket`/`value_bucket` assign 0-based numbers in
    // first-encounter order, `feat_n`/`val_n` are the next-to-assign counters.
    let mut feature_bucket: BTreeMap<String, u16> = BTreeMap::new();
    let mut value_bucket: BTreeMap<String, i16> = BTreeMap::new();
    let mut feat_n: u16 = 0;
    let mut val_n: i16 = 0;

    // `length` = Σ over the ORIGINAL strings of (byte length + 1) — the byte
    // size the alphabet section occupies in the OL binary (NUL terminators).
    let mut length: usize = 0;

    for (i, key) in symbols.iter().enumerate() {
        let i = i as u16;
        length += key.len() + 1;

        // divvunspell: `key.len() > 1 && starts_with('@') && ends_with('@')`.
        if key.len() > 1 && key.starts_with('@') && key.ends_with('@') {
            // A flag is @<op>.FEATURE.VALUE@: at least 5 bytes with '.' at
            // index 2 (`@<op>.…`).
            let is_flag = key.len() >= 5 && key.as_bytes().get(2) == Some(&b'.');

            if is_flag {
                // [spec:hfst:sem:thfst-backend.alphabet-json]
                // Operator = char at index 1; must be one of P N R D C U, else
                // a hard error mirroring divvunspell's parser.
                let op_char = key.chars().nth(1).ok_or_else(|| {
                    crate::err!(Hfst, format!("THFST alphabet: malformed flag key '{key}'"))
                })?;
                let operation = ThfstFlagOperator::from_char(op_char).ok_or_else(|| {
                    crate::err!(
                        Hfst,
                        format!("THFST alphabet: unknown flag diacritic operator in key '{key}'")
                    )
                })?;

                // divvunspell splits the WHOLE key on '.': chunk[0] is the head
                // (`@<op>`), chunk[1] the feature, chunk[2] the value; '@' chars
                // are stripped from feature/value so the trailing '@' of the
                // last chunk falls away. Missing chunks default to "" (so
                // valueless flags like `@P.FOO@` get value "").
                let mut chunks = key.split('.');
                let _head = chunks.next();
                let feature: String = chunks
                    .next()
                    .unwrap_or("")
                    .chars()
                    .filter(|c| *c != '@')
                    .collect();
                let value: String = chunks
                    .next()
                    .unwrap_or("")
                    .chars()
                    .filter(|c| *c != '@')
                    .collect();

                // First-encounter numbering over the shared buckets.
                let feat = *feature_bucket.entry(feature).or_insert_with(|| {
                    let n = feat_n;
                    feat_n += 1;
                    n
                });
                let val = *value_bucket.entry(value).or_insert_with(|| {
                    let n = val_n;
                    val_n += 1;
                    n
                });

                operations.insert(
                    i,
                    ThfstFlagOp {
                        operation,
                        feature: feat,
                        value: val,
                    },
                );
                key_table.push(key.clone());
            } else if key == "@_EPSILON_SYMBOL_@" {
                // Epsilon: key_table gets "", and "" is inserted into the value
                // bucket consuming the next value number (val 0 for symbol-0).
                value_bucket.entry("".to_string()).or_insert_with(|| {
                    let n = val_n;
                    val_n += 1;
                    n
                });
                key_table.push(Symbol::default());
            } else if key == "@_IDENTITY_SYMBOL_@" {
                identity_symbol = Some(i);
                key_table.push(key.clone());
            } else if key == "@_UNKNOWN_SYMBOL_@" {
                unknown_symbol = Some(i);
                key_table.push(key.clone());
            } else {
                // An unrecognised @...@ special: push "" with a warning. The
                // string is lost, exactly as in divvunspell.
                tracing::warn!("unhandled THFST alphabet key '{key}'");
                key_table.push(Symbol::default());
            }
        } else {
            // A plain symbol: pushed literally with a string_to_symbol entry.
            key_table.push(key.clone());
            string_to_symbol.insert(key.clone(), i);
        }
    }

    Ok(ThfstAlphabet {
        key_table,
        initial_symbol_count: n,
        flag_state_size: u16::try_from(feature_bucket.len())
            .map_err(|_| crate::err!(Hfst, "THFST alphabet: too many flag features for u16"))?,
        length,
        string_to_symbol,
        operations,
        identity_symbol,
        unknown_symbol,
    })
}

// -----------------------------------------------------------------------------
// Table writers — flat little-endian record arrays.
// -----------------------------------------------------------------------------

/// Serialize the `index` file: an 8-byte LE record per index-table slot
/// { u16 input_symbol, u16 padding = 0, u32 weight_or_target }. The u32 is the
/// OLW `first_transition_index` copied RAW (it already carries f32 weight bits
/// for final entries and 0xFFFFFFFF for empty slots) — never decode/re-encode.
// [spec:hfst:def:thfst-backend.index-record]
// [spec:hfst:sem:thfst-backend.index-record]
fn write_index_table(t: &Transducer<WeightedTables>, out: &mut Vec<u8>) {
    let size = t.get_header().index_table_size();
    out.reserve(size as usize * 8);
    for i in 0..size {
        let input: u16 = t.get_index_input(i);
        // Raw stored u32 — bit copy, never a decode/re-encode.
        let raw: u32 = t.get_index_target(i);
        out.extend_from_slice(&input.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&raw.to_le_bytes());
    }
}

/// Serialize the `transition` file: a 12-byte LE record per transition-table
/// slot { u16 input, u16 output, u32 target, f32 weight } — the weight as the
/// LE bytes of its f32 bits. 0xFFFF / 0xFFFFFFFF are the no-symbol / no-target
/// markers, emitted verbatim from the accessors.
// [spec:hfst:def:thfst-backend.transition-record]
// [spec:hfst:sem:thfst-backend.transition-record]
fn write_transition_table(t: &Transducer<WeightedTables>, out: &mut Vec<u8>) {
    let size = t.get_header().target_table_size();
    out.reserve(size as usize * 12);
    for i in 0..size {
        let input: u16 = t.get_transition_input(i);
        let output: u16 = t.get_transition_output(i);
        let target: u32 = t.get_transition_target(i);
        let weight: f32 = t.get_transition_weight(i);
        out.extend_from_slice(&input.to_le_bytes());
        out.extend_from_slice(&output.to_le_bytes());
        out.extend_from_slice(&target.to_le_bytes());
        out.extend_from_slice(&weight.to_bits().to_le_bytes());
    }
}

/// Write a weighted optimized-lookup engine as a `X.thfst/` directory: create
/// `dir` (and parents) if absent, then write the three members. Against the
/// reference producer (`thfst-tools hfst-to-thfst` on the same OLW input),
/// `index` and `transition` are byte-identical and `alphabet` is semantically
/// equal.
// [spec:hfst:def:thfst-backend.write-dir-fn]
// [spec:hfst:sem:thfst-backend.write-dir-fn]
pub fn write_dir(t: &Transducer<WeightedTables>, dir: &Path) -> crate::error::Result<()> {
    let alphabet = build_alphabet_json(t)?;

    std::fs::create_dir_all(dir)
        .map_err(|e| crate::err!(Hfst, format!("THFST: cannot create dir {dir:?}: {e}")))?;

    let mut transition_bytes = Vec::new();
    write_transition_table(t, &mut transition_bytes);
    write_file(&dir.join("transition"), &transition_bytes)?;

    let mut index_bytes = Vec::new();
    write_index_table(t, &mut index_bytes);
    write_file(&dir.join("index"), &index_bytes)?;

    // Pretty-printed JSON, matching divvunspell's `to_writer_pretty`.
    let alphabet_path = dir.join("alphabet");
    let json = serde_json::to_vec_pretty(&alphabet)
        .map_err(|e| crate::err!(Hfst, format!("THFST: alphabet JSON encode failed: {e}")))?;
    write_file(&alphabet_path, &json)?;

    Ok(())
}

fn write_file(path: &Path, bytes: &[u8]) -> crate::error::Result<()> {
    let mut f = std::fs::File::create(path)
        .map_err(|e| crate::err!(Hfst, format!("THFST: cannot create {path:?}: {e}")))?;
    f.write_all(bytes)
        .map_err(|e| crate::err!(Hfst, format!("THFST: cannot write {path:?}: {e}")))?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Reader — reconstruct the OLW engine under the THFST tag.
// -----------------------------------------------------------------------------

/// Read a `X.thfst/` directory into a weighted optimized-lookup engine. The
/// directory must contain all three member files (`alphabet`, `index`,
/// `transition`), else `NotTransducerStream`. The alphabet's key_table rebuilds
/// the engine symbol table (index 0's "" restored to `@_EPSILON_SYMBOL_@`),
/// re-deriving flag operations and identity/unknown from the strings — the JSON
/// operation/feature/value NUMBERS are ignored. The OL header THFST does not
/// store is synthesized: both symbol counts = key_table length, table sizes =
/// the exact record counts, weighted = true, all property flags false.
// [spec:hfst:def:thfst-backend.read-dir-fn]
// [spec:hfst:sem:thfst-backend.read-dir-fn]
pub fn read_dir(dir: &Path) -> crate::error::Result<Transducer<WeightedTables>> {
    let alphabet_path = dir.join("alphabet");
    let index_path = dir.join("index");
    let transition_path = dir.join("transition");

    // Require all three member files; naming them in the message.
    if !alphabet_path.is_file() || !index_path.is_file() || !transition_path.is_file() {
        crate::bail!(
            NotTransducerStream,
            format!(
                "THFST directory {dir:?} must contain all three files: alphabet, index, transition"
            )
        );
    }

    let alphabet_bytes = read_file(&alphabet_path)?;
    let alphabet: ThfstAlphabet = serde_json::from_slice(&alphabet_bytes)
        .map_err(|e| crate::err!(Hfst, format!("THFST: alphabet JSON parse failed: {e}")))?;

    // Rebuild the engine symbol table from key_table: index 0's "" is the
    // canonical epsilon; other "" entries stay "" (lost specials). The engine's
    // own symbol-table constructor re-derives fd ops + identity/unknown.
    // [spec:hfst:sem:thfst-backend.read-dir-fn]
    let mut symbol_table: SymbolTable = Vec::with_capacity(alphabet.key_table.len());
    for (i, s) in alphabet.key_table.iter().enumerate() {
        if i == 0 && s.is_empty() {
            symbol_table.push("@_EPSILON_SYMBOL_@".into());
        } else {
            symbol_table.push(s.clone());
        }
    }
    let n_syms = u16::try_from(symbol_table.len())
        .map_err(|_| crate::err!(Hfst, "THFST: too many symbols for u16 symbol count"))?;
    let engine_alphabet = TransducerAlphabet::new_symboltable(&symbol_table);

    // Read the raw table bytes with length-multiple guards (8 / 12).
    let index_raw = read_file(&index_path)?;
    if index_raw.len() % 8 != 0 {
        crate::bail!(
            NotTransducerStream,
            format!(
                "THFST index file length {} is not a multiple of 8",
                index_raw.len()
            )
        );
    }
    let transition_raw = read_file(&transition_path)?;
    if transition_raw.len() % 12 != 0 {
        crate::bail!(
            NotTransducerStream,
            format!(
                "THFST transition file length {} is not a multiple of 12",
                transition_raw.len()
            )
        );
    }

    let index_records = (index_raw.len() / 8) as u32;
    let transition_records = (transition_raw.len() / 12) as u32;

    // Decode index records into TransitionWIndex. The 8-byte LE record is
    // { u16 input, u16 padding, u32 weight_or_target }; the u32 is restored RAW
    // into `first_transition_index` (it already carries the f32 weight bits for
    // final entries and 0xFFFFFFFF for empty slots).
    // [spec:hfst:sem:thfst-backend.index-record]
    let mut index_table: TransducerTable<TransitionWIndex> = TransducerTable::new();
    for r in 0..index_records as usize {
        let base = r * 8;
        let input = u16::from_le_bytes([index_raw[base], index_raw[base + 1]]);
        let raw = u32::from_le_bytes([
            index_raw[base + 4],
            index_raw[base + 5],
            index_raw[base + 6],
            index_raw[base + 7],
        ]);
        index_table.append(TransitionWIndex::new_values(input, raw));
    }

    // Decode transition records into TransitionW. The 12-byte LE record is
    // { u16 input, u16 output, u32 target, f32 weight-bits }.
    // [spec:hfst:sem:thfst-backend.transition-record]
    let mut transition_table: TransducerTable<TransitionW> = TransducerTable::new();
    for r in 0..transition_records as usize {
        let base = r * 12;
        let input = u16::from_le_bytes([transition_raw[base], transition_raw[base + 1]]);
        let output = u16::from_le_bytes([transition_raw[base + 2], transition_raw[base + 3]]);
        let target = u32::from_le_bytes([
            transition_raw[base + 4],
            transition_raw[base + 5],
            transition_raw[base + 6],
            transition_raw[base + 7],
        ]);
        let weight_bits = u32::from_le_bytes([
            transition_raw[base + 8],
            transition_raw[base + 9],
            transition_raw[base + 10],
            transition_raw[base + 11],
        ]);
        transition_table.append(TransitionW::new_values(
            input,
            output,
            target,
            f32::from_bits(weight_bits),
        ));
    }

    // Synthesize the OL header THFST does not store: both symbol counts =
    // n_syms, table sizes = exact record counts, weighted = true; `new_sizes`
    // leaves state/transition counts 0 and all nine property flags false (the
    // documented display-only + is_infinitely_ambiguous divergence).
    // [spec:hfst:sem:thfst-backend.read-dir-fn]
    let header =
        TransducerHeader::new_sizes(n_syms, n_syms, index_records, transition_records, true);

    let tables = TransducerTables::new_tables(index_table, transition_table);
    Ok(Transducer::new_from_tables(
        &header,
        &engine_alphabet,
        tables,
    ))
}

fn read_file(path: &Path) -> crate::error::Result<Vec<u8>> {
    let mut f = std::fs::File::open(path)
        .map_err(|e| crate::err!(Hfst, format!("THFST: cannot open {path:?}: {e}")))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| crate::err!(Hfst, format!("THFST: cannot read {path:?}: {e}")))?;
    Ok(buf)
}
