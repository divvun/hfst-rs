//! Port of
//! 'libhfst/src/implementations/compose_intersect/ComposeIntersectFst.{h,cc}'.
//!
//! The "left-hand side" automaton of the compose-intersect machinery: it wraps
//! an ['HfstBasicTransducer'], sorts its arcs, and indexes the transitions out
//! of every state by input (or output) symbol number, so that
//! ['ComposeIntersectFst::get_transitions'] can be answered quickly during the
//! product construction. Identity transitions ('@_IDENTITY_SYMBOL_@') are kept
//! aside in 'identity_transition_vector' and synthesised on demand for unknown
//! symbols.
//!
//! 1:1 literal C++ -> Rust translation, bugs preserved.
//!
//! Notable structural mappings:
//! * 'TransitionSet = SpaceSavingSet<Transition, CompareTransitions>' from
//!   ['crate::compose_intersect_utilities']; the C++ static member
//!   'template<> CompareTransitions TransitionSet::comparator = ...;' is carried
//!   by the 'CompareTransitions' type parameter / its ['Comparator'] impl.
//! * 'SymbolTransitionMap = std::map<size_t, TransitionSet>' -> 'BTreeMap'.
//! * The C++ 'get_transitions', 'get_identity_transition', 'has_identity_transition'
//!   and 'get_symbol_number' take a non-const 'this' (the first because 'operator[]'
//!   on the 'std::map' may insert; the others are declared non-const in the .h);
//!   they are ported with '&mut self'. ('get_final_weight', 'get_symbols',
//!   'is_known_symbol' stay '&self'.)
//! * 'HFST_THROW(StateNotDefined)' -> 'crate::HFST_THROW!(StateNotDefined)'; the
//!   'StateNotDefined' child exception (declared/defined in the '.cc') is
//!   reproduced here with the same shape as 'hfst_exception_child!'.

use std::collections::{BTreeMap, BTreeSet};

use crate::compose_intersect_utilities::{Comparator, SpaceSavingSet};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_exception_defs::HfstException;
use crate::hfst_tropical_transducer_transition_data::HfstTropicalTransducerTransitionData;

// HFST_EXCEPTION_CHILD_DECLARATION(StateNotDefined);  (in the .h)
// HFST_EXCEPTION_CHILD_DEFINITION(StateNotDefined);   (in the .cc)
//
// A subclass of 'HfstException' whose constructor forwards '(name, file, line)'
// to the base — exactly what the (non-exported) 'hfst_exception_child!' macro
// generates. Reproduced here because the macro is private to
// 'hfst_exception_defs', and 'HFST_THROW!(StateNotDefined)' needs
// 'StateNotDefined::new(String, String, usize)' in scope.
// [spec:hfst:def:compose-intersect-fst.state-not-defined]
#[derive(Clone, Debug)]
pub struct StateNotDefined {
    pub base: HfstException,
}

impl StateNotDefined {
    pub fn new(name: String, file: String, line: usize) -> Self {
        StateNotDefined {
            base: HfstException::new(name, file, line),
        }
    }
}

// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition]
#[derive(Clone, Debug)]
pub struct Transition {
    pub ilabel: usize,
    pub olabel: usize,
    pub weight: f32,
    pub target: HfstState,
}

impl Transition {
    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.transition-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.transition-fn]
    // [spec:hfst:def:transducer.hfst-ol.transition.transition-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.transition-fn]
    pub fn new_from_basic(t: &HfstBasicTransition) -> Self {
        let transition = Transition {
            ilabel: HfstTropicalTransducerTransitionData::get_number(
                &t.get_transition_data().get_input_symbol(),
            ) as usize,
            olabel: HfstTropicalTransducerTransitionData::get_number(
                &t.get_transition_data().get_output_symbol(),
            ) as usize,
            weight: t.get_weight(),
            target: t.get_target_state(),
        };
        assert!(t.get_input_symbol() != "");
        assert!(t.get_output_symbol() != "");
        transition
    }

    pub fn new(target: HfstState, ilabel: usize, olabel: usize, weight: f32) -> Self {
        Transition {
            ilabel,
            olabel,
            weight,
            target,
        }
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.operator-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.operator-fn]
    pub fn operator_eq(&self, another: &Transition) -> bool {
        self.ilabel == another.ilabel
            && self.olabel == another.olabel
            && self.weight == another.weight
            && self.target == another.target
    }
}

