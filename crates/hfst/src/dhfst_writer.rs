//! Writing an error model as DHFST ([`crate::dhfst`]). Authored greenfield
//! against `docs/spec/port/back-ends/dhfst/dhfst.md`; the algorithm is
//! divvunspell's DHFST writer (`src/transducer/dhfst/writer.rs`) for plain
//! version 1 files, and for the same optimized-lookup input the bytes are the
//! bytes `dhfst-tools write` writes, but for the program `meta` names as the
//! writer.
//!
//! The source model is [`SourceModel`]. The writer finds, for every state, at most one default arc per kind and a
//! fallback state whose row it mostly repeats, then stores only what is left.
//! The automaton, its states and its weights are the ones it was given, bit
//! for bit. Nothing is written on trust: every `(state, pair)` of the
//! encoding is resolved and compared with the source before serialising, and
//! the bytes are then read back through [`DhfstReader`] and every `(state,
//! input)` compared with the source again. Any difference is an error.
//!
//! The search for defaults and fallbacks is heuristic (the largest
//! single-valued group per kind; fallback candidates from MinHash buckets
//! over the rows plus each state's most frequent targets; a greedy assignment
//! that rejects cycles and chains deeper than the bound). A better optimiser
//! could only write a smaller file; it could not change what the file means.

use std::collections::HashMap;

use crate::dhfst::{
    DefaultKind, DhfstReader, FLAG_DEFAULTS, FLAG_FALLBACK, FLAG_TROPICAL, HEADER_LEN, NONE,
    SECTION_ENTRY_LEN, is_regular_name, tag,
};
use crate::dhfst_header::DhfstType;
pub use crate::dhfst_source::{SourceArc, SourceModel, SourceState};

/// The writer the `meta` section names: hfst and its version, the number
/// `--version` prints, without the build date or commit so that the bytes
/// stay the same within a version.
// [spec:hfst:sem:dhfst.meta+1]
pub const WRITER: &str = concat!("Divvun HFST v", env!("CARGO_PKG_VERSION"));

pub(crate) fn verification(detail: impl std::fmt::Display) -> crate::error::Error {
    crate::err!(
        Hfst,
        format!("DHFST self-check failed, nothing was written: {detail}")
    )
}

/// How to write.
#[derive(Clone, Debug)]
pub struct WriteOptions {
    /// Longest fallback chain to allow; `None` for no bound.
    pub max_fallback_depth: Option<u32>,
    /// Worker threads for the search and the checks; `0` for all cores. The
    /// bytes written do not depend on it.
    pub threads: usize,
    /// A name for the source, recorded in the `meta` section.
    pub source_name: String,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            max_fallback_depth: Some(4),
            threads: 0,
            source_name: String::new(),
        }
    }
}

/// What the encoding came to.
#[derive(Clone, Debug, Default)]
pub struct WriteReport {
    /// states
    pub states: usize,
    /// arcs in the source
    pub source_arcs: u64,
    /// explicit arcs written
    pub explicit_arcs: u64,
    /// blockers written
    pub blockers: u64,
    /// default records written, per kind (identity, substitution, deletion,
    /// insertion)
    pub defaults: [u64; 4],
    /// distinct classes
    pub classes: usize,
    /// distinct class pairs
    pub class_pairs: usize,
    /// states with a fallback
    pub states_with_fallback: usize,
    /// states per fallback chain length
    pub depth_histogram: Vec<usize>,
    /// longest fallback chain written
    pub max_depth: u32,
    /// source arcs by what answers them: own explicit, own default (identity,
    /// substitution, deletion, insertion), inherited explicit, inherited
    /// default
    pub answered_by: [u64; 7],
    /// `(state, pair)` resolutions checked on the encoding
    pub pairs_checked: u64,
    /// `(state, input)` queries checked on the written bytes
    pub queries_checked: u64,
    /// arcs compared on the written bytes
    pub arcs_checked: u64,
}

/// A written file and what it holds.
pub struct Written {
    /// the file
    pub bytes: Vec<u8>,
    /// what the encoding came to
    pub report: WriteReport,
}

/// Encode `model`, check the encoding against it, serialise, read the bytes
/// back and check them against it again.
// [spec:hfst:def:dhfst.write+1]
// [spec:hfst:sem:dhfst.write+1]
pub fn write(model: &SourceModel, options: &WriteOptions) -> crate::error::Result<Written> {
    let threads = match options.threads {
        0 => std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
        n => n,
    };
    let max_depth = options
        .max_fallback_depth
        .map(|d| d as usize)
        .unwrap_or(usize::MAX);

    let m = Model::build(model);
    let groups: Vec<[Option<Group>; 4]> = (0..m.n_states).map(|q| groups_of(&m, q)).collect();
    let cands = candidates(&m, model, 16, 6, threads);
    let scored = score_candidates(&m, &groups, &cands, threads);
    let enc = assign(&m, &groups, &scored, max_depth);
    let (answered_by, pairs_checked) = verify_encoding(&m, &groups, &enc, threads)?;
    let (bytes, mut report) = serialise(model, &m, &groups, &enc, options);
    report.answered_by = answered_by;
    report.pairs_checked = pairs_checked;

    let reader = DhfstReader::parse(&bytes)
        .map_err(|e| verification(format!("the written bytes do not load: {e}")))?;
    let (queries, arcs) = verify_reader(model, &reader, threads)?;
    report.queries_checked = queries;
    report.arcs_checked = arcs;
    Ok(Written { bytes, report })
}

