//! Native foma backend — makes `ImplementationType::FOMA_TYPE` a real,
//! usable transducer implementation, backed by the standalone Rust port of
//! foma (the `foma` crate, a path dependency).
//!
//! The whole module is gated behind the `foma` Cargo feature (see the module
//! declaration in `lib.rs`); with the feature off, nothing here compiles and
//! the facade behaves exactly as before (FOMA_TYPE stays unavailable). The
//! upstream C++ `FomaTransducer.*` / `ConvertFomaTransducer.*` are excluded
//! from the source-impl scope, so these rules are authored greenfield against
//! the contract in `docs/spec/port/back-ends/foma/foma-backend.md`.

use crate::backend::{AlgebraBackend, Backend, FlagDiacriticOperation, LookupBackend};
use crate::backend_foma_sigma::{
    EPSILON_SYMBOL, IDENTITY_SYMBOL, UNKNOWN_SYMBOL, is_reserved_symbol, sigma_declare, sym,
    transducing_pairs,
};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPath, HfstTwoLevelPaths, ImplementationType,
    StringPair, StringPairSet, StringPairVector, StringVector, Symbol,
};
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_flag_diacritics::{FdOperation, FdState, FdTable};
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_transducer::FlagDiacriticOverlay;
use crate::hfst_tropical_transducer_transition_data::{SymbolType, WeightType};

use foma::options::FomaOptions;
use std::collections::BTreeMap;

mod algebra;

/// Snapshot of a basic transducer's recognized relation and alphabet:
/// (state count, final states, alphabet, arcs).
#[allow(dead_code)]
type FomaSnapshot = (
    usize,
    std::collections::BTreeSet<u32>,
    std::collections::BTreeSet<String>,
    std::collections::BTreeSet<(u32, String, String, u32)>,
);

/// The backend's transducer handle: foma's `Fsm` (the sentinel-terminated
/// line table plus its `Sigma` alphabet, with reserved numbers EPSILON=0,
/// UNKNOWN=1, IDENTITY=2) together with the foma option set its operations
/// run under.
// [spec:hfst:def:foma-backend.foma-transducer]
#[derive(Clone, Debug)]
pub struct FomaTransducer {
    pub net: foma::types::Fsm,
    /// The option set passed into every foma construction this transducer
    /// performs (C foma's former `g_*` globals — see `foma::options`).
    /// OpenFST-style object-carried knobs: owned per transducer, explicit at
    /// each call, no process-global state. Constructors start from foma's C
    /// defaults; results of operations inherit the receiving operand's
    /// options; tune fields directly to steer subsequent operations.
    pub opts: FomaOptions,
}

// [spec:hfst:def:foma-backend.backend-impl]
impl Backend for FomaTransducer {
    const TYPE: ImplementationType = ImplementationType::FOMA_TYPE;

    fn empty() -> Self {
        // fsm_empty_set returns Box<Fsm>; move the Fsm out of the box.
        Self::wrap(foma::structures::fsm_empty_set())
    }

    fn copy(&self) -> crate::error::Result<Self> {
        // fsm_copy borrows &mut Fsm (it refreshes the source's counts) and
        // returns a deep Box<Fsm> copy; clone into an owned mutable Fsm first.
        let mut src = self.net.clone();
        let copied = foma::structures::fsm_copy(&mut src);
        Ok(self.wrap_with(copied))
    }

    // [spec:hfst:def:foma-backend.to-basic-fn]
    // [spec:hfst:sem:foma-backend.to-basic-fn]
    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        let mut net = HfstBasicTransducer::new();
        let sigma = &self.net.sigma;

        // Walk the line table in order, stopping at the sentinel row.
        for line in self.net.states.rows().iter() {
            if line.state_no == -1 {
                break;
            }
            let s = line.state_no as u32;
            // Ensure the state exists even if it has no arcs.
            net.add_state(s);
            // foma is unweighted -> final weight 0.0. foma's start state is
            // always state 0, matching HFST, so no start remapping is needed.
            if line.final_state == 1 {
                net.set_final_weight(s, &(0.0 as WeightType));
            }
            // An arc row has a real input symbol and target.
            if line.r#in != -1 && line.target != -1 {
                let isym = sym(line.r#in as i32, sigma);
                let osym = sym(line.out as i32, sigma);
                let tr = HfstBasicTransition::new_symbols(
                    line.target as u32,
                    isym,
                    osym,
                    0.0 as WeightType,
                    net.coder_mut(),
                );
                net.add_transition(s, &tr, true);
            }
        }

