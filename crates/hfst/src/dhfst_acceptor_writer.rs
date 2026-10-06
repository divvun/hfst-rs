//! Writing an acceptor as DHFST type 2 ([`crate::dhfst_acceptor`]). Authored
//! greenfield against `docs/spec/port/back-ends/dhfst/dhfst.md`; the algorithm
//! is divvunspell's acceptor writer (`src/transducer/dhfst/acceptor_writer.rs`)
//! with depth-first placement, and for the same optimized-lookup input the
//! bytes are the bytes `dhfst-tools acceptor` writes, but for the program
//! `meta` names as the writer.
//!
//! The writer reads the source exactly as the suggestion search does
//! ([`AcceptorSource`]), so what it stores is what the search sees: the same
//! arcs, in the same order, with the same weights, bit for bit, and each
//! state's distance to a final state. States are numbered first fit in
//! depth-first pre-order from the start state, which takes 0: each takes the
//! least number no other state has whose slots are all empty.
//!
//! Nothing is written on trust. The bytes are read back through
//! [`AcceptorReader`] and every state's finality, final weight, distance,
//! free arcs and answer for every symbol of the alphabet are compared with
//! the source's. Any difference is an error.

use std::collections::HashMap;

use crate::dhfst_acceptor::{
    AcceptorReader, FINL_STATES, FLAG_TROPICAL, HEADER_LEN, PADDING, SECTION_ENTRY_LEN, tag,
};
pub use crate::dhfst_acceptor_source::{AcceptorSource, Transition};
use crate::dhfst_header::DhfstType;
use crate::dhfst_writer::{WRITER, in_chunks, symbols_section, verification};

fn unsupported(detail: impl std::fmt::Display) -> crate::error::Error {
    crate::err!(
        Hfst,
        format!("the acceptor cannot be written as DHFST: {detail}")
    )
}

/// How to write an acceptor.
#[derive(Clone, Debug, Default)]
pub struct AcceptorOptions {
    /// Worker threads for the self-check; `0` for all cores. The bytes
    /// written do not depend on it.
    pub threads: usize,
    /// A name for the source, recorded in the `meta` section.
    pub source_name: String,
}

/// A written acceptor and what it holds.
#[derive(Clone, Debug)]
pub struct WrittenAcceptor {
    /// the file
    pub bytes: Vec<u8>,
    /// states written
    pub states: u32,
    /// state numbers: every state is numbered below this
    pub ids: u32,
    /// slots in the table
    pub slots: u32,
    /// slots that hold arcs, free or regular
    pub used_slots: u32,
    /// records in `LIST`
    pub list_records: u32,
    /// arcs written, free and regular
    pub arcs: u64,
    /// free (epsilon and flag diacritic) arcs written
    pub free_arcs: u64,
    /// distinct regular arc weights
    pub weights: usize,
    /// distinct free symbol and weight pairs
    pub free_pairs: usize,
    /// whether the file stores distances to a final state
    pub distances: bool,
    /// `(state, symbol)` answers compared against the source
    pub checked: u64,
    /// free symbols on which the source's regular lookup answers yes: a
    /// quirk of the optimized-lookup cursor that the search never asks
    pub free_symbol_quirks: u64,
    /// the source address behind each state, in depth-first order
    pub order: Vec<u32>,
    /// the number of each state of `order`
    pub ids_of: Vec<u32>,
}

/// One arc read from the source: symbol, source target, weight bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Arc {
    symbol: u16,
    target: u32,
    weight: u32,
}

/// A source state as the search reads it.
#[derive(Clone, Debug, Default)]
struct State {
    source: u32,
    final_weight: Option<u32>,
    /// the bits of the source's distance to a final state
    distance: u32,
    free: Vec<Arc>,
    regular: Vec<Arc>,
}

/// An arc from a transition that must be an acceptor arc on `symbol`.
fn acceptor_arc(state: u32, symbol: u16, t: Transition) -> crate::error::Result<Arc> {
    match (t.output, t.target, t.weight) {
        (Some(output), Some(target), Some(weight)) if output == symbol => Ok(Arc {
            symbol,
            target,
            weight: weight.to_bits(),
        }),
        _ => Err(unsupported(format!(
            "arc {t:?} of state {state} on symbol {symbol} is not an acceptor arc"
        ))),
    }
}

/// State `state` as the search reads it.
// [spec:hfst:sem:dhfst.acceptor-source]
fn read_state(source: &AcceptorSource<'_>, state: u32) -> crate::error::Result<State> {
    let final_weight = if source.is_final(state) {
        let w = source
            .final_weight(state)
            .ok_or_else(|| unsupported(format!("final state {state} has no final weight")))?;
        if !w.is_finite() {
            return Err(unsupported(format!("state {state} has final weight {w}")));
        }
        Some(w.to_bits())
    } else {
        None
    };
    let distance = source.distance(state);
    if distance.is_nan() || distance == f32::NEG_INFINITY {
        return Err(unsupported(format!(
            "state {state} has distance {distance} to a final state"
        )));
    }
    let free = source
        .free_arcs(state)
        .map(|(input, t)| acceptor_arc(state, input, t))
        .collect::<crate::error::Result<Vec<Arc>>>()?;
    let mut regular = Vec::new();
    for symbol in 1..source.symbol_count() as u16 {
        if source.is_free(symbol) || !source.has_transitions(state, symbol) {
            continue;
        }
        for t in source.transitions(state, symbol) {
            regular.push(acceptor_arc(state, symbol, t)?);
        }
    }
    if let Some(arc) = free.iter().chain(regular.iter()).find(|a| {
        let w = f32::from_bits(a.weight);
        w.is_nan() || w == f32::NEG_INFINITY
    }) {
        return Err(unsupported(format!(
            "state {state} has an arc weighing {}",
            f32::from_bits(arc.weight)
        )));
    }
    Ok(State {
        source: state,
        final_weight,
        distance: distance.to_bits(),
        free,
        regular,
    })
}