/// Check that `reader` answers every `(state, input)` of `model` with exactly
/// the model's arcs, each once, and the same final weights. Answers
/// `(queries, arcs)` compared.
// [spec:hfst:sem:dhfst.self-check]
pub fn verify_reader(
    model: &SourceModel,
    reader: &DhfstReader<'_>,
    threads: usize,
) -> crate::error::Result<(u64, u64)> {
    let n_states = model.states.len();
    if reader.state_count() as usize != n_states {
        return Err(verification(format!(
            "reader has {} states, source has {n_states}",
            reader.state_count()
        )));
    }
    if reader.symbol_names() != model.symbols.as_slice() {
        return Err(verification(
            "reader's symbol table differs from the source's",
        ));
    }
    let results = in_chunks(n_states, threads, |range| {
        let mut total = (0u64, 0u64);
        for q in range {
            let (queries, arcs) = verify_reader_state(model, reader, q)?;
            total.0 += queries;
            total.1 += arcs;
        }
        Ok(total)
    });
    let mut total = (0u64, 0u64);
    for r in results {
        let (q, a) = r.map_err(verification)?;
        total.0 += q;
        total.1 += a;
    }
    Ok(total)
}

/// [`verify_reader`] for one state.
fn verify_reader_state(
    model: &SourceModel,
    reader: &DhfstReader<'_>,
    q: usize,
) -> Result<(u64, u64), String> {
    let state = &model.states[q];
    let want_final = state.final_weight.map(f32::to_bits);
    if want_final != reader.final_weight(q as u32).map(f32::to_bits) {
        return Err(format!("state {q}: final weight differs"));
    }
    let mut got: Vec<(u16, u32, u32)> = Vec::new();
    let (mut queries, mut arcs) = (0u64, 0u64);
    let mut at = 0usize;
    for x in 0..model.symbols.len() as u16 {
        let start = at;
        while at < state.arcs.len() && state.arcs[at].input == x {
            at += 1;
        }
        let want = &state.arcs[start..at];
        got.clear();
        reader.for_each_arc(q as u32, x, |o, t, w| got.push((o, t, w.to_bits())));
        got.sort_unstable();
        queries += 1;
        arcs += got.len() as u64;
        let same = got.len() == want.len()
            && got
                .iter()
                .zip(want)
                .all(|(g, w)| *g == (w.output, w.target, w.weight.to_bits()));
        if !same {
            return Err(format!(
                "state {q} input {x}: reader answers {} arcs, source has {}",
                got.len(),
                want.len()
            ));
        }
    }
    Ok((queries, arcs))
}

/// Run `work` over `0..n` split into one contiguous range per thread, and
/// answer the results in range order.
pub(crate) fn in_chunks<T, F>(n: usize, threads: usize, work: F) -> Vec<Result<T, String>>
where
    T: Send,
    F: Fn(std::ops::Range<usize>) -> Result<T, String> + Sync,
{
    let threads = threads.max(1);
    let chunk = n.div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let work = &work;
        let handles: Vec<_> = (0..threads)
            .map(|t| scope.spawn(move || work(t * chunk..((t + 1) * chunk).min(n).max(t * chunk))))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("a checker thread panicked".to_string()))
            })
            .collect()
    })
}

/// Pair kinds, with "special" for pairs only explicit entries may answer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Default(DefaultKind),
    Special,
}

/// The value id of a pair with no arcs.
const EMPTY: u32 = 0;

/// Dense per-state rows over every `(input, output)` pair. Each cell is a
/// value id naming the sorted set of `(target, weight)` the pair leads to.
struct Model {
    n: usize,
    np: usize,
    dense: Vec<u32>,
    values: Vec<Vec<u64>>,
    pair_kind: Vec<Kind>,
    n_states: usize,
}

fn target_weight(target: u32, weight: f32) -> u64 {
    ((target as u64) << 32) | weight.to_bits() as u64
}

impl Model {
    fn build(model: &SourceModel) -> Model {
        let n = model.symbols.len();
        let np = n * n;
        let regular: Vec<bool> = model
            .symbols
            .iter()
            .enumerate()
            .map(|(s, name)| is_regular_name(s, name))
            .collect();
        let mut pair_kind = vec![Kind::Special; np];
        for i in 0..n {
            for o in 0..n {
                if let Some(k) = crate::dhfst::pair_kind(i as u16, o as u16, regular[i], regular[o])
                {
                    pair_kind[i * n + o] = Kind::Default(k);
                }
            }
        }

        let mut values: Vec<Vec<u64>> = vec![Vec::new()];
        let mut intern: HashMap<Vec<u64>, u32> = HashMap::new();
        let ns = model.states.len();
        let mut dense = vec![EMPTY; ns * np];
        for (q, state) in model.states.iter().enumerate() {
            // Arcs arrive sorted by (input, output, target, weight): each
            // pair's set is one run.
            let mut at = 0usize;
            while at < state.arcs.len() {
                let (i, o) = (state.arcs[at].input, state.arcs[at].output);
                let mut set: Vec<u64> = Vec::new();
                while at < state.arcs.len()
                    && (state.arcs[at].input, state.arcs[at].output) == (i, o)
                {
                    set.push(target_weight(state.arcs[at].target, state.arcs[at].weight));
                    at += 1;
                }
                set.sort_unstable();
                set.dedup();
                let id = match intern.get(&set) {
                    Some(id) => *id,
                    None => {
                        let id = values.len() as u32;
                        values.push(set.clone());
                        intern.insert(set, id);
                        id
                    }
                };
                dense[q * np + i as usize * n + o as usize] = id;
            }
        }
        Model {
            n,
            np,
            dense,
            values,
            pair_kind,
            n_states: ns,
        }
    }

