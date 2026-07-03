//! Port of
//! 'libhfst/src/implementations/compose_intersect/ComposeIntersectRulePair.{h,cc}'.
//!
//! A 'ComposeIntersectRulePair' lazily realises the *intersection* of two
//! 'ComposeIntersectRule's as a single (on-demand) product transducer: its
//! states are pairs '(s1, s2)' of states of 'fst1' / 'fst2', and the
//! transitions out of a product state for a given symbol are the merge-join of
//! the two component transition sets on a shared output label. It is itself a
//! 'ComposeIntersectRule' so several rules can be intersected by nesting pairs.
//!
//! 1:1 literal C++ -> Rust translation, bugs preserved.
//!
//! Structural mappings:
//! * C++ class inheritance 'ComposeIntersectRulePair : public ComposeIntersectRule'
//!   provides two things this port must reproduce:
//!   1. *runtime polymorphism* — the C++ machinery holds the components as
//!      'ComposeIntersectRule *' and dispatches the **virtual** 'get_transitions'
//!      / 'get_final_weight' (and the non-virtual-but-inherited 'get_symbols')
//!      through that pointer, so the pointee may be a plain 'ComposeIntersectRule'
//!      *or* another 'ComposeIntersectRulePair'. This is modelled with the closed
//!      ['ComposeIntersectRuleComponent'] enum (a two-arm runtime sum, since the
//!      nesting depth is runtime data); 'fst1' / 'fst2' own their components (the
//!      owning 'ComposeIntersectRule *', 'delete'd in the C++ destructor — here
//!      freed automatically on drop).
//!   2. *inherited state* — the constructor assigns the inherited
//!      'ComposeIntersectFst::symbol_set' ('ComposeIntersectRule::symbol_set =
//!      fst1->get_symbols();'), and the inherited (non-overridden) 'get_symbols'
//!      returns it. That is the *only* inherited member 'ComposeIntersectRulePair'
//!      uses (its own 'state_*' members replace the rest), and the field is
//!      'private' to 'compose_intersect_fst' with no setter, so the inheritance is
//!      flattened to a single owned 'symbol_set' field here rather than carried as
//!      a dead 'base: ComposeIntersectRule' subobject.
//! * 'typedef std::pair<HfstState,HfstState> StatePair' -> '(HfstState, HfstState)'.
//! * 'std::map' -> 'BTreeMap', 'std::vector' -> 'Vec'.
//! * 'ComposeIntersectRule::START' (the inherited 'ComposeIntersectFst::START', = 0)
//!   -> ['ComposeIntersectFst::START']; the pair's own 'START' static -> the
//!   'ComposeIntersectRulePair::START' associated const (also 0).
//! * 'hfst::size_t_to_uint' -> an inline 'u32::try_from' narrowing.
//! * 'HFST_THROW(StateNotDefined)' -> 'crate::bail!(StateNotDefined)' with the
//!   'StateNotDefined' child exception owned by 'compose_intersect_fst'.
//! * The merge-join in 'compute_transition_set' holds 'const TransitionSet &'s into
//!   'fst1' / 'fst2' while calling the *self*-mutating 'get_state'; since 'get_state'
//!   never touches 'fst1' / 'fst2', the component sets are snapshotted into
//!   'Vec<Transition>' (sorted-vector order == C++ 'begin()..end()') so the borrows
//!   are released before '&mut self' is needed.
//!
//! The '#ifdef MAIN_TEST' section ('print', 'operator<<', 'main') is omitted per
//! the port conventions.

use std::collections::BTreeMap;

use crate::compose_intersect_fst::{ComposeIntersectFst, SymbolSet, Transition};
use crate::compose_intersect_rule::ComposeIntersectRule;
use crate::hfst_data_types::implementations::HfstState;

// [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.transition-set]
//
// 'typedef ComposeIntersectRule::TransitionSet TransitionSet;' — the same
// 'SpaceSavingSet<Transition, CompareTransitions>' owned by 'compose_intersect_fst'.
pub type TransitionSet = crate::compose_intersect_fst::TransitionSet;

// [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.state-pair]
//
// 'typedef std::pair<HfstState,HfstState> StatePair;'
pub type StatePair = (HfstState, HfstState);
// [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.state-pair-vector]
//
// 'typedef std::vector<StatePair> StatePairVector;'
pub type StatePairVector = Vec<StatePair>;
// [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.pair-state-map]
//
// 'typedef std::map<StatePair,HfstState> PairStateMap;'
pub type PairStateMap = BTreeMap<StatePair, HfstState>;
// [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.symbol-transition-map]
//
// 'typedef std::map<size_t,TransitionSet> SymbolTransitionMap;'
pub type SymbolTransitionMap = BTreeMap<usize, TransitionSet>;
// [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.state-transition-vector]
//
// 'typedef std::vector<SymbolTransitionMap> StateTransitionVector;'
pub type StateTransitionVector = Vec<SymbolTransitionMap>;