/// Every state reachable from the start state, as the search reads it, in
/// depth-first pre-order with successors in stored order, and each source
/// address's position in that order.
fn read_states(
    source: &AcceptorSource<'_>,
) -> crate::error::Result<(Vec<State>, HashMap<u32, u32>)> {
    let mut states: Vec<State> = Vec::new();
    let mut number: HashMap<u32, u32> = HashMap::new();
    let mut stack = vec![0u32];
    while let Some(s) = stack.pop() {
        if number.contains_key(&s) {
            continue;
        }
        number.insert(s, states.len() as u32);
        let state = read_state(source, s)?;
        for arc in state.free.iter().chain(state.regular.iter()).rev() {
            if !number.contains_key(&arc.target) {
                stack.push(arc.target);
            }
        }
        states.push(state);
    }
    Ok((states, number))
}

/// Bits needed to hold every value in `0..=max`.
fn bits(max: u64) -> u32 {
    64 - max.leading_zeros()
}

/// A dictionary of `values` by descending frequency (ties by value), and each
/// value's index in it.
fn dictionary<V>(values: impl Iterator<Item = V>) -> (Vec<V>, HashMap<V, u32>)
where
    V: Copy + Ord + std::hash::Hash,
{
    let mut count: HashMap<V, u64> = HashMap::new();
    for v in values {
        *count.entry(v).or_default() += 1;
    }
    let mut sorted: Vec<(V, u64)> = count.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let index = sorted
        .iter()
        .enumerate()
        .map(|(i, (v, _))| (*v, i as u32))
        .collect();
    (sorted.into_iter().map(|(v, _)| v).collect(), index)
}

/// A growable bit set.
#[derive(Default)]
struct Bits(Vec<u64>);

impl Bits {
    fn set(&mut self, i: usize) {
        if i / 64 >= self.0.len() {
            self.0.resize(i / 64 + 1, 0);
        }
        self.0[i / 64] |= 1 << (i % 64);
    }

    /// The least clear bit at `i` or after it.
    fn next_clear(&self, i: usize) -> usize {
        let mut w = i / 64;
        let mut word = match self.0.get(w) {
            Some(word) => word | ((1u64 << (i % 64)) - 1),
            None => return i,
        };
        loop {
            if word != u64::MAX {
                return 64 * w + word.trailing_ones() as usize;
            }
            w += 1;
            word = match self.0.get(w) {
                Some(word) => *word,
                None => return 64 * w,
            };
        }
    }

    /// Bits `i .. i + 64`, bit `i` lowest.
    fn window(&self, i: usize) -> u64 {
        let word = |w: usize| self.0.get(w).copied().unwrap_or(0);
        let (w, shift) = (i / 64, i % 64);
        if shift == 0 {
            word(w)
        } else {
            word(w) >> shift | word(w + 1) << (64 - shift)
        }
    }
}

/// The number of each state, placing the states of `order` first fit; each
/// state needs the slots `labels[state]` (ascending) above its number. A
/// state that needs none takes the least number left once the others are
/// placed.
// [spec:hfst:sem:dhfst.acceptor-placement]
fn place(labels: &[Vec<u16>], order: &[usize]) -> Vec<u32> {
    let mut number = vec![u32::MAX; labels.len()];
    let mut occupied = Bits::default();
    let mut taken = Bits::default();
    let mut first_clear = 0usize;
    let mut fitted: HashMap<&[u16], usize> = HashMap::new();
    for &q in order {
        let Some(&low) = labels[q].first() else {
            continue;
        };
        // No slot below `first_clear` is empty, and slots and numbers only
        // get taken: a number that did not fit a set of labels before never
        // will. Sixty-four candidate numbers at a time: those no state has
        // and whose slots are all empty.
        let tried = fitted.entry(labels[q].as_slice()).or_insert(0);
        let mut window = first_clear.saturating_sub(low as usize).max(*tried);
        let base = loop {
            let mut fits = !taken.window(window);
            for &x in &labels[q] {
                fits &= !occupied.window(window + x as usize);
            }
            if fits != 0 {
                break window + fits.trailing_zeros() as usize;
            }
            window += 64;
        };
        for &x in &labels[q] {
            occupied.set(base + x as usize);
        }
        taken.set(base);
        *tried = base + 1;
        number[q] = base as u32;
        first_clear = occupied.next_clear(first_clear);
    }
    let mut next = 0usize;
    for &q in order {
        if labels[q].is_empty() {
            next = taken.next_clear(next);
            taken.set(next);
            number[q] = next as u32;
        }
    }
    number
}

