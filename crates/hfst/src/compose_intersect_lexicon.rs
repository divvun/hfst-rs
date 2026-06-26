//! Port of
//! 'libhfst/src/implementations/compose_intersect/ComposeIntersectLexicon.{h,cc}'.
//!
//! The "lexicon" automaton of the compose-intersect machinery, derived from
//! ['ComposeIntersectFst']. It drives the lazy product construction
//! (['ComposeIntersectLexicon::compose_with_rules']) between this lexicon and a
//! rule transducer: pairs of (lexicon-state, rule-state) are mapped to states of
//! the 'result' ['HfstBasicTransducer'] on demand, with an agenda (FIFO queue)
//! holding the states still to expand.
//!
//! 1:1 literal C++ -> Rust translation, bugs preserved.
//!
//! Structural mappings:
//! * C++ inheritance 'ComposeIntersectLexicon : public ComposeIntersectFst' ->
//!   struct composition with a 'base: ComposeIntersectFst' field (Wave-2 port
//!   convention). The protected base member 'transition_map_vector', read
//!   directly by 'compute_state', is exposed as 'pub(crate)' on the base.
//! * 'std::pair<HfstState,HfstState>' -> '(HfstState, HfstState)' tuple;
//!   'std::map' -> 'BTreeMap'; 'std::set' -> 'BTreeSet'; 'std::vector' -> 'Vec';
//!   'std::queue<HfstState>' -> 'VecDeque<HfstState>'
//!   ('empty()'/'front()'/'pop()'/'push()' -> 'is_empty()'/'front()'/
//!   'pop_front()'/'push_back()').
//! * 'HFST_THROW(StateNotDefined)' -> 'crate::HFST_THROW!(StateNotDefined)',
//!   using the 'StateNotDefined' child exception defined in
//!   ['crate::compose_intersect_fst'].
//! * 'FdOperation::is_diacritic' -> ['crate::hfst_flag_diacritics::FdOperation::is_diacritic'].
//! * The C++ parameter type of ['ComposeIntersectLexicon::compose_with_rules'] is
//!   'ComposeIntersectRule *', a base pointer that may point at a plain
//!   'ComposeIntersectRule' (the single-rule path) or a 'ComposeIntersectRulePair'
//!   (the multi-rule path), with 'get_transitions' / 'get_final_weight' dispatched
//!   virtually. The port models that pointer as '&mut dyn ComposeIntersectRuleObject'
//!   (the trait defined in ['crate::compose_intersect_rule_pair']); 'known_symbol'
//!   is the C++ *non-virtual* 'ComposeIntersectRule::known_symbol', reproduced on the
//!   same trait so the pair's (empty-'symbols') override matches C++ bug-for-bug.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::compose_intersect_fst::{ComposeIntersectFst, StateNotDefined, TransitionSet};
use crate::compose_intersect_rule_pair::ComposeIntersectRuleObject;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_data_types::size_t_to_uint;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_tropical_transducer_transition_data::HfstTropicalTransducerTransitionData;

// [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.symbol-transition-map]
// typedef ComposeIntersectFst::SymbolTransitionMap SymbolTransitionMap;
pub use crate::compose_intersect_fst::SymbolTransitionMap;

// [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.state-pair]
pub type StatePair = (HfstState, HfstState);
// [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.state-pair-map]
pub type StatePairMap = BTreeMap<StatePair, HfstState>;
// [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.state-set]
pub type StateSet = BTreeSet<HfstState>;
// [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.pair-vector]
pub type PairVector = Vec<StatePair>;
// [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.state-queue]
pub type StateQueue = VecDeque<HfstState>;

// [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon]
pub struct ComposeIntersectLexicon {
    // C++: 'class ComposeIntersectLexicon : public ComposeIntersectFst'.
    base: ComposeIntersectFst,

    // protected
    state_pair_map: StatePairMap,
    pair_vector: PairVector,
    agenda: StateQueue,
    result: HfstBasicTransducer,
    lexicon_non_epsilon_states: StateSet,
}

