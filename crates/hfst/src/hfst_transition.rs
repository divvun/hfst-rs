//! Port of 'libhfst/src/implementations/HfstTransition.h'.
//!
//! The transition template 'HfstTransition<C>': a target state plus transition
//! data of type 'C'. The C++ template's implicit requirements on 'C' are
//! captured by the Rust ['TransitionData'] trait (a port-only construct, no spec
//! id of its own), implemented for ['HfstTropicalTransducerTransitionData'].
//!
//! 'HfstTransition<C>::get_symbol_number' calls 'C::get_symbol_number', which
//! 'HfstTropicalTransducerTransitionData' does not provide. It is ported behind
//! the port-only ['SymbolNumberData'] bound, so it stays an uninstantiable
//! (never-compiled) member for that data type, exactly as in C++.
//! 'HfstFastTransition' is likewise not ported: its data type
//! 'HfstFastTransitionData' does not exist (its include is commented out).

use std::cmp::Ordering;

use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_tropical_transducer_transition_data::HfstTropicalTransducerTransitionData;

/// The implicit C++ template requirements on the transition-data parameter 'C'.
pub trait TransitionData {
    type SymbolType;
    type WeightType;

    fn data_default() -> Self;
    fn from_symbols(
        isymbol: Self::SymbolType,
        osymbol: Self::SymbolType,
        weight: Self::WeightType,
    ) -> Self;
    fn from_numbers(inumber: u32, onumber: u32, weight: Self::WeightType) -> Self;
    fn data_get_input_symbol(&self) -> Self::SymbolType;
    fn data_get_output_symbol(&self) -> Self::SymbolType;
    fn data_get_input_number(&self) -> u32;
    fn data_get_output_number(&self) -> u32;
    fn data_get_weight(&self) -> Self::WeightType;
    fn data_set_weight(&mut self, w: f32);
    fn data_lt(&self, other: &Self) -> bool;
}

/// The C++ template member 'HfstTransition<C>::get_symbol_number' forwards to the
/// static 'C::get_symbol_number'. Only transition-data types that actually
/// provide it satisfy this bound; it is a never-instantiated member for
/// 'HfstTropicalTransducerTransitionData', which does not implement it. (A
/// port-only construct, no spec id of its own.)
pub trait SymbolNumberData: TransitionData {
    fn get_symbol_number(symbol: &Self::SymbolType) -> u32;
}

impl TransitionData for HfstTropicalTransducerTransitionData {
    type SymbolType = crate::hfst_tropical_transducer_transition_data::SymbolType;
    type WeightType = crate::hfst_tropical_transducer_transition_data::WeightType;

    fn data_default() -> Self {
        Self::new()
    }
    fn from_symbols(
        _isymbol: Self::SymbolType,
        _osymbol: Self::SymbolType,
        _weight: Self::WeightType,
    ) -> Self {
        // The generic 'HfstTransition<C>' path is never instantiated for this data
        // type (the concrete 'HfstBasicTransition' in 'hfst_basic_transition' is
        // used everywhere). Symbol interning now requires an owning graph's
        // 'SymbolCoder', which this coderless trait method cannot supply.
        unimplemented!(
            "HfstTransition<HfstTropicalTransducerTransitionData>::from_symbols is never \
             instantiated; symbol interning routes through a graph's SymbolCoder"
        )
    }
    fn from_numbers(inumber: u32, onumber: u32, weight: Self::WeightType) -> Self {
        Self::new_numbers(inumber, onumber, weight)
    }
    fn data_get_input_symbol(&self) -> Self::SymbolType {
        unimplemented!(
            "HfstTransition<HfstTropicalTransducerTransitionData>::get_input_symbol is never \
             instantiated; symbol resolution routes through a graph's SymbolCoder"
        )
    }
    fn data_get_output_symbol(&self) -> Self::SymbolType {
        unimplemented!(
            "HfstTransition<HfstTropicalTransducerTransitionData>::get_output_symbol is never \
             instantiated; symbol resolution routes through a graph's SymbolCoder"
        )
    }
    fn data_get_input_number(&self) -> u32 {
        self.get_input_number()
    }
    fn data_get_output_number(&self) -> u32 {
        self.get_output_number()
    }
    fn data_get_weight(&self) -> Self::WeightType {
        self.get_weight()
    }
    fn data_set_weight(&mut self, w: f32) {
        self.set_weight(w);
    }
    fn data_lt(&self, other: &Self) -> bool {
        self.operator_lt(other)
    }
}