        // Every non-reserved sigma symbol joins the alphabet. Reserved numbers
        // 0/1/2 are represented by their HFST special strings, never added as
        // ordinary alphabet members.
        for n in &self.net.sigma {
            if n.number > foma::types::IDENTITY {
                net.add_symbol_to_alphabet(&SymbolType::from(n.symbol.as_str()));
            }
        }

        net.name = self.net.name.to_string();
        Ok(net)
    }

    // [spec:hfst:def:foma-backend.from-basic-fn]
    // [spec:hfst:sem:foma-backend.from-basic-fn]
    fn from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        // foma's dynarray construction API interns each symbol string into a
        // sigma number: the special strings map to reserved numbers (0/1/2 via
        // the FOMA_RESERVED_SYMBOLS table), every other distinct symbol gets a
        // fresh number >= 3. HFST state 0 is the foma start state.
        let mut handle = foma::dynarray::fsm_construct_init(&net.name);
        foma::dynarray::fsm_construct_set_initial(&mut handle, 0);

        let coder = net.coder();
        for (s, transitions) in net.states_and_transitions().iter().enumerate() {
            let origin = s as i32;
            if net.is_final_state(s as u32) {
                foma::dynarray::fsm_construct_set_final(&mut handle, origin);
            }
            for tr in transitions.iter() {
                let isym = tr.get_input_symbol(coder);
                let osym = tr.get_output_symbol(coder);
                foma::dynarray::fsm_construct_add_arc(
                    &mut handle,
                    origin,
                    tr.get_target_state() as i32,
                    isym.as_str(),
                    osym.as_str(),
                );
            }
        }

        let mut fsm = foma::dynarray::fsm_construct_done(handle);
        // The construction API interns only what it is handed, so the sigma it
        // produces is the set of symbols seen on an arc. An HFST alphabet is
        // wider than that by design — `insert_to_alphabet`, `prune_alphabet`,
        // the flag encode/decode and substitution all carry symbols that no arc
        // mentions — and those are the ones foma's `?`/`@` must stop matching.
        // Declaring them here is what makes the round trip alphabet-preserving
        // for every operation routed through it, not just the alphabet edits.
        sigma_declare(&mut fsm, net.get_alphabet().iter().map(|s| s.as_str()));
        Ok(Self::wrap(fsm))
    }

    fn get_alphabet(&self) -> StringSet {
        // The sigma's non-reserved symbols (numbers > IDENTITY), plus the three
        // special strings unconditionally: foma tracks them as reserved numbers
        // instead of sigma entries, but an HFST alphabet always contains them
        // (HfstBasicTransducer seeds them; the SymbolTable backends carry them
        // at labels 0/1/2). Callers that build one arc per alphabet member —
        // hfst-affix-guessify's guess state, for one — otherwise construct a
        // strictly smaller relation here than on every other backend.
        let mut out = StringSet::new();
        for n in &self.net.sigma {
            if n.number > foma::types::IDENTITY {
                out.insert(SymbolType::from(n.symbol.as_str()));
            }
        }
        out.insert(SymbolType::from(EPSILON_SYMBOL));
        out.insert(SymbolType::from(UNKNOWN_SYMBOL));
        out.insert(SymbolType::from(IDENTITY_SYMBOL));
        out
    }

    fn is_cyclic(&self) -> crate::error::Result<bool> {
        // fsm_topsort sets is_loop_free (1 acyclic, 0 cyclic) on the net it
        // returns; run it on a copy so this query stays non-destructive.
        let sorted = foma::topsort::fsm_topsort(self.net.clone());
        Ok(sorted.is_loop_free == foma::types::Tern::No)
    }

    /// C `FomaTransducer::number_of_states` counted the state-number runs of the
    /// flat line table, and the compressed table stores exactly one block per
    /// run — so the block count is that number, read without materializing the
    /// rows. Not `Fsm::statecount`: that field is `maxstate + 1` as of whenever
    /// `fsm_count` last ran, neither the same quantity under gapped numbering
    /// nor guaranteed current.
    fn number_of_states(&self) -> u32 {
        self.net.states.blocks().len() as u32
    }

    /// C counted the rows with `in != -1` — every row but the arc-less-state
    /// markers, which the compressed table keeps out of its arc array. That is
    /// also the predicate `to_basic` filters arcs by, so this count cannot
    /// disagree with the interchange graph.
    fn number_of_arcs(&self) -> u32 {
        self.net.states.arc_count() as u32
    }

    /// foma's line table carries no weight field, so no net it holds can hold a
    /// weight. Stated rather than inherited, so the answer is a survey result.
    fn has_weights(&self) -> bool {
        false
    }

    // The three alphabet edits are in-place sigma work. The round-trip defaults
    // would also answer correctly now that `from_basic` carries the alphabet,
    // but at the cost of two whole-graph rebuilds for what is a `Vec<Sigma>`
    // edit plus (at most) one arc-relabelling pass.
    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        sigma_declare(&mut self.net, [symbol]);
        Ok(())
    }

    fn add_symbols_to_alphabet(&mut self, symbols: &StringSet) -> crate::error::Result<()> {
        sigma_declare(&mut self.net, symbols.iter().map(|s| s.as_str()));
        Ok(())
    }

    /// Removal is alphabet-only: it never touches the graph.
    ///
    /// A foma arc addresses its symbol by sigma NUMBER, so dropping a sigma
    /// entry that some arc still carries does not un-declare a symbol — it
    /// leaves the arc pointing into a hole, and the following `sigma_sort`
    /// renumbers a neighbouring symbol into that hole, silently relabelling the
    /// arc. There is no representation in foma for "this arc is labelled X but
    /// X is not in the alphabet", so the request is unsatisfiable, and the two
    /// satisfiable readings of it are both worse: deleting the arcs changes the
    /// language, and refusing breaks callers that strip a marker set wholesale.
    /// So a symbol still on an arc keeps its sigma entry, and every entry for it
    /// that no arc uses is dropped.
    ///
    /// In practice the distinction never bites: every caller (hfst_xerox_rules'
    /// marker cleanup, xre/xfst definition expansion) removes a symbol it has
    /// just substituted off the graph, which is also foma's own idiom for the
    /// operation — `sigma_remove` followed by `sigma_sort`, as in rewrite.rs.
    fn remove_from_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        if is_reserved_symbol(symbol) || foma::sigma::sigma_find(symbol, &self.net.sigma).is_none()
        {
            return Ok(());
        }
        let mut used = std::collections::BTreeSet::new();
        for line in self.net.states.rows().iter() {
            if line.state_no == -1 {
                break;
            }
            if line.r#in != -1 && line.target != -1 {
                used.insert(line.r#in as i32);
                used.insert(line.out as i32);
            }
        }
        let mut removed = false;
        self.net.sigma.retain(|s| {
            let drop =
                s.number > foma::types::IDENTITY && s.symbol == symbol && !used.contains(&s.number);
            removed |= drop;
            !drop
        });
        if removed {
            foma::sigma::sigma_sort(&mut self.net);
        }
        Ok(())
    }

    // [spec:hfst:def:foma-backend.stream-io]
    // [spec:hfst:sem:foma-backend.stream-io]
    // Write half of the stream-io node: serialize the native gzip-compressed
    // `.foma` binary image straight to the writer via foma's generic stream API
    // (no temp file). `hfst_format` is not consulted: the payload is always the
    // native foma image; the HfstOutputStream layer prepends any HFST framing.
    fn write(&self, os: &mut dyn std::io::Write, _hfst_format: bool) -> crate::error::Result<()> {
        // foma's fn is generic over `W: Write`; `&mut dyn Write` implements
        // Write, so this monomorphises without a temp file.
        foma::io::fsm_write_binary(&self.net, os)
            .map_err(|e| crate::err!(StreamCannotBeWritten, format!("foma write: {e}")))
    }

    // [spec:hfst:def:foma-backend.lookup-impl]
    // [spec:hfst:sem:foma-backend.lookup-impl]
    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        self.walk_relation(callback, cycles, None, false);
    }

    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        let fd = self.flag_diacritics();
        self.walk_relation(callback, cycles, Some(&fd), filter_fd);
    }
}

