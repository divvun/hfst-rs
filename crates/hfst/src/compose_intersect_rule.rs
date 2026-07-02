//! Port of
//! 'libhfst/src/implementations/compose_intersect/ComposeIntersectRule.{h,cc}'.
//!
//! A 'ComposeIntersectFst' specialised for a (two-level / Xerox) rule transducer
//! of the compose-intersect machinery. It is always indexed by *input* symbol
//! (the base is constructed with 'input_keys = true') and additionally keeps the
//! rule's own alphabet as a 'StringSet' ('symbols') so that ['known_symbol'] can
//! report, for a symbol *number*, whether the corresponding symbol *string*
//! belongs to this rule's alphabet.
//!
//! 1:1 literal C++ -> Rust translation, bugs preserved.
//!
//! Structural mappings:
//! * C++ class inheritance 'ComposeIntersectRule : public ComposeIntersectFst'
//!   -> struct composition with a 'base: ComposeIntersectFst' field (per the
//!   Wave-2 port conventions); the public base methods are re-exposed by
//!   delegation so a 'ComposeIntersectRule' can be used wherever the C++ code
//!   used the inherited interface.
//! * The base constructors 'ComposeIntersectFst(t, true)' / 'ComposeIntersectFst()'
//!   map to ['ComposeIntersectFst::new_from_transducer'] / ['ComposeIntersectFst::new'].
//! * 'StringSet symbols' -> ['crate::hfst_symbol_defs::StringSet']; the C++
//!   assignment 'symbols = t.get_alphabet();' copies the alphabet, which the
//!   Rust side reproduces with '.clone()' ('HfstBasicTransducer::get_alphabet'
//!   returns '&HfstAlphabet', i.e. '&BTreeSet<String> = &StringSet').
//! * 'symbols.count(...) > 0' (count on a 'std::set', 0 or 1) -> '.contains(...)'.
//! * 'HfstTropicalTransducerTransitionData::get_symbol(hfst::size_t_to_uint(symbol))'
//!   -> the crate-visible 'get_symbol' plus an inline 'u32::try_from' narrowing.

use crate::compose_intersect_fst::{ComposeIntersectFst, SymbolSet, TransitionSet};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_symbol_defs::StringSet;

// [spec:hfst:def:compose-intersect-rule.hfst.implementations.compose-intersect-rule]
pub struct ComposeIntersectRule {
    // C++: 'class ComposeIntersectRule : public ComposeIntersectFst'.
    base: ComposeIntersectFst,
    // protected: StringSet symbols;
    symbols: StringSet,
}

impl ComposeIntersectRule {
    // [spec:hfst:def:compose-intersect-rule.hfst.implementations.compose-intersect-rule.compose-intersect-rule-fn]
    // [spec:hfst:sem:compose-intersect-rule.hfst.implementations.compose-intersect-rule.compose-intersect-rule-fn]
    //
    // ComposeIntersectRule::ComposeIntersectRule(const HfstBasicTransducer &t):
    //   ComposeIntersectFst(t,true)
    // { symbols = t.get_alphabet(); }
    pub fn new_from_transducer(t: &HfstBasicTransducer) -> Self {
        let base = ComposeIntersectFst::new_from_transducer(t, true);
        let symbols = t.get_alphabet().clone();
        ComposeIntersectRule { base, symbols }
    }

    // ComposeIntersectRule::ComposeIntersectRule(void):
    //   ComposeIntersectFst()
    // {}
    pub fn new() -> Self {
        ComposeIntersectRule {
            base: ComposeIntersectFst::new(),
            symbols: StringSet::new(),
        }
    }

    // [spec:hfst:def:compose-intersect-rule.hfst.implementations.compose-intersect-rule.known-symbol-fn]
    // [spec:hfst:sem:compose-intersect-rule.hfst.implementations.compose-intersect-rule.known-symbol-fn]
    //
    // bool ComposeIntersectRule::known_symbol(size_t symbol)
    // { return
    //     symbols.count(HfstTropicalTransducerTransitionData::get_symbol(hfst::size_t_to_uint(symbol)))
    //   > 0; }
    pub fn known_symbol(&self, symbol: usize) -> crate::error::Result<bool> {
        // 'symbol' is a number in the shared (lexicon/canonical) coding, which the
        // rule has been reindexed onto, so its own coder resolves it.
        Ok(self.symbols.contains(
            &self
                .base
                .coder()
                .get_symbol(u32::try_from(symbol).expect("value out of u32 range"))?,
        ))
    }

    // -- inherited (public) interface of ComposeIntersectFst, re-exposed by
    //    delegation since Rust has no class inheritance --

    pub fn get_transitions(
        &mut self,
        s: HfstState,
        symbol: usize,
    ) -> crate::error::Result<&TransitionSet> {
        self.base.get_transitions(s, symbol)
    }

    pub fn get_final_weight(&self, s: HfstState) -> crate::error::Result<f32> {
        self.base.get_final_weight(s)
    }

    pub fn get_symbols(&self) -> &SymbolSet {
        self.base.get_symbols()
    }
}

impl Default for ComposeIntersectRule {
    fn default() -> Self {
        Self::new()
    }
}