/// The regular arcs of `state`, one run per symbol; the source reading
/// sorts them by symbol.
fn groups(state: &State) -> Vec<&[Arc]> {
    state
        .regular
        .chunk_by(|a, b| a.symbol == b.symbol)
        .collect()
}

/// The slots each state needs above its number: 0 for its free arcs, then
/// its regular labels, ascending.
fn labels_of(states: &[State], groups: &[Vec<&[Arc]>]) -> Vec<Vec<u16>> {
    states
        .iter()
        .zip(groups)
        .map(|(state, groups)| {
            let free = (!state.free.is_empty()).then_some(0u16);
            free.into_iter()
                .chain(groups.iter().map(|g| g[0].symbol))
                .collect()
        })
        .collect()
}

/// The numbering: each state's number, the states by number, `n_ids` and
/// the slot count.
struct Numbering {
    ids_of: Vec<u32>,
    by_id: Vec<Option<usize>>,
    n_ids: u32,
    n_slots: usize,
}

/// Number the states, the start state first, the rest in depth-first order.
// [spec:hfst:sem:dhfst.acceptor-placement]
fn number_states(labels: &[Vec<u16>], label_max: u16) -> crate::error::Result<Numbering> {
    let order: Vec<usize> = (0..labels.len()).collect();
    let ids_of = place(labels, &order);
    let n_ids = ids_of.iter().copied().max().map_or(1, |m| m + 1);
    let mut by_id: Vec<Option<usize>> = vec![None; n_ids as usize];
    for (q, &id) in ids_of.iter().enumerate() {
        if by_id[id as usize].replace(q).is_some() {
            return Err(verification(format!("two states were given number {id}")));
        }
    }
    let used_end = ids_of
        .iter()
        .zip(labels)
        .filter_map(|(id, l)| l.last().map(|x| *id as usize + *x as usize + 1))
        .max()
        .unwrap_or(0);
    let n_slots = used_end.max(n_ids as usize + label_max as usize);
    if n_slots >= u32::MAX as usize {
        return Err(unsupported("too many slots"));
    }
    Ok(Numbering {
        ids_of,
        by_id,
        n_ids,
        n_slots,
    })
}

/// The width of a record and of its two fields.
#[derive(Clone, Copy)]
struct Fields {
    record_bytes: usize,
    target_bits: u32,
    index_bits: u32,
}

impl Fields {
    /// A record of `first` and `second`.
    fn record(&self, first: u64, second: u64) -> u64 {
        first | second << self.target_bits
    }

    /// The second field that marks a slot whose arcs are listed.
    fn list_mark(&self) -> u64 {
        (1u64 << self.index_bits) - 1
    }
}

/// The records of `LIST`: every state's free arcs, and its arcs on each
/// symbol that has more than one plus a count before them.
fn list_len(states: &[State], groups: &[Vec<&[Arc]>]) -> u64 {
    states
        .iter()
        .zip(groups)
        .map(|(s, g)| {
            s.free.len() as u64
                + g.iter()
                    .filter(|g| g.len() > 1)
                    .map(|g| 1 + g.len() as u64)
                    .sum::<u64>()
        })
        .sum()
}

/// Field widths: targets and list positions in the first field, weight and
/// free pair indices, free arc counts and the list mark in the second.
// [spec:hfst:sem:dhfst.acceptor-layout]
fn field_widths(
    n_ids: u32,
    n_list: u64,
    n_weights: usize,
    n_pairs: usize,
    max_free: u64,
) -> crate::error::Result<Fields> {
    let target_bits = bits((n_ids as u64 - 1).max(n_list.saturating_sub(1))).max(1);
    let index_bits = bits(
        (n_weights as u64)
            .max(n_pairs.saturating_sub(1) as u64)
            .max(max_free),
    )
    .max(1);
    let record_bytes = (target_bits + index_bits).div_ceil(8) as usize;
    if target_bits > 32 || index_bits > 32 || record_bytes > 8 {
        return Err(unsupported(format!(
            "records of {target_bits} and {index_bits} bits do not fit"
        )));
    }
    Ok(Fields {
        record_bytes,
        target_bits,
        index_bits,
    })
}

/// The checks, slot records and list, in slot order.
struct Table {
    checks: Vec<u16>,
    slots: Vec<u64>,
    list: Vec<u64>,
}

/// What [`fill_table`] reads: the states, their regular arcs by symbol,
/// their numbers, the source address to state map, the dictionaries and the
/// field widths.
struct TableInput<'s> {
    states: &'s [State],
    groups: &'s [Vec<&'s [Arc]>],
    numbering: &'s Numbering,
    number: &'s HashMap<u32, u32>,
    weight_index: &'s HashMap<u32, u32>,
    pair_index: &'s HashMap<u64, u32>,
    fields: Fields,
}

/// The key of a free arc's symbol and weight pair.
fn pair_key(a: &Arc) -> u64 {
    (a.symbol as u64) << 32 | a.weight as u64
}

