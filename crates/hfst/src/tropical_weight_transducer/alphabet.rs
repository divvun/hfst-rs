//! Alphabet and symbol-table handling, flag renaming, and harmonization.

use std::sync::Arc;

use super::*;
// Flag encode/decode helpers shared with the Backend round-trip default.
use crate::backend::check_reserved_flag_collision;
use crate::hfst_data_types::Symbol;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_transducer::{decode_flag, encode_flag};

impl TropicalWeightTransducer {
    // ---- alphabet / symbol-table handling ----

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-to-alphabet-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-to-alphabet-fn]
    pub fn insert_to_alphabet(t: &mut StdVectorFst, symbol: &str) {
        assert!(t.input_symbols().is_some());
        let mut st = t
            .input_symbols()
            .expect("input symbols present: asserted above")
            .as_ref()
            .clone();
        st.add_symbol(symbol);
        t.set_input_symbols(Arc::new(st));
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-from-alphabet-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-from-alphabet-fn]
    pub fn remove_from_alphabet(t: &mut StdVectorFst, symbol: &str) {
        assert!(t.input_symbols().is_some());
        let old = t
            .input_symbols()
            .expect("input symbols present: asserted above")
            .as_ref()
            .clone();
        let mut st = SymbolTable::empty();
        for (label, sym) in old.iter() {
            if sym != symbol {
                // Preserve each symbol's original label (C++ 'AddSymbol(sym,
                // label)') so arc numbers stay valid; removing one entry leaves
                // a gap rather than shifting everything after it down.
                st.add_symbol_with_key(sym, label);
            }
        }
        t.set_input_symbols(Arc::new(st));
    }

    // Add every symbol in `symbols` to the alphabet SymbolTable in place. The
    // graph is untouched: this is the tropical (in-place, O(alphabet)) analog
    // of the round-trip 'net.add_symbols_to_alphabet_set' — no state/arc copy.
    pub fn add_symbols_to_alphabet(t: &mut StdVectorFst, symbols: &StringSet) {
        assert!(t.input_symbols().is_some());
        let mut st = t
            .input_symbols()
            .expect("input symbols present: asserted above")
            .as_ref()
            .clone();
        for symbol in symbols.iter() {
            st.add_symbol(symbol.as_str());
        }
        t.set_input_symbols(Arc::new(st));
    }

    // In-place O(states + arcs + alphabet) alphabet prune: drop every alphabet
    // symbol that occurs on no arc, keeping the surviving symbols at their
    // original labels (so no arc is renumbered). The tropical analog of the
    // round-trip 'net.prune_alphabet(force)' — no whole-graph deep copy.
    pub fn prune_alphabet(t: &mut StdVectorFst, force: bool) {
        assert!(t.input_symbols().is_some());

        // Numeric labels that actually occur on some transition.
        let mut used_labels: BTreeSet<u32> = BTreeSet::new();
        for s in t.states_iter() {
            for arc in t
                .get_trs(s)
                .expect("s is a valid state of this fst")
                .trs()
                .iter()
            {
                used_labels.insert(arc.ilabel);
                used_labels.insert(arc.olabel);
            }
        }

        let old = t
            .input_symbols()
            .expect("input symbols present: asserted above")
            .as_ref()
            .clone();

        // We cannot prune if unknown or identity symbols occur in transitions
        // (they stand for the rest of the alphabet), unless forced.
        let unknown_or_identity_used = old.iter().any(|(label, sym)| {
            used_labels.contains(&label) && (sym == internal_unknown || sym == internal_identity)
        });
        if !force && unknown_or_identity_used {
            return;
        }

        let mut st = SymbolTable::empty();
        for (label, sym) in old.iter() {
            // Special symbols are always known; everything else is kept only if
            // some arc uses its label.
            let keep = used_labels.contains(&label)
                || sym == internal_epsilon
                || sym == internal_unknown
                || sym == internal_identity;
            if keep {
                st.add_symbol_with_key(sym, label);
            }
        }
        t.set_input_symbols(Arc::new(st));
    }

    // In-place flag encode: rename each flag-diacritic symbol '@...@' to its
    // escaped form '%...%' in the SymbolTable, keeping every symbol at its
    // ORIGINAL label so no arc is renumbered. Pure alphabet metadata edit —
    // the arc graph is untouched. Runs the reserved-symbol collision guard
    // (identical to the basic round-trip default) before renaming. This is
    // the tropical override for the whole-graph round-trip; equivalent
    // automaton, divergent bytes by design ([node:flag-encode-diverge]).
    // [spec:hfst:def:hfst-transducer.hfst.encode-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.encode-flag-diacritics-fn]
    pub fn encode_flag_diacritics(t: &mut StdVectorFst) {
        assert!(t.input_symbols().is_some());
        let old = t
            .input_symbols()
            .expect("input symbols present: asserted above")
            .as_ref()
            .clone();
        let mut st = SymbolTable::empty();
        for (label, sym) in old.iter() {
            check_reserved_flag_collision(sym);
            let new_sym: Symbol = if FdOperation::is_diacritic(sym) {
                encode_flag(sym)
            } else {
                Symbol::from(sym)
            };
            st.add_symbol_with_key(new_sym.as_str(), label);
        }
        Self::set_both_symbol_tables(t, st);
    }

    // An HFST tropical transducer may carry an output symbol table, and when
    // it does that table is equivalent to the input one (the invariant
    // `handle_symbol_tables` relies on). The in-place flag rename replaces
    // the input table wholesale, so a surviving output table would still hold
    // the pre-rename spelling of every flag. `copy_alphabet` unions BOTH
    // tables into the interchange graph's alphabet, so that stale table
    // reintroduces the un-encoded `@...@` names beside their `%...%`
    // encodings; the matching decode then renames `%X%` back onto a name the
    // alphabet already carries and the flag arcs are lost. The C++ rebuilds
    // the whole graph and cannot drift this way — so mirror the rename onto
    // the output table whenever one is present, which is what keeps the
    // in-place shortcut equivalent to that rebuild.
    // [spec:hfst:req:flag-encode-symbol-tables.table-parity]
    fn set_both_symbol_tables(t: &mut StdVectorFst, st: SymbolTable) {
        let st = Arc::new(st);
        if t.output_symbols().is_some() {
            t.set_output_symbols(Arc::clone(&st));
        }
        t.set_input_symbols(st);
    }

    // In-place flag decode: the exact inverse of `encode_flag_diacritics`,
    // restoring '%...%' back to '@...@' at each symbol's original label.
    // [spec:hfst:def:hfst-transducer.hfst.decode-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.decode-flag-diacritics-fn]
    pub fn decode_flag_diacritics(t: &mut StdVectorFst) {
        assert!(t.input_symbols().is_some());
        let old = t
            .input_symbols()
            .expect("input symbols present: asserted above")
            .as_ref()
            .clone();
        let mut st = SymbolTable::empty();
        for (label, sym) in old.iter() {
            let decoded = decode_flag(sym);
            let new_sym: Symbol = if FdOperation::is_diacritic(&decoded) {
                decoded
            } else {
                Symbol::from(sym)
            };
            st.add_symbol_with_key(new_sym.as_str(), label);
        }
        Self::set_both_symbol_tables(t, st);
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-alphabet-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-alphabet-fn]
    pub fn get_alphabet(t: &StdVectorFst) -> StringSet {
        assert!(t.input_symbols().is_some());
        let mut s = StringSet::new();
        let st = t
            .input_symbols()
            .expect("input symbols present: asserted above");
        for (_l, sym) in st.iter() {
            s.insert(crate::hfst_data_types::Symbol::new(sym));
        }
        s
    }

    /* recursive helper */
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-input-symbols-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-input-symbols-fn]
    pub fn get_initial_input_symbols_rec(
        t: &StdVectorFst,
        s: StateId,
        visited_states: &mut BTreeSet<StateId>,
        symbols: &mut StringSet,
    ) {
        visited_states.insert(s);
        let trs: Vec<StdTransition> = t
            .get_trs(s)
            .expect("s is a valid state of this fst")
            .trs()
            .to_vec();
        for arc in &trs {
            assert!(t.input_symbols().is_some());
            let sym = t
                .input_symbols()
                .expect("input symbols present: asserted above")
                .get_symbol(arc.ilabel)
                .unwrap_or("")
                .to_string();
            assert!(!sym.is_empty());

            if !FdOperation::is_diacritic(&sym) && arc.ilabel != 0 {
                symbols.insert(crate::hfst_data_types::Symbol::from(sym));
            } else if !visited_states.contains(&arc.nextstate) {
                Self::get_initial_input_symbols_rec(t, arc.nextstate, visited_states, symbols);
            }
        }
    }

    pub fn get_initial_input_symbols(t: &StdVectorFst) -> StringSet {
        assert!(t.input_symbols().is_some());
        let mut symbols = StringSet::new();
        let s = match t.start() {
            // This can apparently happen with empty transducers (segfault in C++).
            None => return symbols,
            Some(s) => s,
        };
        let mut visited_states: BTreeSet<StateId> = BTreeSet::new();
        Self::get_initial_input_symbols_rec(t, s, &mut visited_states, &mut symbols);
        symbols
    }

    /* recursive helper */
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-first-input-symbols-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-first-input-symbols-fn]
    pub fn get_first_input_symbols_rec(
        t: &StdVectorFst,
        s: StateId,
        visited_states: &mut BTreeSet<StateId>,
        symbols: &mut StringSet,
    ) {
        visited_states.insert(s);
        let trs: Vec<StdTransition> = t
            .get_trs(s)
            .expect("s is a valid state of this fst")
            .trs()
            .to_vec();
        for arc in &trs {
            assert!(t.input_symbols().is_some());
            let sym = t
                .input_symbols()
                .expect("input symbols present: asserted above")
                .get_symbol(arc.ilabel)
                .unwrap_or("")
                .to_string();
            assert!(!sym.is_empty());

            if !FdOperation::is_diacritic(&sym) && arc.ilabel != 0 {
                symbols.insert(crate::hfst_data_types::Symbol::new(
                    t.input_symbols()
                        .expect("input symbols present: asserted above")
                        .get_symbol(arc.ilabel)
                        .unwrap_or(""),
                ));
            }
            if !visited_states.contains(&arc.nextstate) {
                Self::get_first_input_symbols_rec(t, arc.nextstate, visited_states, symbols);
            }
        }
    }

    pub fn get_first_input_symbols(t: &StdVectorFst) -> StringSet {
        assert!(t.input_symbols().is_some());
        let mut symbols = StringSet::new();
        if t.num_states() == 0 {
            return symbols;
        }
        let s = t
            .start()
            .expect("start state present: num_states checked nonzero above");
        let mut visited_states: BTreeSet<StateId> = BTreeSet::new();
        Self::get_first_input_symbols_rec(t, s, &mut visited_states, &mut symbols);
        symbols
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-number-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-number-fn]
    pub fn get_symbol_number(t: &StdVectorFst, symbol: &str) -> crate::error::Result<u32> {
        assert!(t.input_symbols().is_some());
        match t
            .input_symbols()
            .expect("input symbols present: asserted above")
            .get_label(symbol)
        {
            None => crate::bail!(SymbolNotFound),
            Some(i) => Ok(i),
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-biggest-symbol-number-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-biggest-symbol-number-fn]
    pub fn get_biggest_symbol_number(t: &StdVectorFst) -> u32 {
        let mut biggest_number = 0u32;
        for (label, _sym) in t
            .input_symbols()
            .expect("transducer has an input symbol table")
            .iter()
        {
            if label > biggest_number {
                biggest_number = label;
            }
        }
        biggest_number
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-vector-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-vector-fn]
    pub fn get_symbol_vector(t: &StdVectorFst) -> StringVector {
        let biggest_symbol_number = Self::get_biggest_symbol_number(t);
        let mut symbol_vector: StringVector =
            vec![crate::hfst_data_types::Symbol::default(); (biggest_symbol_number + 1) as usize];

        let alphabet = Self::get_alphabet(t);
        for it in alphabet.iter() {
            let symbol_number = Self::get_symbol_number(t, it).expect(
                "symbol enumerated from this transducer's own symbol table is present in it",
            );
            symbol_vector[symbol_number as usize] = it.clone();
        }
        symbol_vector
    }

    /* Find the number-to-number mappings needed to be performed to t1 so that
    it will follow the same symbol-to-number encoding as t2. */
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-mapping-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-mapping-fn]
    pub fn create_mapping(t1: &StdVectorFst, t2: &StdVectorFst) -> NumberNumberMap {
        let mut km = NumberNumberMap::new();
        let st1 = t1.input_symbols().expect("t1 has an input symbol table");
        let st2 = t2.input_symbols().expect("t2 has an input symbol table");
        for (label, sym) in st1.iter() {
            let mapped = st2
                .get_label(sym)
                .expect("symbol of t1 is also present in t2's symbol table");
            km.insert(label, mapped);
        }
        km
    }

    /* Recode the symbol numbers in this transducer as indicated in KeyMap km. */
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.recode-symbol-numbers-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.recode-symbol-numbers-fn]
    pub fn recode_symbol_numbers(t: &mut StdVectorFst, km: &mut NumberNumberMap) {
        let states: Vec<StateId> = t.states_iter().collect();
        for s in states {
            let trs = t.pop_trs(s).expect("s is a valid state of this fst");
            for arc in trs {
                // C++ 'km[label]' inserts 0 for a missing key.
                let il = *km.entry(arc.ilabel).or_insert(0);
                let ol = *km.entry(arc.olabel).or_insert(0);
                t.add_tr(s, StdTransition::new(il, ol, arc.weight, arc.nextstate))
                    .expect("transition re-added to a state of this fst");
            }
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-symbol-table-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-symbol-table-fn]
    pub fn set_symbol_table(t: &mut StdVectorFst, symbol_mappings: Vec<(u16, String)>) {
        let mut st = Self::create_symbol_table(String::new());
        for (num, sym) in &symbol_mappings {
            // NOTE: C++ 'AddSymbol(sym, num)' honours the explicit label; rustfst
            // has no add-at-explicit-label, so 'num' is ignored (gap).
            let _ = num;
            st.add_symbol(sym.as_str());
        }
        t.set_input_symbols(Arc::new(st));
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.print-alphabet-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.print-alphabet-fn]
    pub fn print_alphabet(t: &StdVectorFst) {
        let line: String = t
            .input_symbols()
            .expect("transducer has an input symbol table")
            .iter()
            .map(|(_l, sym)| format!("'{}', ", sym))
            .collect();
        tracing::debug!("{}", line);
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-flag-diacritics-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-flag-diacritics-fn]
    pub fn get_flag_diacritics(t: &StdVectorFst) -> FdTable<i64> {
        let mut table: FdTable<i64> = FdTable::new();
        let symbols = t
            .input_symbols()
            .expect("transducer has an input symbol table");
        for (label, sym) in symbols.iter() {
            if FdOperation::is_diacritic(sym) {
                table.define_diacritic(label as i64, sym);
            }
        }
        table
    }

    /* Expand "?:?", "?:x" and "x:?" transitions according to 'unknown'. */
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.expand-arcs-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.expand-arcs-fn]
    pub fn expand_arcs(
        t: &StdVectorFst,
        unknown: &mut StringSet,
        unknown_symbols_in_use: bool,
    ) -> StdVectorFst {
        let mut result = StdVectorFst::new();

        for _ in t.states_iter() {
            result.add_state();
        }

        for s in t.states_iter() {
            let result_s = s;

            if t.start() == Some(s) {
                result
                    .set_start(result_s)
                    .expect("result_s is a state created above in result");
            }
            if t.is_final(s).expect("s is a valid state of this fst") {
                let fw = *t
                    .final_weight(s)
                    .expect("s is a valid state of this fst")
                    .expect("state confirmed final via is_final")
                    .value();
                result
                    .set_final(result_s, fw)
                    .expect("result_s is a state created above in result");
            }

            let trs: Vec<StdTransition> = t
                .get_trs(s)
                .expect("s is a valid state of this fst")
                .trs()
                .to_vec();
            for arc in &trs {
                let result_nextstate = arc.nextstate;

                if unknown_symbols_in_use {
                    let is = t
                        .input_symbols()
                        .expect("transducer has an input symbol table");

                    if arc.ilabel == 1 && arc.olabel == 1 {
                        // cross-product "?:?"
                        for it1 in unknown.iter() {
                            if !FdOperation::is_diacritic(it1) {
                                let inumber = is
                                    .get_label(it1)
                                    .expect("unknown symbol must be in the symbol table");
                                for it2 in unknown.iter() {
                                    if !FdOperation::is_diacritic(it2) {
                                        let onumber = is
                                            .get_label(it2)
                                            .expect("unknown symbol must be in the symbol table");
                                        if inumber != onumber {
                                            result
                                                .add_tr(
                                                    result_s,
                                                    StdTransition::new(
                                                        inumber,
                                                        onumber,
                                                        arc.weight,
                                                        result_nextstate,
                                                    ),
                                                )
                                                .expect("transition added to result_s created above in result");
                                        }
                                    }
                                }
                                result
                                    .add_tr(
                                        result_s,
                                        StdTransition::new(
                                            inumber,
                                            1,
                                            arc.weight,
                                            result_nextstate,
                                        ),
                                    )
                                    .expect("transition added to result_s created above in result");
                                result
                                    .add_tr(
                                        result_s,
                                        StdTransition::new(
                                            1,
                                            inumber,
                                            arc.weight,
                                            result_nextstate,
                                        ),
                                    )
                                    .expect("transition added to result_s created above in result");
                            }
                        }
                    } else if arc.ilabel == 2 || arc.olabel == 2 {
                        // identity "?:?"
                        for it in unknown.iter() {
                            if !FdOperation::is_diacritic(it) {
                                let number = is
                                    .get_label(it)
                                    .expect("unknown symbol must be in the symbol table");
                                result
                                    .add_tr(
                                        result_s,
                                        StdTransition::new(
                                            number,
                                            number,
                                            arc.weight,
                                            result_nextstate,
                                        ),
                                    )
                                    .expect("transition added to result_s created above in result");
                            }
                        }
                    } else if arc.ilabel == 1 {
                        // "?:x"
                        for it in unknown.iter() {
                            if !FdOperation::is_diacritic(it) {
                                let number = is
                                    .get_label(it)
                                    .expect("unknown symbol must be in the symbol table");
                                result
                                    .add_tr(
                                        result_s,
                                        StdTransition::new(
                                            number,
                                            arc.olabel,
                                            arc.weight,
                                            result_nextstate,
                                        ),
                                    )
                                    .expect("transition added to result_s created above in result");
                            }
                        }
                    } else if arc.olabel == 1 {
                        // "x:?"
                        for it in unknown.iter() {
                            if !FdOperation::is_diacritic(it) {
                                let number = is
                                    .get_label(it)
                                    .expect("unknown symbol must be in the symbol table");
                                result
                                    .add_tr(
                                        result_s,
                                        StdTransition::new(
                                            arc.ilabel,
                                            number,
                                            arc.weight,
                                            result_nextstate,
                                        ),
                                    )
                                    .expect("transition added to result_s created above in result");
                            }
                        }
                    }
                }

                // the original transition is copied in all cases
                result
                    .add_tr(
                        result_s,
                        StdTransition::new(arc.ilabel, arc.olabel, arc.weight, result_nextstate),
                    )
                    .expect("transition added to result_s created above in result");
            }
        }

        result
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.harmonize-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.harmonize-fn]
    pub fn harmonize(
        t1: &StdVectorFst,
        t2: &StdVectorFst,
        unknown_symbols_in_use: bool,
    ) -> (StdVectorFst, StdVectorFst) {
        // NOTE: C++ takes 'StdVectorFst*' and mutates the inputs in place; the
        // skeleton hands us '&StdVectorFst', so we work on clones — the caller's
        // transducers are NOT mutated (a divergence from the C++ side effect).
        let mut t1 = t1.clone();
        let mut t2 = t2.clone();

        let debug = false;

        // 1. unknown symbols for t1 and t2
        let t1_symbols = Self::get_alphabet(&t1);
        let t2_symbols = Self::get_alphabet(&t2);
        let (mut unknown_t1, mut unknown_t2) =
            crate::hfst_symbol_defs::symbols::collect_unknown_sets(&t1_symbols, &t2_symbols);

        if debug {
            let line1: String = unknown_t1.iter().map(|it| format!("'{}', ", it)).collect();
            tracing::debug!("New symbols for t1: {}", line1);
            let line2: String = unknown_t2.iter().map(|it| format!("'{}', ", it)).collect();
            tracing::debug!("New symbols for t2: {}", line2);
        }

        // 2. add new symbols from t1 to t2's symbol table...
        let mut st2 = t2
            .input_symbols()
            .expect("t2 has an input symbol table")
            .as_ref()
            .clone();
        for it in unknown_t2.iter() {
            if st2.add_symbol(it.as_str()) < 3 {
                panic!("string {it} got an unexpected symbol number below 3");
            }
        }
        let st2_arc = Arc::new(st2);
        t2.set_input_symbols(Arc::clone(&st2_arc));

        // ...mapping needed in harmonization (t1 OLD table, t2 NEW table)...
        let mut km = Self::create_mapping(&t1, &t2);

        // ...replace t1's table with a copy of t2's table...
        t1.set_input_symbols(Arc::clone(&st2_arc));

        // ...and recode t1's symbol numbers.
        Self::recode_symbol_numbers(&mut t1, &mut km);

        // 3. expand "?:?" transitions.
        let harmonized_t1 = if !unknown_symbols_in_use {
            t1
        } else {
            let mut h = Self::expand_arcs(&t1, &mut unknown_t1, unknown_symbols_in_use);
            h.set_input_symbols(Arc::clone(
                t1.input_symbols().expect("t1 symbol table set above"),
            ));
            h
        };

        let harmonized_t2 = if !unknown_symbols_in_use {
            t2
        } else {
            let mut h = Self::expand_arcs(&t2, &mut unknown_t2, unknown_symbols_in_use);
            h.set_input_symbols(Arc::clone(
                t2.input_symbols().expect("t2 symbol table set above"),
            ));
            h
        };

        (harmonized_t1, harmonized_t2)
    }
}