/// Safety cap on how many paths an unbounded random-path request yields.
const PATH_SAFETY_CAP: usize = 8192;
/// Safety cap on lookup results when the caller passes an unlimited (`< 0`)
/// limit, so a cyclic transducer cannot loop forever.
const LOOKUP_SAFETY_CAP: usize = 8192;

/// Tokenize a foma `apply` output string (produced with `print_space` on, so
/// each emitted symbol is followed by a single space) back into a symbol
/// vector. Documented approximation: a sigma symbol that itself contains a
/// space would be mis-split, but foma symbols are space-free in practice.
fn tokenize_symbols(s: &str) -> StringVector {
    s.split(' ')
        .filter(|t| !t.is_empty())
        .map(Symbol::from)
        .collect()
}

/// Translate an HFST result `limit` (`< 0` == unlimited) into a concrete take
/// bound, clamping the unlimited case to [`LOOKUP_SAFETY_CAP`].
fn lookup_cap(limit: isize) -> usize {
    if limit < 0 {
        LOOKUP_SAFETY_CAP
    } else {
        limit as usize
    }
}

/// One arc of the digested line table: the HFST symbol strings the two sides
/// resolve to, the raw sigma numbers (the flag-diacritic table's keys), and the
/// target state.
struct WalkArc {
    isym: SymbolType,
    osym: SymbolType,
    inum: i32,
    onum: i32,
    target: u32,
}

