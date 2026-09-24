//! What the alphabet knows about a symbol number.

use super::*;

impl PmatchAlphabet {
    // ---- forwards to the base TransducerAlphabet (composition) ----
    pub fn get_symbol_table(&self) -> &crate::transducer::SymbolTable {
        self.base.get_symbol_table()
    }
    pub fn string_from_symbol(&self, symbol: SymbolNumber) -> crate::hfst_data_types::Symbol {
        self.base.string_from_symbol(symbol)
    }
    pub fn symbol_from_string(&self, s: &str) -> Option<SymbolNumber> {
        self.base.symbol_from_string(s)
    }
    pub fn build_string_symbol_map(&self) -> crate::transducer::StringSymbolMap {
        self.base.build_string_symbol_map()
    }
    pub fn is_flag_diacritic(&self, s: SymbolNumber) -> bool {
        self.base.is_flag_diacritic(s)
    }
    pub fn get_operation(
        &self,
        s: SymbolNumber,
    ) -> Option<&crate::hfst_flag_diacritics::FdOperation> {
        self.base.get_operation(s)
    }
    pub fn get_fd_table(&self) -> &FdTable<SymbolNumber> {
        self.base.get_fd_table()
    }
    pub fn get_unknown_symbol(&self) -> SymbolNumber {
        self.base.get_unknown_symbol()
    }
    pub fn get_default_symbol(&self) -> SymbolNumber {
        self.base.get_default_symbol()
    }
    pub fn get_identity_symbol(&self) -> SymbolNumber {
        self.base.get_identity_symbol()
    }
    pub fn get_orig_symbol_count(&self) -> SymbolNumber {
        self.base.get_orig_symbol_count()
    }

    // ---- SymbolNumber predicates (member, non-static) ----

    pub fn is_end_tag_sym(&self, symbol: SymbolNumber) -> bool {
        self.end_tag_map.contains_key(&symbol)
    }
    pub fn is_capture_tag_sym(&self, symbol: SymbolNumber) -> bool {
        self.capture2captured[symbol as usize] != NO_SYMBOL_NUMBER
    }
    pub fn is_captured_tag_sym(&self, symbol: SymbolNumber) -> bool {
        self.captured2capture[symbol as usize] != NO_SYMBOL_NUMBER
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-input-mark-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-input-mark-fn]
    pub fn is_input_mark(&self, symbol: SymbolNumber) -> bool {
        self.input_mark_symbol == symbol
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
    pub fn is_guard_sym(&self, symbol: SymbolNumber) -> bool {
        for it in self.guards.iter() {
            if symbol == *it {
                return true;
            }
        }
        false
    }
    pub fn is_counter_sym(&self, symbol: SymbolNumber) -> bool {
        (symbol as usize) < self.counters.len() && self.counters[symbol as usize] != NO_COUNTER
    }
    pub fn is_global_flag_sym(&self, symbol: SymbolNumber) -> bool {
        // 'add_symbol' does not grow 'global_flags' (C++ leaves it at
        // orig_symbol_count too); symbols added later are never global flags.
        (symbol as usize) < self.global_flags.len() && self.global_flags[symbol as usize]
    }
    pub fn is_printable_sym(&self, symbol: SymbolNumber) -> bool {
        (symbol as usize) < self.printable_vector.len() && self.printable_vector[symbol as usize]
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.end-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.end-tag-fn]
    pub fn end_tag(&self, symbol: SymbolNumber) -> String {
        if !self.end_tag_map.contains_key(&symbol) {
            String::new()
        } else {
            format!("</{}>", self.end_tag_map[&symbol])
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.start-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.start-tag-fn]
    pub fn start_tag(&self, symbol: SymbolNumber) -> String {
        if !self.end_tag_map.contains_key(&symbol) {
            String::new()
        } else {
            format!("<{}>", self.end_tag_map[&symbol])
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-meta-arc-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-meta-arc-fn]
    // override
    pub fn is_meta_arc(&self, symbol: SymbolNumber) -> bool {
        self.base.is_meta_arc(symbol)
            || symbol == self.get_special(SpecialSymbol::UnicodeAlpha)
            || symbol == self.get_special(SpecialSymbol::UnicodeUpperAlpha)
            || symbol == self.get_special(SpecialSymbol::UnicodeLowerAlpha)
            || symbol == self.get_special(SpecialSymbol::UnicodeWhitespace)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.has-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.has-rtn-fn]
    pub fn has_rtn(&self, name: &str) -> bool {
        if name == "TOP" {
            return true;
        }
        self.rtn_names.contains_key(name)
            && (self.rtn_names[name] as usize) < self.rtns.len()
            && self.rtns[self.rtn_names[name] as usize].is_some()
    }
    pub fn has_rtn_sym(&self, symbol: SymbolNumber) -> bool {
        (symbol as usize) < self.rtns.len() && self.rtns[symbol as usize].is_some()
    }

    // The C++ 'get_rtn' returned a raw mutable 'PmatchTransducer *'. An RTN is
    // load-fixed and lives inside the shared core, so it is only ever borrowed:
    // the walk resolves a symbol to a '&PmatchTransducer' itself.

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-counter-name-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-counter-name-fn]
    pub fn get_counter_name(&self, symbol: SymbolNumber) -> String {
        if self.base.symbol_table.len() <= symbol as usize {
            return "INVALID_COUNTER".to_string();
        }
        let name = self.base.symbol_table[symbol as usize].clone();
        if !Self::is_counter(&name) {
            return "INVALID_COUNTER".to_string();
        }
        let begin = "@PMATCH_COUNTER_".len();
        let count = name.len() - "@PMATCH_COUNTER_".len() - 1;
        name[begin..begin + count].to_string()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-special-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-special-fn]
    pub fn get_special(&self, special: SpecialSymbol) -> SymbolNumber {
        self.special_symbols[special as usize]
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-specials-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-specials-fn]
    pub fn get_specials(&self) -> SymbolNumberVector {
        let mut v: SymbolNumberVector = SymbolNumberVector::new();
        for it in self.special_symbols.iter() {
            if *it != NO_SYMBOL_NUMBER {
                v.push(*it);
            }
        }
        v
    }
}