/// Fill the slot table, in state number order.
// [spec:hfst:sem:dhfst.acceptor-layout]
fn fill_table(input: &TableInput<'_>) -> crate::error::Result<Table> {
    let numbering = input.numbering;
    let f = input.fields;
    let target = |a: &Arc| -> crate::error::Result<u64> {
        input
            .number
            .get(&a.target)
            .map(|q| numbering.ids_of[*q as usize] as u64)
            .ok_or_else(|| unsupported(format!("arc to unvisited state {}", a.target)))
    };
    let mut table = Table {
        checks: vec![0u16; numbering.n_slots],
        slots: vec![0u64; numbering.n_slots],
        list: Vec::new(),
    };
    for &q in numbering.by_id.iter().flatten() {
        let base = numbering.ids_of[q] as usize;
        let state = &input.states[q];
        if !state.free.is_empty() {
            table.slots[base] = f.record(table.list.len() as u64, state.free.len() as u64);
            for a in &state.free {
                table
                    .list
                    .push(f.record(target(a)?, input.pair_index[&pair_key(a)] as u64));
            }
        }
        for g in &input.groups[q] {
            let slot = base + g[0].symbol as usize;
            table.checks[slot] = g[0].symbol;
            let arc = |a: &Arc| -> crate::error::Result<u64> {
                Ok(f.record(target(a)?, input.weight_index[&a.weight] as u64))
            };
            if let [a] = g {
                table.slots[slot] = arc(a)?;
            } else {
                table.slots[slot] = f.record(table.list.len() as u64, f.list_mark());
                table.list.push(g.len() as u64);
                for a in g.iter() {
                    table.list.push(arc(a)?);
                }
            }
        }
    }
    Ok(table)
}

/// Append each record of `records`, `bytes` bytes little-endian.
fn push_records(out: &mut Vec<u8>, records: &[u64], bytes: usize) {
    for r in records {
        out.extend_from_slice(&r.to_le_bytes()[..bytes]);
    }
}

fn pad8(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(8) {
        out.push(0);
    }
}

/// `CHCK`: `u32 n_slots; u32 n_ids; u16 label_max; u8 check_bytes; u8
/// 0[5]`, then each plane of check bytes and its padding.
fn check_section(table: &Table, n_ids: u32, label_max: u16, check_bytes: usize) -> Vec<u8> {
    let mut chck = Vec::new();
    chck.extend_from_slice(&(table.checks.len() as u32).to_le_bytes());
    chck.extend_from_slice(&n_ids.to_le_bytes());
    chck.extend_from_slice(&label_max.to_le_bytes());
    chck.extend_from_slice(&[check_bytes as u8, 0, 0, 0, 0, 0]);
    for plane in 0..check_bytes {
        chck.extend(table.checks.iter().map(|c| (c >> (8 * plane)) as u8));
        chck.extend_from_slice(&[0u8; PADDING]);
    }
    chck
}

/// `SLOT` (`u32 n; u8 record_bytes; u8 target_bits; u8 index_bits; u8 0`)
/// and `LIST` (`u32 n; u8 record_bytes; u8 0[3]`), each with its records
/// and padding.
fn record_sections(table: &Table, f: Fields) -> (Vec<u8>, Vec<u8>) {
    let mut slot = Vec::new();
    slot.extend_from_slice(&(table.slots.len() as u32).to_le_bytes());
    slot.extend_from_slice(&[
        f.record_bytes as u8,
        f.target_bits as u8,
        f.index_bits as u8,
        0,
    ]);
    push_records(&mut slot, &table.slots, f.record_bytes);
    slot.extend_from_slice(&[0u8; PADDING]);

    let mut list = Vec::new();
    list.extend_from_slice(&(table.list.len() as u32).to_le_bytes());
    list.extend_from_slice(&[f.record_bytes as u8, 0, 0, 0]);
    push_records(&mut list, &table.list, f.record_bytes);
    list.extend_from_slice(&[0u8; PADDING]);
    (slot, list)
}

/// `FINL`: one final weight's bits, or `None`, per state number.
fn final_section(finals: &[Option<u32>]) -> Vec<u8> {
    let mut finl = Vec::new();
    let final_weights: Vec<u32> = finals.iter().filter_map(|w| *w).collect();
    finl.extend_from_slice(&(final_weights.len() as u32).to_le_bytes());
    finl.extend_from_slice(&0u32.to_le_bytes());
    let mut before = 0u32;
    for chunk in finals.chunks(FINL_STATES) {
        let bits = chunk
            .iter()
            .enumerate()
            .filter(|(_, w)| w.is_some())
            .fold(0u64, |w, (i, _)| w | (1 << i));
        finl.extend_from_slice(&bits.to_le_bytes());
        finl.extend_from_slice(&before.to_le_bytes());
        before += bits.count_ones();
    }
    for w in &final_weights {
        finl.extend_from_slice(&w.to_le_bytes());
    }
    finl.extend_from_slice(&[0u8; PADDING]);
    finl
}

/// `FREE`: the free pairs, each a symbol in the upper and a weight's bits in
/// the lower 32 bits of its key.
fn free_section(pairs: &[u64]) -> Vec<u8> {
    let mut free = Vec::new();
    free.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    free.extend_from_slice(&0u32.to_le_bytes());
    for key in pairs {
        free.extend_from_slice(&((key >> 32) as u16).to_le_bytes());
        free.extend_from_slice(&0u16.to_le_bytes());
        free.extend_from_slice(&(*key as u32).to_le_bytes());
    }
    free.extend_from_slice(&[0u8; PADDING]);
    free
}

