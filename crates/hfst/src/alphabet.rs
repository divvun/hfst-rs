//! ABSOLUTE-faithful C++->Rust port of the twolc parser-support 'Alphabet'
//! class from 'libhfst/src/parsers/alphabet_src/Alphabet.{h,cc}'.
//!
//! This is the alphabet of a twolc-grammar (used by 'LexcCompiler' and the
//! pmatch path via the 'OtherSymbolTransducer'/'XreCompiler' facade), NOT the
//! out-of-scope 'SfstAlphabet'.
//!
//! # Conventions
//!
//! 'HandySet<V>' -> 'BTreeSet<V>' and 'HandyMap<K,V>' -> 'BTreeMap<K,V>'
//! ('has_key'/'has_element' -> 'contains_key'/'contains'). 'std::pair<A,B>' ->
//! '(A,B)'. The supporting typedefs ('SymbolPair'/'SymbolRange'/
//! 'SymbolPairVector') and the 'OtherSymbolTransducer' facade are reused from
//! 'crate::twolc'. The C++ 'std::cerr' diacritic warning becomes 'eprintln!'.
//! The '#ifdef TEST_ALPHABET main()' driver is omitted (out of scope).

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use crate::twolc::{
    OtherSymbolTransducer, SymbolPair, SymbolPairVector, SymbolRange, TWOLC_EPSILON, TWOLC_UNKNOWN,
};

// [spec:hfst:def:alphabet.alphabet]
#[derive(Default)]
pub struct Alphabet {
    pub(crate) alphabet_set: BTreeSet<SymbolPair>,
    pub(crate) input_symbols: BTreeSet<String>,
    pub(crate) output_symbols: BTreeSet<String>,
    pub(crate) diacritics: BTreeSet<String>,
    pub(crate) alphabet: BTreeMap<SymbolPair, OtherSymbolTransducer>,
    pub(crate) sets: BTreeMap<String, SymbolRange>,
}

impl Alphabet {
    // ----- protected -----

