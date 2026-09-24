//! Building a PmatchAlphabet and registering its symbols.

use super::*;

// ==================== PmatchAlphabet (impl from workflow body agent) ====================
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
impl Default for PmatchAlphabet {
    fn default() -> Self {
        Self::new()
    }
}

impl PmatchAlphabet {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.pmatch-alphabet-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.pmatch-alphabet-fn]
    // ctor from istream: PmatchAlphabet(std::istream&, SymbolNumber, PmatchContainer*)
    // Deferred: the C++ ctor reads via TransducerAlphabet(inputstream, symbol_count, true)
    // and touches hfst::FdOperation::get_feature/get_value plus fd_table mutation,
    // which is part of the istream-reading facade path.
    pub fn new_from_stream(
        inputstream: &mut dyn std::io::BufRead,
        symbol_count: SymbolNumber,
        cont: &mut PmatchCore,
    ) -> crate::error::Result<PmatchAlphabet> {
        // C++ 'PmatchAlphabet(istream, n, cont)' derives from
        // 'TransducerAlphabet(istream, n, true)' then builds the pmatch symbol
        // maps; read the base alphabet from the stream and reuse the same
        // map-building done by 'new_from_alphabet'.
        let base = TransducerAlphabet::read_from(inputstream, symbol_count, true)?;
        Ok(Self::new_from_alphabet(&base, cont))
    }

    // ctor from existing alphabet: PmatchAlphabet(TransducerAlphabet const&, PmatchContainer*)
    pub fn new_from_alphabet(a: &TransducerAlphabet, cont: &mut PmatchCore) -> PmatchAlphabet {
        let base = a.clone();
        let orig_symbol_count = base.get_orig_symbol_count();
        let mut alpha = PmatchAlphabet {
            base,
            rtns: RtnVector::new(),
            input_mark_symbol: 0,
            special_symbols: vec![NO_SYMBOL_NUMBER; SpecialSymbol::SPECIALSYMBOL_NR_ITEMS as usize],
            end_tag_map: BTreeMap::new(),
            capture_tag_map: BTreeMap::new(),
            captured_tag_map: BTreeMap::new(),
            capture2captured: SymbolNumberVector::new(),
            captured2capture: SymbolNumberVector::new(),
            rtn_names: RtnNameMap::new(),
            symbol2lists: SymbolNumberVector::new(),
            list2symbols: SymbolNumberVector::new(),
            exclusionary_lists: SymbolNumberVector::new(),
            symbol_lists: Vec::new(),
            symbol_list_members: Vec::new(),
            counters: Vec::new(),
            guards: SymbolNumberVector::new(),
            global_flags: Vec::new(),
            printable_vector: Vec::new(),
        };
        alpha.symbol2lists = vec![NO_SYMBOL_NUMBER; orig_symbol_count as usize];
        alpha.list2symbols = vec![NO_SYMBOL_NUMBER; orig_symbol_count as usize];
        alpha.capture2captured = vec![NO_SYMBOL_NUMBER; orig_symbol_count as usize];
        alpha.captured2capture = vec![NO_SYMBOL_NUMBER; orig_symbol_count as usize];
        alpha.rtns = (0..orig_symbol_count as usize).map(|_| None).collect();
        // We initialize the vector of which symbols have a printable
        // representation with false, then flip those that actually do to true
        alpha.printable_vector = vec![false; orig_symbol_count as usize];
        alpha.global_flags = vec![false; orig_symbol_count as usize];
        let mut i: SymbolNumber = 1;
        while (i as usize) < alpha.base.symbol_table.len() {
            let sym = alpha.base.symbol_table[i as usize].clone();
            if Self::is_special(&sym) {
                alpha.add_special_symbol(&sym, i, cont);
            } else if sym == "@PMATCH_INPUT_MARK@" {
                alpha.input_mark_symbol = i;
            } else if !alpha.is_flag_diacritic(i) {
                alpha.printable_vector[i as usize] = true;
            } else if Self::is_global_flag(&sym) {
                alpha.global_flags[i as usize] = true;
                // redefine it as a non-global flag, removing the
                // PMATCH_GLOBAL_ part
                let feature = crate::hfst_flag_diacritics::FdOperation::get_feature(&sym)
                    ["PMATCH_GLOBAL_".len()..]
                    .to_string();
                let value = crate::hfst_flag_diacritics::FdOperation::get_value(&sym);
                let new_diacritic = format!(
                    "{}{}{}@",
                    &sym[..3],
                    feature,
                    if value.is_empty() {
                        String::new()
                    } else {
                        format!(".{}", value)
                    }
                );
                alpha.base.fd_table.define_diacritic(i, &new_diacritic);
                // finally go over all other known flag diacritics with the
                // non-globalized feature and mark them global too
                for it in alpha.base.fd_table.get_symbols_with_feature(&feature) {
                    alpha.global_flags[it as usize] = true;
                }
            }
            i += 1;
        }
        alpha
    }

    // PmatchAlphabet(void)
    pub fn new() -> PmatchAlphabet {
        PmatchAlphabet {
            base: TransducerAlphabet::new(),
            rtns: RtnVector::new(),
            input_mark_symbol: 0,
            special_symbols: SymbolNumberVector::new(),
            end_tag_map: BTreeMap::new(),
            capture_tag_map: BTreeMap::new(),
            captured_tag_map: BTreeMap::new(),
            capture2captured: SymbolNumberVector::new(),
            captured2capture: SymbolNumberVector::new(),
            rtn_names: RtnNameMap::new(),
            symbol2lists: SymbolNumberVector::new(),
            list2symbols: SymbolNumberVector::new(),
            exclusionary_lists: SymbolNumberVector::new(),
            symbol_lists: Vec::new(),
            symbol_list_members: Vec::new(),
            counters: Vec::new(),
            guards: SymbolNumberVector::new(),
            global_flags: Vec::new(),
            printable_vector: Vec::new(),
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-symbol-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-symbol-fn]
    // override void add_symbol(const std::string &)
    pub fn add_symbol(&mut self, symbol: &crate::hfst_data_types::Symbol) {
        self.symbol2lists.push(NO_SYMBOL_NUMBER);
        self.list2symbols.push(NO_SYMBOL_NUMBER);
        self.capture2captured.push(NO_SYMBOL_NUMBER);
        self.captured2capture.push(NO_SYMBOL_NUMBER);
        self.rtns.push(None);
        self.printable_vector.push(true);
        if !self.exclusionary_lists.is_empty() {
            // if there are exclusionary lists, they should all accept the new
            // symbol
            self.symbol2lists[self.base.symbol_table.len()] =
                u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
            self.symbol_lists.push(self.exclusionary_lists.clone());
            for exc in self.exclusionary_lists.clone() {
                let idx = self.list2symbols[exc as usize] as usize;
                self.symbol_list_members[idx].push(
                    u16::try_from(self.base.symbol_table.len()).expect("value out of u16 range"),
                );
            }
        }
        self.base.add_symbol(symbol);
    }
    // convenience for the &str-taking callers (e.g. add_symbol(new_symbol) where
    // new_symbol is a char*); forwards to add_symbol.
    pub fn add_symbol_str(&mut self, symbol: &str) {
        self.add_symbol(&crate::hfst_data_types::Symbol::new(symbol))
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-special-symbol-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-special-symbol-fn]
    pub fn add_special_symbol(
        &mut self,
        str: &str,
        symbol_number: SymbolNumber,
        container: &mut PmatchCore,
    ) {
        if str == "@PMATCH_ENTRY@" {
            self.special_symbols[SpecialSymbol::entry as usize] = symbol_number;
        } else if str == "@PMATCH_EXIT@" {
            self.special_symbols[SpecialSymbol::exit as usize] = symbol_number;
        } else if str == "@PMATCH_LC_ENTRY@" {
            self.special_symbols[SpecialSymbol::LC_entry as usize] = symbol_number;
        } else if str == "@PMATCH_RC_ENTRY@" {
            self.special_symbols[SpecialSymbol::RC_entry as usize] = symbol_number;
        } else if str == "@PMATCH_LC_EXIT@" {
            self.special_symbols[SpecialSymbol::LC_exit as usize] = symbol_number;
        } else if str == "@PMATCH_RC_EXIT@" {
            self.special_symbols[SpecialSymbol::RC_exit as usize] = symbol_number;
        } else if str == "@PMATCH_NLC_ENTRY@" {
            self.special_symbols[SpecialSymbol::NLC_entry as usize] = symbol_number;
        } else if str == "@PMATCH_NRC_ENTRY@" {
            self.special_symbols[SpecialSymbol::NRC_entry as usize] = symbol_number;
        } else if str == "@PMATCH_NLC_EXIT@" {
            self.special_symbols[SpecialSymbol::NLC_exit as usize] = symbol_number;
        } else if str == "@PMATCH_NRC_EXIT@" {
            self.special_symbols[SpecialSymbol::NRC_exit as usize] = symbol_number;
        } else if str == "@PMATCH_PASSTHROUGH@" {
            self.special_symbols[SpecialSymbol::Pmatch_passthrough as usize] = symbol_number;
        } else if str == "@BOUNDARY@" {
            self.special_symbols[SpecialSymbol::boundary as usize] = symbol_number;
        } else if str == "@UNICODE_ALPHA@" {
            self.special_symbols[SpecialSymbol::UnicodeAlpha as usize] = symbol_number;
        } else if str == "@UNICODE_UPPERALPHA@" {
            self.special_symbols[SpecialSymbol::UnicodeUpperAlpha as usize] = symbol_number;
        } else if str == "@UNICODE_LOWERALPHA@" {
            self.special_symbols[SpecialSymbol::UnicodeLowerAlpha as usize] = symbol_number;
        } else if str == "@UNICODE_WHITESPACE@" {
            self.special_symbols[SpecialSymbol::UnicodeWhitespace as usize] = symbol_number;
        } else if Self::is_end_tag(str) {
            // Fetch the part between @PMATCH_ENDTAG_ and @
            // str.substr(sizeof("@PMATCH_ENDTAG_") - 1,
            //            str.size() - (sizeof("@PMATCH_ENDTAG_@") - 1))
            let begin = "@PMATCH_ENDTAG_".len();
            let count = str.len() - ("@PMATCH_ENDTAG_@".len());
            self.end_tag_map
                .insert(symbol_number, str[begin..begin + count].to_string());
        } else if Self::is_capture_tag(str) {
            let begin = "@PMATCH_CAPTURE_".len();
            let count = str.len() - ("@PMATCH_CAPTURE_@".len());
            let name_of_capture = str[begin..begin + count].to_string();
            self.capture_tag_map
                .insert(name_of_capture.clone(), symbol_number);
            if self.captured_tag_map.contains_key(&name_of_capture) {
                let captured = self.captured_tag_map[&name_of_capture];
                self.capture2captured[symbol_number as usize] = captured;
                self.captured2capture[captured as usize] = symbol_number;
            }
        } else if Self::is_captured_tag(str) {
            let begin = "@PMATCH_CAPTURED_".len();
            let count = str.len() - ("@PMATCH_CAPTURED_@".len());
            let name_of_captured = str[begin..begin + count].to_string();
            self.captured_tag_map
                .insert(name_of_captured.clone(), symbol_number);
            if self.capture_tag_map.contains_key(&name_of_captured) {
                let capture = self.capture_tag_map[&name_of_captured];
                self.captured2capture[symbol_number as usize] = capture;
                self.capture2captured[capture as usize] = symbol_number;
            }
        } else if Self::is_insertion(str) {
            self.rtn_names
                .insert(Self::name_from_insertion(str), symbol_number);
        } else if Self::is_guard(str) {
            self.guards.push(symbol_number);
        } else if Self::is_underscored_list(str) {
            self.process_underscored_symbol_list(str, symbol_number);
        } else if Self::is_list(str) {
            self.process_symbol_list(str, symbol_number, container);
        } else if Self::is_counter(str) {
            self.process_counter(str.to_string(), symbol_number);
        } else {
            self.printable_vector[symbol_number as usize] = true;
            // it's a regular symbol, we shouldn't be here!
            //        std::cerr << "pmatch: warning: symbol " << str << " was
            //        wrongly given as a special symbol\n";
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-underscored-symbol-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-underscored-symbol-list-fn]
    pub fn process_underscored_symbol_list(&mut self, str: &str, sym: SymbolNumber) {
        let mut list_symbols: SymbolNumberVector = SymbolNumberVector::new();
        let ss = self.build_string_symbol_map();
        // regular list or exlusionary list?
        let polarity = str.as_bytes()[1] == b'L';
        let mut begin = "@L.".len();
        let mut collected_symbols: Vec<crate::hfst_data_types::Symbol> = Vec::new();
        while let Some(stop) = str[begin..].find('_').map(|p| p + begin) {
            // For each underscore after the prelude, grab the substring
            let mut symbol = crate::hfst_data_types::Symbol::new(&str[begin..stop]);
            if symbol.is_empty() {
                // If the symbol _is_ an underscore it looks like we got an empty
                // string
                symbol = crate::hfst_data_types::Symbol::new_static("_");
                begin = stop + 2;
            } else {
                begin = stop + 1;
            }
            collected_symbols.push(symbol);
        }
        // Process the symbols we found
        for it in collected_symbols.iter() {
            let str_sym: SymbolNumber;
            if !ss.contains_key(it) {
                // This symbol isn't mentioned elsewhere in the alphabet
                self.add_symbol(it);
                str_sym = self.base.orig_symbol_count;
                self.base.orig_symbol_count += 1;
            } else {
                str_sym = ss[it];
            }
            list_symbols.push(str_sym);
            if polarity {
                if self.symbol2lists[str_sym as usize] == NO_SYMBOL_NUMBER {
                    self.symbol2lists[str_sym as usize] =
                        u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
                    self.symbol_lists.push(vec![sym]);
                } else {
                    let idx = self.symbol2lists[str_sym as usize] as usize;
                    self.symbol_lists[idx].push(sym);
                }
            }
        }
        self.list2symbols[sym as usize] =
            u16::try_from(self.symbol_list_members.len()).expect("value out of u16 range");
        if !polarity {
            let mut excl_symbols: SymbolNumberVector = SymbolNumberVector::new();
            self.exclusionary_lists.push(sym);
            let mut candidate_for_list: SymbolNumber = 1;
            while (candidate_for_list as usize) < self.base.symbol_table.len() {
                if Self::is_printable(&self.base.symbol_table[candidate_for_list as usize])
                    && !list_symbols.contains(&candidate_for_list)
                {
                    excl_symbols.push(candidate_for_list);
                    if self.symbol2lists[candidate_for_list as usize] == NO_SYMBOL_NUMBER {
                        // This symbol is not yet associated with any list
                        self.symbol2lists[candidate_for_list as usize] =
                            u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
                        self.symbol_lists.push(vec![sym]);
                    } else {
                        let idx = self.symbol2lists[candidate_for_list as usize] as usize;
                        self.symbol_lists[idx].push(sym);
                    }
                }
                candidate_for_list += 1;
            }
            self.symbol_list_members.push(excl_symbols);
        } else {
            self.symbol_list_members.push(list_symbols);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-symbol-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-symbol-list-fn]
    // C++ calls container->symbol_vector_from_symbols, hence the &mut container param.
    pub fn process_symbol_list(
        &mut self,
        str: &str,
        sym: SymbolNumber,
        container: &mut PmatchCore,
    ) {
        let polarity = str.as_bytes()[1] == b'L';
        let begin = "@L.".len();
        let stop = str.len() - begin - "@".len();

        let list_symbols: SymbolNumberVector =
            container.symbol_vector_from_symbols(&str[begin..begin + stop]);

        // Process the symbols we found
        for it in list_symbols.iter() {
            if polarity {
                if self.symbol2lists[*it as usize] == NO_SYMBOL_NUMBER {
                    self.symbol2lists[*it as usize] =
                        u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
                    self.symbol_lists.push(vec![sym]);
                } else {
                    let idx = self.symbol2lists[*it as usize] as usize;
                    self.symbol_lists[idx].push(sym);
                }
            }
        }
        self.list2symbols[sym as usize] =
            u16::try_from(self.symbol_list_members.len()).expect("value out of u16 range");
        if !polarity {
            let mut excl_symbols: SymbolNumberVector = SymbolNumberVector::new();
            self.exclusionary_lists.push(sym);
            let mut candidate_for_list: SymbolNumber = 1;
            while (candidate_for_list as usize) < self.base.symbol_table.len() {
                if Self::is_printable(&self.base.symbol_table[candidate_for_list as usize])
                    && !list_symbols.contains(&candidate_for_list)
                {
                    excl_symbols.push(candidate_for_list);
                    if self.symbol2lists[candidate_for_list as usize] == NO_SYMBOL_NUMBER {
                        self.symbol2lists[candidate_for_list as usize] =
                            u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
                        self.symbol_lists.push(vec![sym]);
                    } else {
                        // NOTE: faithful to the C++ bug — indexes by symbol2lists[sym]
                        // and pushes sym (not candidate_for_list).
                        let idx = self.symbol2lists[sym as usize] as usize;
                        self.symbol_lists[idx].push(sym);
                    }
                }
                candidate_for_list += 1;
            }
            self.symbol_list_members.push(excl_symbols);
        } else {
            self.symbol_list_members.push(list_symbols);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-counter-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-counter-fn]
    pub fn process_counter(&mut self, str: String, sym: SymbolNumber) {
        let _ = str;
        // Fill up non-counter spots in the counter vector with blanks
        while self.counters.len() < sym as usize {
            self.counters.push(NO_COUNTER);
        }
        self.counters.push(0);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-rtn-fn]
    pub fn add_rtn(&mut self, rtn: Box<PmatchTransducer>, name: &str) {
        // C++ 'rtn_names[name]' on std::map default-inserts 0 for an unknown
        // name; mirror that so archives carrying named transducers that TOP
        // never references still load.
        let symbol = *self.rtn_names.entry(name.to_string()).or_insert(0);
        self.rtns[symbol as usize] = Some(rtn);
    }
}