// [spec:hfst:def:hfst-transition.hfst.implementations.hfst-transition]
// The C++ destructor '~HfstTransition() {}' is empty; dropping the fields is
// the faithful equivalent.
// [spec:hfst:def:hfst-transition.hfst.implementations.hfst-transition-fn]
// [spec:hfst:sem:hfst-transition.hfst.implementations.hfst-transition-fn]
#[derive(Clone, Debug)]
pub struct HfstTransition<C: TransitionData> {
    // the state where the transition leads
    target_state: HfstState,
    // the actual transition data
    transition_data: C,
}

impl<C: TransitionData> Default for HfstTransition<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: TransitionData> HfstTransition<C> {
    /* Get the number that represents the symbol in the transition data. */
    // [spec:hfst:def:hfst-transition.hfst.implementations.hfst-transition.get-symbol-number-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.hfst-transition.get-symbol-number-fn]
    #[allow(dead_code)]
    fn get_symbol_number(symbol: &C::SymbolType) -> u32
    where
        C: SymbolNumberData,
    {
        C::get_symbol_number(symbol)
    }

    pub fn new() -> Self {
        HfstTransition {
            target_state: 0,
            transition_data: C::data_default(),
        }
    }

    pub fn new_symbols(
        s: HfstState,
        isymbol: C::SymbolType,
        osymbol: C::SymbolType,
        weight: C::WeightType,
    ) -> Self {
        HfstTransition {
            target_state: s,
            transition_data: C::from_symbols(isymbol, osymbol, weight),
        }
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.hfst-transition.hfst-transition-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.hfst-transition.hfst-transition-fn]
    pub fn new_numbers(
        s: HfstState,
        inumber: u32,
        onumber: u32,
        weight: C::WeightType,
        _foo: bool,
    ) -> Self {
        HfstTransition {
            target_state: s,
            transition_data: C::from_numbers(inumber, onumber, weight),
        }
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.operator-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.operator-fn]
    pub fn operator_lt(&self, another: &HfstTransition<C>) -> bool {
        if self.target_state == another.target_state {
            return self.transition_data.data_lt(&another.transition_data);
        }
        self.target_state < another.target_state
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.get-target-state-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.get-target-state-fn]
    pub fn get_target_state(&self) -> HfstState {
        self.target_state
    }

    pub fn get_transition_data(&self) -> &C {
        &self.transition_data
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.typename-c.symbol-type-get-input-symbol-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.typename-c.symbol-type-get-input-symbol-fn]
    pub fn get_input_symbol(&self) -> C::SymbolType {
        self.transition_data.data_get_input_symbol()
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.typename-c.symbol-type-get-output-symbol-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.typename-c.symbol-type-get-output-symbol-fn]
    pub fn get_output_symbol(&self) -> C::SymbolType {
        self.transition_data.data_get_output_symbol()
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.get-input-number-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.get-input-number-fn]
    pub fn get_input_number(&self) -> u32 {
        self.transition_data.data_get_input_number()
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.get-output-number-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.get-output-number-fn]
    pub fn get_output_number(&self) -> u32 {
        self.transition_data.data_get_output_number()
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.typename-c.weight-type-get-weight-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.typename-c.weight-type-get-weight-fn]
    pub fn get_weight(&self) -> C::WeightType {
        self.transition_data.data_get_weight()
    }

    // [spec:hfst:def:hfst-transition.hfst.implementations.set-weight-fn]
    // [spec:hfst:sem:hfst-transition.hfst.implementations.set-weight-fn]
    pub fn set_weight(&mut self, w: f32) {
        self.transition_data.data_set_weight(w);
    }
}

// 'operator<' ('bool operator<') made usable in ordered containers. Requires the
// data to be ordered.
impl<C: TransitionData + Ord> PartialEq for HfstTransition<C> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl<C: TransitionData + Ord> Eq for HfstTransition<C> {}
impl<C: TransitionData + Ord> PartialOrd for HfstTransition<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<C: TransitionData + Ord> Ord for HfstTransition<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.target_state
            .cmp(&other.target_state)
            .then(self.transition_data.cmp(&other.transition_data))
    }
}

/// \brief An HfstTransition with transition data of type
/// HfstTropicalTransducerTransitionData. Compatible with HfstBasicTransducer.
// [spec:hfst:def:hfst-transition.hfst.hfst-basic-transition]
pub type HfstBasicTransition = HfstTransition<HfstTropicalTransducerTransitionData>;