/// The closed component sum behind a 'ComposeIntersectRule *'.
///
/// C++ reached the components via virtual dispatch on 'ComposeIntersectRule *'
/// (which may point to a 'ComposeIntersectRule' or a nested
/// 'ComposeIntersectRulePair'):
/// * 'get_transitions' / 'get_final_weight' are 'virtual' (overridden by the pair);
/// * 'get_symbols' is inherited and not overridden, so it returns the component's
///   own 'symbol_set';
/// * 'known_symbol' is the *non-virtual* 'ComposeIntersectRule::known_symbol'
///   that 'ComposeIntersectLexicon' calls on the same pointer — see the 'Pair'
///   arm below for how the non-virtual dispatch is reproduced.
///
/// The implementor set is closed (exactly these two), but the nesting depth is
/// runtime data (one pair per extra rule in 'compose_intersect'), so this is
/// the one place in the compose-intersect machinery that keeps a runtime sum —
/// the same shape as the facade's 'AnyTransducer'
/// ([dec:hfst:monomorphic-backends]); the former 'dyn' trait dispatch becomes
/// a two-arm match.
pub enum ComposeIntersectRuleComponent {
    Rule(ComposeIntersectRule),
    Pair(Box<ComposeIntersectRulePair>),
}

impl ComposeIntersectRuleComponent {
    pub fn get_transitions(
        &mut self,
        s: HfstState,
        symbol: usize,
    ) -> crate::error::Result<&TransitionSet> {
        match self {
            ComposeIntersectRuleComponent::Rule(r) => {
                ComposeIntersectRule::get_transitions(r, s, symbol)
            }
            ComposeIntersectRuleComponent::Pair(p) => {
                ComposeIntersectRulePair::get_transitions(p, s, symbol)
            }
        }
    }
    pub fn get_final_weight(&self, s: HfstState) -> crate::error::Result<f32> {
        match self {
            ComposeIntersectRuleComponent::Rule(r) => ComposeIntersectRule::get_final_weight(r, s),
            ComposeIntersectRuleComponent::Pair(p) => {
                ComposeIntersectRulePair::get_final_weight(p, s)
            }
        }
    }
    pub fn get_symbols(&self) -> &SymbolSet {
        match self {
            ComposeIntersectRuleComponent::Rule(r) => ComposeIntersectRule::get_symbols(r),
            // 'get_symbols' is inherited and not overridden by the pair, so it
            // returns the 'symbol_set' assigned in the pair's constructor.
            ComposeIntersectRuleComponent::Pair(p) => &p.symbol_set,
        }
    }
    pub fn known_symbol(&self, symbol: usize) -> crate::error::Result<bool> {
        match self {
            ComposeIntersectRuleComponent::Rule(r) => ComposeIntersectRule::known_symbol(r, symbol),
            // 'known_symbol' is the *non-virtual* 'ComposeIntersectRule::
            // known_symbol', so calling it through a 'ComposeIntersectRule *'
            // that actually points at a 'ComposeIntersectRulePair' reads the
            // pair's *inherited* 'symbols' StringSet. That member is never
            // populated for a pair (the constructor only assigns the numeric
            // 'symbol_set'), so 'symbols.count(...) > 0' is always false. The
            // flattened port has no 'symbols' field on the pair, so this
            // returns 'false' unconditionally — bug-for-bug identical.
            ComposeIntersectRuleComponent::Pair(_) => Ok(false),
        }
    }
}

// [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair]
pub struct ComposeIntersectRulePair {
    // Inherited 'ComposeIntersectFst::symbol_set' — the only inherited member used
    // by 'ComposeIntersectRulePair' (assigned in the constructor, read back by the
    // inherited 'get_symbols'). See the module docs for why inheritance is flattened.
    pub(crate) symbol_set: SymbolSet,

    // protected:
    state_pair_vector: StatePairVector,
    pair_state_map: PairStateMap,
    state_transition_vector: StateTransitionVector,

    fst1: ComposeIntersectRuleComponent,
    fst2: ComposeIntersectRuleComponent,
}

impl ComposeIntersectRulePair {
    // 'const HfstState ComposeIntersectRulePair::START = 0;'
    pub const START: HfstState = 0;