/// foma's sentinel-terminated line table digested into per-state adjacency.
/// The table is a flat row list, so a walk that rescanned it at every state
/// (as the C++ `FomaTransducer::extract_paths` did) is quadratic in its length.
struct Walk {
    arcs: Vec<Vec<WalkArc>>,
    finals: Vec<bool>,
}

/// Gathers complete paths off a `walk_relation` traversal, stopping once `cap`
/// of them are in hand.
struct CollectPaths<'a> {
    results: &'a mut HfstTwoLevelPaths,
    cap: usize,
}

impl ExtractStringsCb for CollectPaths<'_> {
    fn operator_call(
        &mut self,
        path: &mut HfstTwoLevelPath,
        is_final: bool,
    ) -> crate::hfst_extract_strings::RetVal {
        if is_final && self.results.len() < self.cap {
            self.results.insert(path.clone());
        }
        crate::hfst_extract_strings::RetVal::new(self.results.len() < self.cap, true)
    }
}

/// The recursive path-extraction worker: the same traversal the OpenFst and
/// optimized-lookup backends run, over foma's line table. `all_visitations` /
/// `path_visitations` are per-call copies; `spv` and `fd_state_stack` are
/// shared down the recursion.
#[allow(clippy::too_many_arguments)]
fn walk_paths(
    w: &Walk,
    state: u32,
    mut all_visitations: BTreeMap<u32, u16>,
    mut path_visitations: BTreeMap<u32, u16>,
    callback: &mut dyn ExtractStringsCb,
    cycles: i32,
    mut fd_state_stack: Option<&mut Vec<FdState<i32>>>,
    filter_fd: bool,
    spv: &mut StringPairVector,
) -> bool {
    if cycles >= 0 && (*path_visitations.entry(state).or_insert(0) as i32) > cycles {
        return true;
    }
    *all_visitations.entry(state).or_insert(0) += 1;
    *path_visitations.entry(state).or_insert(0) += 1;

    if !spv.is_empty() {
        let is_final = w.finals[state as usize];
        // foma is unweighted, so every path weighs 0.
        let mut path = HfstTwoLevelPath {
            first: 0.0 as WeightType,
            second: spv.clone(),
        };
        let ret = callback.operator_call(&mut path, is_final);
        if !ret.continueSearch || !ret.continuePath {
            *path_visitations.entry(state).or_insert(0) -= 1;
            return ret.continueSearch;
        }
    }

    // Visit the least-travelled targets first (stable insertion sort, ascending).
    let mut order: Vec<&WalkArc> = Vec::new();
    for arc in w.arcs[state as usize].iter() {
        let mut i = 0usize;
        while i < order.len() {
            let av_a = *all_visitations.get(&arc.target).unwrap_or(&0);
            let av_i = *all_visitations.get(&order[i].target).unwrap_or(&0);
            if av_a < av_i {
                break;
            }
            i += 1;
        }
        order.insert(i, arc);
    }

    let mut res = true;
    let mut idx = 0usize;
    while idx < order.len() && res {
        let arc = order[idx];
        let mut added_fd_state = false;

        if let Some(stack) = fd_state_stack.as_deref_mut()
            && stack
                .last()
                .expect("fd state stack always holds the current state")
                .get_table()
                .get_operation(arc.inum)
                .is_some()
        {
            let top = stack
                .last()
                .expect("fd state stack always holds the current state")
                .clone();
            stack.push(top);
            if stack
                .last_mut()
                .expect("fd state stack always holds the current state")
                .apply_operation_symbol(arc.inum)
            {
                added_fd_state = true;
            } else {
                stack.pop();
                idx += 1;
                continue; // don't follow the transition
            }
        }

        // Special symbols (epsilons, and flags unless filtered) are inserted as
        // themselves; a filtered flag occupies its column as the empty symbol.
        let mut istring = SymbolType::default();
        let mut ostring = SymbolType::default();

        let flag_filtered = |sigma_number: i32, stack: Option<&Vec<FdState<i32>>>| {
            filter_fd
                && stack
                    .expect("fd state stack is Some whenever filter_fd is set")
                    .last()
                    .expect("fd state stack always holds the current state")
                    .get_table()
                    .get_operation(sigma_number)
                    .is_some()
        };

        if !flag_filtered(arc.inum, fd_state_stack.as_deref()) {
            istring = arc.isym.clone();
        }
        if !flag_filtered(arc.onum, fd_state_stack.as_deref()) {
            ostring = arc.osym.clone();
        }

        spv.push((istring, ostring));

        res = walk_paths(
            w,
            arc.target,
            all_visitations.clone(),
            path_visitations.clone(),
            callback,
            cycles,
            fd_state_stack.as_deref_mut(),
            filter_fd,
            spv,
        );

        spv.pop();

        if added_fd_state {
            fd_state_stack
                .as_deref_mut()
                .expect("added_fd_state implies fd_state_stack is present")
                .pop();
        }
        idx += 1;
    }

    *path_visitations.entry(state).or_insert(0) -= 1;
    res
}

