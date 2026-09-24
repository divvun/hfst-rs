//! The tropical backend's FST algebra.

use super::*;

impl AlgebraBackend for StdVectorFst {
    const SUPPORTS_VIRTUAL_FLAG_COMPOSE: bool = true;
    const SUPPORTS_COMPOSE_LOOKAHEAD: bool = true;
    const SUPPORTS_VIRTUAL_FLAG_INTERSECTION: bool = true;
    const SUPPORTS_VIRTUAL_FLAG_SUBTRACTION: bool = true;

    fn remove_epsilons(&self) -> crate::error::Result<Self> {
        TropicalWeightTransducer::remove_epsilons(self)
    }
    fn determinize(self, encode_weights: bool) -> crate::error::Result<Self> {
        TropicalWeightTransducer::determinize(self, encode_weights)
    }
    fn minimize(self, encode_weights: bool) -> crate::error::Result<Self> {
        TropicalWeightTransducer::minimize(self, encode_weights)
    }
    fn repeat_star(&self) -> Self {
        TropicalWeightTransducer::repeat_star(self)
    }
    fn repeat_plus(&self) -> Self {
        TropicalWeightTransducer::repeat_plus(self)
    }
    fn repeat_n(&self, n: u32) -> crate::error::Result<Self> {
        TropicalWeightTransducer::repeat_n(self, n)
    }
    fn repeat_le_n(&self, n: u32) -> crate::error::Result<Self> {
        TropicalWeightTransducer::repeat_le_n(self, n)
    }
    fn optionalize(&self) -> crate::error::Result<Self> {
        TropicalWeightTransducer::optionalize(self)
    }
    fn invert(&self) -> Self {
        TropicalWeightTransducer::invert(self)
    }
    fn reverse(&self) -> crate::error::Result<Self> {
        TropicalWeightTransducer::reverse(self)
    }
    fn extract_input_language(&self) -> Self {
        TropicalWeightTransducer::extract_input_language(self)
    }
    fn extract_output_language(&self) -> Self {
        TropicalWeightTransducer::extract_output_language(self)
    }

    fn concatenate(&self, another: &Self) -> crate::error::Result<Self> {
        TropicalWeightTransducer::concatenate(self, another)
    }
    fn disjunct(&self, another: &Self) -> crate::error::Result<Self> {
        TropicalWeightTransducer::disjunct(self, another)
    }
    fn intersect(&self, another: &Self) -> crate::error::Result<Self> {
        TropicalWeightTransducer::intersect(self, another)
    }
    fn subtract(&self, another: &Self) -> crate::error::Result<Self> {
        TropicalWeightTransducer::subtract(self, another)
    }
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-fn]
    fn compose(&self, another: &Self) -> crate::error::Result<Self> {
        TropicalWeightTransducer::try_compose_owned(self.clone(), another.clone(), None, None)
    }
    fn try_flag_operation_owned(
        self,
        another: Self,
        operation: FlagDiacriticOperation,
        flag_overlay: Option<&FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<Self> {
        match operation {
            FlagDiacriticOperation::Compose => TropicalWeightTransducer::try_compose_owned(
                self,
                another,
                flag_overlay,
                memory_limit_bytes,
            ),
            FlagDiacriticOperation::ComposeFlagsAsEpsilon => {
                TropicalWeightTransducer::try_compose_mode(
                    self,
                    another,
                    flag_overlay,
                    memory_limit_bytes,
                    true,
                )
            }
            FlagDiacriticOperation::Intersect => TropicalWeightTransducer::try_intersect_owned(
                self,
                another,
                flag_overlay,
                memory_limit_bytes,
            ),
            FlagDiacriticOperation::Subtract => TropicalWeightTransducer::try_subtract_owned(
                self,
                another,
                flag_overlay,
                memory_limit_bytes,
            ),
        }
    }

    fn try_compose_lookahead_owned(
        self,
        another: Self,
        flag_overlay: Option<&FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<Self> {
        TropicalWeightTransducer::try_compose_lookahead_owned(
            self,
            another,
            flag_overlay,
            memory_limit_bytes,
        )
    }

    fn define_transducer_spv(spv: &StringPairVector) -> Self {
        TropicalWeightTransducer::define_transducer_spv(spv)
    }
    fn define_transducer_sps(sps: &StringPairSet, cyclic: bool) -> Self {
        TropicalWeightTransducer::define_transducer_sps(sps, cyclic)
    }
    fn define_transducer_spsv(spsv: &[StringPairSet]) -> Self {
        TropicalWeightTransducer::define_transducer_spsv(spsv)
    }
    fn define_transducer_symbol(symbol: &str) -> Self {
        TropicalWeightTransducer::define_transducer_symbol(symbol)
    }
    fn define_transducer_symbol_pair(isymbol: &str, osymbol: &str) -> Self {
        TropicalWeightTransducer::define_transducer_symbol_pair(isymbol, osymbol)
    }

    fn are_equivalent(&self, another: &Self, encode_weights: bool) -> crate::error::Result<bool> {
        TropicalWeightTransducer::are_equivalent(self, another, encode_weights)
    }
    fn is_automaton(&self) -> bool {
        TropicalWeightTransducer::is_automaton(self)
    }
    fn get_initial_input_symbols(&self) -> StringSet {
        TropicalWeightTransducer::get_initial_input_symbols(self)
    }
    fn get_first_input_symbols(&self) -> StringSet {
        TropicalWeightTransducer::get_first_input_symbols(self)
    }

    fn n_best(&self, n: u32) -> crate::error::Result<Self> {
        TropicalWeightTransducer::n_best(self, n)
    }
    fn extract_random_paths(&self, results: &mut HfstTwoLevelPaths, max_num: i32) {
        TropicalWeightTransducer::extract_random_paths(self, results, max_num);
    }
    fn set_final_weights(&self, weight: f32, increment: bool) -> Self {
        TropicalWeightTransducer::set_final_weights(self, weight, increment)
    }
    fn push_labels(&self, to_initial_state: bool) -> crate::error::Result<Self> {
        TropicalWeightTransducer::push_labels(self, to_initial_state)
    }
    fn push_weights(&self, to_initial_state: bool) -> crate::error::Result<Self> {
        TropicalWeightTransducer::push_weights(self, to_initial_state)
    }
    fn transform_weights(&self, func: fn(f32) -> f32) -> Self {
        TropicalWeightTransducer::transform_weights(self, func)
    }

    fn substitute_symbol_fast(&self, _old_symbol: &str, _new_symbol: &str) -> Option<Self> {
        // do not use until substituted symbols are correctly erased from the
        // alphabet
        // (tropical fast path is dead code: 'if false && ...'.)
        None
    }
    fn substitute_string_transducer(&self, old_symbol_pair: StringPair, transducer: &Self) -> Self {
        TropicalWeightTransducer::substitute_string_transducer(self, old_symbol_pair, transducer)
    }
    fn disjunct_spv(&mut self, spv: &StringPairVector) {
        TropicalWeightTransducer::disjunct_spv(self, spv);
    }
}