    fn row(&self, q: usize) -> &[u32] {
        &self.dense[q * self.np..(q + 1) * self.np]
    }

    fn cost(&self, v: u32) -> u64 {
        if v == EMPTY {
            1
        } else {
            self.values[v as usize].len() as u64
        }
    }

    fn single(&self, v: u32) -> Option<u64> {
        if v == EMPTY {
            return None;
        }
        let s = &self.values[v as usize];
        if s.len() == 1 { Some(s[0]) } else { None }
    }
}

/// The largest single-valued group of one kind at one state. Its members are
/// the pairs of that kind whose only arc is `tw`; its span is the members for
/// identity, deletion and insertion, and `A × B` less the diagonal for
/// substitution.
#[derive(Clone)]
struct Group {
    tw: u64,
    a: Vec<u16>,
    b: Vec<u16>,
    a_mask: Vec<bool>,
    b_mask: Vec<bool>,
}

impl Group {
    fn in_span(&self, kind: DefaultKind, n: usize, p: usize) -> bool {
        let (i, o) = (p / n, p % n);
        match kind {
            DefaultKind::Identity => i == o && self.a_mask[i],
            DefaultKind::Substitution => i != o && self.a_mask[i] && self.b_mask[o],
            DefaultKind::Deletion => o == 0 && self.a_mask[i],
            DefaultKind::Insertion => i == 0 && self.b_mask[o],
        }
    }
}

// [spec:hfst:sem:dhfst.encoding]
fn groups_of(m: &Model, q: usize) -> [Option<Group>; 4] {
    let row = m.row(q);
    let mut by: [HashMap<u64, Vec<u32>>; 4] = Default::default();
    for (p, v) in row.iter().enumerate() {
        let Kind::Default(k) = m.pair_kind[p] else {
            continue;
        };
        if let Some(tw) = m.single(*v) {
            by[k as usize].entry(tw).or_default().push(p as u32);
        }
    }
    let mut out: [Option<Group>; 4] = [None, None, None, None];
    for k in DefaultKind::ALL {
        let ki = k as usize;
        let Some((tw, members)) = by[ki]
            .iter()
            .max_by_key(|(tw, v)| (v.len(), std::cmp::Reverse(**tw)))
        else {
            continue;
        };
        if members.len() < 2 {
            continue;
        }
        let mut a_mask = vec![false; m.n];
        let mut b_mask = vec![false; m.n];
        for p in members {
            a_mask[*p as usize / m.n] = true;
            b_mask[*p as usize % m.n] = true;
        }
        let a: Vec<u16> = (0..m.n as u16).filter(|s| a_mask[*s as usize]).collect();
        let b: Vec<u16> = (0..m.n as u16).filter(|s| b_mask[*s as usize]).collect();
        out[ki] = Some(Group {
            tw: *tw,
            a,
            b,
            a_mask,
            b_mask,
        });
    }
    out
}

/// Per-kind cost of state `q` given fallback `f`, without and with that
/// kind's default, plus the special pairs' cost.
fn kind_costs(
    m: &Model,
    q: usize,
    f: Option<usize>,
    groups: &[Option<Group>; 4],
) -> ([u64; 4], [u64; 4], u64) {
    let row = m.row(q);
    let frow = f.map(|f| m.row(f));
    let mut without = [0u64; 4];
    let mut with = [0u64; 4];
    let mut special = 0u64;
    for p in 0..m.np {
        let v = row[p];
        let differs = v != frow.map_or(EMPTY, |r| r[p]);
        let Kind::Default(k) = m.pair_kind[p] else {
            if differs {
                special += m.cost(v);
            }
            continue;
        };
        let ki = k as usize;
        if differs {
            without[ki] += m.cost(v);
        }
        // A pair whose only arc is the default's needs no entry.
        if let Some(g) = &groups[ki]
            && m.single(v) != Some(g.tw)
            && (g.in_span(k, m.n, p) || differs)
        {
            with[ki] += m.cost(v);
        }
    }
    for ki in 0..4 {
        if groups[ki].is_some() {
            with[ki] += 1;
        } else {
            with[ki] = u64::MAX;
        }
    }
    (without, with, special)
}

#[derive(Clone)]
enum EncEntry {
    /// explicit arcs for a pair (value id), or a blocker when `EMPTY`
    Pair(u32, u32),
    /// a default of this kind
    Default(DefaultKind),
}

struct Encoded {
    fallback: Vec<Option<usize>>,
    entries: Vec<Vec<EncEntry>>,
}

fn encode_state(
    m: &Model,
    q: usize,
    f: Option<usize>,
    groups: &[Option<Group>; 4],
    use_default: [bool; 4],
) -> Vec<EncEntry> {
    let row = m.row(q);
    let frow = f.map(|f| m.row(f));
    let mut out = Vec::new();
    for k in DefaultKind::ALL {
        if use_default[k as usize] {
            out.push(EncEntry::Default(k));
        }
    }
    for p in 0..m.np {
        let v = row[p];
        let differs = v != frow.map_or(EMPTY, |r| r[p]);
        let need = match m.pair_kind[p] {
            Kind::Special => differs,
            Kind::Default(k) => match (&groups[k as usize], use_default[k as usize]) {
                (Some(g), true) => m.single(v) != Some(g.tw) && (g.in_span(k, m.n, p) || differs),
                _ => differs,
            },
        };
        if need {
            out.push(EncEntry::Pair(p as u32, v));
        }
    }
    out
}

