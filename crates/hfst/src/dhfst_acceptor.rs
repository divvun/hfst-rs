//! DHFST acceptors (type 2): their layout and a reader. Authored greenfield
//! against `docs/spec/port/back-ends/dhfst/dhfst.md`. The consumer of record
//! is divvunspell's `src/transducer/dhfst/acceptor.rs`, and this reader is a
//! port of it: the same validation on load, and the same answers to the
//! lookups the suggestion search makes. The writer is
//! [`crate::dhfst_acceptor_writer`].
//!
//! A state is a number `q` below `n_ids`, the start state 0. Its arcs are
//! found at slots `q + x` of a table like optimized lookup's index table:
//! slot `q + s`, for a regular symbol `s` (neither epsilon nor a flag
//! diacritic), holds the arcs of `q` on `s` when its check is `s`, and slot
//! `q` holds the free arcs of `q` (epsilon and flag diacritics) when its check
//! is 0 and its record is not zero. No two states share a number or a slot,
//! so a slot whose check is `s` belongs to the state `s` below it. Every arc's
//! output is its input.
//!
//! After the eight bytes of [`crate::dhfst_header`] come `u32` flags (bit 0,
//! tropical `f32` weights, is required), the `u32` section count and eight
//! reserved zero bytes; then the section table and the sections, as in an
//! error model. The sections are `SYMS`, `CHCK` (the checks), `SLOT` (a
//! record per slot), `LIST` (arcs that do not fit their slot), `FINL` (the
//! final states), `FREE` (free arc symbols and weights), `WGHT` (regular arc
//! weights), the optional `DIST` (every state's distance to a final state)
//! and the ancillary `meta`. Every array is followed by at least 8 zero bytes
//! inside its section.

use crate::dhfst::{corrupt, parse_symbols, u16_at, u32_at, u64_at};
use crate::dhfst_header::{DhfstType, read_type, wrong_type};
use crate::dhfst_source::flag_name;

/// Bytes before the section table.
pub const HEADER_LEN: usize = 24;
/// Bytes per section table record.
pub const SECTION_ENTRY_LEN: usize = 24;
/// Zero bytes after every array, so that 8 bytes may be read at any element.
pub const PADDING: usize = 8;
/// State numbers per `FINL` entry.
pub const FINL_STATES: usize = 64;
/// Bytes per `FINL` entry.
pub const FINL_ENTRY_LEN: usize = 12;
/// Bytes of the `CHCK` head.
pub const CHCK_HEAD_LEN: usize = 16;
/// Bytes of the `SLOT` and `LIST` heads.
pub const RECORD_HEAD_LEN: usize = 8;
/// Header flag: weights are tropical `f32`.
pub const FLAG_TROPICAL: u32 = 1 << 0;

/// Section tags. An upper-case tag is critical, a lower-case one ancillary.
// [spec:hfst:def:dhfst.acceptor-layout]
pub mod tag {
    /// symbol table
    pub const SYMS: [u8; 4] = *b"SYMS";
    /// the check of every slot
    pub const CHCK: [u8; 4] = *b"CHCK";
    /// the record of every slot
    pub const SLOT: [u8; 4] = *b"SLOT";
    /// arcs that do not fit in their slot
    pub const LIST: [u8; 4] = *b"LIST";
    /// final states
    pub const FINL: [u8; 4] = *b"FINL";
    /// free arc symbols and weights
    pub const FREE: [u8; 4] = *b"FREE";
    /// regular arc weights
    pub const WGHT: [u8; 4] = *b"WGHT";
    /// every state's distance to a final state
    pub const DIST: [u8; 4] = *b"DIST";
    /// writer metadata
    pub const META: [u8; 4] = *b"meta";
}

/// The sections this reader knows.
const KNOWN: [[u8; 4]; 9] = [
    tag::SYMS,
    tag::CHCK,
    tag::SLOT,
    tag::LIST,
    tag::FINL,
    tag::FREE,
    tag::WGHT,
    tag::DIST,
    tag::META,
];

/// A section's byte range.
type Extent = (usize, usize);

