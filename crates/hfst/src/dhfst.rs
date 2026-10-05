//! DHFST, the compact error-model format of divvunspell: its constants and a
//! reader. Authored greenfield against
//! `docs/spec/port/back-ends/dhfst/dhfst.md`. The consumer of record is
//! divvunspell's `src/transducer/dhfst/` reader, and this reader is a port of
//! the part of it a plain error model needs: the validation every load runs,
//! and the arc walk the suggestion search does.
//!
//! A state holds explicit entries, at most one default arc per kind (identity
//! `x:x`, substitution `x:y`, deletion `x:ε`, insertion `ε:y`, over regular
//! symbols) and optionally a fallback state. The arcs of a state for a pair
//! are its explicit entries for the pair if it has any (a blocker entry means
//! none), else its default arc of the pair's kind if the default's span holds
//! the pair, else whatever its fallback state answers. Finality is never
//! inherited.
//!
//! Every DHFST file starts with `DHFST`, a type byte (1 an error model, 2 an
//! acceptor), a version byte (1 for both) and a reserved zero byte. An error
//! model goes on to a 24-byte header (flags, section count, the longest
//! fallback chain), a table of 24-byte section records, and the sections at
//! 8-byte boundaries: `SYMS`, `CLAS`, `CPAI`, `STAT`, `ENTR` and the
//! ancillary `meta`. Every field is little-endian. The first eight bytes are
//! read by [`crate::dhfst_header`], and the writer is [`crate::dhfst_writer`].

use crate::dhfst_header::{DhfstType, read_type};

/// Bytes before the section table of an error model.
pub const HEADER_LEN: usize = 24;
/// Bytes per section table record.
pub const SECTION_ENTRY_LEN: usize = 24;
/// Bytes per state record.
pub const STATE_LEN: usize = 16;
/// Bytes per entry.
pub const ENTRY_LEN: usize = 12;
/// A fallback of no state, and the target of a blocker.
pub const NONE: u32 = u32::MAX;
/// Input field of the first default record kind; the kinds follow in order.
pub const DEFAULT_BASE: u16 = 0xFFF0;
/// Symbol numbers from here on are reserved for default records.
pub const MAX_SYMBOLS: u32 = DEFAULT_BASE as u32;

/// Header flag: weights are tropical `f32`.
pub const FLAG_TROPICAL: u32 = 1 << 0;
/// Header flag: some state has a fallback.
pub const FLAG_FALLBACK: u32 = 1 << 1;
/// Header flag: some state has a default record.
pub const FLAG_DEFAULTS: u32 = 1 << 2;
/// Header flag: a `RULE` section is present. No version 1 reader reads one.
pub const FLAG_RULES: u32 = 1 << 3;
/// The flags this reader knows. A file that sets any other, such as one that
/// carries search-time stages, is refused rather than read wrong.
const KNOWN_FLAGS: u32 = FLAG_TROPICAL | FLAG_FALLBACK | FLAG_DEFAULTS | FLAG_RULES;

/// Section tags. An upper-case tag is critical, a lower-case one ancillary.
// [spec:hfst:def:dhfst.layout]
pub mod tag {
    /// symbol table
    pub const SYMS: [u8; 4] = *b"SYMS";
    /// symbol classes
    pub const CLAS: [u8; 4] = *b"CLAS";
    /// class pairs
    pub const CPAI: [u8; 4] = *b"CPAI";
    /// states
    pub const STAT: [u8; 4] = *b"STAT";
    /// entries
    pub const ENTR: [u8; 4] = *b"ENTR";
    /// writer metadata
    pub const META: [u8; 4] = *b"meta";
}

/// The kind of a default record, and of a pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum DefaultKind {
    /// `x:x`
    Identity = 0,
    /// `x:y`, `x ≠ y`
    Substitution = 1,
    /// `x:ε`
    Deletion = 2,
    /// `ε:y`
    Insertion = 3,
}

impl DefaultKind {
    /// Every kind, in record order.
    pub const ALL: [DefaultKind; 4] = [
        DefaultKind::Identity,
        DefaultKind::Substitution,
        DefaultKind::Deletion,
        DefaultKind::Insertion,
    ];

    /// The kind a default record's input field names.
    pub fn from_record(input: u16) -> Option<DefaultKind> {
        match input.checked_sub(DEFAULT_BASE)? {
            0 => Some(DefaultKind::Identity),
            1 => Some(DefaultKind::Substitution),
            2 => Some(DefaultKind::Deletion),
            3 => Some(DefaultKind::Insertion),
            _ => None,
        }
    }

    /// The input field of a default record of this kind.
    pub fn record(self) -> u16 {
        DEFAULT_BASE + self as u16
    }
}

