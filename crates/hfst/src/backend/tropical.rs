//! The tropical backend: the Backend surface of StdVectorFst.

use super::*;

impl Backend for StdVectorFst {
    const TYPE: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;
    fn empty() -> Self {
        TropicalWeightTransducer::create_empty_transducer()
    }
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.copy-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.copy-fn]
    fn copy(&self) -> crate::error::Result<Self> {
        Ok(self.clone())
    }
    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(self, true)
    }
    fn from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(ConversionFunctions::hfst_basic_transducer_to_tropical_ofst(
            net,
        ))
    }
    fn from_basic_owned(net: HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(ConversionFunctions::basic_to_tropical_ofst_owned(net))
    }
    fn get_alphabet(&self) -> StringSet {
        TropicalWeightTransducer::get_alphabet(self)
    }
    fn is_cyclic(&self) -> bool {
        TropicalWeightTransducer::is_cyclic(self)
    }
    fn number_of_states(&self) -> u32 {
        TropicalWeightTransducer::number_of_states(self)
    }
    fn number_of_arcs(&self) -> u32 {
        TropicalWeightTransducer::number_of_arcs(self)
    }
    fn print_alphabet(&self) {
        TropicalWeightTransducer::print_alphabet(self)
    }
    fn has_weights(&self) -> bool {
        TropicalWeightTransducer::has_weights(self)
    }
    // Alphabet edits are pure SymbolTable metadata for the tropical backend — the
    // arc graph is untouched — so these override the round-trip default with an
    // in-place edit that avoids two O(states) whole-graph deep copies.
    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        TropicalWeightTransducer::insert_to_alphabet(self, symbol);
        Ok(())
    }
    fn add_symbols_to_alphabet(&mut self, symbols: &StringSet) -> crate::error::Result<()> {
        TropicalWeightTransducer::add_symbols_to_alphabet(self, symbols);
        Ok(())
    }
    fn remove_from_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        TropicalWeightTransducer::remove_from_alphabet(self, symbol);
        Ok(())
    }
    fn prune_alphabet(&mut self, force: bool) -> crate::error::Result<()> {
        TropicalWeightTransducer::prune_alphabet(self, force);
        Ok(())
    }
    // A flag encode/decode is a pure alphabet rename — the arc graph is untouched
    // — so these override the whole-graph basic round-trip with an in-place
    // SymbolTable edit that preserves each symbol's original label. Equivalent
    // automaton, but the serialized bytes diverge from the round-trip (which
    // renumbers symbols in arc-walk order) by design ([node:flag-encode-diverge]).
    fn encode_flag_diacritics(&mut self) {
        TropicalWeightTransducer::encode_flag_diacritics(self);
    }
    fn decode_flag_diacritics(&mut self) {
        TropicalWeightTransducer::decode_flag_diacritics(self);
    }
    fn to_hfst_ol(
        &self,
        weighted: bool,
        options: &str,
        harmonizer: Option<&crate::transducer::Transducer>,
    ) -> crate::error::Result<crate::transducer::Transducer> {
        match harmonizer {
            Some(h) => ConversionFunctions::tropical_ofst_to_hfst_ol(self, weighted, options, h),
            None => ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                &self.to_basic()?,
                weighted,
                options,
                None,
            ),
        }
    }
    fn write(&self, os: &mut dyn std::io::Write, hfst_format: bool) -> crate::error::Result<()> {
        TropicalWeightTransducer::write_transducer_to(self, os, hfst_format)
    }
    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        TropicalWeightTransducer::extract_paths(self, callback, cycles, None, false);
    }
    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        let t_tropical_ofst = TropicalWeightTransducer::get_flag_diacritics(self);
        TropicalWeightTransducer::extract_paths(
            self,
            callback,
            cycles,
            Some(&t_tropical_ofst),
            filter_fd,
        );
    }
}