/// `WGHT` or `DIST`: `u32 n; u32 0; f32 values[n]` from the values' bits,
/// and the padding.
fn float_section(values: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0u8; PADDING]);
    out
}

/// The `meta` section: one line of JSON, its keys in alphabetical order, as
/// divvunspell's writer serialises it.
// [spec:hfst:def:dhfst.acceptor-meta]
// [spec:hfst:sem:dhfst.acceptor-meta]
fn meta_section(written: &WrittenAcceptor, source_name: &str) -> crate::error::Result<Vec<u8>> {
    let quote = |s: &str| {
        serde_json::to_string(s)
            .map_err(|e| crate::err!(Hfst, format!("cannot write the meta section: {e}")))
    };
    Ok(format!(
        "{{\"arcs\":{},\"distances\":{},\"free_arcs\":{},\"placement\":\"depth-first\",\"source\":{},\"states\":{},\"writer\":{}}}",
        written.arcs,
        written.distances,
        written.free_arcs,
        quote(source_name)?,
        written.states,
        quote(WRITER)?,
    )
    .into_bytes())
}

/// The whole file: the eight-byte prefix, the header flags, the section
/// table and the sections, each at a multiple of 8.
// [spec:hfst:sem:dhfst.acceptor-layout]
fn assemble(sections: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&DhfstType::Acceptor.prefix());
    out.extend_from_slice(&FLAG_TROPICAL.to_le_bytes());
    out.extend_from_slice(&(sections.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    debug_assert_eq!(out.len(), HEADER_LEN);
    let table = out.len();
    out.resize(table + SECTION_ENTRY_LEN * sections.len(), 0);
    for (i, (section_tag, body)) in sections.iter().enumerate() {
        pad8(&mut out);
        let at = table + SECTION_ENTRY_LEN * i;
        let offset = out.len() as u64;
        out[at..at + 4].copy_from_slice(section_tag);
        out[at + 8..at + 16].copy_from_slice(&offset.to_le_bytes());
        out[at + 16..at + 24].copy_from_slice(&(body.len() as u64).to_le_bytes());
        out.extend_from_slice(body);
    }
    out
}

/// Write the acceptor `source` as DHFST type 2, read the bytes back and
/// check them against the source.
// [spec:hfst:def:dhfst.acceptor-write]
// [spec:hfst:sem:dhfst.acceptor-write]
pub fn write(
    source: &AcceptorSource<'_>,
    options: &AcceptorOptions,
) -> crate::error::Result<WrittenAcceptor> {
    let names = source.symbol_names();
    let (states, number) = read_states(source)?;
    let groups: Vec<Vec<&[Arc]>> = states.iter().map(groups).collect();
    let labels = labels_of(&states, &groups);
    let label_max = labels
        .iter()
        .filter_map(|l| l.last())
        .copied()
        .max()
        .unwrap_or(0);
    let check_bytes = if label_max <= 0xFF { 1 } else { 2 };
    let numbering = number_states(&labels, label_max)?;

    // Weights and free pairs, most frequent first.
    let (weights, weight_index) = dictionary(
        states
            .iter()
            .flat_map(|s| s.regular.iter().map(|a| a.weight)),
    );
    let (pairs, pair_index) = dictionary(states.iter().flat_map(|s| s.free.iter().map(pair_key)));
    let max_free = states.iter().map(|s| s.free.len()).max().unwrap_or(0) as u64;
    let fields = field_widths(
        numbering.n_ids,
        list_len(&states, &groups),
        weights.len(),
        pairs.len(),
        max_free,
    )?;
    let table = fill_table(&TableInput {
        states: &states,
        groups: &groups,
        numbering: &numbering,
        number: &number,
        weight_index: &weight_index,
        pair_index: &pair_index,
        fields,
    })?;

    // Finality and distance by state number; a number no state has is not
    // final and is 0 from a final state.
    let mut finals = vec![None; numbering.n_ids as usize];
    let mut distances = vec![0u32; numbering.n_ids as usize];
    for (q, state) in states.iter().enumerate() {
        finals[numbering.ids_of[q] as usize] = state.final_weight;
        distances[numbering.ids_of[q] as usize] = state.distance;
    }
    let with_distances = distances.iter().any(|d| *d != 0);

    let mut written = WrittenAcceptor {
        bytes: Vec::new(),
        states: states.len() as u32,
        ids: numbering.n_ids,
        slots: numbering.n_slots as u32,
        used_slots: table
            .checks
            .iter()
            .zip(&table.slots)
            .filter(|(c, r)| **c != 0 || **r != 0)
            .count() as u32,
        list_records: table.list.len() as u32,
        arcs: states
            .iter()
            .map(|s| (s.free.len() + s.regular.len()) as u64)
            .sum(),
        free_arcs: states.iter().map(|s| s.free.len() as u64).sum(),
        weights: weights.len(),
        free_pairs: pairs.len(),
        distances: with_distances,
        checked: 0,
        free_symbol_quirks: 0,
        order: states.iter().map(|s| s.source).collect(),
        ids_of: numbering.ids_of.clone(),
    };
    drop(groups);
    drop(states);

    let (slot, list) = record_sections(&table, fields);
    let mut sections: Vec<([u8; 4], Vec<u8>)> = vec![
        (tag::SYMS, symbols_section(&names)),
        (
            tag::CHCK,
            check_section(&table, numbering.n_ids, label_max, check_bytes),
        ),
        (tag::SLOT, slot),
        (tag::LIST, list),
        (tag::FINL, final_section(&finals)),
        (tag::FREE, free_section(&pairs)),
        (tag::WGHT, float_section(&weights)),
    ];
    if with_distances {
        sections.push((tag::DIST, float_section(&distances)));
    }
    sections.push((tag::META, meta_section(&written, &options.source_name)?));
    written.bytes = assemble(&sections);

    let reader = AcceptorReader::parse(&written.bytes)
        .map_err(|e| verification(format!("the written bytes do not load: {e}")))?;
    let (checked, quirks) = verify(source, &reader, &written, options.threads)?;
    written.checked = checked;
    written.free_symbol_quirks = quirks;
    Ok(written)
}

/// Check that `reader` answers as `source` does for every state `written`
/// wrote: its finality and final weight, its distance to a final state, its
/// free arcs, and for every symbol of the alphabet and three past it whether
/// it has arcs on that symbol and which. Answers the number of `(state,
/// symbol)` answers compared and the number of free symbols on which the
/// source's regular lookup answered yes, which the search never asks and the
/// reader answers no.
// [spec:hfst:sem:dhfst.acceptor-self-check]
pub fn verify(
    source: &AcceptorSource<'_>,
    reader: &AcceptorReader<'_>,
    written: &WrittenAcceptor,
    threads: usize,
) -> crate::error::Result<(u64, u64)> {
    if reader.id_count() != written.ids || written.ids_of.len() != written.order.len() {
        return Err(verification(format!(
            "reader numbers states below {}, the writer below {}",
            reader.id_count(),
            written.ids
        )));
    }
    let n_symbols = source.symbol_count();
    if reader.symbol_names() != source.symbol_names().as_slice()
        || (0..n_symbols as u16).any(|s| reader.is_free(s) != source.is_free(s))
    {
        return Err(verification(
            "the reader's symbols differ from the source's",
        ));
    }
    let number: HashMap<u32, u32> = written
        .order
        .iter()
        .zip(&written.ids_of)
        .map(|(s, id)| (*s, *id))
        .collect();
    let check = Check {
        source,
        reader,
        number: &number,
        n_symbols: n_symbols as u32,
    };
    let threads = match threads {
        0 => std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
        n => n,
    };
    let results = in_chunks(written.order.len(), threads, |range| {
        let mut total = (0u64, 0u64);
        for q in range {
            let (checked, quirks) = check.state(written.order[q], written.ids_of[q])?;
            total.0 += checked;
            total.1 += quirks;
        }
        Ok(total)
    });
    let mut total = (0u64, 0u64);
    for r in results {
        let (checked, quirks) = r.map_err(verification)?;
        total.0 += checked;
        total.1 += quirks;
    }
    Ok(total)
}

/// One arc as an answer compares it: `(output, target, weight bits)`.
type Answer = (Option<u16>, Option<u32>, Option<u32>);

/// The self-check of one state against the source.
struct Check<'c, 's, 'r> {
    source: &'c AcceptorSource<'s>,
    reader: &'c AcceptorReader<'r>,
    number: &'c HashMap<u32, u32>,
    n_symbols: u32,
}

