//! Port of `libhfst/src/implementations/HfstBasicTransition.{h,cc}`.
//!
//! The concrete (non-template) transition class used by [`HfstBasicTransducer`].
//! Structurally identical to `HfstTransition<HfstTropicalTransducerTransitionData>`,
//! but the C++ keeps it as a separate concrete class, so it is ported as one.
//!
//! [`HfstBasicTransducer`]: crate::hfst_basic_transducer

use std::cmp::Ordering;

use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_tropical_transducer_transition_data::{
    HfstTropicalTransducerTransitionData, SymbolType, WeightType,
};

// [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition]
#[derive(Clone, Debug)]
pub struct HfstBasicTransition {
    // the state where the transition leads
    target_state: HfstState,
    // the actual transition data
    transition_data: HfstTropicalTransducerTransitionData,
}

impl HfstBasicTransition {
    pub fn new() -> Self {
        HfstBasicTransition {
            target_state: 0,
            transition_data: HfstTropicalTransducerTransitionData::new(),
        }
    }

    pub fn new_symbols(
        s: HfstState,
        isymbol: SymbolType,
        osymbol: SymbolType,
        weight: WeightType,
    ) -> Self {
        HfstBasicTransition {
            target_state: s,
            transition_data: HfstTropicalTransducerTransitionData::new_symbols(
                isymbol, osymbol, weight,
            ),
        }
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.hfst-basic-transition-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.hfst-basic-transition-fn]
    pub fn new_numbers(
        s: HfstState,
        inumber: u32,
        onumber: u32,
        weight: WeightType,
        _foo: bool,
    ) -> Self {
        HfstBasicTransition {
            target_state: s,
            transition_data: HfstTropicalTransducerTransitionData::new_numbers(
                inumber, onumber, weight,
            ),
        }
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.operator-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.operator-fn]
    pub fn operator_lt(&self, another: &HfstBasicTransition) -> bool {
        if self.target_state == another.target_state {
            return self.transition_data.operator_lt(&another.transition_data);
        }
        self.target_state < another.target_state
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-target-state-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-target-state-fn]
    pub fn get_target_state(&self) -> HfstState {
        self.target_state
    }

    pub fn get_transition_data(&self) -> &HfstTropicalTransducerTransitionData {
        &self.transition_data
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-input-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-input-symbol-fn]
    pub fn get_input_symbol(&self) -> SymbolType {
        self.transition_data.get_input_symbol()
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-input-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-input-symbol-fn]
    pub fn set_input_symbol(&mut self, symbol: &SymbolType) {
        self.transition_data.set_input_symbol(symbol);
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-output-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-output-symbol-fn]
    pub fn get_output_symbol(&self) -> SymbolType {
        self.transition_data.get_output_symbol()
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-output-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-output-symbol-fn]
    pub fn set_output_symbol(&mut self, symbol: &SymbolType) {
        self.transition_data.set_output_symbol(symbol);
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-input-number-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-input-number-fn]
    pub fn get_input_number(&self) -> u32 {
        self.transition_data.get_input_number()
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-output-number-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-output-number-fn]
    pub fn get_output_number(&self) -> u32 {
        self.transition_data.get_output_number()
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-weight-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-weight-fn]
    pub fn get_weight(&self) -> WeightType {
        self.transition_data.get_weight()
    }

    // [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-weight-fn]
    // [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-weight-fn]
    pub fn set_weight(&mut self, w: WeightType) {
        self.transition_data.set_weight(w);
    }
}

// `operator<` made usable in ordered containers.
impl PartialEq for HfstBasicTransition {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for HfstBasicTransition {}
impl PartialOrd for HfstBasicTransition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HfstBasicTransition {
    fn cmp(&self, other: &Self) -> Ordering {
        self.target_state
            .cmp(&other.target_state)
            .then(self.transition_data.cmp(&other.transition_data))
    }
}

impl Default for HfstBasicTransition {
    fn default() -> Self {
        Self::new()
    }
}