    // [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.compose-intersect-rule-pair-fn]
    // [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.compose-intersect-rule-pair-fn]
    //
    // ComposeIntersectRulePair::ComposeIntersectRulePair
    // (ComposeIntersectRule * fst1,ComposeIntersectRule * fst2):
    //   fst1(fst1), fst2(fst2)
    // {
    //   ComposeIntersectRule::symbol_set = fst1->get_symbols();
    //   pair_state_map[StatePair(ComposeIntersectRule::START,
    //                            ComposeIntersectRule::START)] = START;
    //   state_pair_vector.push_back(StatePair(ComposeIntersectRule::START,
    //                                         ComposeIntersectRule::START));
    //   state_transition_vector.push_back(SymbolTransitionMap());
    // }
    pub fn new(fst1: ComposeIntersectRuleComponent, fst2: ComposeIntersectRuleComponent) -> Self {
        // ComposeIntersectRule::symbol_set = fst1->get_symbols();
        let symbol_set = fst1.get_symbols().clone();

        let mut pair_state_map = PairStateMap::new();
        pair_state_map.insert(
            (ComposeIntersectFst::START, ComposeIntersectFst::START),
            Self::START,
        );

        let mut state_pair_vector = StatePairVector::new();
        state_pair_vector.push((ComposeIntersectFst::START, ComposeIntersectFst::START));

        let mut state_transition_vector = StateTransitionVector::new();
        state_transition_vector.push(SymbolTransitionMap::new());

        ComposeIntersectRulePair {
            symbol_set,
            state_pair_vector,
            pair_state_map,
            state_transition_vector,
            fst1,
            fst2,
        }
    }

    // ComposeIntersectRulePair::~ComposeIntersectRulePair(void)
    // { delete fst1; delete fst2; }
    //
    // 'fst1' / 'fst2' are owning 'Box'es; their 'Drop' frees the pointees exactly
    // as the C++ destructor's 'delete's do, so no explicit 'Drop' impl is needed.

    // (no [spec] annotation in the C++ source)
    //
    // const TransitionSet & ComposeIntersectRulePair::get_transitions
    // (HfstState s,size_t symbol)
    // {
    //   if (! has_state(s)) { HFST_THROW(StateNotDefined); }
    //   if (! transitions_computed(s,symbol)) { compute_transition_set(s,symbol); }
    //   return state_transition_vector[s][symbol];
    // }
    pub fn get_transitions(
        &mut self,
        s: HfstState,
        symbol: usize,
    ) -> crate::error::Result<&TransitionSet> {
        if !self.has_state(s) {
            crate::bail!(StateNotDefined);
        }
        if !self.transitions_computed(s, symbol) {
            self.compute_transition_set(s, symbol)?;
        }
        Ok(&self.state_transition_vector[s as usize][&symbol])
    }

    // [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.has-state-fn]
    // [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.has-state-fn]
    //
    // bool ComposeIntersectRulePair::has_state(HfstState s) const
    // { return s < state_pair_vector.size(); }
    fn has_state(&self, s: HfstState) -> bool {
        (s as usize) < self.state_pair_vector.len()
    }

    // [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.has-pair-fn]
    // [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.has-pair-fn]
    //
    // bool ComposeIntersectRulePair::has_pair(const StatePair &p) const
    // { return pair_state_map.find(p) != pair_state_map.end(); }
    fn has_pair(&self, p: &StatePair) -> bool {
        self.pair_state_map.contains_key(p)
    }

    // [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.transitions-computed-fn]
    // [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.transitions-computed-fn]
    //
    // bool ComposeIntersectRulePair::transitions_computed(HfstState state,size_t symbol)
    // { return state_transition_vector.at(state).find(symbol)
    //     != state_transition_vector.at(state).end(); }
    fn transitions_computed(&self, state: HfstState, symbol: usize) -> bool {
        self.state_transition_vector[state as usize].contains_key(&symbol)
    }

    // [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.get-state-fn]
    // [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.get-state-fn]
    //
    // HfstState ComposeIntersectRulePair::get_state(const StatePair &p)
    // {
    //   if (! has_pair(p))
    //   {
    //     pair_state_map[p] = hfst::size_t_to_uint(state_pair_vector.size());
    //     state_pair_vector.push_back(p);
    //     state_transition_vector.push_back(SymbolTransitionMap());
    //     return hfst::size_t_to_uint(state_pair_vector.size() - 1);
    //   }
    //   return pair_state_map[p];
    // }
    fn get_state(&mut self, p: &StatePair) -> HfstState {
        if !self.has_pair(p) {
            self.pair_state_map.insert(
                *p,
                u32::try_from(self.state_pair_vector.len()).expect("value out of u32 range"),
            );
            self.state_pair_vector.push(*p);
            self.state_transition_vector
                .push(SymbolTransitionMap::new());
            return u32::try_from(self.state_pair_vector.len() - 1)
                .expect("value out of u32 range");
        }
        self.pair_state_map[p]
    }