/// One state's MinHash signature over its `(pair, value)` cells, written into
/// `s`.
fn minhash(m: &Model, q: usize, seeds: &[u64], s: &mut [u64]) {
    for (p, v) in m.row(q).iter().enumerate() {
        if *v == EMPTY {
            continue;
        }
        let base = ((p as u64) << 32) | *v as u64;
        for (j, seed) in seeds.iter().enumerate() {
            let mut h = base ^ seed;
            h ^= h >> 33;
            h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
            h ^= h >> 33;
            h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
            h ^= h >> 33;
            if h < s[j] {
                s[j] = h;
            }
        }
    }
}

/// Fallback candidates: states sharing a MinHash bucket over `(pair, value)`,
/// plus the state's most frequent targets.
// [spec:hfst:sem:dhfst.encoding]
fn candidates(
    m: &Model,
    model: &SourceModel,
    k: usize,
    per_bucket: usize,
    threads: usize,
) -> Vec<Vec<usize>> {
    let ns = m.n_states;
    let seeds: Vec<u64> = (0..k as u64)
        .map(|i| 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(i + 1) ^ 0xD1B5_4A32_D192_ED03)
        .collect();
    let mut sig = vec![u64::MAX; ns * k];
    std::thread::scope(|scope| {
        let chunk = ns.div_ceil(threads).max(1);
        for (ci, part) in sig.chunks_mut(chunk * k).enumerate() {
            let seeds = &seeds;
            scope.spawn(move || {
                for (local, s) in part.chunks_mut(k).enumerate() {
                    minhash(m, ci * chunk + local, seeds, s);
                }
            });
        }
    });
    let mut cands: Vec<Vec<usize>> = vec![Vec::new(); ns];
    for j in 0..k {
        let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
        for q in 0..ns {
            buckets.entry(sig[q * k + j]).or_default().push(q);
        }
        for members in buckets.values().filter(|members| members.len() >= 2) {
            for (idx, q) in members.iter().enumerate() {
                for d in 1..=per_bucket {
                    let other = members[(idx + d) % members.len()];
                    if other != *q {
                        cands[*q].push(other);
                    }
                }
            }
        }
    }
    for (q, state) in model.states.iter().enumerate() {
        let mut freq: HashMap<u32, u32> = HashMap::new();
        for a in &state.arcs {
            *freq.entry(a.target).or_default() += 1;
        }
        let mut f: Vec<_> = freq.into_iter().collect();
        f.sort_by_key(|(t, c)| (std::cmp::Reverse(*c), *t));
        for (t, _) in f.into_iter().take(8) {
            if t as usize != q {
                cands[q].push(t as usize);
            }
        }
        cands[q].sort_unstable();
        cands[q].dedup();
    }
    cands
}

/// Every candidate's cost, cheapest first; `usize::MAX` stands for no
/// fallback.
fn score_candidates(
    m: &Model,
    groups: &[[Option<Group>; 4]],
    cands: &[Vec<usize>],
    threads: usize,
) -> Vec<Vec<(u64, usize)>> {
    let mut out: Vec<Vec<(u64, usize)>> = vec![Vec::new(); m.n_states];
    std::thread::scope(|scope| {
        let chunk = m.n_states.div_ceil(threads).max(1);
        for (ci, part) in out.chunks_mut(chunk).enumerate() {
            scope.spawn(move || {
                for (local, slot) in part.iter_mut().enumerate() {
                    let q = ci * chunk + local;
                    let score = |f: Option<usize>| -> u64 {
                        let (without, with, special) = kind_costs(m, q, f, &groups[q]);
                        special + (0..4).map(|ki| without[ki].min(with[ki])).sum::<u64>()
                    };
                    let mut scored: Vec<(u64, usize)> =
                        cands[q].iter().map(|f| (score(Some(*f)), *f)).collect();
                    scored.push((score(None), usize::MAX));
                    scored.sort_unstable();
                    *slot = scored;
                }
            });
        }
    });
    out
}

/// How many fallback steps lie below state `x`.
fn chain_depth(fallback: &[Option<usize>], mut x: usize) -> usize {
    let mut d = 0;
    while let Some(y) = fallback[x] {
        d += 1;
        x = y;
    }
    d
}

/// Whether letting `q` fall back to `f` would close a cycle.
fn closes_cycle(fallback: &[Option<usize>], q: usize, f: usize) -> bool {
    let mut x = f;
    loop {
        if x == q {
            return true;
        }
        match fallback[x] {
            Some(y) => x = y,
            None => return false,
        }
    }
}

/// Greedy fallback assignment: the states that save most choose first; a
/// choice that closes a cycle or makes a chain deeper than `max_depth` is
/// passed over.
// [spec:hfst:sem:dhfst.encoding]
fn assign(
    m: &Model,
    groups: &[[Option<Group>; 4]],
    scored: &[Vec<(u64, usize)>],
    max_depth: usize,
) -> Encoded {
    let mut order: Vec<usize> = (0..m.n_states).collect();
    let saving = |q: usize| -> u64 {
        let none = scored[q]
            .iter()
            .find(|(_, f)| *f == usize::MAX)
            .map(|x| x.0)
            .unwrap_or(0);
        none.saturating_sub(scored[q][0].0)
    };
    order.sort_by_key(|q| std::cmp::Reverse(saving(*q)));
    let mut fallback: Vec<Option<usize>> = vec![None; m.n_states];
    let mut height: Vec<usize> = vec![0; m.n_states];
    for &q in &order {
        let chosen = scored[q]
            .iter()
            .map(|(_, f)| *f)
            .take_while(|f| *f != usize::MAX)
            .find(|f| {
                !closes_cycle(&fallback, q, *f)
                    && (max_depth == usize::MAX
                        || chain_depth(&fallback, *f) + 1 + height[q] <= max_depth)
            });
        let Some(f) = chosen else {
            continue;
        };
        fallback[q] = Some(f);
        let mut h = height[q] + 1;
        let mut x = Some(f);
        while let Some(y) = x {
            if height[y] >= h {
                break;
            }
            height[y] = h;
            h += 1;
            x = fallback[y];
        }
    }
    let mut entries = Vec::with_capacity(m.n_states);
    for q in 0..m.n_states {
        let f = fallback[q];
        let (without, with, _) = kind_costs(m, q, f, &groups[q]);
        let use_default: [bool; 4] = std::array::from_fn(|ki| with[ki] < without[ki]);
        entries.push(encode_state(m, q, f, &groups[q], use_default));
    }
    Encoded { fallback, entries }
}