impl Check<'_, '_, '_> {
    /// The reader's number of the state at source address `target`.
    fn map(&self, target: Option<u32>) -> Option<u32> {
        target.and_then(|t| self.number.get(&t).copied())
    }

    /// Check the state at source address `s`, numbered `r`. Answers the
    /// `(state, symbol)` answers compared and the free-symbol quirks met.
    fn state(&self, s: u32, r: u32) -> Result<(u64, u64), String> {
        let (source, reader) = (self.source, self.reader);
        let bits = |w: Option<f32>| w.map(f32::to_bits);
        if source.is_final(s) != reader.is_final(r)
            || bits(source.final_weight(s).filter(|_| source.is_final(s)))
                != bits(reader.final_weight(r))
        {
            return Err(format!("finality of state {s} differs"));
        }
        let (want, got) = (source.distance(s), reader.distance(r));
        if want.to_bits() != got.to_bits() {
            return Err(format!(
                "distance of state {s} to a final state: source {want}, reader {got}"
            ));
        }
        if source.has_free_arcs(s) != reader.has_free_arcs(r) {
            return Err(format!("free arcs of state {s} differ"));
        }
        let want: Vec<(u16, Answer)> = source
            .free_arcs(s)
            .map(|(input, t)| (input, (t.output, self.map(t.target), bits(t.weight))))
            .collect();
        let mut got: Vec<(u16, Answer)> = Vec::new();
        reader.for_each_free_arc(r, |symbol, target, weight| {
            got.push((symbol, (Some(symbol), Some(target), Some(weight.to_bits()))))
        });
        if want != got {
            return Err(format!(
                "free arcs of state {s}: source {want:?}, reader {got:?}"
            ));
        }
        let mut quirks = 0u64;
        for symbol in 0..self.n_symbols + 3 {
            quirks += self.symbol(s, r, symbol as u16)?;
        }
        Ok((self.n_symbols as u64 + 3, quirks))
    }

    /// Check what the state at source address `s`, numbered `r`, answers on
    /// `symbol`. Answers 1 for a free-symbol quirk, else 0.
    fn symbol(&self, s: u32, r: u32, symbol: u16) -> Result<u64, String> {
        let (source, reader) = (self.source, self.reader);
        let source_has = source.has_transitions(s, symbol);
        let reader_has = reader.has_transitions(r, symbol);
        if source.is_free(symbol) {
            if reader_has {
                return Err(format!(
                    "reader answers yes for free symbol {symbol} at state {s}"
                ));
            }
            return Ok(u64::from(source_has));
        }
        if source_has != reader_has {
            return Err(format!(
                "state {s} on symbol {symbol}: source {source_has}, reader {reader_has}"
            ));
        }
        if !source_has {
            return Ok(0);
        }
        let want: Vec<Answer> = source
            .transitions(s, symbol)
            .map(|t| (t.output, self.map(t.target), t.weight.map(f32::to_bits)))
            .collect();
        let mut got: Vec<Answer> = Vec::new();
        reader.for_each_arc(r, symbol, |target, weight| {
            got.push((Some(symbol), Some(target), Some(weight.to_bits())))
        });
        if want != got || want.is_empty() {
            return Err(format!(
                "state {s} on symbol {symbol}: source {want:?}, reader {got:?}"
            ));
        }
        Ok(0)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::dhfst_distance::tests::olw;
    use crate::hfst_transducer::HfstTransducer;
    use crate::transducer::{Transducer, WeightedTables};

    /// A lexicon with what a real one has: two arcs on one symbol, an
    /// epsilon arc, flag diacritic arcs, weights that leave distances other
    /// than 0, and two final states.
    pub(crate) const LEXICON: &str = "0\t1\ta\ta\t0.5\n0\t2\ta\ta\t1\n0\t3\tb\tb\t0.25\n\
        0\t4\t@0@\t@0@\t0.75\n0\t5\t@P.CASE.NOM@\t@P.CASE.NOM@\t0\n1\t6\tc\tc\t1\n\
        2\t6\tc\tc\t2\n3\t6\td\td\t0\n4\t6\te\te\t0.5\n5\t6\tf\tf\t0\n\
        5\t1\t@R.CASE.NOM@\t@R.CASE.NOM@\t0\n6\t7\tg\tg\t0.125\n6\t2.5\n7\t1\n";

    /// A lexicon whose labels go past 255, its weights all 0.
    pub(crate) fn wide() -> String {
        let mut att: String = (0..300)
            .map(|i| format!("0\t1\ts{i:03}\ts{i:03}\t0\n"))
            .collect();
        att.push_str("0\t2\ts299\ts299\t0\n1\t2\t@0@\t@0@\t0\n1\t3\tz\tz\t0\n2\t0\n3\t0\n");
        att
    }

    /// Weighted optimized lookup from AT&T text, as a transducer.
    pub(crate) fn lexicon(att: &str) -> HfstTransducer<Transducer<WeightedTables>> {
        HfstTransducer::wrap(olw(att))
    }

    /// `t` written with `threads` checking threads.
    pub(crate) fn written(
        t: &HfstTransducer<Transducer<WeightedTables>>,
        threads: usize,
    ) -> WrittenAcceptor {
        let source = AcceptorSource::new(t).expect("the lexicon opens");
        write(
            &source,
            &AcceptorOptions {
                threads,
                source_name: "lexicon".into(),
            },
        )
        .expect("the lexicon writes and checks")
    }

    // [spec:hfst:def:dhfst.acceptor-write/test]
    // [spec:hfst:sem:dhfst.acceptor-write/test]
    // [spec:hfst:sem:dhfst.acceptor-self-check/test]
    // [spec:hfst:def:dhfst.acceptor-layout/test]
    // [spec:hfst:sem:dhfst.acceptor-layout/test]
    #[test]
    fn writes_reads_back_and_checks_a_lexicon() {
        let t = lexicon(LEXICON);
        let w = written(&t, 1);
        for threads in [2, 3, 8] {
            assert_eq!(w.bytes, written(&t, threads).bytes, "{threads} threads");
        }
        assert_eq!(&w.bytes[..8], b"DHFST\x02\x01\0");
        assert_eq!((w.states, w.arcs, w.free_arcs), (8, 12, 3));
        assert_eq!(w.ids_of[0], 0, "the start state is not 0");
        let reader = AcceptorReader::parse(&w.bytes).expect("the written file loads");
        let info = reader.info();
        assert_eq!(info.check_bytes, 1);
        assert_eq!((info.arcs, info.free_arcs), (w.arcs, w.free_arcs));
        assert_eq!(info.list_records, w.list_records);
        assert!(info.distances && w.distances);
        let n_symbols = reader.symbol_names().len() as u64;
        assert_eq!(w.checked, w.states as u64 * (n_symbols + 3));
        // The start state's arcs on `a` are listed, two of them.
        let a = reader
            .symbol_names()
            .iter()
            .position(|s| s == "a")
            .expect("a is a symbol") as u16;
        let mut on_a = 0;
        reader.for_each_arc(0, a, |_, _| on_a += 1);
        assert_eq!(on_a, 2);
        let mut free: Vec<u16> = Vec::new();
        reader.for_each_free_arc(0, |symbol, _, _| free.push(symbol));
        assert_eq!(free.len(), 2);
        assert!(free.iter().all(|s| reader.is_free(*s)));
    }

    // [spec:hfst:sem:dhfst.acceptor-layout/test]
    #[test]
    fn labels_past_one_byte_take_two_byte_checks() {
        let t = lexicon(&wide());
        let w = written(&t, 0);
        let info = AcceptorReader::parse(&w.bytes)
            .expect("the written file loads")
            .info();
        assert_eq!(info.check_bytes, 2);
        assert!(info.label_max > 255, "label_max {}", info.label_max);
        assert_eq!((w.arcs, w.free_arcs), (303, 1));
        assert!(!w.distances && !info.distances);
    }

    // [spec:hfst:sem:dhfst.acceptor-distance/test]
    // [spec:hfst:sem:dhfst.acceptor-write/test]
    #[test]
    fn distances_are_the_sources() {
        let t = lexicon(LEXICON);
        let source = AcceptorSource::new(&t).expect("the lexicon opens");
        let w = written(&t, 0);
        let reader = AcceptorReader::parse(&w.bytes).expect("the written file loads");
        let mut nonzero = 0;
        for (s, id) in w.order.iter().zip(&w.ids_of) {
            let want = source.distance(*s);
            assert_eq!(reader.distance(*id).to_bits(), want.to_bits(), "state {s}");
            nonzero += usize::from(want.to_bits() != 0);
        }
        assert!(nonzero > 0);

        // Weights that leave every state a way to finish for nothing.
        let flat = lexicon("0\t1\ta\ta\t0\n0\t1\tb\tb\t1\n1\t0\n");
        let w = written(&flat, 0);
        let reader = AcceptorReader::parse(&w.bytes).expect("the written file loads");
        assert!(!w.distances && !reader.info().distances);
        assert!((0..reader.id_count() + 2).all(|q| reader.distance(q).to_bits() == 0));
    }

    // [spec:hfst:sem:dhfst.acceptor-source/test]
    #[test]
    fn a_transducer_is_refused() {
        let t = lexicon("0\t1\ta\tb\t0\n1\t0\n");
        let source = AcceptorSource::new(&t).expect("the transducer opens");
        let refusal = match write(&source, &AcceptorOptions::default()) {
            Ok(_) => panic!("a transducer was written as an acceptor"),
            Err(e) => e.to_string(),
        };
        assert!(refusal.contains("is not an acceptor arc"), "{refusal}");
    }

    // [spec:hfst:sem:dhfst.acceptor-self-check/test]
    #[test]
    fn the_check_catches_a_wrong_numbering() {
        let t = lexicon(LEXICON);
        let source = AcceptorSource::new(&t).expect("the lexicon opens");
        let mut w = written(&t, 0);
        let reader_bytes = w.bytes.clone();
        let reader = AcceptorReader::parse(&reader_bytes).expect("the written file loads");
        w.ids_of.swap(1, 2);
        assert!(verify(&source, &reader, &w, 1).is_err());
    }

    // [spec:hfst:sem:dhfst.acceptor-placement/test]
    #[test]
    fn states_are_placed_first_fit() {
        // State 0 needs slots 1 and 2 above its number, state 1 slot 1, state
        // 2 its free slot and slot 1, and state 3 none.
        let labels = vec![vec![1, 2], vec![1], vec![0, 1], vec![]];
        assert_eq!(place(&labels, &[0, 1, 2, 3]), vec![0, 2, 4, 1]);
        let numbering = number_states(&labels, 2).expect("the states are numbered");
        assert_eq!(numbering.n_ids, 5);
        assert_eq!(numbering.n_slots, 7);
        assert_eq!(
            numbering.by_id,
            vec![Some(0), Some(3), Some(1), None, Some(2)]
        );
    }

    // [spec:hfst:def:dhfst.acceptor-meta/test]
    // [spec:hfst:sem:dhfst.acceptor-meta/test]
    #[test]
    fn meta_names_hfst_and_the_source() {
        let t = lexicon(LEXICON);
        let source = AcceptorSource::new(&t).expect("the lexicon opens");
        let w = write(
            &source,
            &AcceptorOptions {
                threads: 1,
                source_name: "lex\"icon\n.hfst".into(),
            },
        )
        .expect("the lexicon writes");
        let reader = AcceptorReader::parse(&w.bytes).expect("the written file loads");
        let expected = format!(
            "{{\"arcs\":12,\"distances\":true,\"free_arcs\":3,\"placement\":\"depth-first\",\"source\":\"lex\\\"icon\\n.hfst\",\"states\":8,\"writer\":\"Divvun HFST v{}\"}}",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(reader.meta(), Some(expected.as_str()));
    }
}