// 'bool operator==(const Transition&) const' made usable by 'SpaceSavingSet'
// (which compares elements with '==').
impl PartialEq for Transition {
    fn eq(&self, other: &Self) -> bool {
        self.operator_eq(other)
    }
}

// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compare-transitions]
//
// The default-constructed functor 'static CompareTransitions comparator;', used
// as the 'SpaceSavingSet' template parameter 'C'. In Rust it carries no state, so
// the comparison is provided as the 'Comparator<Transition>' trait impl.
pub struct CompareTransitions;

impl Comparator<Transition> for CompareTransitions {
    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compare-transitions.operator-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compare-transitions.operator-fn]
    fn compare(tr1: &Transition, tr2: &Transition) -> bool {
        if tr1.ilabel == tr2.ilabel {
            if tr1.olabel == tr2.olabel {
                if tr1.weight == tr2.weight {
                    tr1.target < tr2.target
                } else {
                    tr1.weight < tr2.weight
                }
            } else {
                tr1.olabel < tr2.olabel
            }
        } else {
            tr1.ilabel < tr2.ilabel
        }
    }
}

// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition-set]
pub type TransitionSet = SpaceSavingSet<Transition, CompareTransitions>;
// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.symbol-set]
pub type SymbolSet = BTreeSet<usize>;

// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.symbol-transition-map]
pub type SymbolTransitionMap = BTreeMap<usize, TransitionSet>;
// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition-map-vector]
pub type TransitionMapVector = Vec<SymbolTransitionMap>;
// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition-vector]
pub type TransitionVector = Vec<Transition>;
// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.float-vector]
pub type FloatVector = Vec<f32>;

// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst]
pub struct ComposeIntersectFst {
    // protected
    t: HfstBasicTransducer,
    symbol_set: SymbolSet,
    pub(crate) transition_map_vector: TransitionMapVector,
    finality_vector: FloatVector,
    identity_transition_vector: TransitionVector,
}

impl ComposeIntersectFst {
    pub const START: HfstState = 0;

    pub fn new() -> Self {
        // ComposeIntersectFst::ComposeIntersectFst(void): t(HfstBasicTransducer()) {}
        ComposeIntersectFst {
            t: HfstBasicTransducer::new(),
            symbol_set: SymbolSet::new(),
            transition_map_vector: TransitionMapVector::new(),
            finality_vector: FloatVector::new(),
            identity_transition_vector: TransitionVector::new(),
        }
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compose-intersect-fst-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compose-intersect-fst-fn]
    pub fn new_from_transducer(t: &HfstBasicTransducer, input_keys: bool) -> Self {
        let mut this = ComposeIntersectFst {
            t: t.clone(),
            symbol_set: SymbolSet::new(),
            transition_map_vector: TransitionMapVector::new(),
            finality_vector: FloatVector::new(),
            identity_transition_vector: TransitionVector::new(),
        };

        this.t.sort_arcs();
        let alphabet = this.t.get_alphabet().clone();

        for it in alphabet.iter() {
            this.symbol_set
                .insert(HfstTropicalTransducerTransitionData::get_number(it) as usize);
        }

        let mut source_state: u32 = 0;
        // for (HfstBasicTransducer::const_iterator it = this->t.begin(); ...)
        let states: Vec<crate::hfst_basic_transducer::HfstBasicTransitions> =
            this.t.iter().cloned().collect();
        for it in states.iter() {
            this.transition_map_vector.push(SymbolTransitionMap::new());
            if this.t.is_final_state(source_state) {
                this.finality_vector
                    .push(this.t.get_final_weight(source_state));
            } else {
                this.finality_vector.push(f32::INFINITY);
            }
            source_state += 1;
            // SymbolTransitionMap &symbol_transition_map = transition_map_vector.back();
            let symbol_transition_map = this.transition_map_vector.last_mut().unwrap();
            let mut identity_found = false;
            for jt in it.iter() {
                if jt.get_input_symbol() == "@_IDENTITY_SYMBOL_@" {
                    identity_found = true;
                    this.identity_transition_vector
                        .push(Transition::new_from_basic(jt));
                } else {
                    let key = if input_keys {
                        HfstTropicalTransducerTransitionData::get_number(&jt.get_input_symbol())
                            as usize
                    } else {
                        HfstTropicalTransducerTransitionData::get_number(&jt.get_output_symbol())
                            as usize
                    };
                    symbol_transition_map
                        .entry(key)
                        .or_insert_with(TransitionSet::new)
                        .insert(&Transition::new_from_basic(jt));
                }
            }
            if !identity_found {
                this.identity_transition_vector.push(Transition::new(
                    0,
                    HfstTropicalTransducerTransitionData::get_number("@_EPSILON_SYMBOL_@") as usize,
                    HfstTropicalTransducerTransitionData::get_number("@_EPSILON_SYMBOL_@") as usize,
                    0.0,
                ));
            }
        }

        this
    }