/// Each level of the encoding: its explicit pairs sorted, and which kinds it
/// defaults.
type Level = (Vec<(u32, u32)>, [bool; 4]);

/// What answers a pair: explicit entries holding a value id, or a default
/// of a kind; `None` when the chain ends unanswered.
#[derive(Clone, Copy)]
enum Answer {
    Explicit(u32),
    Default(u64, usize),
}

/// Resolve pair `p` at state `q` through the encoding, answering what
/// answered it and at what depth.
fn resolve(
    m: &Model,
    groups: &[[Option<Group>; 4]],
    enc: &Encoded,
    levels: &[Level],
    q: usize,
    p: usize,
) -> Result<(Option<Answer>, usize), String> {
    let mut level = Some(q);
    let mut depth = 0usize;
    while let Some(l) = level {
        if depth > m.n_states {
            return Err(format!("fallback cycle at state {q}"));
        }
        let (pairs, defaults) = &levels[l];
        if let Ok(at) = pairs.binary_search_by_key(&(p as u32), |e| e.0) {
            return Ok((Some(Answer::Explicit(pairs[at].1)), depth));
        }
        if let Kind::Default(k) = m.pair_kind[p]
            && defaults[k as usize]
            && let Some(g) = &groups[l][k as usize]
            && g.in_span(k, m.n, p)
        {
            return Ok((Some(Answer::Default(g.tw, k as usize)), depth));
        }
        level = enc.fallback[l];
        depth += 1;
    }
    Ok((None, depth))
}

/// Resolve every pair of state `q` through the encoding and compare it with
/// the source, adding the arcs to `breakdown` by what answered them.
fn verify_state_encoding(
    m: &Model,
    groups: &[[Option<Group>; 4]],
    enc: &Encoded,
    levels: &[Level],
    q: usize,
    breakdown: &mut [u64; 7],
) -> Result<u64, String> {
    for (p, &v) in m.row(q).iter().enumerate() {
        let (answer, depth) = resolve(m, groups, enc, levels, q, p)?;
        let ok = match answer {
            None => v == EMPTY,
            Some(Answer::Explicit(stored)) => stored == v,
            Some(Answer::Default(tw, _)) => m.single(v) == Some(tw),
        };
        if !ok {
            return Err(format!("state {q} pair {p}: encoding differs"));
        }
        if v == EMPTY {
            continue;
        }
        let slot = match (depth, answer) {
            (0, Some(Answer::Default(_, k))) => 1 + k,
            (0, _) => 0,
            (_, Some(Answer::Default(..))) => 6,
            (_, _) => 5,
        };
        breakdown[slot] += m.values[v as usize].len() as u64;
    }
    Ok(m.np as u64)
}

/// Resolve every `(state, pair)` through the encoding and compare it with the
/// source. Answers the breakdown of source arcs by what answered them, and the
/// number of resolutions checked.
// [spec:hfst:sem:dhfst.self-check]
fn verify_encoding(
    m: &Model,
    groups: &[[Option<Group>; 4]],
    enc: &Encoded,
    threads: usize,
) -> crate::error::Result<([u64; 7], u64)> {
    let levels: Vec<Level> = enc
        .entries
        .iter()
        .map(|es| {
            let mut pairs = Vec::new();
            let mut defaults = [false; 4];
            for e in es {
                match e {
                    EncEntry::Pair(p, v) => pairs.push((*p, *v)),
                    EncEntry::Default(k) => defaults[*k as usize] = true,
                }
            }
            pairs.sort_unstable();
            (pairs, defaults)
        })
        .collect();
    let results = in_chunks(m.n_states, threads, |range| {
        let mut breakdown = [0u64; 7];
        let mut checked = 0u64;
        for q in range {
            checked += verify_state_encoding(m, groups, enc, &levels, q, &mut breakdown)?;
        }
        Ok((breakdown, checked))
    });
    let mut total = [0u64; 7];
    let mut checked = 0u64;
    for r in results {
        let (b, c) = r.map_err(verification)?;
        for i in 0..7 {
            total[i] += b[i];
        }
        checked += c;
    }
    Ok((total, checked))
}

