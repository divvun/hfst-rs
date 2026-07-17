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

use crate::backend::{AlgebraBackend, Backend, LookupBackend};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPath, HfstTwoLevelPaths, ImplementationType,
    StringPair, StringPairSet, StringPairVector, StringVector, Symbol,
};
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_tropical_transducer_transition_data::{SymbolType, WeightType};

use foma::options::FomaOptions;
use foma::types::Sigma;

/// The HFST special-symbol strings for foma's three reserved sigma numbers.
const EPSILON_SYMBOL: &str = "@_EPSILON_SYMBOL_@";
const UNKNOWN_SYMBOL: &str = "@_UNKNOWN_SYMBOL_@";
const IDENTITY_SYMBOL: &str = "@_IDENTITY_SYMBOL_@";

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

/// Map a foma sigma number to its HFST symbol string. The three reserved
/// numbers map to their HFST special strings; every other number is resolved
/// through the sigma alphabet.
fn sym(n: i32, sigma: &[Sigma]) -> SymbolType {
    match n {
        foma::types::EPSILON => SymbolType::from(EPSILON_SYMBOL),
        foma::types::UNKNOWN => SymbolType::from(UNKNOWN_SYMBOL),
        foma::types::IDENTITY => SymbolType::from(IDENTITY_SYMBOL),
        _ => SymbolType::from(
            foma::sigma::sigma_string(n, sigma).expect("arc symbol number resolves in sigma"),
        ),
    }
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

        let fsm = foma::dynarray::fsm_construct_done(handle);
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
        let sorted = foma::topsort::fsm_topsort(Box::new(self.net.clone()));
        sorted.is_loop_free == foma::types::Tern::No
    }

    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        // `Fsm.sigma` is a plain `Vec<Sigma>` (empty ↔ absent), so there is no
        // lazy-create guard — append straight into the alphabet.
        foma::sigma::sigma_add(symbol, &mut self.net.sigma);
        Ok(())
    }

    fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        // Approximation for the seam: a cyclic transducer is infinitely
        // ambiguous on some input. Refined by foma-backend.lookup, which can
        // restrict cyclicity to the relevant input projection.
        Ok(self.is_cyclic())
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
    // Path extraction via foma apply: enumerate the recognized relation and feed
    // each complete (input, output) path to the callback. See `extract_paths_via_apply`.
    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        self.extract_paths_via_apply(callback, cycles);
    }

    // Flag-diacritic variant: foma's apply obeys flag diacritics internally
    // (`apply_init` sets `obey_flags = 1`), so `filter_fd` needs no extra work
    // here — the enumeration already respects the flags. Documented approximation:
    // the non-filtering distinction the OpenFst backends draw is not modelled.
    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        _filter_fd: bool,
    ) {
        self.extract_paths_via_apply(callback, cycles);
    }
}

/// Safety cap on how many paths a bounded enumeration (extract-paths /
/// random-paths) yields on a cyclic net, where a faithful HFST `cycles` bound
/// is not expressible through foma's `apply` enumerator. The callback's
/// `continueSearch` flag is the real terminator; this only prevents runaway.
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

/// Translate an HFST `cycles` bound into a path enumeration cap. foma's
/// enumerator has no per-cycle traversal count, so a non-negative `cycles` is
/// approximated by a proportional cap and the unlimited case by the safety cap.
fn path_bound(cycles: i32) -> usize {
    if cycles <= 0 {
        PATH_SAFETY_CAP
    } else {
        PATH_SAFETY_CAP.min((cycles as usize).saturating_mul(64))
    }
}

impl FomaTransducer {
    /// Clone the inner net into an owned `Box<Fsm>` for foma's consuming
    /// construction API (every op takes and returns `Box<Fsm>`).
    fn boxed(&self) -> Box<foma::types::Fsm> {
        Box::new(self.net.clone())
    }

    /// Wrap a foma construction result (`Box<Fsm>`) as a `FomaTransducer`
    /// running under foma's default (C) options.
    fn wrap(fsm: Box<foma::types::Fsm>) -> Self {
        FomaTransducer {
            net: *fsm,
            opts: FomaOptions::default(),
        }
    }

    /// Wrap a foma construction result, inheriting this transducer's options.
    fn wrap_with(&self, fsm: Box<foma::types::Fsm>) -> Self {
        FomaTransducer {
            net: *fsm,
            opts: self.opts.clone(),
        }
    }