impl ComposeIntersectLexicon {
    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-intersect-lexicon-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-intersect-lexicon-fn]
    //
    // ComposeIntersectLexicon::ComposeIntersectLexicon(const HfstBasicTransducer &t):
    //   ComposeIntersectFst(t,false)
    // {}
    pub fn new_from_transducer(t: &HfstBasicTransducer) -> Self {
        ComposeIntersectLexicon {
            base: ComposeIntersectFst::new_from_transducer(t, false),
            state_pair_map: StatePairMap::new(),
            pair_vector: PairVector::new(),
            agenda: StateQueue::new(),
            result: HfstBasicTransducer::new(),
            lexicon_non_epsilon_states: StateSet::new(),
        }
    }

    // ComposeIntersectLexicon::ComposeIntersectLexicon(void):
    //   ComposeIntersectFst()
    // {}
    pub fn new() -> Self {
        ComposeIntersectLexicon {
            base: ComposeIntersectFst::new(),
            state_pair_map: StatePairMap::new(),
            pair_vector: PairVector::new(),
            agenda: StateQueue::new(),
            result: HfstBasicTransducer::new(),
            lexicon_non_epsilon_states: StateSet::new(),
        }
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.is-flag-diacritic-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.is-flag-diacritic-fn]
    //
    // bool ComposeIntersectLexicon::is_flag_diacritic(size_t symbol)
    // { return FdOperation::is_diacritic
    //     (HfstTropicalTransducerTransitionData::get_symbol(hfst::size_t_to_uint(symbol))); }
    fn is_flag_diacritic(&self, symbol: usize) -> bool {
        FdOperation::is_diacritic(&HfstTropicalTransducerTransitionData::get_symbol(
            size_t_to_uint(symbol),
        ))
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.clear-all-info-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.clear-all-info-fn]
    fn clear_all_info(&mut self) {
        self.state_pair_map.clear();
        self.pair_vector.clear();

        while !self.agenda.is_empty() {
            self.agenda.pop_front();
        }

        // NB: matches the C++ — 'lexicon_non_epsilon_states' is *not* cleared here.
        self.result = HfstBasicTransducer::new();
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.map-state-and-add-to-agenda-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.map-state-and-add-to-agenda-fn]
    //
    // 'allow_lexicon_epsilons' is unused in the C++ body too; carried for fidelity.
    fn map_state_and_add_to_agenda(
        &mut self,
        p: &StatePair,
        allow_lexicon_epsilons: bool,
    ) -> HfstState {
        let _ = allow_lexicon_epsilons;
        let s: HfstState;

        // ComposeIntersectRule::START is the inherited ComposeIntersectFst::START (== 0).
        if p.0 == ComposeIntersectFst::START && p.1 == ComposeIntersectFst::START {
            s = 0;
        } else {
            s = self.result.add_state_new();
        }

        // Sanity check...
        assert!(s as usize == self.state_pair_map.len());

        self.state_pair_map.insert(*p, s);
        self.pair_vector.push(*p);
        self.agenda.push_back(s);
        self.lexicon_non_epsilon_states.insert(s);

        s
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.can-have-lexicon-epsilons-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.can-have-lexicon-epsilons-fn]
    //
    // bool ComposeIntersectLexicon::can_have_lexicon_epsilons(HfstState s)
    // { return lexicon_non_epsilon_states.count(s) > 0; }
    fn can_have_lexicon_epsilons(&self, s: HfstState) -> bool {
        self.lexicon_non_epsilon_states.contains(&s)
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-with-rules-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-with-rules-fn]
    pub fn compose_with_rules(
        &mut self,
        rules: &mut dyn ComposeIntersectRuleObject,
    ) -> HfstBasicTransducer {
        self.clear_all_info();
        let start_pair: StatePair = (ComposeIntersectFst::START, ComposeIntersectFst::START);

        // This will return 0.
        let _ = self.map_state_and_add_to_agenda(&start_pair, true);

        self.compute_composition_result(rules).clone()
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.get-state-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.get-state-fn]
    //
    // C++ default arg 'bool allow_lexicon_epsilons = true'; call sites pass the
    // value the default would have produced.
    fn get_state(&mut self, p: &StatePair, allow_lexicon_epsilons: bool) -> HfstState {
        if !self.state_pair_map.contains_key(p) {
            return self.map_state_and_add_to_agenda(p, allow_lexicon_epsilons);
        }

        self.state_pair_map[p]
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.set-final-state-weights-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.set-final-state-weights-fn]
    fn set_final_state_weights(&mut self, rules: &mut dyn ComposeIntersectRuleObject) {
        for s in 0..self.pair_vector.len() {
            let pair = self.pair_vector[s];
            let lexicon_weight = self.base.get_final_weight(pair.0);
            let rules_weight = rules.get_final_weight(pair.1);
            if lexicon_weight != f32::INFINITY && rules_weight != f32::INFINITY {
                self.result
                    .set_final_weight(size_t_to_uint(s), &(lexicon_weight + rules_weight));
            }
        }
    }

    // HfstBasicTransducer &ComposeIntersectLexicon::compute_composition_result(...)
    fn compute_composition_result(
        &mut self,
        rules: &mut dyn ComposeIntersectRuleObject,
    ) -> &HfstBasicTransducer {
        while !self.agenda.is_empty() {
            let s = *self.agenda.front().unwrap();
            self.agenda.pop_front();

            let allow = self.can_have_lexicon_epsilons(s);
            self.compute_state(s, rules, allow);
        }

        self.set_final_state_weights(rules);
        &self.result
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.get-pair-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.get-pair-fn]
    fn get_pair(&self, s: HfstState) -> StatePair {
        if s as usize >= self.pair_vector.len() {
            crate::HFST_THROW!(StateNotDefined);
        }

        self.pair_vector[s as usize]
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compute-state-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compute-state-fn]
    fn compute_state(
        &mut self,
        state: HfstState,
        rules: &mut dyn ComposeIntersectRuleObject,
        allow_lexicon_epsilons: bool,
    ) {
        let p = self.get_pair(state);

        //bool lexicon_eps_transition_found = false;

        // The C++ iterates 'transition_map_vector[p.first]' (a base/protected
        // member) while mutating 'result'/'agenda'/... through the called
        // helpers. Snapshot the per-state map so the iteration does not alias
        // '&mut self'; the lexicon's own 'transition_map_vector' is never
        // modified inside this loop, so the snapshot is observably identical.
        let entries: Vec<(usize, TransitionSet)> = self.base.transition_map_vector[p.0 as usize]
            .iter()
            .map(|(k, v)| {
                let mut copy = TransitionSet::new();
                copy.assign(v);
                (*k, copy)
            })
            .collect();

        for (first, second) in entries.iter() {
            if *first
                == HfstTropicalTransducerTransitionData::get_number("@_EPSILON_SYMBOL_@") as usize
            {
                if allow_lexicon_epsilons {
                    self.lexicon_skip_symbol_compose(second, p.1, state);
                    //lexicon_eps_transition_found = true;
                }
            } else if self.is_flag_diacritic(*first) && (!rules.known_symbol(*first)) {
                self.lexicon_skip_symbol_compose(second, p.1, state);
                //lexicon_eps_transition_found = true;
            } else {
                self.compose(second, rules.get_transitions(p.1, *first), state);
            }
        }

        self.rule_skip_symbol_compose(
            rules.get_transitions(
                p.1,
                HfstTropicalTransducerTransitionData::get_number("@_EPSILON_SYMBOL_@") as usize,
            ),
            p.0,
            state,
        );
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.lexicon-skip-symbol-compose-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.lexicon-skip-symbol-compose-fn]
    fn lexicon_skip_symbol_compose(
        &mut self,
        transitions: &TransitionSet,
        rule_state: HfstState,
        origin: HfstState,
    ) {
        for it in transitions.begin() {
            let target = self.get_state(&(it.target, rule_state), true);
            self.add_transition(origin, it.ilabel, it.olabel, it.weight, target);
        }
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.rule-skip-symbol-compose-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.rule-skip-symbol-compose-fn]
    fn rule_skip_symbol_compose(
        &mut self,
        transitions: &TransitionSet,
        lex_state: HfstState,
        origin: HfstState,
    ) {
        for it in transitions.begin() {
            let target = self.get_state(&(lex_state, it.target), false);
            self.add_transition(origin, it.ilabel, it.olabel, it.weight, target);
        }
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-fn]
    fn compose(
        &mut self,
        lex_transitions: &TransitionSet,
        rule_transitions: &TransitionSet,
        origin: HfstState,
    ) {
        let p = self.get_pair(origin);
        let _ = p;
        for it in lex_transitions.begin() {
            for jt in rule_transitions.begin() {
                let target = self.get_state(&(it.target, jt.target), true);
                self.add_transition(origin, it.ilabel, jt.olabel, it.weight + jt.weight, target);
            }
        }
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.add-transition-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.add-transition-fn]
    fn add_transition(
        &mut self,
        origin: HfstState,
        input: usize,
        output: usize,
        weight: f32,
        target: HfstState,
    ) {
        self.result.add_transition(
            origin,
            &HfstBasicTransition::new_symbols(
                target,
                HfstTropicalTransducerTransitionData::get_symbol(size_t_to_uint(input)),
                HfstTropicalTransducerTransitionData::get_symbol(size_t_to_uint(output)),
                weight,
            ),
            true,
        );
    }

    // [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.identity-compose-fn]
    // [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.identity-compose-fn]
    //
    // Declared in the header ('ComposeIntersectLexicon.h') but never defined in
    // 'ComposeIntersectLexicon.cc'; reproduced as an unimplemented stub.
    #[allow(dead_code)]
    fn identity_compose(
        &mut self,
        _transitions: &TransitionSet,
        _transition: &HfstBasicTransition,
        _origin: HfstState,
    ) {
        unimplemented!(
            "ComposeIntersectLexicon::identity_compose: declared in header but never defined in C++"
        )
    }
}

impl Default for ComposeIntersectLexicon {
    fn default() -> Self {
        Self::new()
    }
}
