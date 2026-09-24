//! The alphabet, the symbol coding, and symbol queries and flag purges.

use super::*;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::{StringPair, StringPairSet, StringSet};

impl HfstBasicTransducer {
    /* Print the alphabet of the graph to the standard error stream. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-alphabet-fn]
    pub fn print_alphabet(&self) {
        let line = self
            .alphabet
            .iter()
            .map(HfstSymbol::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        tracing::debug!("{}", line);
    }

    /* Get the number of the 'symbol'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-symbol-number-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-symbol-number-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-symbol-number-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-symbol-number-fn]
    pub fn get_symbol_number(&mut self, symbol: &HfstSymbol) -> u32 {
        self.coder.get_number(symbol)
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbol-to-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbol-to-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbol-to-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbol-to-alphabet-fn]
    pub fn add_symbol_to_alphabet(&mut self, symbol: &HfstSymbol) {
        self.alphabet.insert(symbol.clone());
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbol-from-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbol-from-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbol-from-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbol-from-alphabet-fn]
    pub fn remove_symbol_from_alphabet(&mut self, symbol: &HfstSymbol) {
        self.alphabet.remove(symbol);
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbols-from-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbols-from-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbols-from-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbols-from-alphabet-fn]
    pub fn remove_symbols_from_alphabet(&mut self, symbols: &HfstSymbolSet) {
        for symbol in symbols.iter() {
            self.alphabet.remove(symbol);
        }
    }

    pub fn add_symbols_to_alphabet_set(&mut self, symbols: &HfstSymbolSet) {
        for symbol in symbols.iter() {
            self.alphabet.insert(symbol.clone());
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbols-to-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbols-to-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbols-to-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbols-to-alphabet-fn]
    pub fn add_symbols_to_alphabet_pair_set(&mut self, symbols: &HfstSymbolPairSet) {
        for symbol in symbols.iter() {
            self.alphabet.insert(symbol.0.clone());
            self.alphabet.insert(symbol.1.clone());
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-after-substitution-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-after-substitution-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-after-substitution-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-after-substitution-fn]
    pub fn prune_alphabet_after_substitution(&mut self, symbols: &BTreeSet<u32>) {
        if symbols.is_empty() {
            return;
        }

        let mut symbols_found: Vec<bool> = Vec::new();
        symbols_found.resize((self.coder.get_max_number() + 1) as usize, false);

        // Go through all transitions
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                symbols_found[data.get_input_number() as usize] = true;
                symbols_found[data.get_output_number() as usize] = true;
            }
        }

        // Remove symbols in 'symbols' from the alphabet if they did not occur.
        for &symbol in symbols.iter() {
            if !symbols_found[symbol as usize] {
                self.alphabet.remove(
                    &self
                        .coder
                        .get_symbol(symbol)
                        .expect("symbols are numbers drawn from this transducer's own coder"),
                );
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.symbols-used-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.symbols-used-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.symbols-used-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.symbols-used-fn]
    pub fn symbols_used(&self) -> HfstAlphabet {
        let mut retval = HfstAlphabet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(data.get_input_symbol(&self.coder));
                retval.insert(data.get_output_symbol(&self.coder));
            }
        }
        retval
    }

    /// The set of input symbols occurring on this graph's transitions — the
    /// input-only sibling of [`Self::symbols_used`]. Used by alphabet-compatibility
    /// diagnostics (e.g. hfst-compose-intersect checks whether a rule's input
    /// alphabet covers a lexicon's output symbols).
    pub fn input_symbols_used(&self) -> HfstAlphabet {
        let mut retval = HfstAlphabet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                retval.insert(tr_it.get_transition_data().get_input_symbol(&self.coder));
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-fn]
    pub fn prune_alphabet(&mut self, force: bool) {
        // Which symbols occur in the graph
        let mut symbols_found = self.symbols_used();

        // Whether unknown or identity symbols are used
        let unknowns_or_identities_used = symbols_found.contains("@_UNKNOWN_SYMBOL_@")
            || symbols_found.contains("@_IDENTITY_SYMBOL_@");

        // We cannot prune if unknowns or identities are used in its transitions.
        if !force && unknowns_or_identities_used {
            return;
        }

        // Special symbols are always known
        symbols_found.insert(HfstSymbol::new_static("@_EPSILON_SYMBOL_@"));
        symbols_found.insert(HfstSymbol::new_static("@_UNKNOWN_SYMBOL_@"));
        symbols_found.insert(HfstSymbol::new_static("@_IDENTITY_SYMBOL_@"));

        // Which symbols in the graph's alphabet did not occur in the graph
        let mut symbols_not_found = HfstAlphabet::new();

        for it in self.alphabet.iter() {
            if !symbols_found.contains(it) {
                symbols_not_found.insert(it.clone());
            }
        }

        // Remove the symbols that did not occur from the alphabet
        for it in symbols_not_found.iter() {
            self.alphabet.remove(it);
        }
    }

    pub fn get_alphabet(&self) -> &HfstAlphabet {
        &self.alphabet
    }

    /// This graph's own symbol<->number coding (idiom5 keystone). All tropical
    /// symbol resolution (number->string) and interning (string->number) for
    /// this graph's arcs goes through it; binary ops harmonize two graphs'
    /// codings via [`SymbolCoder::create_translator_from`].
    pub fn coder(&self) -> &SymbolCoder {
        &self.coder
    }

    pub fn coder_mut(&mut self) -> &mut SymbolCoder {
        &mut self.coder
    }

    /// Intern this graph's coder symbols *and* its full alphabet into the shared
    /// `canonical` coder, without changing this graph. Call this for every graph
    /// participating in a binary op BEFORE [`Self::reindex_into`] so that
    /// `canonical` already holds a number for every symbol any of them uses; that
    /// makes the per-graph numbering agree even for alphabet-only symbols (which a
    /// graph's own coder may lack until interned).
    pub fn intern_into(&self, canonical: &mut SymbolCoder) {
        for symbol in self.coder.number2symbol_slice() {
            if !symbol.is_empty() {
                canonical.get_number(symbol);
            }
        }
        for symbol in self.alphabet.iter() {
            if !symbol.is_empty() {
                canonical.get_number(symbol);
            }
        }
    }

    /// Re-number every arc so its symbols are coded by the shared `canonical`
    /// coder, then adopt a clone of `canonical` as this graph's own coding. Pair
    /// with [`Self::intern_into`]: intern *all* participating graphs into one
    /// `canonical` first, then `reindex_into` each. After that they all share one
    /// numbering, so their symbol numbers can be combined directly — the
    /// per-graph-coder replacement for the former process-global numbering,
    /// applied ONCE at a binary-op boundary.
    pub fn reindex_into(&mut self, canonical: &mut SymbolCoder) {
        // translator[old_number] = number of the same symbol in the shared coding.
        let translator = canonical.create_translator_from(&self.coder);
        for transitions in self.state_vector.iter_mut() {
            for tr in transitions.iter_mut() {
                let new_in = translator[tr.get_input_number() as usize];
                let new_out = translator[tr.get_output_number() as usize];
                let target = tr.get_target_state();
                let weight = tr.get_weight();
                *tr = HfstBasicTransition::new_numbers(target, new_in, new_out, weight, false);
            }
        }
        self.coder = canonical.clone();
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-transition-pairs-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-transition-pairs-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-transition-pairs-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-transition-pairs-fn]
    pub fn get_transition_pairs(&self) -> StringPairSet {
        let mut retval = StringPairSet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(StringPair::from((
                    data.get_input_symbol(&self.coder),
                    data.get_output_symbol(&self.coder),
                )));
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-input-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-input-symbols-fn]
    pub fn get_input_symbols(&self) -> StringSet {
        let mut retval = StringSet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(data.get_input_symbol(&self.coder));
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-output-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-output-symbols-fn]
    pub fn get_output_symbols(&self) -> StringSet {
        let mut retval = StringSet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(data.get_output_symbol(&self.coder));
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-special-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-special-symbol-fn]
    // [spec:hfst:def:hfst-transition-graph.is-special-symbol-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-special-symbol-fn]
    pub fn is_special_symbol(symbol: &str) -> bool {
        if symbol.len() < 2 {
            return false;
        }
        let bytes = symbol.as_bytes();
        if bytes[0] == b'@' && bytes[1] == b'_' {
            return true;
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-flags-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-flags-fn]
    // [spec:hfst:def:hfst-transition-graph.get-flags-fn]
    // [spec:hfst:sem:hfst-transition-graph.get-flags-fn]
    pub fn get_flags(&self) -> StringSet {
        let mut flags = StringSet::new();
        for it in self.alphabet.iter() {
            if FdOperation::is_diacritic(it) {
                flags.insert(it.clone());
            }
        }
        flags
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.purge-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.purge-symbol-fn]
    // [spec:hfst:def:hfst-transition-graph.purge-symbol-fn]
    // [spec:hfst:sem:hfst-transition-graph.purge-symbol-fn]
    pub fn purge_symbol(symbol: &str, flag: &str) -> bool {
        if !FdOperation::is_diacritic(symbol) {
            return false;
        }
        if flag.is_empty() || FdOperation::get_feature(symbol) == flag {
            return true;
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.flag-purge-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.flag-purge-fn]
    // [spec:hfst:def:hfst-transition-graph.flag-purge-fn]
    // [spec:hfst:sem:hfst-transition-graph.flag-purge-fn]
    pub fn flag_purge(&mut self, flag: &str) {
        // (1) Go through all states and transitions
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let isym = self.state_vector[s][i].get_input_symbol(&self.coder);
                let osym = self.state_vector[s][i].get_output_symbol(&self.coder);
                if Self::purge_symbol(&isym, flag) || Self::purge_symbol(&osym, flag) {
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        HfstSymbol::new_static("@_EPSILON_SYMBOL_@"),
                        HfstSymbol::new_static("@_EPSILON_SYMBOL_@"),
                        weight,
                        self.coder_mut(),
                    );
                    self.state_vector[s][i] = tr;
                }
            }
        }
        // (2) Go through the alphabet
        let mut extra_symbols = StringSet::new();
        for it in self.alphabet.iter() {
            if Self::purge_symbol(it, flag) {
                extra_symbols.insert(it.clone());
            }
        }
        self.remove_symbols_from_alphabet(&extra_symbols);
    }
}