/// Whether a symbol is regular, one a default arc may cover: anything but
/// epsilon (symbol 0) and the `@…@` specials (flag diacritics, the identity
/// and unknown wildcards, and anything else HFST reserves that way).
// [spec:hfst:def:dhfst.regular-symbol]
pub fn is_regular_name(symbol: usize, name: &str) -> bool {
    symbol != 0 && !(name.len() > 1 && name.starts_with('@') && name.ends_with('@'))
}

/// The kind of the pair `input:output`, or `None` for a pair only explicit
/// entries may answer.
// [spec:hfst:def:dhfst.regular-symbol]
pub fn pair_kind(
    input: u16,
    output: u16,
    input_regular: bool,
    output_regular: bool,
) -> Option<DefaultKind> {
    match (input_regular, output_regular) {
        (true, true) if input == output => Some(DefaultKind::Identity),
        (true, true) => Some(DefaultKind::Substitution),
        (true, false) if output == 0 => Some(DefaultKind::Deletion),
        (false, true) if input == 0 => Some(DefaultKind::Insertion),
        _ => None,
    }
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(bytes)
}

pub(crate) fn corrupt(detail: impl std::fmt::Display) -> crate::error::Error {
    crate::err!(Hfst, format!("not a valid DHFST file: {detail}"))
}

/// One entry of a state's run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    /// input symbol, or a default record's kind
    pub input: u16,
    /// output symbol, or a default record's class or class pair
    pub output: u16,
    /// target state, [`NONE`] for a blocker
    pub target: u32,
    /// weight
    pub weight: f32,
}

/// Where the sections of a validated file are.
#[derive(Clone, Copy, Debug, Default)]
struct Layout {
    n_symbols: u32,
    words: usize,
    clas: usize,
    n_classes: u32,
    cpai: usize,
    n_pairs: u32,
    stat: usize,
    n_states: u32,
    start: u32,
    entr: usize,
    n_entries: u32,
    max_fallback_depth: u32,
    flags: u32,
}

/// A section's tag and byte range.
type Section = ([u8; 4], usize, usize);

/// A validated DHFST file, read in place.
///
/// Every section, run, class, target and fallback chain is checked by
/// [`DhfstReader::parse`], so nothing read afterwards falls outside the bytes.
pub struct DhfstReader<'a> {
    bytes: &'a [u8],
    layout: Layout,
    regular: Vec<u64>,
    names: Vec<String>,
    meta: Option<String>,
}

