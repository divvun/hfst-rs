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
use crate::backend_foma_sigma::{is_reserved_symbol, sigma_declare, sym};
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
        // The sigma's non-reserved symbols (numbers > IDENTITY).
        let mut out = StringSet::new();
        for n in &self.net.sigma {
            if n.number > foma::types::IDENTITY {
                out.insert(SymbolType::from(n.symbol.as_str()));
            }
        }
        out
    }

    fn is_cyclic(&self) -> bool {
        // fsm_topsort sets is_loop_free (1 acyclic, 0 cyclic) on the net it
        // returns; run it on a copy so this query stays non-destructive.
        let sorted = foma::topsort::fsm_topsort(self.net.clone());
        sorted.is_loop_free == foma::types::Tern::No
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

// [spec:hfst:def:foma-backend.algebra-impl]
// [spec:hfst:sem:foma-backend.algebra-impl]
// Every op maps to its foma construction, all unweighted: weight arguments and
// the weight-transform ops are no-ops (foma is a boolean/unweighted algebra).
// Inputs are cloned into owned `Box<Fsm>` (foma's ops consume their arguments).
impl AlgebraBackend for FomaTransducer {
    const SUPPORTS_FLAG_OVERLAY: bool = true;
    const SUPPORTS_VIRTUAL_FLAG_INTERSECTION: bool = true;

    fn remove_epsilons(&self) -> Self {
        self.wrap_with(foma::determinize::fsm_epsilon_remove(self.net.clone()))
    }
    fn determinize(self, _encode_weights: bool) -> Self {
        self.wrap_with(foma::determinize::fsm_determinize(self.net.clone()))
    }
    fn minimize(self, _encode_weights: bool) -> Self {
        self.wrap_with(foma::minimize::fsm_minimize(&self.opts, self.net.clone()))
    }
    fn repeat_star(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_kleene_star(
            &self.opts,
            self.net.clone(),
        ))
    }
    fn repeat_plus(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_kleene_plus(
            &self.opts,
            self.net.clone(),
        ))
    }
    fn repeat_n(&self, n: u32) -> Self {
        self.wrap_with(foma::constructions::fsm_concat_n(
            &self.opts,
            self.net.clone(),
            n as i32,
        ))
    }
    fn repeat_le_n(&self, n: u32) -> Self {
        self.wrap_with(foma::constructions::fsm_concat_m_n(
            &self.opts,
            self.net.clone(),
            0,
            n as i32,
        ))
    }
    fn optionalize(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_optionality(
            &self.opts,
            self.net.clone(),
        ))
    }
    fn invert(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_invert(self.net.clone()))
    }
    fn reverse(&self) -> Self {
        self.wrap_with(foma::reverse::fsm_reverse(self.net.clone()))
    }
    fn extract_input_language(&self) -> Self {
        self.wrap_with(foma::extract::fsm_upper(self.net.clone()))
    }
    fn extract_output_language(&self) -> Self {
        self.wrap_with(foma::extract::fsm_lower(self.net.clone()))
    }

    fn concatenate(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_concat(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        ))
    }
    fn disjunct(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_union(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        ))
    }
    fn intersect(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_intersect(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        ))
    }
    fn subtract(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_minus(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        ))
    }
    fn compose(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_compose(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        ))
    }

    // [spec:hfst:req:foma-transducer.hfst.implementations.foma-transducer.resource-controlled-compose]
    // [spec:hfst:req:virtual-flag-algebra.backend-core]
    fn try_flag_operation_owned(
        self,
        another: Self,
        operation: FlagDiacriticOperation,
        flag_overlay: Option<&FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<Self> {
        let overlay = flag_overlay
            .map(|overlay| {
                foma::constructions::FlagOverlay::new(
                    overlay.left_self_loops.iter().cloned().collect(),
                    overlay.right_self_loops.iter().cloned().collect(),
                    overlay.enforce_left_before_right,
                )
            })
            .transpose()
            .map_err(|error| crate::err!(Hfst, format!("Foma flag overlay: {error}")))?
            .unwrap_or_default();
        let overlay = if operation == FlagDiacriticOperation::ComposeFlagsAsEpsilon {
            overlay.with_flags_as_epsilon()
        } else {
            overlay
        };

        if operation == FlagDiacriticOperation::Intersect {
            let FomaTransducer { net, opts } = self;
            let result = foma::constructions::fsm_intersect_with_flag_overlay(
                &opts,
                net,
                another.net,
                &overlay,
            )
            .map_err(|error| crate::err!(Hfst, format!("Foma intersection: {error}")))?;
            return Ok(FomaTransducer { net: result, opts });
        }
        if operation == FlagDiacriticOperation::Subtract {
            if flag_overlay.is_some() {
                crate::bail!(
                    Hfst,
                    "this backend does not support virtual flag subtraction"
                );
            }
            return Ok(self.subtract(&another));
        }

        let resources = match memory_limit_bytes {
            Some(allowance_bytes) => {
                let scratch_parent = std::env::current_dir().map_err(|error| {
                    crate::err!(
                        Hfst,
                        format!("resolve Foma compose scratch directory: {error}")
                    )
                })?;
                foma::constructions::ComposeResourceConfig::bounded(allowance_bytes, scratch_parent)
            }
            None => foma::constructions::ComposeResourceConfig::unbounded(),
        };

        let FomaTransducer { net, opts } = self;
        let result = foma::constructions::fsm_compose_with_config(
            &opts,
            net,
            another.net,
            &overlay,
            &resources,
        )
        .map_err(|error| crate::err!(Hfst, format!("Foma composition: {error}")))?;
        Ok(FomaTransducer { net: result, opts })
    }

    fn define_transducer_spv(spv: &StringPairVector) -> Self {
        // Concatenate each (in, out) pair's cross-product; the empty product is
        // the empty-string net (concat identity). Constructors have no receiving
        // operand, so they build under foma's default (C) options.
        let opts = FomaOptions::default();
        let mut acc = foma::structures::fsm_empty_string();
        for (i, o) in spv {
            let pair = foma::constructions::fsm_cross_product(
                &opts,
                foma::constructions::fsm_symbol(i.as_str()),
                foma::constructions::fsm_symbol(o.as_str()),
            );
            acc = foma::constructions::fsm_concat(&opts, acc, pair);
        }
        FomaTransducer { net: acc, opts }
    }
    fn define_transducer_sps(sps: &StringPairSet, cyclic: bool) -> Self {
        // Union each pair's cross-product; the empty union is the empty set
        // (union identity). Kleene-star when a cyclic set is requested.
        let opts = FomaOptions::default();
        let mut acc = foma::structures::fsm_empty_set();
        for (i, o) in sps {
            let pair = foma::constructions::fsm_cross_product(
                &opts,
                foma::constructions::fsm_symbol(i.as_str()),
                foma::constructions::fsm_symbol(o.as_str()),
            );
            acc = foma::constructions::fsm_union(&opts, acc, pair);
        }
        if cyclic {
            acc = foma::constructions::fsm_kleene_star(&opts, acc);
        }
        FomaTransducer { net: acc, opts }
    }
    fn define_transducer_spsv(spsv: &[StringPairSet]) -> Self {
        // Concatenate each set's (acyclic) union.
        let opts = FomaOptions::default();
        let mut acc = foma::structures::fsm_empty_string();
        for sps in spsv {
            let seg = Self::define_transducer_sps(sps, false);
            acc = foma::constructions::fsm_concat(&opts, acc, seg.net.clone());
        }
        FomaTransducer { net: acc, opts }
    }
    fn define_transducer_symbol(symbol: &str) -> Self {
        let opts = FomaOptions::default();
        let net = foma::constructions::fsm_symbol(symbol);
        FomaTransducer { net, opts }
    }
    fn define_transducer_symbol_pair(isymbol: &str, osymbol: &str) -> Self {
        let opts = FomaOptions::default();
        let net = foma::constructions::fsm_cross_product(
            &opts,
            foma::constructions::fsm_symbol(isymbol),
            foma::constructions::fsm_symbol(osymbol),
        );
        FomaTransducer { net, opts }
    }

    fn are_equivalent(&self, another: &Self, _encode_weights: bool) -> bool {
        // `fsm_equivalent` does a parallel deterministic traversal that assumes
        // both inputs are deterministic and trim, so canonicalize each with
        // `fsm_minimize` first (it determinizes + coaccessible-prunes internally;
        // foma is unweighted, so this is a cheap boolean minimization).
        let lhs = foma::minimize::fsm_minimize(&self.opts, self.net.clone());
        let rhs = foma::minimize::fsm_minimize(&another.opts, another.net.clone());
        foma::constructions::fsm_equivalent(&self.opts, lhs, rhs)
    }
    fn is_automaton(&self) -> bool {
        // An acceptor: every arc has input == output. IDENTITY/UNKNOWN arcs have
        // in == out and so are automaton arcs; only a genuine `a:b` breaks it.
        for line in self.net.states.rows().iter() {
            if line.state_no == -1 {
                break;
            }
            if line.r#in != -1 && line.target != -1 && line.r#in != line.out {
                return false;
            }
        }
        true
    }
    /// The input symbols a path can start with: descend from the start state
    /// (foma's is always state 0) THROUGH epsilon and flag-diacritic arcs, and
    /// stop on each branch at the first arc carrying a real input symbol.
    /// `@_UNKNOWN_@` / `@_IDENTITY_@` are real symbols here, as they are on the
    /// tropical side — reserved sigma numbers, but not epsilon.
    fn get_initial_input_symbols(&self) -> StringSet {
        let walk = self.digest();
        let mut out = StringSet::new();
        let mut visited: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut pending: Vec<u32> = vec![0];
        while let Some(s) = pending.pop() {
            if !visited.insert(s) {
                continue;
            }
            let Some(arcs) = walk.arcs.get(s as usize) else {
                continue;
            };
            for arc in arcs {
                if arc.inum != foma::types::EPSILON && !FdOperation::is_diacritic(arc.isym.as_str())
                {
                    out.insert(arc.isym.clone());
                } else {
                    pending.push(arc.target);
                }
            }
        }
        out
    }
    /// Every input symbol appearing anywhere in the net — the whole reachable
    /// graph, not just the symbols a path can start with. Epsilon and flag arcs
    /// contribute nothing here either, but unlike `get_initial_input_symbols`
    /// the descent continues past every arc rather than stopping at the first
    /// real symbol, which is what makes this a superset and a different walk.
    fn get_first_input_symbols(&self) -> StringSet {
        let walk = self.digest();
        let mut out = StringSet::new();
        let mut visited: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut pending: Vec<u32> = vec![0];
        while let Some(s) = pending.pop() {
            if !visited.insert(s) {
                continue;
            }
            let Some(arcs) = walk.arcs.get(s as usize) else {
                continue;
            };
            for arc in arcs {
                if arc.inum != foma::types::EPSILON && !FdOperation::is_diacritic(arc.isym.as_str())
                {
                    out.insert(arc.isym.clone());
                }
                pending.push(arc.target);
            }
        }
        out
    }

    fn n_best(&self, _n: u32) -> Self {
        // unweighted: no shortest-path pruning; return an identity copy.
        self.clone()
    }
    fn extract_random_paths(&self, results: &mut HfstTwoLevelPaths, max_num: i32) {
        // Best-effort (unweighted): the first `max_num` complete paths of a
        // cycle-free traversal, not a weighted random walk. The C++ backend
        // left this unimplemented entirely.
        let cap = if max_num < 0 {
            PATH_SAFETY_CAP
        } else {
            max_num as usize
        };
        let mut cb = CollectPaths { results, cap };
        self.walk_relation(&mut cb, 0, None, false);
    }
    fn set_final_weights(&self, _weight: f32, _increment: bool) -> Self {
        // unweighted: no-op copy.
        self.clone()
    }
    fn push_labels(&self, _to_initial_state: bool) -> Self {
        // unweighted: no-op copy.
        self.clone()
    }
    fn push_weights(&self, _to_initial_state: bool) -> Self {
        // unweighted: no-op copy.
        self.clone()
    }
    fn transform_weights(&self, _func: fn(f32) -> f32) -> Self {
        // unweighted: no weights to transform; no-op copy.
        self.clone()
    }

    fn substitute_symbol_fast(&self, old_symbol: &str, new_symbol: &str) -> Option<Self> {
        Some(self.wrap_with(foma::constructions::fsm_substitute_symbol(
            self.net.clone(),
            old_symbol,
            new_symbol,
        )))
    }
    fn substitute_string_transducer(&self, old_symbol_pair: StringPair, transducer: &Self) -> Self {
        let mut net = self.net.clone();
        let mut sub = transducer.net.clone();
        self.wrap_with(foma::constructions::fsm_substitute_pair(
            &self.opts,
            &mut net,
            &old_symbol_pair.0,
            &old_symbol_pair.1,
            &mut sub,
        ))
    }
    fn disjunct_spv(&mut self, spv: &StringPairVector) {
        // self := self ∪ define_transducer_spv(spv).
        let added = Self::define_transducer_spv(spv);
        let unioned =
            foma::constructions::fsm_union(&self.opts, self.net.clone(), added.net.clone());
        self.net = unioned;
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
mod tests {
    use super::*;
    use crate::hfst_data_types::ImplementationType;
    use crate::hfst_input_stream::HfstInputStream;
    use crate::hfst_output_stream::HfstOutputStream;
    use crate::hfst_transducer::{AnyTransducer, HfstTransducer};
    use std::collections::BTreeSet;

    // Write a foma transducer through the REAL HfstOutputStream facade (the
    // FOMA_TYPE arm: operator<< writes the "FOMA" header + Backend::write
    // payload), then read it back via HfstInputStream — a true facade
    // round-trip, no hand-assembled framing.
    // [spec:hfst:sem:foma-backend.stream-io/test]
    #[test]
    fn write_through_hfst_output_stream_round_trip() {
        let ab = FomaTransducer::define_transducer_symbol_pair("a", "b");
        let cd = FomaTransducer::define_transducer_symbol_pair("c", "d");
        let original = ab.disjunct(&cd);
        let states1 = original.to_basic().unwrap().states().len();

        let mut tr = HfstTransducer::wrap(original);
        let path = std::env::temp_dir().join(format!(
            "hfst_foma_facade_{}_{}.hfst",
            std::process::id(),
            line!()
        ));
        {
            let mut out = HfstOutputStream::new_filename(
                path.to_str().unwrap(),
                ImplementationType::FOMA_TYPE,
                true,
            )
            .expect("HfstOutputStream(FOMA_TYPE)");
            out.write(&mut tr).expect("write foma transducer <<");
            out.close();
        }

        let mut instream = HfstInputStream::new_filename(path.to_str().unwrap())
            .expect("HfstInputStream over facade-written foma");
        let any = instream.read().expect("read foma transducer back");
        instream.close();
        let _ = std::fs::remove_file(&path);

        match any {
            AnyTransducer::Foma(t) => {
                let states2 = t.to_basic().unwrap().states().len();
                assert_eq!(states1, states2, "state count survives facade round-trip");
            }
            other @ (AnyTransducer::Tropical(_)
            | AnyTransducer::OlW(_)
            | AnyTransducer::OlU(_)
            | AnyTransducer::Thfst(_)) => {
                panic!("expected AnyTransducer::Foma, got {:?}", other.get_type())
            }
        }
    }

    /// Reduce a basic transducer to a value that captures its recognized
    /// relation and alphabet: (state count, final states, alphabet, arcs).
    fn snapshot(net: &HfstBasicTransducer) -> FomaSnapshot {
        let coder = net.coder();
        let n_states = (net.get_max_state() + 1) as usize;
        let mut finals = BTreeSet::new();
        let mut arcs = BTreeSet::new();
        for (s, transitions) in net.states_and_transitions().iter().enumerate() {
            let s = s as u32;
            if net.is_final_state(s) {
                finals.insert(s);
            }
            for tr in transitions.iter() {
                arcs.insert((
                    s,
                    tr.get_input_symbol(coder).to_string(),
                    tr.get_output_symbol(coder).to_string(),
                    tr.get_target_state(),
                ));
            }
        }
        let alphabet = net
            .get_alphabet()
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<String>>();
        (n_states, finals, alphabet, arcs)
    }

    /// Build the foma net for the relation a:b (state 0 -a:b-> state 1 final)
    /// via foma's construction API.
    fn build_ab() -> FomaTransducer {
        let mut handle = foma::dynarray::fsm_construct_init("ab");
        foma::dynarray::fsm_construct_set_initial(&mut handle, 0);
        foma::dynarray::fsm_construct_add_arc(&mut handle, 0, 1, "a", "b");
        foma::dynarray::fsm_construct_set_final(&mut handle, 1);
        FomaTransducer::wrap(foma::dynarray::fsm_construct_done(handle))
    }

    // [spec:hfst:sem:foma-backend.to-basic-fn/test]
    // [spec:hfst:sem:foma-backend.from-basic-fn/test]
    #[test]
    fn round_trip_preserves_relation_and_alphabet() {
        let foma = build_ab();

        // foma -> basic
        let basic1 = foma.to_basic().expect("to_basic");
        // basic -> foma -> basic
        let foma2 = FomaTransducer::from_basic(&basic1).expect("from_basic");
        let basic2 = foma2.to_basic().expect("to_basic (round-trip)");

        let s1 = snapshot(&basic1);
        let s2 = snapshot(&basic2);

        // The round trip is stable across states, finals, alphabet and arcs.
        assert_eq!(s1, s2, "round-trip to_basic∘from_basic must be stable");

        // And the concrete shape is what we built.
        assert_eq!(s1.0, 2, "two states");
        assert_eq!(s1.1, BTreeSet::from([1u32]), "state 1 is final");
        assert!(s1.2.contains("a"), "alphabet has a");
        assert!(s1.2.contains("b"), "alphabet has b");
        assert!(
            s1.3.contains(&(0u32, "a".to_string(), "b".to_string(), 1u32)),
            "arc 0 -a:b-> 1 recognized"
        );
    }

    // [spec:hfst:sem:foma-backend.algebra-impl/test]
    #[test]
    fn algebra_union_determinize_minimize_equivalence() {
        // Two symbol acceptors and their union {a, b}.
        let a = FomaTransducer::define_transducer_symbol("a");
        let b = FomaTransducer::define_transducer_symbol("b");
        let u = a.disjunct(&b);

        // Determinize + minimize the union (the very ops foma exists to run
        // unweighted); the recognized alphabet still carries a and b.
        let d = u.determinize(false).minimize(false);
        let basic = d.to_basic().expect("to_basic after determinize/minimize");
        let alphabet = snapshot(&basic).2;
        assert!(alphabet.contains("a"), "alphabet retains a");
        assert!(alphabet.contains("b"), "alphabet retains b");

        // Union is commutative up to equivalence, and {a,b} != {a}.
        let u_rev = b.disjunct(&a);
        assert!(
            d.are_equivalent(&u_rev, false),
            "det/min union equivalent to reverse-order union"
        );
        assert!(
            !d.are_equivalent(&a, false),
            "union {{a,b}} is not equivalent to {{a}}"
        );

        // Lookup confirms the recognized language: a -> a, b -> b, c -> nothing.
        let mut d = d;
        let out_a: Vec<String> = d
            .lookup_fd_str("a", -1, 0.0)
            .iter()
            .map(|p| p.second.iter().map(|s| s.as_str()).collect::<String>())
            .collect();
        assert_eq!(out_a, vec!["a".to_string()]);
        assert!(d.lookup_fd_str("c", -1, 0.0).is_empty(), "c not accepted");
    }

    // [spec:hfst:sem:foma-backend.lookup-impl/test]
    #[test]
    fn lookup_ab_yields_b() {
        let mut t = build_ab();

        // Applying the input "a" through a:b yields exactly the output "b".
        let paths = t.lookup_fd_str("a", -1, 0.0);
        let outputs: Vec<String> = paths
            .iter()
            .map(|p| p.second.iter().map(|s| s.as_str()).collect::<String>())
            .collect();
        assert_eq!(outputs, vec!["b".to_string()], "a -> b");

        // "b" is not in the input language of a:b.
        assert!(t.lookup_fd_str("b", -1, 0.0).is_empty(), "b not an input");

        // The two-level view pairs the whole input with the whole output.
        let pairs = t.lookup_fd_pairs_str("a", -1, 0.0);
        assert_eq!(pairs.len(), 1);
        let p = pairs.iter().next().unwrap();
        assert_eq!(p.second.len(), 1);
        assert_eq!(p.second[0].0.as_str(), "a");
        assert_eq!(p.second[0].1.as_str(), "b");
    }
}