    // 'const OtherSymbolTransducer &Alphabet::compute(const SymbolPair &pair)'.
    // (No [spec] annotation in the C++ source.)
    fn compute(&mut self, pair: &SymbolPair) -> &OtherSymbolTransducer {
        if !self.sets.contains_key(&pair.0) {
            self.define_singleton_set(&pair.0);
        }
        if !self.sets.contains_key(&pair.1) {
            self.define_singleton_set(&pair.1);
        }

        let input = pair.0.clone();
        let output = pair.1.clone();

        let mut pair_transducer = OtherSymbolTransducer::new();

        if self.diacritics.contains(&input) {
            pair_transducer.disjunct(&OtherSymbolTransducer::new_pair(&input, &input));
            if input != output && output != TWOLC_EPSILON && output != TWOLC_UNKNOWN {
                eprintln!(
                    "Warning: Diacritic {} in pair {}:{} will correspond 0.",
                    input, input, output
                );
            }
        } else if input == TWOLC_UNKNOWN && output == TWOLC_UNKNOWN {
            let alphabet_set = self.alphabet_set.clone();
            for it in alphabet_set.iter() {
                if self.is_set_pair(it) {
                    continue;
                }

                pair_transducer.disjunct(&OtherSymbolTransducer::new_pair(&it.0, &it.1));
            }
            pair_transducer.disjunct(&OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN));
        } else if input == TWOLC_UNKNOWN {
            self.output_symbols.insert(pair.1.clone());
            let output_set = self.sets[&output].clone();
            let alphabet_set = self.alphabet_set.clone();
            for it in output_set.iter() {
                for jt in alphabet_set.iter() {
                    if self.is_set_pair(jt) {
                        continue;
                    }

                    if *it == jt.1 {
                        pair_transducer.disjunct(&OtherSymbolTransducer::new_pair(&jt.0, &jt.1));
                    }
                }
            }
        } else if output == TWOLC_UNKNOWN {
            self.input_symbols.insert(pair.0.clone());
            let input_set = self.sets[&input].clone();
            let alphabet_set = self.alphabet_set.clone();
            for it in input_set.iter() {
                for jt in alphabet_set.iter() {
                    if self.is_set_pair(jt) {
                        continue;
                    }

                    if *it == jt.0 {
                        pair_transducer.disjunct(&OtherSymbolTransducer::new_pair(&jt.0, &jt.1));
                    }
                }
            }
        } else {
            let input_set = self.sets[&input].clone();
            let output_set = self.sets[&output].clone();

            for it in input_set.iter() {
                for jt in output_set.iter() {
                    if self.is_pair(it, jt) {
                        pair_transducer.disjunct(&OtherSymbolTransducer::new_pair(it, jt));
                    }
                }
            }
        }
        self.alphabet.insert(pair.clone(), pair_transducer);
        self.alphabet_set.insert(pair.clone());
        &self.alphabet[pair]
    }

    // [spec:hfst:def:alphabet.alphabet.is-pair-fn]
    // [spec:hfst:sem:alphabet.alphabet.is-pair-fn]
    fn is_pair(&self, input: &str, output: &str) -> bool {
        if input == TWOLC_UNKNOWN && output == TWOLC_UNKNOWN {
            return true;
        }
        if self.diacritics.contains(input) && input == output {
            return true;
        }
        if self.diacritics.contains(input) && output == TWOLC_UNKNOWN {
            return true;
        }
        if input == TWOLC_UNKNOWN {
            return self.output_symbols.contains(output);
        }
        if output == TWOLC_UNKNOWN {
            return self.input_symbols.contains(input);
        }

        self.alphabet_set
            .contains(&(input.to_string(), output.to_string()))
    }

    // [spec:hfst:def:alphabet.alphabet.define-singleton-set-fn]
    // [spec:hfst:sem:alphabet.alphabet.define-singleton-set-fn]
    fn define_singleton_set(&mut self, name: &str) {
        self.sets
            .insert(name.to_string(), vec![name.to_string(); 1]);
    }

    // [spec:hfst:def:alphabet.alphabet.is-set-pair-fn]
    // [spec:hfst:sem:alphabet.alphabet.is-set-pair-fn]
    fn is_set_pair(&self, pair: &SymbolPair) -> bool {
        pair.0.contains("__HFST_TWOLC_SET_NAME=") || pair.1.contains("__HFST_TWOLC_SET_NAME=")
    }

    // ----- public -----

    // [spec:hfst:def:alphabet.alphabet.define-set-fn]
    // [spec:hfst:sem:alphabet.alphabet.define-set-fn]
    pub fn define_set(&mut self, name: &str, elements: &SymbolRange) {
        self.sets.insert(name.to_string(), elements.clone());
    }

    // [spec:hfst:def:alphabet.alphabet.define-alphabet-pair-fn]
    // [spec:hfst:sem:alphabet.alphabet.define-alphabet-pair-fn]
    pub fn define_alphabet_pair(&mut self, pair: &SymbolPair) {
        self.alphabet_set.insert(pair.clone());
        self.input_symbols.insert(pair.0.clone());
        self.output_symbols.insert(pair.1.clone());
    }

    // [spec:hfst:def:alphabet.alphabet.alphabet-done-fn]
    // [spec:hfst:sem:alphabet.alphabet.alphabet-done-fn]
    pub fn alphabet_done(&self) {
        OtherSymbolTransducer::set_symbol_pairs(&self.alphabet_set);
    }

    // [spec:hfst:def:alphabet.alphabet.define-diacritics-fn]
    // [spec:hfst:sem:alphabet.alphabet.define-diacritics-fn]
    pub fn define_diacritics(&mut self, diacs: &SymbolRange) {
        for d in diacs.iter() {
            self.diacritics.insert(d.clone());
        }
        // The C++ iterates the 'diacritics' member while erasing from the OTHER
        // containers ('alphabet_set'/'input_symbols'/'output_symbols'), so the
        // loop's set is never mutated; iterate a snapshot to mirror that.
        for it in self.diacritics.clone().iter() {
            self.alphabet_set.remove(&(it.clone(), it.clone()));
            self.alphabet_set
                .remove(&(it.clone(), TWOLC_EPSILON.to_string()));
            self.input_symbols.remove(it);
            self.output_symbols.remove(it);
        }
    }

    // [spec:hfst:def:alphabet.alphabet.is-empty-pair-fn]
    // [spec:hfst:sem:alphabet.alphabet.is-empty-pair-fn]
    pub fn is_empty_pair(&mut self, pair: &SymbolPair) -> bool {
        assert!(self.is_pair(&pair.0, &pair.1));
        // 'alphabet[pair]' (std::map::operator[]) default-constructs a missing
        // value; mirror that so a never-computed pair yields an empty fst.
        self.alphabet
            .entry(pair.clone())
            .or_insert_with(OtherSymbolTransducer::new)
            .is_empty()
    }

    // 'const OtherSymbolTransducer &Alphabet::get_transducer(const SymbolPair
    // &pair)'. (No [spec] annotation in the C++ source.)
    pub fn get_transducer(&mut self, pair: &SymbolPair) -> &OtherSymbolTransducer {
        if self.alphabet.contains_key(pair) {
            return &self.alphabet[pair];
        }
        self.compute(pair)
    }

    // [spec:hfst:def:alphabet.alphabet.get-symbol-pair-vector-fn]
    // [spec:hfst:sem:alphabet.alphabet.get-symbol-pair-vector-fn]
    pub fn get_symbol_pair_vector(&mut self, pair: &SymbolPair) -> SymbolPairVector {
        let result_fst = self.get_transducer(pair).clone();
        let mut result = SymbolPairVector::new();
        result_fst.get_initial_transition_pairs(&mut result);
        result
    }
}
