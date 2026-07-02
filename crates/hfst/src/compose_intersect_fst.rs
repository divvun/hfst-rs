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
//! * 'HFST_THROW(StateNotDefined)' -> 'crate::bail!(StateNotDefined)'; the
//!   'StateNotDefined' child exception (declared/defined in the '.cc') is
//!   reproduced here with the same shape as 'hfst_exception_child!'.

use std::collections::{BTreeMap, BTreeSet};

use crate::compose_intersect_utilities::{Comparator, SpaceSavingSet};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_tropical_transducer_transition_data::SymbolCoder;

// The compose-intersect `StateNotDefined` signal (a C++ `HfstException` child)
// is now `crate::error::ErrorKind::StateNotDefined`; `crate::bail!(StateNotDefined)`
// raises it via the unified `Error` payload.
// [spec:hfst:def:compose-intersect-fst.state-not-defined]

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
    pub fn new_from_basic(t: &HfstBasicTransition, coder: &mut SymbolCoder) -> Self {
        let isym = t.get_transition_data().get_input_symbol(coder);
        let osym = t.get_transition_data().get_output_symbol(coder);
        let transition = Transition {
            ilabel: coder.get_number(&isym) as usize,
            olabel: coder.get_number(&osym) as usize,
            weight: t.get_weight(),
            target: t.get_target_state(),
        };
        assert!(isym != "");
        assert!(osym != "");
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
}

// 'bool operator==(const Transition&) const' made usable by 'SpaceSavingSet'
// (which compares elements with '==').
// [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.operator-fn]
// [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.operator-fn]
impl PartialEq for Transition {
    fn eq(&self, other: &Self) -> bool {
        self.ilabel == other.ilabel
            && self.olabel == other.olabel
            && self.weight == other.weight
            && self.target == other.target
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
            let n = this.t.coder_mut().get_number(it) as usize;
            this.symbol_set.insert(n);
        }

        let mut source_state: u32 = 0;
        // for (HfstBasicTransducer::const_iterator it = this->t.begin(); ...)
        let states: Vec<crate::hfst_basic_transducer::HfstBasicTransitions> =
            this.t.iter().cloned().collect();
        for it in states.iter() {
            this.transition_map_vector.push(SymbolTransitionMap::new());
            if this.t.is_final_state(source_state) {
                this.finality_vector.push(
                    this.t
                        .get_final_weight(source_state)
                        .expect("state was confirmed final via is_final_state"),
                );
            } else {
                this.finality_vector.push(f32::INFINITY);
            }
            source_state += 1;
            let mut identity_found = false;
            for jt in it.iter() {
                let jt_isym = jt.get_input_symbol(this.t.coder());
                if jt_isym == "@_IDENTITY_SYMBOL_@" {
                    identity_found = true;
                    let tr = Transition::new_from_basic(jt, this.t.coder_mut());
                    this.identity_transition_vector.push(tr);
                } else {
                    let key = if input_keys {
                        this.t.coder_mut().get_number(&jt_isym) as usize
                    } else {
                        let jt_osym = jt.get_output_symbol(this.t.coder());
                        this.t.coder_mut().get_number(&jt_osym) as usize
                    };
                    let tr = Transition::new_from_basic(jt, this.t.coder_mut());
                    this.transition_map_vector
                        .last_mut()
                        .unwrap()
                        .entry(key)
                        .or_insert_with(TransitionSet::new)
                        .insert(&tr);
                }
            }
            if !identity_found {
                let eps = this.t.coder_mut().get_number("@_EPSILON_SYMBOL_@") as usize;
                this.identity_transition_vector
                    .push(Transition::new(0, eps, eps, 0.0));
            }
        }

        this
    }

    // ComposeIntersectFst::~ComposeIntersectFst(void) {}  (trivial)

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-final-weight-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-final-weight-fn]
    pub fn get_final_weight(&self, s: HfstState) -> crate::error::Result<f32> {
        if s as usize >= self.transition_map_vector.len() {
            crate::bail!(StateNotDefined);
        }
        Ok(self.finality_vector[s as usize])
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbol-number-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbol-number-fn]
    pub fn get_symbol_number(&mut self, symbol: &str) -> usize {
        self.t.coder_mut().get_number(symbol) as usize
    }

    pub fn get_transitions(
        &mut self,
        s: HfstState,
        symbol: usize,
    ) -> crate::error::Result<&TransitionSet> {
        if s as usize >= self.transition_map_vector.len() {
            crate::bail!(StateNotDefined);
        }
        // if (transition_map_vector.at(s).find(symbol) ==
        //     transition_map_vector.at(s).end())
        if self.transition_map_vector[s as usize]
            .get(&symbol)
            .is_none()
        {
            if self.is_known_symbol(symbol) || !self.has_identity_transition(s)? {
                // return transition_map_vector.at(s)[symbol] = TransitionSet();
                self.transition_map_vector[s as usize].insert(symbol, TransitionSet::new());
                return Ok(&self.transition_map_vector[s as usize][&symbol]);
            } else {
                let identity_transition = self.get_identity_transition(s)?;
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
                return Ok(&self.transition_map_vector[s as usize][&symbol]);
            }
        }
        // return transition_map_vector.at(s)[symbol];
        Ok(&self.transition_map_vector[s as usize][&symbol])
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.is-known-symbol-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.is-known-symbol-fn]
    pub fn is_known_symbol(&self, symbol: usize) -> bool {
        self.symbol_set.contains(&symbol)
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-identity-transition-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-identity-transition-fn]
    pub fn get_identity_transition(&mut self, s: HfstState) -> crate::error::Result<Transition> {
        if s as usize >= self.transition_map_vector.len() {
            crate::bail!(StateNotDefined);
        }
        Ok(self.identity_transition_vector[s as usize].clone())
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.has-identity-transition-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.has-identity-transition-fn]
    pub fn has_identity_transition(&mut self, s: HfstState) -> crate::error::Result<bool> {
        if s as usize >= self.transition_map_vector.len() {
            crate::bail!(StateNotDefined);
        }
        Ok(self.identity_transition_vector[s as usize].ilabel
            == self.t.coder_mut().get_number("@_IDENTITY_SYMBOL_@") as usize)
    }

    // [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbols-fn]
    // [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbols-fn]
    pub fn get_symbols(&self) -> &SymbolSet {
        &self.symbol_set
    }

    /// This automaton's underlying graph's symbol coding. The compose-intersect
    /// machinery harmonizes the lexicon's and rules' codings against each other
    /// before combining their labels (the per-graph-coder replacement for the
    /// former process-global numbering).
    pub fn coder(&self) -> &SymbolCoder {
        self.t.coder()
    }

    pub fn coder_mut(&mut self) -> &mut SymbolCoder {
        self.t.coder_mut()
    }
}

impl Default for ComposeIntersectFst {
    fn default() -> Self {
        Self::new()
    }
}
