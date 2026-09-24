//! The foma backend's FST algebra: every op maps to its foma construction.

use super::*;

/// The complement foma's own 'fsm_complement' cannot take: one over label
/// PAIRS.
///
/// 'fsm_completes' (constructions/boolean.rs, the body behind both complete and
/// complement) rebuilds a machine one symbol NUMBER at a time —
/// 'add_fsm_arc(..., j, j, ...)' — so what it hands back is single-tape: every
/// arc of it has its two sides equal, and a mapping arc like 'a:x' has nothing
/// there to meet. Intersecting a real transducer with such a complement is the
/// empty net whatever the operands were, which is no difference of relations.
///
/// Completing over the pairs the operands actually use instead makes the
/// universe a relation: '(identity symbols | mapping pairs)*' minus the
/// subtrahend, taken with foma's own difference — 'fsm_minus' matches a
/// subtrahend arc on BOTH labels, so it is already pair-aware. Unlike folding
/// each pair onto a symbol of its own, this leaves every label where it was,
/// which the flag overlay depends on: it reads the arcs' output labels to tell
/// a flag from a true epsilon from an ordinary symbol.
///
/// The identity half is exactly what 'fsm_completes' would have completed over:
/// every ordinary sigma symbol of the subtrahend, plus foma's IDENTITY, which
/// it declares itself when the net has none.
fn pair_complement(
    opts: &FomaOptions,
    subtrahend: foma::types::Fsm,
    mapping_pairs: &std::collections::BTreeSet<(SymbolType, SymbolType)>,
) -> foma::types::Fsm {
    let mut handle = foma::dynarray::fsm_construct_init("");
    foma::dynarray::fsm_construct_set_initial(&mut handle, 0);
    foma::dynarray::fsm_construct_set_final(&mut handle, 0);
    for entry in &subtrahend.sigma {
        if entry.number > foma::types::IDENTITY {
            foma::dynarray::fsm_construct_add_arc(&mut handle, 0, 0, &entry.symbol, &entry.symbol);
        }
    }
    foma::dynarray::fsm_construct_add_arc(&mut handle, 0, 0, IDENTITY_SYMBOL, IDENTITY_SYMBOL);
    for (input, output) in mapping_pairs {
        foma::dynarray::fsm_construct_add_arc(&mut handle, 0, 0, input, output);
    }
    let universe = foma::dynarray::fsm_construct_done(handle);
    foma::constructions::fsm_minus(opts, universe, subtrahend)
}

// [spec:hfst:def:foma-backend.algebra-impl]
// [spec:hfst:sem:foma-backend.algebra-impl]
// Every op maps to its foma construction, all unweighted: weight arguments and
// the weight-transform ops are no-ops (foma is a boolean/unweighted algebra).
// Inputs are cloned into owned `Box<Fsm>` (foma's ops consume their arguments).
impl AlgebraBackend for FomaTransducer {
    const SUPPORTS_VIRTUAL_FLAG_COMPOSE: bool = true;
    const SUPPORTS_VIRTUAL_FLAG_INTERSECTION: bool = true;
    const SUPPORTS_VIRTUAL_FLAG_SUBTRACTION: bool = true;