fn pad8(v: &mut Vec<u8>) {
    while !v.len().is_multiple_of(8) {
        v.push(0);
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Symbol classes and class pairs, numbered in the order the states first
/// use them.
#[derive(Default)]
struct Classes {
    classes: Vec<Vec<u16>>,
    class_ix: HashMap<Vec<u16>, u16>,
    pairs: Vec<(u16, u16)>,
    pair_ix: HashMap<(u16, u16), u16>,
}

impl Classes {
    fn class(&mut self, c: &[u16]) -> u16 {
        if let Some(i) = self.class_ix.get(c) {
            return *i;
        }
        let i = self.classes.len() as u16;
        self.classes.push(c.to_vec());
        self.class_ix.insert(c.to_vec(), i);
        i
    }

    /// The class, or class pair for substitution, a default of kind `k`
    /// covers.
    fn of_default(&mut self, k: DefaultKind, g: &Group) -> u16 {
        match k {
            DefaultKind::Identity | DefaultKind::Deletion => self.class(&g.a),
            DefaultKind::Insertion => self.class(&g.b),
            DefaultKind::Substitution => {
                let a = self.class(&g.a);
                let b = self.class(&g.b);
                *self.pair_ix.entry((a, b)).or_insert_with(|| {
                    self.pairs.push((a, b));
                    (self.pairs.len() - 1) as u16
                })
            }
        }
    }
}

/// The `STAT` records and `ENTR` entries of every state, in state order.
/// Answers `(states, entries, entry count)`.
// [spec:hfst:sem:dhfst.layout]
fn encode_runs(
    model: &SourceModel,
    m: &Model,
    groups: &[[Option<Group>; 4]],
    enc: &Encoded,
    classes: &mut Classes,
    report: &mut WriteReport,
) -> (Vec<u8>, Vec<u8>, u32) {
    let mut states: Vec<u8> = Vec::with_capacity(16 * m.n_states);
    let mut entries: Vec<u8> = Vec::new();
    let mut n_entries: u32 = 0;
    for (q, state_groups) in groups.iter().enumerate() {
        let mut explicit: Vec<(u16, u16, u32, f32)> = Vec::new();
        let mut defaults: Vec<(u16, u16, u32, f32)> = Vec::new();
        for e in &enc.entries[q] {
            match e {
                EncEntry::Pair(p, v) => {
                    let (i, o) = ((*p as usize / m.n) as u16, (*p as usize % m.n) as u16);
                    if *v == EMPTY {
                        explicit.push((i, o, NONE, 0.0));
                        report.blockers += 1;
                    } else {
                        for tw in &m.values[*v as usize] {
                            explicit.push((i, o, (tw >> 32) as u32, f32::from_bits(*tw as u32)));
                            report.explicit_arcs += 1;
                        }
                    }
                }
                EncEntry::Default(k) => {
                    let Some(g) = &state_groups[*k as usize] else {
                        continue;
                    };
                    let class = classes.of_default(*k, g);
                    report.defaults[*k as usize] += 1;
                    defaults.push((
                        k.record(),
                        class,
                        (g.tw >> 32) as u32,
                        f32::from_bits(g.tw as u32),
                    ));
                }
            }
        }
        explicit.sort_by_key(|e| (e.0, e.1, e.2, e.3.to_bits()));
        defaults.sort_by_key(|e| e.0);
        let first = n_entries;
        for (i, o, t, w) in explicit.iter().chain(defaults.iter()) {
            entries.extend_from_slice(&i.to_le_bytes());
            entries.extend_from_slice(&o.to_le_bytes());
            entries.extend_from_slice(&t.to_le_bytes());
            entries.extend_from_slice(&w.to_bits().to_le_bytes());
            n_entries += 1;
        }
        let fallback = enc.fallback[q].map(|f| f as u32).unwrap_or(NONE);
        let final_weight = model.states[q].final_weight.unwrap_or(f32::INFINITY);
        states.extend_from_slice(&first.to_le_bytes());
        states.extend_from_slice(&((explicit.len() + defaults.len()) as u32).to_le_bytes());
        states.extend_from_slice(&fallback.to_le_bytes());
        states.extend_from_slice(&final_weight.to_bits().to_le_bytes());
    }
    (states, entries, n_entries)
}

/// Chain depths, for the header's guarantee and the report. Answers the
/// longest chain.
fn chain_report(enc: &Encoded, report: &mut WriteReport) -> u32 {
    let mut depth_histogram = vec![0usize; 1];
    let mut max_depth = 0u32;
    for q in 0..enc.fallback.len() {
        if enc.fallback[q].is_some() {
            report.states_with_fallback += 1;
        }
        let d = chain_depth(&enc.fallback, q);
        if depth_histogram.len() <= d {
            depth_histogram.resize(d + 1, 0);
        }
        depth_histogram[d] += 1;
        max_depth = max_depth.max(d as u32);
    }
    report.depth_histogram = depth_histogram;
    report.max_depth = max_depth;
    max_depth
}

/// `SYMS`: `u32 n; u32 offsets[n + 1]; u8 names[]`.
pub(crate) fn symbols_section(symbols: &[String]) -> Vec<u8> {
    let mut syms: Vec<u8> = Vec::new();
    syms.extend_from_slice(&(symbols.len() as u32).to_le_bytes());
    let mut off = 0u32;
    let mut blob: Vec<u8> = Vec::new();
    for s in symbols {
        syms.extend_from_slice(&off.to_le_bytes());
        blob.extend_from_slice(s.as_bytes());
        off += s.len() as u32;
    }
    syms.extend_from_slice(&off.to_le_bytes());
    syms.extend_from_slice(&blob);
    syms
}

/// `CLAS` (`u32 words; u32 n; u64 bits[n * words]`) and `CPAI` (`u32 n; u32
/// 0; { u16; u16 }[n]`).
fn class_sections(classes: &Classes, words: usize) -> (Vec<u8>, Vec<u8>) {
    let mut clas: Vec<u8> = Vec::new();
    clas.extend_from_slice(&(words as u32).to_le_bytes());
    clas.extend_from_slice(&(classes.classes.len() as u32).to_le_bytes());
    for c in &classes.classes {
        let mut bits = vec![0u64; words];
        for s in c {
            bits[*s as usize / 64] |= 1u64 << (*s as usize % 64);
        }
        for b in bits {
            clas.extend_from_slice(&b.to_le_bytes());
        }
    }
    let mut cpai: Vec<u8> = Vec::new();
    cpai.extend_from_slice(&(classes.pairs.len() as u32).to_le_bytes());
    cpai.extend_from_slice(&0u32.to_le_bytes());
    for (a, b) in &classes.pairs {
        cpai.extend_from_slice(&a.to_le_bytes());
        cpai.extend_from_slice(&b.to_le_bytes());
    }
    (clas, cpai)
}

/// A section that is a `u32` count, a zero `u32`, and the records. For
/// `STAT` the zero is the start state: the writer stores the start first.
fn counted_section(count: u32, records: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(8 + records.len());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(records);
    out
}

/// The `meta` section: one line of JSON saying how the file was written.
// [spec:hfst:def:dhfst.meta+1]
// [spec:hfst:sem:dhfst.meta+1]
fn meta_section(model: &SourceModel, options: &WriteOptions, report: &WriteReport) -> Vec<u8> {
    let bound = match options.max_fallback_depth {
        Some(d) => d.to_string(),
        None => "null".to_string(),
    };
    format!(
        "{{\"writer\":\"{WRITER}\",\"source\":\"{}\",\"max-fallback-depth-bound\":{},\"source-states\":{},\"source-arcs\":{},\"source-duplicate-arcs\":{}}}",
        json_escape(&options.source_name),
        bound,
        report.states,
        report.source_arcs,
        model.duplicate_arcs,
    )
    .into_bytes()
}

/// The header, the section table and the sections, each section at a
/// multiple of 8 and zero-padded to one.
// [spec:hfst:sem:dhfst.header+2]
// [spec:hfst:sem:dhfst.layout]
fn assemble(sections: &[([u8; 4], Vec<u8>)], flags: u32, max_depth: u32) -> Vec<u8> {
    let table_len = sections.len() * SECTION_ENTRY_LEN;
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&DhfstType::ErrorModel.prefix());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&(sections.len() as u32).to_le_bytes());
    out.extend_from_slice(&max_depth.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(out.len(), HEADER_LEN);
    let mut offset = (HEADER_LEN + table_len).div_ceil(8) * 8;
    for (t, body) in sections {
        out.extend_from_slice(t);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(offset as u64).to_le_bytes());
        out.extend_from_slice(&(body.len() as u64).to_le_bytes());
        offset = (offset + body.len()).div_ceil(8) * 8;
    }
    pad8(&mut out);
    for (_, body) in sections {
        out.extend_from_slice(body);
        pad8(&mut out);
    }
    out
}