impl<'a> DhfstReader<'a> {
    /// Validate a whole file, exactly as divvunspell does before it reads one.
    // [spec:hfst:def:dhfst.reader]
    // [spec:hfst:sem:dhfst.reader]
    pub fn parse(bytes: &'a [u8]) -> crate::error::Result<DhfstReader<'a>> {
        let (flags, n_sections, max_fallback_depth) = parse_header(bytes)?;
        let sections = parse_sections(bytes, n_sections)?;
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

        let names = parse_symbols(bytes, need(tag::SYMS)?)?;
        let n_symbols = names.len() as u32;
        let words = names.len().div_ceil(64);
        let mut regular = vec![0u64; words];
        for (s, name) in names.iter().enumerate() {
            if is_regular_name(s, name) {
                regular[s / 64] |= 1u64 << (s % 64);
            }
        }
        let (clas, n_classes) = match find(tag::CLAS) {
            Some(range) => parse_classes(bytes, range, &regular)?,
            None => (0, 0),
        };
        let (cpai, n_pairs) = match find(tag::CPAI) {
            Some(range) => parse_class_pairs(bytes, range, n_classes)?,
            None => (0, 0),
        };
        let (entr, n_entries) = parse_counted(bytes, need(tag::ENTR)?, ENTRY_LEN, "ENTR")?;
        let (stat, n_states) = parse_counted(bytes, need(tag::STAT)?, STATE_LEN, "STAT")?;
        let start = u32_at(bytes, stat - 4);
        if n_states == 0 || n_states == NONE || start >= n_states {
            return Err(corrupt("STAT has no states, or no valid start state"));
        }

        let layout = Layout {
            n_symbols,
            words,
            clas,
            n_classes,
            cpai,
            n_pairs,
            stat,
            n_states,
            start,
            entr,
            n_entries,
            max_fallback_depth,
            flags,
        };
        let reader = DhfstReader {
            bytes,
            layout,
            regular,
            names,
            meta: find(tag::META)
                .and_then(|(start, end)| std::str::from_utf8(&bytes[start..end]).ok())
                .map(|s| s.trim_end_matches('\0').to_string()),
        };
        reader.check_states()?;
        reader.check_chains()?;
        Ok(reader)
    }

    /// Number of states.
    pub fn state_count(&self) -> u32 {
        self.layout.n_states
    }

    /// The longest fallback chain the header admits.
    pub fn max_fallback_depth(&self) -> u32 {
        self.layout.max_fallback_depth
    }

    /// The header flags.
    pub fn flags(&self) -> u32 {
        self.layout.flags
    }

    /// Symbol names as stored, `@_EPSILON_SYMBOL_@` first.
    pub fn symbol_names(&self) -> &[String] {
        &self.names
    }

    /// The writer's `meta` section, if the file carries one.
    pub fn meta(&self) -> Option<&str> {
        self.meta.as_deref()
    }

    /// The final weight of `state`, as callers number it.
    pub fn final_weight(&self, state: u32) -> Option<f32> {
        if state >= self.layout.n_states {
            return None;
        }
        let (_, _, _, weight) = self.state_record(self.swap_start(state));
        weight.is_finite().then_some(weight)
    }

    /// Hand `visit` every arc of `state` (as callers number it) on `input`,
    /// as `(output, target, weight)`, each answered pair once, level by
    /// level down the fallback chain.
    // [spec:hfst:sem:dhfst.reader]
    pub fn for_each_arc<V: FnMut(u16, u32, f32)>(&self, state: u32, input: u16, mut visit: V) {
        if state >= self.layout.n_states {
            return;
        }
        let words = self.layout.words;
        let mut walk = Walk {
            decided: vec![0u64; words],
            newly: vec![0u64; words],
            input,
            input_regular: self.is_regular(input),
        };
        let mut level = self.swap_start(state);
        loop {
            let (first, len, fallback, _) = self.state_record(level);
            let explicit = self.explicit_len(first, len);
            self.walk_explicit(&mut walk, first, first + explicit, &mut visit);
            for index in first + explicit..first + len {
                self.walk_default(&mut walk, self.entry(index), &mut visit);
            }
            if fallback == NONE {
                return;
            }
            for word in 0..words {
                walk.decided[word] |= walk.newly[word];
                walk.newly[word] = 0;
            }
            level = fallback;
        }
    }

    /// Whether `symbol` is regular.
    fn is_regular(&self, symbol: u16) -> bool {
        let s = symbol as usize;
        self.regular
            .get(s / 64)
            .is_some_and(|word| word & (1u64 << (s % 64)) != 0)
    }

    /// The stored state behind the state number callers see, and back: state
    /// 0 and the stored start state trade places, so a search that starts in
    /// state 0 starts where the file says.
    fn swap_start(&self, state: u32) -> u32 {
        let start = self.layout.start;
        if state == 0 {
            start
        } else if state == start {
            0
        } else {
            state
        }
    }

    /// `(first entry, entry count, fallback, final weight)` of a stored state.
    fn state_record(&self, state: u32) -> (u32, u32, u32, f32) {
        let at = self.layout.stat + STATE_LEN * state as usize;
        (
            u32_at(self.bytes, at),
            u32_at(self.bytes, at + 4),
            u32_at(self.bytes, at + 8),
            f32::from_bits(u32_at(self.bytes, at + 12)),
        )
    }

    /// One entry.
    pub fn entry(&self, index: u32) -> Entry {
        let at = self.layout.entr + ENTRY_LEN * index as usize;
        Entry {
            input: u16_at(self.bytes, at),
            output: u16_at(self.bytes, at + 2),
            target: u32_at(self.bytes, at + 4),
            weight: f32::from_bits(u32_at(self.bytes, at + 8)),
        }
    }

    /// How many of a run's entries are explicit: default records sort last
    /// and there are at most four of them.
    fn explicit_len(&self, first: u32, len: u32) -> u32 {
        let mut explicit = len;
        while explicit > 0 && self.entry(first + explicit - 1).input >= DEFAULT_BASE {
            explicit -= 1;
        }
        explicit
    }

    fn class_word(&self, class: u16, word: usize) -> u64 {
        u64_at(
            self.bytes,
            self.layout.clas + 8 * (class as usize * self.layout.words + word),
        )
    }

    fn class_has(&self, class: u16, symbol: u16) -> bool {
        let s = symbol as usize;
        self.class_word(class, s / 64) & (1u64 << (s % 64)) != 0
    }

    /// The input and output class of a class pair.
    fn class_pair(&self, pair: u16) -> (u16, u16) {
        let at = self.layout.cpai + 8 + 4 * pair as usize;
        (u16_at(self.bytes, at), u16_at(self.bytes, at + 2))
    }

    /// First entry in `[lo, hi)` whose input is not below `input`.
    fn lower_bound(&self, mut lo: u32, mut hi: u32, input: u16) -> u32 {
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.entry(mid).input < input {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// One level's explicit entries: they answer their pair unless an
    /// earlier level already did. Several arcs may share a pair, so the test
    /// is against earlier levels only.
    fn walk_explicit<V: FnMut(u16, u32, f32)>(
        &self,
        walk: &mut Walk,
        first: u32,
        end: u32,
        visit: &mut V,
    ) {
        let mut at = self.lower_bound(first, end, walk.input);
        while at < end {
            let entry = self.entry(at);
            if entry.input != walk.input {
                break;
            }
            let o = entry.output as usize;
            let bit = 1u64 << (o % 64);
            if walk.decided[o / 64] & bit == 0 {
                walk.newly[o / 64] |= bit;
                if entry.target != NONE {
                    visit(entry.output, self.swap_start(entry.target), entry.weight);
                }
            }
            at += 1;
        }
    }

    /// One default record: it answers every pair of its span that neither an
    /// earlier level nor this level's explicit entries answered.
    fn walk_default<V: FnMut(u16, u32, f32)>(&self, walk: &mut Walk, record: Entry, visit: &mut V) {
        let target = self.swap_start(record.target);
        let input = walk.input;
        let x = input as usize;
        let open = |walk: &Walk, s: usize| {
            (walk.decided[s / 64] | walk.newly[s / 64]) & (1u64 << (s % 64)) == 0
        };
        match DefaultKind::from_record(record.input) {
            Some(DefaultKind::Identity) => {
                if walk.input_regular && self.class_has(record.output, input) && open(walk, x) {
                    walk.newly[x / 64] |= 1u64 << (x % 64);
                    visit(input, target, record.weight);
                }
            }
            Some(DefaultKind::Substitution) => {
                let (from, to) = self.class_pair(record.output);
                if walk.input_regular && self.class_has(from, input) {
                    self.offer(walk, to, Some(x), target, record.weight, visit);
                }
            }
            Some(DefaultKind::Deletion) => {
                if walk.input_regular && self.class_has(record.output, input) && open(walk, 0) {
                    walk.newly[0] |= 1;
                    visit(0, target, record.weight);
                }
            }
            Some(DefaultKind::Insertion) if input == 0 => {
                self.offer(walk, record.output, None, target, record.weight, visit);
            }
            Some(DefaultKind::Insertion) | None => {}
        }
    }

    /// Answer every output of `class` still open, less `except`, with one
    /// arc each to `target` at `weight`.
    fn offer<V: FnMut(u16, u32, f32)>(
        &self,
        walk: &mut Walk,
        class: u16,
        except: Option<usize>,
        target: u32,
        weight: f32,
        visit: &mut V,
    ) {
        for word in 0..self.layout.words {
            let mut bits = self.class_word(class, word) & !(walk.decided[word] | walk.newly[word]);
            if let Some(x) = except
                && word == x / 64
            {
                bits &= !(1u64 << (x % 64));
            }
            walk.newly[word] |= bits;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                visit((word * 64 + bit) as u16, target, weight);
            }
        }
    }

    /// Check every state record and the run of entries it names.
    fn check_states(&self) -> crate::error::Result<()> {
        let mut any_fallback = false;
        let mut any_default = false;
        for q in 0..self.layout.n_states {
            let (first, len, fallback, final_weight) = self.state_record(q);
            if final_weight.is_nan() || final_weight == f32::NEG_INFINITY {
                return Err(corrupt(format!("state {q} has an invalid final weight")));
            }
            if first
                .checked_add(len)
                .is_none_or(|end| end > self.layout.n_entries)
            {
                return Err(corrupt(format!("run of state {q} is out of range")));
            }
            if fallback != NONE {
                if fallback >= self.layout.n_states || fallback == q {
                    return Err(corrupt(format!("fallback of state {q} is invalid")));
                }
                any_fallback = true;
            }
            any_default |= self.check_run(q, first, len)?;
        }
        if any_fallback && self.layout.flags & FLAG_FALLBACK == 0 {
            return Err(corrupt("fallback rows are used but not declared"));
        }
        if any_default && self.layout.flags & FLAG_DEFAULTS == 0 {
            return Err(corrupt("default records are used but not declared"));
        }
        Ok(())
    }

    /// Check the run of state `q`: explicit entries sorted, in range and not
    /// sharing a pair with a blocker, then default records sorted by kind,
    /// each naming a class or class pair that exists. Answers whether the run
    /// has a default record.
    fn check_run(&self, q: u32, first: u32, len: u32) -> crate::error::Result<bool> {
        let layout = &self.layout;
        let mut previous: Option<Entry> = None;
        let mut previous_kind: Option<DefaultKind> = None;
        for e in first..first + len {
            let entry = self.entry(e);
            if entry.weight.is_nan() {
                return Err(corrupt(format!("entry {e} has a NaN weight")));
            }
            if entry.input >= DEFAULT_BASE {
                let kind = DefaultKind::from_record(entry.input)
                    .ok_or_else(|| corrupt(format!("entry {e} has a reserved input symbol")))?;
                if previous_kind.is_some_and(|p| p >= kind) {
                    return Err(corrupt(format!(
                        "default records of state {q} are repeated or unsorted"
                    )));
                }
                previous_kind = Some(kind);
                let class_ok = match kind {
                    DefaultKind::Substitution => (entry.output as u32) < layout.n_pairs,
                    DefaultKind::Identity | DefaultKind::Deletion | DefaultKind::Insertion => {
                        (entry.output as u32) < layout.n_classes
                    }
                };
                if !class_ok || entry.target >= layout.n_states {
                    return Err(corrupt(format!(
                        "default record {e} names a missing class or state"
                    )));
                }
                continue;
            }
            if previous_kind.is_some() {
                return Err(corrupt(format!(
                    "explicit entry {e} of state {q} follows a default record"
                )));
            }
            if entry.input as u32 >= layout.n_symbols || entry.output as u32 >= layout.n_symbols {
                return Err(corrupt(format!("entry {e} names a missing symbol")));
            }
            if entry.target != NONE && entry.target >= layout.n_states {
                return Err(corrupt(format!("entry {e} targets a missing state")));
            }
            if let Some(p) = previous {
                check_order(q, &p, &entry)?;
            }
            previous = Some(entry);
        }
        Ok(previous_kind.is_some())
    }

    /// Fallback chains: acyclic and no longer than the header promises.
    fn check_chains(&self) -> crate::error::Result<()> {
        const UNKNOWN: u32 = u32::MAX;
        const IN_PROGRESS: u32 = u32::MAX - 1;
        let max = self.layout.max_fallback_depth;
        let too_long = |q: u32| {
            corrupt(format!(
                "fallback chain from state {q} is longer than {max}"
            ))
        };
        let mut depth: Vec<u32> = vec![UNKNOWN; self.layout.n_states as usize];
        let mut chain: Vec<u32> = Vec::new();
        for q in 0..self.layout.n_states {
            if depth[q as usize] != UNKNOWN {
                continue;
            }
            chain.clear();
            let mut s = q;
            // The depth of the last state pushed: 0 when its chain ends, one
            // more than the known depth of the state it falls back to.
            let mut next_depth = loop {
                match depth[s as usize] {
                    UNKNOWN => {}
                    IN_PROGRESS => {
                        return Err(corrupt(format!("fallback chain from state {q} is cyclic")));
                    }
                    known => break known + 1,
                }
                depth[s as usize] = IN_PROGRESS;
                chain.push(s);
                if chain.len() as u64 > max as u64 + 1 {
                    return Err(too_long(q));
                }
                let (_, _, fallback, _) = self.state_record(s);
                if fallback == NONE {
                    break 0;
                }
                s = fallback;
            };
            for state in chain.iter().rev() {
                if next_depth > max {
                    return Err(too_long(q));
                }
                depth[*state as usize] = next_depth;
                next_depth += 1;
            }
        }
        Ok(())
    }
}

/// The pairs a walk has answered so far: `decided` at earlier levels,
/// `newly` at the current one.
struct Walk {
    decided: Vec<u64>,
    newly: Vec<u64>,
    input: u16,
    input_regular: bool,
}

/// Two consecutive explicit entries of state `q` must be sorted by `(input,
/// output, target)`, and a blocker may not share its pair with an arc.
fn check_order(q: u32, previous: &Entry, entry: &Entry) -> crate::error::Result<()> {
    let key = (entry.input, entry.output, entry.target);
    if key < (previous.input, previous.output, previous.target) {
        return Err(corrupt(format!(
            "explicit entries of state {q} are not sorted"
        )));
    }
    if (entry.input, entry.output) == (previous.input, previous.output)
        && (entry.target == NONE || previous.target == NONE)
    {
        return Err(corrupt(format!(
            "a blocker of state {q} shares its pair with an arc"
        )));
    }
    Ok(())
}

/// The header: magic, type, version, reserved fields and flags. Answers
/// `(flags, section count, longest fallback chain)`.
// [spec:hfst:sem:dhfst.header+1]
fn parse_header(b: &[u8]) -> crate::error::Result<(u32, usize, u32)> {
    let kind = read_type(b)?;
    if kind != DhfstType::ErrorModel {
        return Err(crate::dhfst_header::wrong_type(kind, DhfstType::ErrorModel));
    }
    if b.len() < HEADER_LEN {
        return Err(corrupt("the header is truncated"));
    }
    if u32_at(b, 20) != 0 {
        return Err(corrupt("reserved header fields are not zero"));
    }
    let flags = u32_at(b, 8);
    if flags & !KNOWN_FLAGS != 0 {
        return Err(corrupt(format!("unknown header flags {flags:#x}")));
    }
    if flags & FLAG_TROPICAL == 0 {
        return Err(corrupt("weights are not declared tropical f32"));
    }
    if flags & FLAG_RULES != 0 {
        return Err(corrupt(
            "the file carries a RULE section, which no version 1 reader reads",
        ));
    }
    Ok((flags, u32_at(b, 12) as usize, u32_at(b, 16)))
}

/// The section table: every section at a multiple of 8 past the table and
/// inside the file, no tag twice, and no critical tag this reader does not
/// know.
fn parse_sections(b: &[u8], n_sections: usize) -> crate::error::Result<Vec<Section>> {
    let table_end = n_sections
        .checked_mul(SECTION_ENTRY_LEN)
        .and_then(|n| n.checked_add(HEADER_LEN))
        .filter(|end| *end <= b.len())
        .ok_or_else(|| corrupt("the section table runs past the end of the file"))?;
    let known = [
        tag::SYMS,
        tag::CLAS,
        tag::CPAI,
        tag::STAT,
        tag::ENTR,
        tag::META,
    ];
    let mut sections: Vec<Section> = Vec::with_capacity(n_sections);
    for s in 0..n_sections {
        let at = HEADER_LEN + SECTION_ENTRY_LEN * s;
        let tag = [b[at], b[at + 1], b[at + 2], b[at + 3]];
        let name = String::from_utf8_lossy(&tag).into_owned();
        let offset = u64_at(b, at + 8);
        let end = offset.checked_add(u64_at(b, at + 16));
        let (Ok(offset), Some(Ok(end))) = (usize::try_from(offset), end.map(usize::try_from))
        else {
            return Err(corrupt("a section extent overflows"));
        };
        if offset % 8 != 0 || offset < table_end || end > b.len() {
            return Err(corrupt(format!(
                "section {name} at {offset}..{end} is misplaced in a {} byte file",
                b.len()
            )));
        }
        if sections.iter().any(|(t, _, _)| *t == tag) {
            return Err(corrupt(format!("section {name} appears twice")));
        }
        if tag[0].is_ascii_uppercase() && !known.contains(&tag) {
            return Err(corrupt(format!(
                "critical section {name} is not known to this reader"
            )));
        }
        sections.push((tag, offset, end));
    }
    Ok(sections)
}

/// `SYMS`: `u32 n; u32 offsets[n + 1]; u8 names[]`, UTF-8, symbol 0 being
/// `@_EPSILON_SYMBOL_@`.
fn parse_symbols(b: &[u8], (syms, syms_end): (usize, usize)) -> crate::error::Result<Vec<String>> {
    if syms_end - syms < 4 {
        return Err(corrupt("SYMS is truncated"));
    }
    let n_symbols = u32_at(b, syms);
    if n_symbols == 0 || n_symbols > MAX_SYMBOLS {
        return Err(corrupt(format!("{n_symbols} symbols is out of range")));
    }
    let blob = syms + 4 + 4 * (n_symbols as usize + 1);
    if blob > syms_end {
        return Err(corrupt("SYMS offsets run past the section"));
    }
    let mut names = Vec::with_capacity(n_symbols as usize);
    let mut previous = 0usize;
    for s in 0..n_symbols as usize {
        let start = u32_at(b, syms + 4 + 4 * s) as usize;
        let end = u32_at(b, syms + 8 + 4 * s) as usize;
        if start != previous || end < start || blob + end > syms_end {
            return Err(corrupt(format!("SYMS offsets of symbol {s} are invalid")));
        }
        previous = end;
        let name = std::str::from_utf8(&b[blob + start..blob + end])
            .map_err(|_| corrupt(format!("symbol {s} is not UTF-8")))?;
        if name.contains('\0') {
            return Err(corrupt(format!("symbol {s} contains NUL")));
        }
        names.push(name.to_string());
    }
    if names[0] != "@_EPSILON_SYMBOL_@" {
        return Err(corrupt("symbol 0 is not @_EPSILON_SYMBOL_@"));
    }
    Ok(names)
}

/// `CLAS`: `u32 words_per_class; u32 n; u64 bits[n * words_per_class]`, every
/// member regular. Answers where the bits start, and `n`.
fn parse_classes(
    b: &[u8],
    (start, end): (usize, usize),
    regular: &[u64],
) -> crate::error::Result<(usize, u32)> {
    let words = regular.len();
    if end - start < 8 || u32_at(b, start) as usize != words {
        return Err(corrupt("CLAS words per class do not match SYMS"));
    }
    let n = u32_at(b, start + 4);
    (n as usize)
        .checked_mul(words * 8)
        .and_then(|len| len.checked_add(start + 8))
        .filter(|e| *e <= end)
        .ok_or_else(|| corrupt("CLAS runs past the section"))?;
    for c in 0..n as usize {
        for (w, regular_word) in regular.iter().enumerate() {
            if u64_at(b, start + 8 + 8 * (c * words + w)) & !regular_word != 0 {
                return Err(corrupt(format!(
                    "class {c} holds a symbol that is not regular"
                )));
            }
        }
    }
    if n > u16::MAX as u32 + 1 {
        return Err(corrupt("too many classes"));
    }
    Ok((start + 8, n))
}

/// `CPAI`: `u32 n; u32 0; { u16 input_class; u16 output_class }[n]`, every
/// class one that exists. Answers where the section starts, and `n`.
fn parse_class_pairs(
    b: &[u8],
    (start, end): (usize, usize),
    n_classes: u32,
) -> crate::error::Result<(usize, u32)> {
    if end - start < 8 {
        return Err(corrupt("CPAI is truncated"));
    }
    let n = u32_at(b, start);
    if (n as usize) * 4 + 8 > end - start {
        return Err(corrupt("CPAI runs past the section"));
    }
    for p in 0..n as usize {
        let at = start + 8 + 4 * p;
        if u16_at(b, at) as u32 >= n_classes || u16_at(b, at + 2) as u32 >= n_classes {
            return Err(corrupt(format!("class pair {p} names a missing class")));
        }
    }
    if n > u16::MAX as u32 + 1 {
        return Err(corrupt("too many class pairs"));
    }
    Ok((start, n))
}

/// `STAT` or `ENTR`: a `u32` count, a second `u32`, then `count` records of
/// `record_len` bytes inside the section. Answers where the records start,
/// and the count.
fn parse_counted(
    b: &[u8],
    (start, end): (usize, usize),
    record_len: usize,
    name: &str,
) -> crate::error::Result<(usize, u32)> {
    if end - start < 8 {
        return Err(corrupt(format!("{name} is truncated")));
    }
    let n = u32_at(b, start);
    if (n as usize)
        .checked_mul(record_len)
        .and_then(|len| len.checked_add(start + 8))
        .is_none_or(|e| e > end)
    {
        return Err(corrupt(format!("{name} runs past the section")));
    }
    Ok((start + 8, n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dhfst_header::{PREFIX_LEN, has_magic};
    use crate::dhfst_writer::tests::edit_model;
    use crate::dhfst_writer::{WriteOptions, verify_reader, write};

    fn written() -> Vec<u8> {
        write(&edit_model(), &WriteOptions::default())
            .expect("the edit model writes")
            .bytes
    }

    /// Where the section tagged `wanted` starts.
    fn section(bytes: &[u8], wanted: [u8; 4]) -> usize {
        (0..u32_at(bytes, 12) as usize)
            .map(|s| HEADER_LEN + SECTION_ENTRY_LEN * s)
            .find(|at| bytes[*at..*at + 4] == wanted)
            .map(|at| u64_at(bytes, at + 8) as usize)
            .expect("the section is in the table")
    }

    /// Parsing `bytes` fails, and says `reason`.
    fn assert_refused(bytes: &[u8], reason: &str) {
        match DhfstReader::parse(bytes) {
            Ok(_) => panic!("a file that should fail with {reason:?} parsed"),
            Err(e) => assert!(
                e.to_string().contains(reason),
                "{e} does not say {reason:?}"
            ),
        }
    }

    /// `bytes` with the `u32` at `at` set to `value`.
    fn patched(bytes: &[u8], at: usize, value: u32) -> Vec<u8> {
        let mut out = bytes.to_vec();
        out[at..at + 4].copy_from_slice(&value.to_le_bytes());
        out
    }

    // [spec:hfst:def:dhfst.reader/test]
    // [spec:hfst:sem:dhfst.reader/test]
    #[test]
    fn answers_every_arc_of_the_source() {
        let model = edit_model();
        let bytes = written();
        let reader = DhfstReader::parse(&bytes).expect("the written file parses");
        assert_eq!(reader.state_count(), 6);
        assert_eq!(reader.symbol_names(), model.symbols());
        let (_, arcs) = verify_reader(&model, &reader, 1).expect("the reader answers the source");
        assert_eq!(arcs, model.arc_count());
        // Insertions answer only an epsilon input, one arc per output.
        let mut inserted: Vec<(u16, u32, f32)> = Vec::new();
        reader.for_each_arc(0, 0, |o, t, w| inserted.push((o, t, w)));
        inserted.sort_by_key(|a| a.0);
        assert_eq!(inserted, (1..=6).map(|o| (o, 3, 3.0)).collect::<Vec<_>>());
        assert_eq!(reader.final_weight(4), Some(0.5));
        assert_eq!(reader.final_weight(5), None);
    }

    // [spec:hfst:def:dhfst.regular-symbol/test]
    #[test]
    fn classifies_symbols_and_pairs() {
        let regular: Vec<bool> = [
            "@_EPSILON_SYMBOL_@",
            "a",
            "@P.X.Y@",
            "@_UNKNOWN_SYMBOL_@",
            "@",
        ]
        .iter()
        .enumerate()
        .map(|(s, name)| is_regular_name(s, name))
        .collect();
        assert_eq!(regular, [false, true, false, false, true]);
        assert_eq!(pair_kind(1, 1, true, true), Some(DefaultKind::Identity));
        assert_eq!(pair_kind(1, 4, true, true), Some(DefaultKind::Substitution));
        assert_eq!(pair_kind(1, 0, true, false), Some(DefaultKind::Deletion));
        assert_eq!(pair_kind(0, 4, false, true), Some(DefaultKind::Insertion));
        assert_eq!(pair_kind(1, 3, true, false), None);
        assert_eq!(pair_kind(0, 0, false, false), None);
    }

    /// `bytes` with byte `at` set to `value`.
    fn with_byte(bytes: &[u8], at: usize, value: u8) -> Vec<u8> {
        let mut out = bytes.to_vec();
        out[at] = value;
        out
    }

    // [spec:hfst:def:dhfst.header+1/test]
    // [spec:hfst:sem:dhfst.header+1/test]
    #[test]
    fn refuses_headers_it_does_not_read() {
        let bytes = written();
        assert_eq!(bytes[..PREFIX_LEN], DhfstType::ErrorModel.prefix());
        assert_eq!(&DhfstType::ErrorModel.prefix(), b"DHFST\x01\x01\0");
        assert_eq!(&DhfstType::Acceptor.prefix(), b"DHFST\x02\x01\0");
        let flags = u32_at(&bytes, 8);
        for (file, reason) in [
            (b"HFST\0\0\0\0".to_vec(), "does not start with"),
            (
                with_byte(&bytes, 5, 2),
                "this DHFST file is an acceptor (type 2); an error model is type 1",
            ),
            (
                with_byte(&bytes, 5, 9),
                "DHFST type 9 is not a type this reader knows",
            ),
            (with_byte(&bytes, 5, 0), "DHFST type 0 is not a type"),
            (
                with_byte(&bytes, 6, 2),
                "DHFST error model version 2; this reader reads version 1",
            ),
            (
                with_byte(&bytes, 6, 0),
                "DHFST error model version 0; this reader reads version 1",
            ),
            (
                with_byte(&bytes, 7, 1),
                "header byte 7 is 1; it is reserved",
            ),
            (bytes[..7].to_vec(), "truncated before its type and version"),
            (bytes[..12].to_vec(), "truncated"),
            (with_byte(&bytes, 20, 1), "reserved"),
            (patched(&bytes, 8, flags | 1 << 4), "unknown header flags"),
            (patched(&bytes, 8, flags | FLAG_RULES), "RULE"),
            (patched(&bytes, 8, flags & !FLAG_TROPICAL), "tropical"),
        ] {
            assert_refused(&file, reason);
        }
        assert!(has_magic(&bytes) && !has_magic(b"HFST\0"));
        assert_eq!(
            read_type(&with_byte(&bytes, 5, 2)).expect("type 2 is known"),
            DhfstType::Acceptor
        );
    }

    // [spec:hfst:def:dhfst.layout/test]
    // [spec:hfst:sem:dhfst.reader/test]
    #[test]
    fn refuses_damaged_sections() {
        let bytes = written();
        let stat = section(&bytes, tag::STAT) + 8;
        let fallback_of = |q: usize| stat + STATE_LEN * q + 8;
        let entr = section(&bytes, tag::ENTR) + 8;
        let mut renamed = bytes.clone();
        let meta_record = (0..u32_at(&bytes, 12) as usize)
            .map(|s| HEADER_LEN + SECTION_ENTRY_LEN * s)
            .find(|at| bytes[*at..*at + 4] == tag::META)
            .expect("the file has a meta section");
        renamed[meta_record..meta_record + 4].copy_from_slice(b"MXTA");
        let cycle = patched(&patched(&bytes, fallback_of(1), 2), fallback_of(2), 1);
        for (file, reason) in [
            (
                patched(&bytes, fallback_of(1), 1),
                "fallback of state 1 is invalid",
            ),
            (cycle, "cyclic"),
            (patched(&bytes, 16, 0), "longer than 0"),
            (patched(&bytes, entr + 4, 99), "missing"),
            (renamed, "critical section MXTA is not known"),
            (bytes[..bytes.len() - 64].to_vec(), "misplaced"),
        ] {
            assert_refused(&file, reason);
        }
    }
}