    fn remove_epsilons(&self) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::determinize::fsm_epsilon_remove(self.net.clone())))
    }
    fn determinize(self, _encode_weights: bool) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::determinize::fsm_determinize(self.net.clone())))
    }
    fn minimize(self, _encode_weights: bool) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::minimize::fsm_minimize(&self.opts, self.net.clone())))
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
    fn repeat_n(&self, n: u32) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::constructions::fsm_concat_n(
            &self.opts,
            self.net.clone(),
            n as i32,
        )))
    }
    fn repeat_le_n(&self, n: u32) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::constructions::fsm_concat_m_n(
            &self.opts,
            self.net.clone(),
            0,
            n as i32,
        )))
    }
    fn optionalize(&self) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::constructions::fsm_optionality(
            &self.opts,
            self.net.clone(),
        )))
    }
    fn invert(&self) -> Self {
        self.wrap_with(foma::constructions::fsm_invert(self.net.clone()))
    }
    fn reverse(&self) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::reverse::fsm_reverse(self.net.clone())))
    }
    fn extract_input_language(&self) -> Self {
        self.wrap_with(foma::extract::fsm_upper(self.net.clone()))
    }
    fn extract_output_language(&self) -> Self {
        self.wrap_with(foma::extract::fsm_lower(self.net.clone()))
    }

    fn concatenate(&self, another: &Self) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::constructions::fsm_concat(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        )))
    }
    fn disjunct(&self, another: &Self) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::constructions::fsm_union(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        )))
    }
    fn intersect(&self, another: &Self) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::constructions::fsm_intersect(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        )))
    }
    fn subtract(&self, another: &Self) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::constructions::fsm_minus(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        )))
    }
    fn compose(&self, another: &Self) -> crate::error::Result<Self> {
        Ok(self.wrap_with(foma::constructions::fsm_compose(
            &self.opts,
            self.net.clone(),
            another.net.clone(),
        )))
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
            let FomaTransducer { net, opts } = self;

            // With no virtual loops to supply, this is an ordinary difference,
            // and foma's own 'fsm_minus' is the primitive for it: its product
            // matches a subtrahend arc on BOTH labels (constructions/boolean.rs
            // fsm_minus: 'fsm1[ai].in == fsm2[bi].in && fsm1[ai].out ==
            // fsm2[bi].out'), so it subtracts relations, not just languages.
            // The complement route below cannot: 'fsm_completes' rebuilds the
            // machine one symbol NUMBER at a time ('add_fsm_arc(..., j, j,
            // ...)'), so the complement it returns is single-tape and no arc of
            // it can meet a mapping arc whose sides differ — every subtraction
            // from a real transducer came back empty. This is also what the C++
            // did (FomaTransducer.cc:581 'FomaTransducer::subtract' ->
            // 'fsm_minus(fsm_copy(t1), fsm_copy(t2))').
            let no_virtual_loops = flag_overlay.is_none_or(|overlay| {
                overlay.left_self_loops.is_empty() && overlay.right_self_loops.is_empty()
            });
            if no_virtual_loops {
                let result = foma::constructions::fsm_minus(&opts, net, another.net);
                return Ok(FomaTransducer { net: result, opts });
            }

            let mut complement_source = another.net;

            // A missing right-side flag is a self-loop at every state of the
            // subtrahend. Such a loop commutes with determinization, but it
            // must be present before complement: ordinary completion would
            // otherwise send the flag to the accepting complement sink.
            // Remove these alphabet-only labels while complement completes
            // the real alphabet; the lazy intersection overlay restores them
            // as self-loops on every complement state, including its sink.
            if let Some(flag_overlay) = flag_overlay {
                for symbol in &flag_overlay.right_self_loops {
                    foma::sigma::sigma_remove(symbol, &mut complement_source.sigma);
                }
                foma::sigma::sigma_sort(&mut complement_source);
            }
            // The overlay lives in the intersection, so this route has to keep
            // the complement; when either operand maps one symbol to another,
            // that complement has to be taken over pairs (see 'pair_complement').
            let mapping_pairs = transducing_pairs(&net)
                .into_iter()
                .chain(transducing_pairs(&complement_source))
                .collect::<std::collections::BTreeSet<_>>();
            let complement = if mapping_pairs.is_empty() {
                foma::constructions::fsm_complement(&opts, complement_source)
            } else {
                pair_complement(&opts, complement_source, &mapping_pairs)
            };
            let result = foma::constructions::fsm_intersect_with_flag_overlay(
                &opts, net, complement, &overlay,
            )
            .map_err(|error| crate::err!(Hfst, format!("Foma subtraction: {error}")))?;
            return Ok(FomaTransducer { net: result, opts });
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
        // A reserved symbol names an arc label, not a language: foma reads
        // IDENTITY as `?`, so crossing the two sides yields `? .x. ?` — an
        // UNKNOWN:UNKNOWN arc beside the intended one, which then expands
        // independently on each side as the alphabet grows. Build the single
        // arc directly instead.
        if is_reserved_symbol(isymbol) || is_reserved_symbol(osymbol) {
            let mut handle = foma::dynarray::fsm_construct_init("");
            foma::dynarray::fsm_construct_set_initial(&mut handle, 0);
            foma::dynarray::fsm_construct_set_final(&mut handle, 1);
            foma::dynarray::fsm_construct_add_arc(&mut handle, 0, 1, isymbol, osymbol);
            let net = foma::dynarray::fsm_construct_done(handle);
            return FomaTransducer { net, opts };
        }
        let net = foma::constructions::fsm_cross_product(
            &opts,
            foma::constructions::fsm_symbol(isymbol),
            foma::constructions::fsm_symbol(osymbol),
        );
        FomaTransducer { net, opts }
    }

    fn are_equivalent(&self, another: &Self, _encode_weights: bool) -> crate::error::Result<bool> {
        // `fsm_equivalent` does a parallel deterministic traversal that assumes
        // both inputs are deterministic and trim, so canonicalize each with
        // `fsm_minimize` first (it determinizes + coaccessible-prunes internally;
        // foma is unweighted, so this is a cheap boolean minimization).
        let lhs = foma::minimize::fsm_minimize(&self.opts, self.net.clone());
        let rhs = foma::minimize::fsm_minimize(&another.opts, another.net.clone());
        Ok(foma::constructions::fsm_equivalent(&self.opts, lhs, rhs))
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

    fn n_best(&self, _n: u32) -> crate::error::Result<Self> {
        // unweighted: no shortest-path pruning; return an identity copy.
        Ok(self.clone())
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
    fn push_labels(&self, _to_initial_state: bool) -> crate::error::Result<Self> {
        // unweighted: no-op copy.
        Ok(self.clone())
    }
    fn push_weights(&self, _to_initial_state: bool) -> crate::error::Result<Self> {
        // unweighted: no-op copy.
        Ok(self.clone())
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