// [spec:hfst:sem:dhfst.layout]
fn serialise(
    model: &SourceModel,
    m: &Model,
    groups: &[[Option<Group>; 4]],
    enc: &Encoded,
    options: &WriteOptions,
) -> (Vec<u8>, WriteReport) {
    let mut report = WriteReport {
        states: m.n_states,
        source_arcs: model.arc_count(),
        ..WriteReport::default()
    };
    let mut classes = Classes::default();
    let (states, entries, n_entries) =
        encode_runs(model, m, groups, enc, &mut classes, &mut report);
    let max_depth = chain_report(enc, &mut report);
    report.classes = classes.classes.len();
    report.class_pairs = classes.pairs.len();

    let (clas, cpai) = class_sections(&classes, model.symbols.len().div_ceil(64));
    let mut sections: Vec<([u8; 4], Vec<u8>)> = vec![(tag::SYMS, symbols_section(&model.symbols))];
    if !classes.classes.is_empty() {
        sections.push((tag::CLAS, clas));
    }
    if !classes.pairs.is_empty() {
        sections.push((tag::CPAI, cpai));
    }
    sections.push((tag::STAT, counted_section(m.n_states as u32, &states)));
    sections.push((tag::ENTR, counted_section(n_entries, &entries)));
    sections.push((tag::META, meta_section(model, options, &report)));

    let mut flags = FLAG_TROPICAL;
    if report.states_with_fallback > 0 {
        flags |= FLAG_FALLBACK;
    }
    if report.defaults.iter().any(|d| *d > 0) {
        flags |= FLAG_DEFAULTS;
    }
    (assemble(&sections, flags, max_depth), report)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::dhfst::{FLAG_DEFAULTS, FLAG_FALLBACK};

    /// A small error model shaped like a real one: a start state with the full
    /// substitution, deletion and insertion fans, three edit states whose rows
    /// mostly agree (one lacks two substitutions, one weighs two differently),
    /// a pair with two arcs, an exact duplicate arc, a special pair, and a
    /// final state.
    pub(crate) fn edit_model() -> SourceModel {
        let mut symbols: Vec<String> = vec!["@_EPSILON_SYMBOL_@".into()];
        symbols.extend(["a", "b", "c", "d", "e", "f"].map(String::from));
        symbols.push("@_UNKNOWN_SYMBOL_@".into());
        let letters: Vec<u16> = (1..=6).collect();
        let pairs: Vec<(u16, u16)> = letters
            .iter()
            .flat_map(|&x| {
                letters
                    .iter()
                    .filter(move |&&y| y != x)
                    .map(move |&y| (x, y))
            })
            .collect();
        let arc = |input: u16, output: u16, target: u32, weight: f32| SourceArc {
            input,
            output,
            target,
            weight,
        };
        let state =
            |final_weight: Option<f32>, arcs: Vec<SourceArc>| SourceState { final_weight, arcs };

        let mut start = state(Some(0.0), vec![arc(7, 7, 0, 0.0)]);
        let mut end = state(Some(0.5), Vec::new());
        for &x in &letters {
            start
                .arcs
                .extend([arc(x, x, 0, 0.0), arc(x, 0, 2, 2.0), arc(0, x, 3, 3.0)]);
            end.arcs.push(arc(x, x, 4, 0.0));
        }
        start
            .arcs
            .extend(pairs.iter().map(|&(x, y)| arc(x, y, 1, 1.0)));
        let edit = |q: u32| {
            let mut edit = state(Some(0.0), Vec::new());
            for &(x, y) in &pairs {
                if q == 2 && [(1, 2), (3, 4)].contains(&(x, y)) {
                    continue;
                }
                let weight = if q == 3 && (x, y) == (2, 1) {
                    4.0
                } else {
                    1.0 + 0.5 * ((x * 7 + y * 3) % 4) as f32
                };
                edit.arcs.push(arc(x, y, 4, weight));
            }
            for &x in &letters {
                edit.arcs.push(arc(x, x, q, 0.0));
                if q != 2 {
                    edit.arcs.push(arc(x, 0, 4, 2.0 + (x % 2) as f32));
                }
                if q != 1 {
                    edit.arcs.push(arc(0, x, 4, 3.0));
                }
            }
            edit
        };
        let mut first = edit(1);
        first.arcs.extend([arc(1, 2, 5, 0.25), arc(1, 2, 5, 0.5)]);
        let branch = state(None, vec![arc(2, 2, 4, 0.0), arc(2, 2, 4, 0.0)]);
        SourceModel::new(symbols, vec![start, first, edit(2), edit(3), end, branch])
            .expect("the edit model is well formed")
    }

    fn written(model: &SourceModel, options: &WriteOptions) -> Written {
        write(model, options).expect("the edit model writes")
    }

    // [spec:hfst:sem:dhfst.write+1/test]
    #[test]
    fn bytes_do_not_depend_on_thread_count() {
        let model = edit_model();
        let one = written(
            &model,
            &WriteOptions {
                threads: 1,
                ..WriteOptions::default()
            },
        );
        for threads in [2, 3, 8] {
            let many = written(
                &model,
                &WriteOptions {
                    threads,
                    ..WriteOptions::default()
                },
            );
            assert_eq!(one.bytes, many.bytes, "{threads} threads");
        }
        assert_eq!(&one.bytes[..6], b"DHFST\x01");
    }

    // [spec:hfst:sem:dhfst.encoding/test]
    // [spec:hfst:sem:dhfst.self-check/test]
    #[test]
    fn encodes_defaults_fallbacks_and_explicit_arcs() {
        let model = edit_model();
        let w = written(&model, &WriteOptions::default());
        let r = &w.report;
        assert!(r.defaults.iter().all(|d| *d > 0), "{:?}", r.defaults);
        assert!(r.states_with_fallback > 0);
        assert!(r.explicit_arcs > 0);
        assert_eq!(r.arcs_checked, model.arc_count());
        assert_eq!(
            r.queries_checked,
            (model.states().len() * model.symbols().len()) as u64
        );
        assert_eq!(r.answered_by.iter().sum::<u64>(), model.arc_count());
        let reader = DhfstReader::parse(&w.bytes).expect("the written file parses");
        assert_eq!(
            reader.flags() & (FLAG_DEFAULTS | FLAG_FALLBACK),
            FLAG_DEFAULTS | FLAG_FALLBACK
        );
    }

    // [spec:hfst:def:dhfst.write+1/test]
    #[test]
    fn keeps_the_fallback_depth_bound() {
        let model = edit_model();
        let none = written(
            &model,
            &WriteOptions {
                max_fallback_depth: Some(0),
                ..WriteOptions::default()
            },
        );
        assert_eq!(none.report.states_with_fallback, 0);
        let one = written(
            &model,
            &WriteOptions {
                max_fallback_depth: Some(1),
                ..WriteOptions::default()
            },
        );
        assert!(one.report.max_depth <= 1);
        let reader = DhfstReader::parse(&one.bytes).expect("the written file parses");
        assert_eq!(reader.max_fallback_depth(), one.report.max_depth);
        let unbounded = written(
            &model,
            &WriteOptions {
                max_fallback_depth: None,
                ..WriteOptions::default()
            },
        );
        let reader = DhfstReader::parse(&unbounded.bytes).expect("the written file parses");
        assert!(
            reader
                .meta()
                .is_some_and(|m| m.contains("\"max-fallback-depth-bound\":null"))
        );
    }

    // [spec:hfst:def:dhfst.meta+1/test]
    // [spec:hfst:sem:dhfst.meta+1/test]
    #[test]
    fn meta_names_hfst_and_the_source() {
        let model = edit_model();
        let options = WriteOptions {
            source_name: "edit \"model\".hfst".into(),
            ..WriteOptions::default()
        };
        let w = written(&model, &options);
        let reader = DhfstReader::parse(&w.bytes).expect("the written file parses");
        let expected = format!(
            "{{\"writer\":\"Divvun HFST v{}\",\"source\":\"edit \\\"model\\\".hfst\",\"max-fallback-depth-bound\":4,\"source-states\":6,\"source-arcs\":{},\"source-duplicate-arcs\":1}}",
            env!("CARGO_PKG_VERSION"),
            model.arc_count()
        );
        assert_eq!(reader.meta(), Some(expected.as_str()));
    }
}