    // ComposeIntersectFst::~ComposeIntersectFst(void) {}  (trivial)

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-final-weight-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-final-weight-fn]
    pub fn get_final_weight(&self, s: HfstState) -> f32 {
        if s as usize >= self.transition_map_vector.len() {
            crate::HFST_THROW!(StateNotDefined);
        }
        self.finality_vector[s as usize]
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbol-number-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbol-number-fn]
    pub fn get_symbol_number(&mut self, symbol: &str) -> usize {
        HfstTropicalTransducerTransitionData::get_number(symbol) as usize
    }

    pub fn get_transitions(&mut self, s: HfstState, symbol: usize) -> &TransitionSet {
        if s as usize >= self.transition_map_vector.len() {
            crate::HFST_THROW!(StateNotDefined);
        }
        // if (transition_map_vector.at(s).find(symbol) ==
        //     transition_map_vector.at(s).end())
        if self.transition_map_vector[s as usize]
            .get(&symbol)
            .is_none()
        {
            if self.is_known_symbol(symbol) || !self.has_identity_transition(s) {
                // return transition_map_vector.at(s)[symbol] = TransitionSet();
                self.transition_map_vector[s as usize].insert(symbol, TransitionSet::new());
                return &self.transition_map_vector[s as usize][&symbol];
            } else {
                let identity_transition = self.get_identity_transition(s);
                self.transition_map_vector[s as usize].insert(symbol, TransitionSet::new());
                self.transition_map_vector[s as usize]
                    .get_mut(&symbol)
                    .unwrap()
                    .insert(&Transition::new(
                        identity_transition.target,
                        symbol,
                        symbol,
                        identity_transition.weight,
                    ));
                return &self.transition_map_vector[s as usize][&symbol];
            }
        }
        // return transition_map_vector.at(s)[symbol];
        &self.transition_map_vector[s as usize][&symbol]
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.is-known-symbol-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.is-known-symbol-fn]
    pub fn is_known_symbol(&self, symbol: usize) -> bool {
        self.symbol_set.contains(&symbol)
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-identity-transition-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-identity-transition-fn]
    pub fn get_identity_transition(&mut self, s: HfstState) -> Transition {
        if s as usize >= self.transition_map_vector.len() {
            crate::HFST_THROW!(StateNotDefined);
        }
        self.identity_transition_vector[s as usize].clone()
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.has-identity-transition-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.has-identity-transition-fn]
    pub fn has_identity_transition(&mut self, s: HfstState) -> bool {
        if s as usize >= self.transition_map_vector.len() {
            crate::HFST_THROW!(StateNotDefined);
        }
        self.identity_transition_vector[s as usize].ilabel
            == HfstTropicalTransducerTransitionData::get_number("@_IDENTITY_SYMBOL_@") as usize
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbols-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbols-fn]
    pub fn get_symbols(&self) -> &SymbolSet {
        &self.symbol_set
    }
}

impl Default for ComposeIntersectFst {
    fn default() -> Self {
        Self::new()
    }
}