/// Where the sections of a validated file are, and how wide its fields.
#[derive(Clone, Copy, Debug, Default)]
struct Layout {
    label_max: u32,
    n_ids: u32,
    /// the low byte of the check of slot `k` is at `checks + k`
    checks: usize,
    /// the high byte, when the checks have one, at `checks_hi + k`
    wide: bool,
    checks_hi: usize,
    records: usize,
    list: usize,
    record_bytes: usize,
    record_mask: u64,
    target_bits: u32,
    target_mask: u64,
    index_mask: u64,
    n_slots: u32,
    n_list: u32,
    finals: usize,
    final_weights: usize,
    n_finals: u32,
    free: usize,
    n_free: u32,
    weights: usize,
    n_weights: u32,
    distances: usize,
    n_distances: u32,
}

/// How a file stores its arcs, for the writer's report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcceptorInfo {
    /// state numbers
    pub ids: u32,
    /// slots in the table
    pub slots: u32,
    /// slots that hold arcs, free or regular
    pub used_slots: u32,
    /// bytes per check
    pub check_bytes: usize,
    /// the greatest regular label
    pub label_max: u32,
    /// bytes per record
    pub record_bytes: usize,
    /// records in `LIST`
    pub list_records: u32,
    /// arcs, free and regular
    pub arcs: u64,
    /// free arcs
    pub free_arcs: u64,
    /// distinct symbol and weight pairs of the free arcs
    pub free_pairs: u32,
    /// distinct regular arc weights
    pub weights: u32,
    /// final states
    pub finals: u32,
    /// whether the file stores distances to a final state
    pub distances: bool,
}

/// A validated DHFST acceptor, read in place.
///
/// [`AcceptorReader::parse`] checks every section, check, record, list,
/// count, weight and distance, so no lookup afterwards reads outside the
/// bytes.
pub struct AcceptorReader<'a> {
    bytes: &'a [u8],
    layout: Layout,
    names: Vec<String>,
    free_symbols: Vec<bool>,
    meta: Option<String>,
    info: AcceptorInfo,
}