    /// The resolved input symbols on arcs leaving the start state (foma's start
    /// is always state 0). Backs both `get_initial_input_symbols` and
    /// `get_first_input_symbols` (foma draws no distinction between them).
    fn initial_input_symbols(&self) -> StringSet {
        let sigma = &self.net.sigma;
        let mut out = StringSet::new();
        for line in self.net.states.rows().iter() {
            if line.state_no == -1 {
                break;
            }
            if line.state_no == 0 && line.r#in != -1 && line.target != -1 {
                out.insert(sym(line.r#in as i32, sigma));
            }
        }
        out
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

    /// Enumerate the recognized relation and feed each complete path to
    /// `callback`. foma's `words()` yields a display string that concatenates
    /// each arc's `in:out`, which cannot be split back into aligned symbol
    /// columns; instead we enumerate the input side and apply each input
    /// downward, pairing the whole input word with each output word as a single
    /// `StringPair`. Documented approximation: the two-level path collapses to
    /// one pair (input/output strings preserved, per-symbol alignment not),
    /// each reported as `is_final = true`. `cycles` bounds cyclic nets.
    fn extract_paths_via_apply(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        let bound = path_bound(cycles);
        let inputs: Vec<String> = {
            let mut h = foma::apply::apply_init(&self.net);
            h.upper_words().take(bound).collect()
        };
        let mut produced = 0usize;
        'outer: for iw in inputs {
            let mut h = foma::apply::apply_init(&self.net);
            for ow in h.down(&iw).take(bound) {
                let mut path = HfstTwoLevelPath {
                    first: 0.0 as WeightType,
                    second: vec![(Symbol::from(iw.as_str()), Symbol::from(ow.as_str()))],
                };
                let rv = callback.operator_call(&mut path, true);
                produced += 1;
                if !rv.continueSearch || produced >= bound {
                    break 'outer;
                }
            }
        }
    }
}

// [spec:hfst:def:foma-backend.algebra-impl]
// [spec:hfst:sem:foma-backend.algebra-impl]
// Every op maps to its foma construction, all unweighted: weight arguments and
// the weight-transform ops are no-ops (foma is a boolean/unweighted algebra).
// Inputs are cloned into owned `Box<Fsm>` (foma's ops consume their arguments).
impl AlgebraBackend for FomaTransducer {
    fn remove_epsilons(&self) -> Self {
        self.wrap_with(foma::determinize::fsm_epsilon_remove(self.boxed()))
    }
    fn determinize(&self, _encode_weights: bool) -> Self {
        self.wrap_with(foma::determinize::fsm_determinize(self.boxed()))
    }
    fn minimize(&self, _encode_weights: bool) -> Self {
        self.wrap_with(foma::minimize::fsm_minimize(&self.opts, self.boxed()))
    }
    fn repeat_star(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_kleene_star(
            &self.opts,
            self.boxed(),
        ))
    }
    fn repeat_plus(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_kleene_plus(
            &self.opts,
            self.boxed(),
        ))
    }
    fn repeat_n(&self, n: u32) -> Self {
        self.wrap_with(foma::constructions::fsm_concat_n(
            &self.opts,
            self.boxed(),
            n as i32,
        ))
    }
    fn repeat_le_n(&self, n: u32) -> Self {
        self.wrap_with(foma::constructions::fsm_concat_m_n(
            &self.opts,
            self.boxed(),
            0,
            n as i32,
        ))
    }
    fn optionalize(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_optionality(
            &self.opts,
            self.boxed(),
        ))
    }
    fn invert(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_invert(self.boxed()))
    }
    fn reverse(&self) -> Self {
        self.wrap_with(foma::reverse::fsm_reverse(self.boxed()))
    }
    fn extract_input_language(&self) -> Self {
        self.wrap_with(foma::extract::fsm_upper(self.boxed()))
    }
    fn extract_output_language(&self) -> Self {
        self.wrap_with(foma::extract::fsm_lower(self.boxed()))
    }

    fn concatenate(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_concat(
            &self.opts,
            self.boxed(),
            another.boxed(),
        ))
    }
    fn disjunct(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_union(
            &self.opts,
            self.boxed(),
            another.boxed(),
        ))
    }
    fn intersect(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_intersect(
            &self.opts,
            self.boxed(),
            another.boxed(),
        ))
    }
    fn subtract(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_minus(
            &self.opts,
            self.boxed(),
            another.boxed(),
        ))
    }
    fn compose(&self, another: &Self) -> Self {
        self.wrap_with(foma::constructions::fsm_compose(
            &self.opts,
            self.boxed(),
            another.boxed(),
        ))
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
        FomaTransducer { net: *acc, opts }
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
        FomaTransducer { net: *acc, opts }
    }
    fn define_transducer_spsv(spsv: &[StringPairSet]) -> Self {
        // Concatenate each set's (acyclic) union.
        let opts = FomaOptions::default();
        let mut acc = foma::structures::fsm_empty_string();
        for sps in spsv {
            let seg = Self::define_transducer_sps(sps, false);
            acc = foma::constructions::fsm_concat(&opts, acc, seg.boxed());
        }
        FomaTransducer { net: *acc, opts }
    }
    fn define_transducer_symbol(symbol: &str) -> Self {
        let opts = FomaOptions::default();
        let net = foma::constructions::fsm_symbol(symbol);
        FomaTransducer { net: *net, opts }
    }
    fn define_transducer_symbol_pair(isymbol: &str, osymbol: &str) -> Self {
        let opts = FomaOptions::default();
        let net = foma::constructions::fsm_cross_product(
            &opts,
            foma::constructions::fsm_symbol(isymbol),
            foma::constructions::fsm_symbol(osymbol),
        );
        FomaTransducer { net: *net, opts }
    }

    fn are_equivalent(&self, another: &Self, _encode_weights: bool) -> bool {
        // `fsm_equivalent` does a parallel deterministic traversal that assumes
        // both inputs are deterministic and trim, so canonicalize each with
        // `fsm_minimize` first (it determinizes + coaccessible-prunes internally;
        // foma is unweighted, so this is a cheap boolean minimization).
        let lhs = foma::minimize::fsm_minimize(&self.opts, self.boxed());
        let rhs = foma::minimize::fsm_minimize(&another.opts, another.boxed());
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
    fn get_initial_input_symbols(&self) -> StringSet {
        self.initial_input_symbols()
    }
    fn get_first_input_symbols(&self) -> StringSet {
        self.initial_input_symbols()
    }

    fn n_best(&self, _n: u32) -> Self {
        // unweighted: no shortest-path pruning; return an identity copy.
        self.clone()
    }
    fn extract_random_paths(&self, results: &mut HfstTwoLevelPaths, max_num: i32) {
        // Best-effort (unweighted): enumerate up to `max_num` input words and
        // pair each with one output as a single-pair, weight-0 two-level path.
        let cap = if max_num < 0 {
            PATH_SAFETY_CAP
        } else {
            max_num as usize
        };
        let inputs: Vec<String> = {
            let mut h = foma::apply::apply_init(&self.net);
            h.upper_words().take(cap).collect()
        };
        for iw in inputs {
            if results.len() >= cap {
                break;
            }
            let mut h = foma::apply::apply_init(&self.net);
            if let Some(ow) = h.down(&iw).next() {
                results.insert(HfstTwoLevelPath {
                    first: 0.0 as WeightType,
                    second: vec![(Symbol::from(iw.as_str()), Symbol::from(ow.as_str()))],
                });
            }
        }
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
            self.boxed(),
            old_symbol,
            new_symbol,
        )))
    }
    fn substitute_string_transducer(
        &self,
        _old_symbol_pair: StringPair,
        _transducer: &Self,
    ) -> Self {
        // Documented approximation: foma has no faithful "replace this (in, out)
        // label with an arbitrary relation" primitive (its `fsm_substitute_label`
        // replaces a single symbol with a network, not a symbol *pair*), so this
        // returns an unmodified copy. Callers needing the real substitution route
        // through the generic HfstBasicTransducer path.
        self.clone()
    }
    fn disjunct_spv(&mut self, spv: &StringPairVector) {
        // self := self ∪ define_transducer_spv(spv).
        let added = Self::define_transducer_spv(spv);
        let unioned = foma::constructions::fsm_union(&self.opts, self.boxed(), added.boxed());
        self.net = *unioned;
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
    fn is_lookup_infinitely_ambiguous_str(&mut self, _s: &str) -> bool {
        // Approximation: whole-net cyclicity (not restricted to the input's
        // reachable projection).
        self.is_cyclic()
    }
    fn is_lookup_infinitely_ambiguous_strvec(&mut self, _s: &StringVector) -> bool {
        self.is_cyclic()
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
    fn snapshot(
        net: &HfstBasicTransducer,
    ) -> (
        usize,
        BTreeSet<u32>,
        BTreeSet<String>,
        BTreeSet<(u32, String, String, u32)>,
    ) {
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