    // [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.add-transition-fn]
    // [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.add-transition-fn]
    //
    // void ComposeIntersectRulePair::add_transition
    // (TransitionSet &transitions,HfstState target,size_t input_symbol,
    //  size_t output_symbol,float weight)
    // { transitions.insert(Transition(target,input_symbol,output_symbol,weight)); }
    //
    // The C++ member ignores 'this'; rendered as an associated function.
    fn add_transition(
        transitions: &mut TransitionSet,
        target: HfstState,
        input_symbol: usize,
        output_symbol: usize,
        weight: f32,
    ) {
        transitions.insert(&Transition::new(
            target,
            input_symbol,
            output_symbol,
            weight,
        ));
    }

    // [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.get-final-weight-fn]
    // [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.get-final-weight-fn]
    //
    // float ComposeIntersectRulePair::get_final_weight(HfstState s) const
    // {
    //   if (! has_state(s)) { HFST_THROW(StateNotDefined); }
    //   const StatePair &state_pair = state_pair_vector[s];
    //   return fst1->get_final_weight(state_pair.first) +
    //          fst2->get_final_weight(state_pair.second);
    // }
    pub fn get_final_weight(&self, s: HfstState) -> crate::error::Result<f32> {
        if !self.has_state(s) {
            crate::bail!(StateNotDefined);
        }
        let state_pair = self.state_pair_vector[s as usize];
        Ok(self.fst1.get_final_weight(state_pair.0)? + self.fst2.get_final_weight(state_pair.1)?)
    }

    // [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.compute-transition-set-fn]
    // [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.compute-transition-set-fn]
    //
    // void ComposeIntersectRulePair::compute_transition_set
    // (HfstState state, size_t symbol)
    // {
    //   StatePair state_pair = state_pair_vector[state];
    //   const ComposeIntersectRule::TransitionSet &fst1_transitions =
    //     fst1->get_transitions(state_pair.first,symbol);
    //   ComposeIntersectRule::TransitionSet::const_iterator it = fst1_transitions.begin();
    //   const ComposeIntersectRule::TransitionSet &fst2_transitions =
    //     fst2->get_transitions(state_pair.second,symbol);
    //   ComposeIntersectRule::TransitionSet::const_iterator jt = fst2_transitions.begin();
    //
    //   (void)state_transition_vector[state][symbol];
    //   TransitionSet transitions;
    //   while (it != fst1_transitions.end() && jt != fst2_transitions.end())
    //   {
    //     if (it->olabel == jt->olabel)
    //     {
    //       size_t output = it->olabel;
    //       HfstState target = get_state(StatePair(it->target,jt->target));
    //       float weight = it->weight + jt->weight;
    //       add_transition(transitions,target,symbol,output,weight);
    //       ++it; ++jt;
    //     }
    //     else if (it->olabel < jt->olabel) { ++it; }
    //     else { ++jt; }
    //   }
    //   state_transition_vector[state][symbol] = transitions;
    // }
    fn compute_transition_set(
        &mut self,
        state: HfstState,
        symbol: usize,
    ) -> crate::error::Result<()> {
        let state_pair = self.state_pair_vector[state as usize];

        // Snapshot the two component transition sets (sorted-vector order ==
        // C++ 'begin()..end()'); this releases the '&mut fst{1,2}' borrows so the
        // self-mutating 'get_state' can run inside the merge below.
        let fst1_transitions: Vec<Transition> = self
            .fst1
            .get_transitions(state_pair.0, symbol)?
            .begin()
            .cloned()
            .collect();
        let fst2_transitions: Vec<Transition> = self
            .fst2
            .get_transitions(state_pair.1, symbol)?
            .begin()
            .cloned()
            .collect();

        // (void)state_transition_vector[state][symbol];  -- default-insert the key.
        self.state_transition_vector[state as usize]
            .entry(symbol)
            .or_insert_with(TransitionSet::new);

        let mut transitions = TransitionSet::new();
        let mut it = 0usize;
        let mut jt = 0usize;
        while it != fst1_transitions.len() && jt != fst2_transitions.len() {
            if fst1_transitions[it].olabel == fst2_transitions[jt].olabel {
                let output = fst1_transitions[it].olabel;
                let target =
                    self.get_state(&(fst1_transitions[it].target, fst2_transitions[jt].target));
                let weight = fst1_transitions[it].weight + fst2_transitions[jt].weight;
                Self::add_transition(&mut transitions, target, symbol, output, weight);
                it += 1;
                jt += 1;
            } else if fst1_transitions[it].olabel < fst2_transitions[jt].olabel {
                it += 1;
            } else {
                jt += 1;
            }
        }
        // state_transition_vector[state][symbol] = transitions;
        self.state_transition_vector[state as usize].insert(symbol, transitions);
        Ok(())
    }
}

// (The former 'ComposeIntersectRuleObject' impls for 'ComposeIntersectRule' and
// 'ComposeIntersectRulePair' are the two match arms of
// ['ComposeIntersectRuleComponent'] above.)