impl FomaTransducer {
    /// Wrap a foma construction result as a `FomaTransducer` running under
    /// foma's default (C) options.
    fn wrap(fsm: foma::types::Fsm) -> Self {
        FomaTransducer {
            net: fsm,
            opts: FomaOptions::default(),
        }
    }

    /// Wrap a foma construction result, inheriting this transducer's options.
    fn wrap_with(&self, fsm: foma::types::Fsm) -> Self {
        FomaTransducer {
            net: fsm,
            opts: self.opts.clone(),
        }
    }

    /// Apply `input` downward and collect the output words (space-tokenizable),
    /// bounded by `limit`. Shared by the one-level lookup entry points.
    fn apply_down_outputs(&self, input: &str, limit: isize) -> Vec<String> {
        let mut h = foma::apply::apply_init(&self.net);
        // Emit a space after each output symbol so `tokenize_symbols` can split
        // the concatenated result back into a symbol vector.
        foma::apply::apply_set_print_space(&mut h, 1);
        h.down(input).take(lookup_cap(limit)).collect()
    }

    /// Digest the line table into per-state adjacency, resolving each arc's
    /// sigma numbers to their HFST symbol strings exactly as `to_basic` does.
    fn digest(&self) -> Walk {
        let sigma = &self.net.sigma;
        let mut arcs: Vec<Vec<WalkArc>> = Vec::new();
        let mut finals: Vec<bool> = Vec::new();
        let grow = |arcs: &mut Vec<Vec<WalkArc>>, finals: &mut Vec<bool>, s: usize| {
            if arcs.len() <= s {
                arcs.resize_with(s + 1, Vec::new);
                finals.resize(s + 1, false);
            }
        };

        for line in self.net.states.rows().iter() {
            if line.state_no == -1 {
                break;
            }
            let s = line.state_no as usize;
            grow(&mut arcs, &mut finals, s);
            if line.final_state == 1 {
                finals[s] = true;
            }
            if line.r#in != -1 && line.target != -1 {
                grow(&mut arcs, &mut finals, line.target as usize);
                arcs[s].push(WalkArc {
                    isym: sym(line.r#in as i32, sigma),
                    osym: sym(line.out as i32, sigma),
                    inum: line.r#in as i32,
                    onum: line.out as i32,
                    target: line.target as u32,
                });
            }
        }

        Walk { arcs, finals }
    }