impl<'a> AcceptorReader<'a> {
    /// Validate a whole file, exactly as divvunspell does before it loads one.
    // [spec:hfst:def:dhfst.acceptor-reader]
    // [spec:hfst:sem:dhfst.acceptor-reader]
    pub fn parse(bytes: &'a [u8]) -> crate::error::Result<AcceptorReader<'a>> {
        let sections = parse_sections(bytes)?;
        let find = |wanted: [u8; 4]| {
            sections
                .iter()
                .find(|(t, _, _)| *t == wanted)
                .map(|(_, start, end)| (*start, *end))
        };
        let need = |wanted: [u8; 4]| {
            find(wanted).ok_or_else(|| {
                corrupt(format!(
                    "required section {} is missing",
                    String::from_utf8_lossy(&wanted)
                ))
            })
        };

        let names = parse_symbols(bytes, need(tag::SYMS)?, u16::MAX as u32)?;
        let free_symbols = free_symbols(&names)?;
        let mut layout = parse_tables(bytes, need(tag::CHCK)?, need(tag::SLOT)?, need(tag::LIST)?)?;
        if layout.label_max >= names.len() as u32 {
            return Err(corrupt(format!(
                "label {} is outside the alphabet",
                layout.label_max
            )));
        }
        (layout.finals, layout.final_weights, layout.n_finals) =
            parse_finals(bytes, need(tag::FINL)?, layout.n_ids)?;
        (layout.free, layout.n_free) = parse_free_pairs(bytes, need(tag::FREE)?, &free_symbols)?;
        (layout.weights, layout.n_weights) = parse_floats(bytes, need(tag::WGHT)?, "WGHT")?;
        if layout.n_weights as u64 > layout.index_mask {
            return Err(corrupt(format!(
                "{} weights do not fit indices of {} bits",
                layout.n_weights,
                layout.index_mask.count_ones()
            )));
        }
        if let Some(extent) = find(tag::DIST) {
            (layout.distances, layout.n_distances) = parse_floats(bytes, extent, "DIST")?;
            if layout.n_distances != layout.n_ids {
                return Err(corrupt(format!(
                    "DIST holds {} distances for {} state numbers",
                    layout.n_distances, layout.n_ids
                )));
            }
        }
        let (arcs, free_arcs, used_slots) = check_slots(bytes, &layout, &free_symbols)?;
        let info = AcceptorInfo {
            ids: layout.n_ids,
            slots: layout.n_slots,
            used_slots,
            check_bytes: 1 + layout.wide as usize,
            label_max: layout.label_max,
            record_bytes: layout.record_bytes,
            list_records: layout.n_list,
            arcs,
            free_arcs,
            free_pairs: layout.n_free,
            weights: layout.n_weights,
            finals: layout.n_finals,
            distances: layout.n_distances > 0,
        };
        Ok(AcceptorReader {
            bytes,
            layout,
            names,
            free_symbols,
            meta: find(tag::META)
                .and_then(|(start, end)| std::str::from_utf8(&bytes[start..end]).ok())
                .map(|s| s.trim_end_matches('\0').to_string()),
            info,
        })
    }

    /// The number of state numbers: every state is numbered below it.
    pub fn id_count(&self) -> u32 {
        self.layout.n_ids
    }

    /// Symbol names as stored, `@_EPSILON_SYMBOL_@` first.
    pub fn symbol_names(&self) -> &[String] {
        &self.names
    }

    /// Whether `symbol` is free: epsilon or a flag diacritic.
    pub fn is_free(&self, symbol: u16) -> bool {
        self.free_symbols
            .get(symbol as usize)
            .copied()
            .unwrap_or(false)
    }

    /// The writer's `meta` section, if the file carries one.
    pub fn meta(&self) -> Option<&str> {
        self.meta.as_deref()
    }

    /// How the file stores its arcs.
    pub fn info(&self) -> AcceptorInfo {
        self.info
    }

    /// The 8 bytes at `at`, little-endian; validation put them in the file.
    fn word(&self, at: usize) -> u64 {
        u64_at(self.bytes, at)
    }

    /// The record at byte `at`.
    fn record(&self, at: usize) -> u64 {
        self.word(at) & self.layout.record_mask
    }

    /// The check of slot `slot`.
    fn check(&self, slot: usize) -> u32 {
        let l = &self.layout;
        let low = self.bytes[l.checks + slot] as u32;
        if l.wide {
            low | (self.bytes[l.checks_hi + slot] as u32) << 8
        } else {
            low
        }
    }

    /// The slot holding the arcs of state `q` on `symbol`, if it has any.
    /// Symbol 0 and the symbols no label reaches are refused before the table
    /// is read.
    fn slot(&self, q: u32, symbol: u16) -> Option<usize> {
        let l = &self.layout;
        let s = symbol as u32;
        if s.wrapping_sub(1) >= l.label_max || q >= l.n_ids {
            return None;
        }
        let slot = q as usize + s as usize;
        (self.check(slot) == s).then_some(slot)
    }

    /// Whether state `q` has arcs on `symbol`. A free symbol never has: free
    /// arcs are found through [`Self::for_each_free_arc`].
    pub fn has_transitions(&self, q: u32, symbol: u16) -> bool {
        self.slot(q, symbol).is_some()
    }

    /// Hand `visit` the arcs of state `q` on `symbol`, as `(target,
    /// weight)`, in the source's order.
    // [spec:hfst:sem:dhfst.acceptor-reader]
    pub fn for_each_arc<V: FnMut(u32, f32)>(&self, q: u32, symbol: u16, mut visit: V) {
        let Some(slot) = self.slot(q, symbol) else {
            return;
        };
        let l = &self.layout;
        let at = l.records + l.record_bytes * slot;
        let record = self.record(at);
        let (at, end) = if (record >> l.target_bits) & l.index_mask != l.index_mask {
            (at, at + l.record_bytes)
        } else {
            let head = l.list + l.record_bytes * (record & l.target_mask) as usize;
            let n = (self.record(head) & l.target_mask) as usize;
            (head + l.record_bytes, head + l.record_bytes * (n + 1))
        };
        for at in (at..end).step_by(l.record_bytes) {
            let r = self.record(at);
            let weight = self.weight(((r >> l.target_bits) & l.index_mask) as usize);
            visit((r & l.target_mask) as u32, weight);
        }
    }

    /// Whether state `q` has free arcs.
    pub fn has_free_arcs(&self, q: u32) -> bool {
        let (at, end) = self.free_run(q);
        at < end
    }

    /// Hand `visit` the free arcs of state `q`, as `(symbol, target,
    /// weight)`, in the source's order.
    // [spec:hfst:sem:dhfst.acceptor-reader]
    pub fn for_each_free_arc<V: FnMut(u16, u32, f32)>(&self, q: u32, mut visit: V) {
        let l = &self.layout;
        let (at, end) = self.free_run(q);
        for at in (at..end).step_by(l.record_bytes) {
            let r = self.record(at);
            let pair = self.word(l.free + 8 * ((r >> l.target_bits) & l.index_mask) as usize);
            visit(
                pair as u16,
                (r & l.target_mask) as u32,
                f32::from_bits((pair >> 32) as u32),
            );
        }
    }

    /// The records of the free arcs of state `q`, as the bytes `at .. end`:
    /// none when its slot is empty or holds another state's arcs.
    fn free_run(&self, q: u32) -> (usize, usize) {
        let l = &self.layout;
        if q >= l.n_ids || self.check(q as usize) != 0 {
            return (0, 0);
        }
        let record = self.record(l.records + l.record_bytes * q as usize);
        let at = l.list + l.record_bytes * (record & l.target_mask) as usize;
        let n = ((record >> l.target_bits) & l.index_mask) as usize;
        (at, at + l.record_bytes * n)
    }

    /// The regular arc weight `index`.
    fn weight(&self, index: usize) -> f32 {
        f32::from_bits(u32_at(self.bytes, self.layout.weights + 4 * index))
    }

    /// Whether state `q` is final.
    pub fn is_final(&self, q: u32) -> bool {
        self.final_weight(q).is_some()
    }

    /// The final weight of state `q`, if it is final.
    pub fn final_weight(&self, q: u32) -> Option<f32> {
        let l = &self.layout;
        if q >= l.n_ids {
            return None;
        }
        let entry = l.finals + FINL_ENTRY_LEN * (q as usize / FINL_STATES);
        let bits = self.word(entry);
        let bit = q % FINL_STATES as u32;
        if (bits >> bit) & 1 == 0 {
            return None;
        }
        let rank = u32_at(self.bytes, entry + 8) + (bits & ((1u64 << bit) - 1)).count_ones();
        Some(f32::from_bits(u32_at(
            self.bytes,
            l.final_weights + 4 * rank as usize,
        )))
    }

    /// The distance of state `q` to a final state; 0 without `DIST`.
    pub fn distance(&self, q: u32) -> f32 {
        let l = &self.layout;
        if q >= l.n_distances {
            return 0.0;
        }
        f32::from_bits(u32_at(self.bytes, l.distances + 4 * q as usize))
    }
}

/// Which symbols are free: epsilon and the flag diacritics, told by name as
/// divvunspell's alphabet parser tells them, which refuses a flag-shaped name
/// with an operator it does not know.
fn free_symbols(names: &[String]) -> crate::error::Result<Vec<bool>> {
    names
        .iter()
        .enumerate()
        .map(|(s, name)| {
            flag_name(name)
                .map(|flag| s == 0 || flag)
                .map_err(|_| corrupt(format!("symbol {s} has an unknown flag operator")))
        })
        .collect()
}

/// The header and section table: a type-2 file, its flags known and
/// tropical, its reserved bytes zero, every section at a multiple of 8 past
/// the table and inside the file with zero flags, no tag twice, and no
/// critical tag this reader does not know.
// [spec:hfst:sem:dhfst.header+2]
fn parse_sections(b: &[u8]) -> crate::error::Result<Vec<([u8; 4], usize, usize)>> {
    let kind = read_type(b)?;
    if kind != DhfstType::Acceptor {
        return Err(wrong_type(kind, DhfstType::Acceptor));
    }
    if b.len() < HEADER_LEN {
        return Err(corrupt("the header is truncated"));
    }
    let flags = u32_at(b, 8);
    if u64_at(b, 16) != 0 {
        return Err(corrupt("reserved header fields are not zero"));
    }
    if flags & !FLAG_TROPICAL != 0 {
        return Err(corrupt(format!("unknown header flags {flags:#x}")));
    }
    if flags & FLAG_TROPICAL == 0 {
        return Err(corrupt("weights are not declared tropical f32"));
    }
    let n_sections = u32_at(b, 12) as usize;
    let table_end = n_sections
        .checked_mul(SECTION_ENTRY_LEN)
        .and_then(|n| n.checked_add(HEADER_LEN))
        .filter(|end| *end <= b.len())
        .ok_or_else(|| corrupt("the section table runs past the end of the file"))?;
    let mut sections: Vec<([u8; 4], usize, usize)> = Vec::with_capacity(n_sections);
    for s in 0..n_sections {
        let at = HEADER_LEN + SECTION_ENTRY_LEN * s;
        let tag = [b[at], b[at + 1], b[at + 2], b[at + 3]];
        let name = String::from_utf8_lossy(&tag).into_owned();
        let end = u64_at(b, at + 8).checked_add(u64_at(b, at + 16));
        let (Ok(offset), Some(Ok(end))) =
            (usize::try_from(u64_at(b, at + 8)), end.map(usize::try_from))
        else {
            return Err(corrupt("a section extent overflows"));
        };
        if offset % 8 != 0 || offset < table_end || end > b.len() || u32_at(b, at + 4) != 0 {
            return Err(corrupt(format!(
                "section {name} at {offset}..{end} is misplaced in a {} byte file",
                b.len()
            )));
        }
        if sections.iter().any(|(t, _, _)| *t == tag) {
            return Err(corrupt(format!("section {name} appears twice")));
        }
        if tag[0].is_ascii_uppercase() && !KNOWN.contains(&tag) {
            return Err(corrupt(format!(
                "critical section {name} is not known to this reader"
            )));
        }
        sections.push((tag, offset, end));
    }
    Ok(sections)
}

/// Whether `count` items of `size` bytes, then the padding, fit between
/// `start` and `end`.
fn fits(count: usize, size: usize, start: usize, end: usize) -> bool {
    count
        .checked_mul(size)
        .and_then(|len| len.checked_add(start + PADDING))
        .is_some_and(|e| e <= end)
}

/// `CHCK`, `SLOT` and `LIST`: their heads and extents, and the field widths
/// they declare.
fn parse_tables(
    b: &[u8],
    (chck, chck_end): Extent,
    (slot, slot_end): Extent,
    (list, list_end): Extent,
) -> crate::error::Result<Layout> {
    if chck_end - chck < CHCK_HEAD_LEN || b[chck + 11..chck + CHCK_HEAD_LEN] != [0; 5] {
        return Err(corrupt("CHCK is truncated"));
    }
    let n_slots = u32_at(b, chck);
    let n_ids = u32_at(b, chck + 4);
    let label_max = u16_at(b, chck + 8) as u32;
    let check_bytes = b[chck + 10] as usize;
    if !(check_bytes == 1 || check_bytes == 2) {
        return Err(corrupt(format!("checks of {check_bytes} bytes")));
    }
    if label_max > (1u32 << (8 * check_bytes)) - 1 {
        return Err(corrupt(format!(
            "label {label_max} does not fit the checks"
        )));
    }
    if n_ids == 0 || (n_slots as u64) < n_ids as u64 + label_max as u64 {
        return Err(corrupt(format!(
            "{n_slots} slots cannot hold {n_ids} states with labels up to {label_max}"
        )));
    }
    // One plane of low bytes, and one of high bytes after it if the checks
    // have two, each followed by its padding.
    let checks = chck + CHCK_HEAD_LEN;
    let plane = n_slots as usize + PADDING;
    if plane
        .checked_mul(check_bytes)
        .and_then(|len| len.checked_add(checks))
        .is_none_or(|e| e > chck_end)
    {
        return Err(corrupt("CHCK runs past its section"));
    }

    if slot_end - slot < RECORD_HEAD_LEN || b[slot + 7] != 0 || u32_at(b, slot) != n_slots {
        return Err(corrupt("SLOT does not cover the slots"));
    }
    let record_bytes = b[slot + 4] as usize;
    let target_bits = b[slot + 5] as u32;
    let index_bits = b[slot + 6] as u32;
    if !(1..=8).contains(&record_bytes)
        || !(1..=32).contains(&target_bits)
        || !(1..=32).contains(&index_bits)
        || target_bits + index_bits > 8 * record_bytes as u32
    {
        return Err(corrupt(format!(
            "records of {record_bytes} bytes cannot hold fields of {target_bits} and {index_bits} bits"
        )));
    }
    let records = slot + RECORD_HEAD_LEN;
    if !fits(n_slots as usize, record_bytes, records, slot_end) {
        return Err(corrupt("SLOT runs past its section"));
    }

    if list_end - list < RECORD_HEAD_LEN
        || b[list + 4] as usize != record_bytes
        || b[list + 5..list + 8] != [0, 0, 0]
    {
        return Err(corrupt("LIST is truncated"));
    }
    let n_list = u32_at(b, list);
    if !fits(
        n_list as usize,
        record_bytes,
        list + RECORD_HEAD_LEN,
        list_end,
    ) {
        return Err(corrupt("LIST runs past its section"));
    }
    Ok(Layout {
        label_max,
        n_ids,
        checks,
        wide: check_bytes == 2,
        checks_hi: checks + plane,
        records,
        list: list + RECORD_HEAD_LEN,
        record_bytes,
        record_mask: if record_bytes >= 8 {
            u64::MAX
        } else {
            (1u64 << (8 * record_bytes)) - 1
        },
        target_bits,
        target_mask: (1u64 << target_bits) - 1,
        index_mask: (1u64 << index_bits) - 1,
        n_slots,
        n_list,
        ..Layout::default()
    })
}

/// `FINL` over `n_ids` state numbers: where its entries start, where its
/// weights start, and how many final states it marks.
fn parse_finals(
    b: &[u8],
    (finl, finl_end): Extent,
    n_ids: u32,
) -> crate::error::Result<(usize, usize, u32)> {
    if finl_end - finl < 8 || u32_at(b, finl + 4) != 0 {
        return Err(corrupt("FINL is truncated"));
    }
    let n_finals = u32_at(b, finl);
    let finals = finl + 8;
    let entries = (n_ids as usize).div_ceil(FINL_STATES);
    let final_weights = finals + FINL_ENTRY_LEN * entries;
    if !fits(n_finals as usize, 4, final_weights, finl_end) {
        return Err(corrupt("FINL runs past its section"));
    }
    let mut total = 0u32;
    for e in 0..entries {
        let at = finals + FINL_ENTRY_LEN * e;
        let bits = u64_at(b, at);
        let valid = (n_ids as usize - e * FINL_STATES).min(FINL_STATES);
        if u32_at(b, at + 8) != total || (valid < 64 && bits >> valid != 0) {
            return Err(corrupt(format!("FINL entry {e} miscounts")));
        }
        total += bits.count_ones();
    }
    if total != n_finals {
        return Err(corrupt(format!(
            "FINL marks {total} final states and holds {n_finals} weights"
        )));
    }
    for k in 0..n_finals as usize {
        let w = f32::from_bits(u32_at(b, final_weights + 4 * k));
        if !w.is_finite() {
            return Err(corrupt(format!("final weight {k} is {w}")));
        }
    }
    Ok((finals, final_weights, n_finals))
}

/// `FREE`: where its pairs start and how many there are, every symbol a free
/// one and no weight NaN or `-inf`.
fn parse_free_pairs(
    b: &[u8],
    (free, free_end): Extent,
    free_symbols: &[bool],
) -> crate::error::Result<(usize, u32)> {
    if free_end - free < 8 || u32_at(b, free + 4) != 0 {
        return Err(corrupt("FREE is truncated"));
    }
    let n_free = u32_at(b, free);
    let free = free + 8;
    if !fits(n_free as usize, 8, free, free_end) {
        return Err(corrupt("FREE runs past its section"));
    }
    for k in 0..n_free as usize {
        let symbol = u16_at(b, free + 8 * k);
        let w = f32::from_bits(u32_at(b, free + 8 * k + 4));
        if !free_symbols.get(symbol as usize).copied().unwrap_or(false)
            || u16_at(b, free + 8 * k + 2) != 0
            || w.is_nan()
            || w == f32::NEG_INFINITY
        {
            return Err(corrupt(format!("free pair {k} is invalid")));
        }
    }
    Ok((free, n_free))
}

/// `WGHT` or `DIST`, named `name`: `u32 n; u32 0; f32 values[n]` and the
/// padding, no value NaN or `-inf`. Answers where the values start and how
/// many there are.
fn parse_floats(b: &[u8], (start, end): Extent, name: &str) -> crate::error::Result<(usize, u32)> {
    if end - start < 8 || u32_at(b, start + 4) != 0 {
        return Err(corrupt(format!("{name} is truncated")));
    }
    let n = u32_at(b, start);
    if !fits(n as usize, 4, start + 8, end) {
        return Err(corrupt(format!("{name} runs past its section")));
    }
    for k in 0..n as usize {
        let w = f32::from_bits(u32_at(b, start + 8 + 4 * k));
        if w.is_nan() || w == f32::NEG_INFINITY {
            return Err(corrupt(format!("{name} value {k} is {w}")));
        }
    }
    Ok((start + 8, n))
}

/// Check every slot, and every record of `LIST` a slot names: a check of 0
/// or a regular label whose state is a state number, no stray record bits,
/// targets below `n_ids`, indices in range, counts of at least 1 (free) or 2
/// (listed). Answers the number of arcs, of free arcs, and of slots that
/// hold any.
// [spec:hfst:sem:dhfst.acceptor-reader]
fn check_slots(
    b: &[u8],
    l: &Layout,
    free_symbols: &[bool],
) -> crate::error::Result<(u64, u64, u32)> {
    let fields = l.target_bits + l.index_mask.count_ones();
    let stray = if fields >= 64 {
        0
    } else {
        l.record_mask & !((1u64 << fields) - 1)
    };
    let record = |at: usize| u64_at(b, at) & l.record_mask;
    let regular: Vec<bool> = (0..=l.label_max as usize)
        .map(|s| s != 0 && !free_symbols.get(s).copied().unwrap_or(false))
        .collect();
    let (n_ids, n_weights, n_list) = (l.n_ids as u64, l.n_weights as u64, l.n_list as u64);
    // Whether the listed records `k .. k + n` each hold a target and an
    // index below `limit`.
    let listed = |k: u64, n: u64, limit: u64| {
        k + n <= n_list
            && (k..k + n).all(|k| {
                let r = record(l.list + l.record_bytes * k as usize);
                r & stray == 0
                    && r & l.target_mask < n_ids
                    && (r >> l.target_bits) & l.index_mask < limit
            })
    };
    let bad = |p: usize, what: &str| corrupt(format!("slot {p}: {what}"));

    let (mut arcs, mut free_arcs, mut used) = (0u64, 0u64, 0u32);
    for p in 0..l.n_slots as usize {
        let c = if l.wide {
            b[l.checks + p] as u32 | (b[l.checks_hi + p] as u32) << 8
        } else {
            b[l.checks + p] as u32
        };
        let r = record(l.records + l.record_bytes * p);
        let (first, second) = (r & l.target_mask, (r >> l.target_bits) & l.index_mask);
        // Whether the slot's label is regular and its state a state number.
        let owned = regular.get(c as usize).copied().unwrap_or(false)
            && (p as u64).wrapping_sub(c as u64) < n_ids;
        if owned && r & stray == 0 && first < n_ids && second < n_weights {
            used += 1;
            arcs += 1;
            continue;
        }
        if r == 0 && c == 0 {
            continue;
        }
        if r & stray != 0 {
            return Err(bad(p, "its record has stray bits"));
        }
        used += 1;
        if c == 0 {
            if p as u64 >= n_ids || second == 0 || !listed(first, second, l.n_free as u64) {
                return Err(bad(p, "its free arcs are out of range"));
            }
            free_arcs += second;
            arcs += second;
        } else if owned && second == l.index_mask && first < n_list {
            let head = record(l.list + l.record_bytes * first as usize);
            let n = head & l.target_mask;
            if head & !l.target_mask != 0 || n < 2 || !listed(first + 1, n, n_weights) {
                return Err(bad(p, "its arcs are miscounted or out of range"));
            }
            arcs += n;
        } else {
            return Err(bad(p, "its check or its arc is out of range"));
        }
    }
    Ok((arcs, free_arcs, used))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dhfst_acceptor_writer::tests::{LEXICON, lexicon, wide, written};

    /// Parsing `bytes` fails, and says `reason`.
    fn assert_refused(bytes: &[u8], reason: &str) {
        match AcceptorReader::parse(bytes) {
            Ok(_) => panic!("a file that should fail with {reason:?} parsed"),
            Err(e) => assert!(
                e.to_string().contains(reason),
                "{e} does not say {reason:?}"
            ),
        }
    }

    /// `bytes` with byte `at` set to `value`.
    fn with_byte(bytes: &[u8], at: usize, value: u8) -> Vec<u8> {
        let mut out = bytes.to_vec();
        out[at] = value;
        out
    }

    /// Ask a reader every question a search could, for every state number
    /// and symbol and a few past them; answers a sum so nothing is skipped.
    fn sweep(t: &AcceptorReader<'_>) -> u64 {
        let mut seen = 0u64;
        let n = t.symbol_names().len() as u32 + 2;
        for q in 0..t.id_count() + 2 {
            seen += u64::from(t.is_final(q));
            seen += t.final_weight(q).map_or(0, |w| w.to_bits() as u64 & 1);
            seen += t.distance(q).to_bits() as u64 & 1;
            seen += u64::from(t.has_free_arcs(q));
            t.for_each_free_arc(q, |s, target, _| seen += s as u64 + target as u64);
            for s in 0..n {
                if t.has_transitions(q, s as u16) {
                    t.for_each_arc(q, s as u16, |target, _| seen += target as u64);
                }
            }
        }
        seen
    }

    // [spec:hfst:def:dhfst.acceptor-reader/test]
    // [spec:hfst:sem:dhfst.acceptor-reader/test]
    // [spec:hfst:def:dhfst.header+2/test]
    // [spec:hfst:sem:dhfst.header+2/test]
    #[test]
    fn refuses_other_types_and_bad_headers() {
        let bytes = written(&lexicon(LEXICON), 1).bytes;
        AcceptorReader::parse(&bytes).expect("the written file loads");
        let flags = u32_at(&bytes, 8);
        let mut flagged = bytes.clone();
        flagged[8..12].copy_from_slice(&(flags | 1 << 1).to_le_bytes());
        let mut untropical = bytes.clone();
        untropical[8..12].copy_from_slice(&0u32.to_le_bytes());
        for (file, reason) in [
            (
                with_byte(&bytes, 5, 1),
                "this DHFST file is an error model (type 1); an acceptor is type 2",
            ),
            (with_byte(&bytes, 5, 9), "DHFST type 9 is not a type"),
            (with_byte(&bytes, 6, 2), "DHFST acceptor version 2"),
            (with_byte(&bytes, 7, 1), "header byte 7 is 1"),
            (with_byte(&bytes, 20, 1), "reserved header fields"),
            (flagged, "unknown header flags"),
            (untropical, "tropical"),
            (bytes[..20].to_vec(), "truncated"),
            (with_byte(&bytes, 24 + 4, 1), "misplaced"),
        ] {
            assert_refused(&file, reason);
        }
    }

    // [spec:hfst:sem:dhfst.acceptor-reader/test]
    #[test]
    fn damaged_files_load_or_refuse_but_never_misread() {
        for (bytes, stride) in [
            (written(&lexicon(LEXICON), 1).bytes, 1),
            (written(&lexicon(&wide()), 1).bytes, 13),
        ] {
            let reader = AcceptorReader::parse(&bytes).expect("the written file loads");
            sweep(&reader);
            let (mut loaded, mut refused) = (0, 0);
            for at in (0..bytes.len()).step_by(stride) {
                for flip in [0x01u8, 0x80, 0xFF] {
                    let damaged = with_byte(&bytes, at, bytes[at] ^ flip);
                    match AcceptorReader::parse(&damaged) {
                        Ok(t) => {
                            sweep(&t);
                            loaded += 1;
                        }
                        Err(_) => refused += 1,
                    }
                }
            }
            for len in (0..bytes.len()).step_by(3) {
                assert!(
                    AcceptorReader::parse(&bytes[..len]).is_err(),
                    "a file cut to {len} bytes loads"
                );
            }
            assert!(
                loaded > 0 && refused > 0,
                "{loaded} loaded, {refused} refused"
            );
        }
    }
}