    /// The sigma's flag diacritics, keyed by sigma number.
    fn flag_diacritics(&self) -> FdTable<i32> {
        let mut table = FdTable::new();
        for n in &self.net.sigma {
            if FdOperation::is_diacritic(&n.symbol) {
                table.define_diacritic(n.number, &n.symbol);
            }
        }
        table
    }

    /// Traverse the recognized relation from the start state, feeding the
    /// callback the per-symbol aligned path after every transition (and the
    /// empty path when the start state is itself final).
    fn walk_relation(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        fd: Option<&FdTable<i32>>,
        filter_fd: bool,
    ) {
        let w = self.digest();
        // foma's start state is always state 0; an empty line table has none.
        if w.finals.is_empty() {
            return;
        }

        let mut fd_state_stack: Option<Vec<FdState<i32>>> = fd.map(|fd| vec![FdState::new(fd)]);
        let mut spv = StringPairVector::new();
        walk_paths(
            &w,
            0,
            BTreeMap::new(),
            BTreeMap::new(),
            callback,
            cycles,
            fd_state_stack.as_mut(),
            filter_fd,
            &mut spv,
        );

        if w.finals[0] {
            let mut epsilon_path = HfstTwoLevelPath {
                first: 0.0 as WeightType,
                second: StringPairVector::new(),
            };
            callback.operator_call(&mut epsilon_path, true);
        }
    }
}

// [spec:hfst:def:foma-backend.lookup-impl]
// [spec:hfst:sem:foma-backend.lookup-impl]
// Drives foma's `apply` runtime. Outputs carry weight 0.0 (foma is unweighted);
// infinite-ambiguity queries approximate via whole-net cyclicity.
impl LookupBackend for FomaTransducer {
    fn lookup_fd_str(&mut self, s: &str, limit: isize, _time_cutoff: f64) -> HfstOneLevelPaths {
        let mut out = HfstOneLevelPaths::new();
        for ow in self.apply_down_outputs(s, limit) {
            out.insert(HfstOneLevelPath {
                first: 0.0,
                second: tokenize_symbols(&ow),
            });
        }
        out
    }
    fn lookup_fd_strvec(
        &mut self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstOneLevelPaths {
        // Join the symbol vector into one string; foma re-tokenizes it against
        // the sigma during apply.
        let joined: String = s.iter().map(|x| x.as_str()).collect();
        self.lookup_fd_str(&joined, limit, time_cutoff)
    }
    fn lookup_fd_pairs_str(
        &mut self,
        s: &str,
        limit: isize,
        _time_cutoff: f64,
    ) -> HfstTwoLevelPaths {
        // Approximation: pair the whole input `s` with each output word as a
        // single symbol pair (foma apply yields outputs, not per-column
        // alignment); weight 0.0.
        let mut out = HfstTwoLevelPaths::new();
        let mut h = foma::apply::apply_init(&self.net);
        for ow in h.down(s).take(lookup_cap(limit)) {
            out.insert(HfstTwoLevelPath {
                first: 0.0,
                second: vec![(Symbol::from(s), Symbol::from(ow.as_str()))],
            });
        }
        out
    }
    fn is_lookup_infinitely_ambiguous_str(&mut self, s: &str) -> bool {
        let Ok(net) = self.to_basic() else {
            return false;
        };
        // The input is a plain string, so it has to be re-tokenized against the
        // alphabet before the walk can consume it symbol by symbol.
        let mut tok = crate::hfst_tokenizer::HfstTokenizer::new();
        for it in net.get_input_symbols().iter() {
            tok.add_multichar_symbol(it);
        }
        let path = tok.tokenize_one_level(s, false);
        net.is_lookup_infinitely_ambiguous_string_vector(&path, true)
    }
    fn is_lookup_infinitely_ambiguous_strvec(&mut self, s: &StringVector) -> bool {
        match self.to_basic() {
            // foma's apply always obeys flag diacritics (`apply_init` sets
            // `obey_flags`), so the walk is asked to obey them too.
            Ok(net) => net.is_lookup_infinitely_ambiguous_string_vector(s, true),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests;
