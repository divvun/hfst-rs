//! Port of
//! 'libhfst/src/implementations/HfstTropicalTransducerTransitionData.{h,cc}'.
//!
//! One implementation of the transition-data template parameter 'C' used by
//! 'HfstTransition'. Symbols are interned to 'unsigned int' numbers via the C++
//! class-static 'number2symbol_map'/'symbol2number_map'/'max_number' (seeded by
//! the 'dummy1'/'dummy2' globals). Those three statics are encapsulated in an
//! owned ['SymbolCoder'] (the divvunspell 'TransducerAlphabet' shape). The
//! idiom5 de-globalization moves the coder onto each 'HfstBasicTransducer': the
//! symbol getters/setters and 'new_symbols' take the owning graph's 'SymbolCoder'
//! explicitly, so resolution and interning go through that per-graph coder and
//! binary ops harmonize across graphs via 'SymbolCoder::create_translator_from'.
//! No process-global coder remains. Symbol getters return an owned 'String' (the
//! C++ returns a 'const std::string&' into the key table; the equal value is
//! cloned out).

use std::collections::{BTreeMap, BTreeSet};

use crate::hfst_exception_defs::{EmptyStringException, HfstFatalException};

// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.symbol-type]
pub type SymbolType = String;
// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.weight-type]
pub type WeightType = f32;
// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.symbol-type-set]
pub type SymbolTypeSet = BTreeSet<SymbolType>;

// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.number2-symbol-vector]
pub type Number2SymbolVector = Vec<SymbolType>;
// 'std::map<SymbolType, unsigned int, string_comparison>'. The 'string_comparison'
// comparator is plain lexicographic '<', which is exactly 'BTreeMap<String, _>''s
// own ordering.
// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.symbol2-number-map]
pub type Symbol2NumberMap = BTreeMap<SymbolType, u32>;

// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.string-comparison]
pub struct string_comparison;

impl string_comparison {
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.string-comparison.operator-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.string-comparison.operator-fn]
    pub fn operator_call(str1: &str, str2: &str) -> bool {
        // str1.compare(str2) < 0
        str1 < str2
    }
}

// The C++ class-static symbol coding (number2symbol_map / symbol2number_map /
// max_number, seeded by the 'dummy1'/'dummy2' globals) is encapsulated in an
// owned 'SymbolCoder' modelled on divvunspell's 'TransducerAlphabet'. The
// de-globalization keystone (idiom5) moves the coder onto each
// 'HfstBasicTransducer' (reached via '&self.coder'); resolution and interning go
// through the owning graph's coder, and binary ops harmonize across graphs via
// 'SymbolCoder::create_translator_from'. No process-global coder remains.
// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.dummy1-fn]
// [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.dummy1-fn]
// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.dummy2-fn]
// [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.dummy2-fn]

/// Owned symbol<->number coding (the divvunspell 'TransducerAlphabet' shape):
/// 'number2symbol' is the key table (index = number), 'symbol2number' its
/// inverse, and 'max_number' the highest number assigned. Seeded with the three
/// special symbols (epsilon/unknown/identity) at 0/1/2.
#[derive(Clone, Debug)]
pub struct SymbolCoder {
    number2symbol: Number2SymbolVector,
    symbol2number: Symbol2NumberMap,
    max_number: u32,
}

impl SymbolCoder {
    pub fn new() -> Self {
        let mut number2symbol = Number2SymbolVector::new();
        Number2SymbolVectorInitializer::new(&mut number2symbol);
        let mut symbol2number = Symbol2NumberMap::new();
        Symbol2NumberMapInitializer::new(&mut symbol2number);
        SymbolCoder {
            number2symbol,
            symbol2number,
            max_number: 2,
        }
    }

    pub fn get_max_number(&self) -> u32 {
        self.max_number
    }

    /// The key table (index = number, value = symbol), in number order. Used by
    /// the harmonization pre-pass to intern every symbol this coder knows into a
    /// shared coder in a deterministic order.
    pub fn number2symbol_slice(&self) -> &[SymbolType] {
        &self.number2symbol
    }

    /// Map 'number' back to its symbol (throws if out of range, as the C++ does).
    pub fn get_symbol(&self, number: u32) -> String {
        if number as usize >= self.number2symbol.len() {
            let mut message = String::from("HfstTropicalTransducerTransitionData: number ");
            message.push_str(&number.to_string());
            message.push_str(" is not mapped to any symbol");
            crate::HFST_THROW_MESSAGE!(HfstFatalException, message);
        }
        self.number2symbol[number as usize].clone()
    }

    /// Map 'symbol' to its number, interning a fresh number if unseen.
    pub fn get_number(&mut self, symbol: &str) -> u32 {
        if symbol.is_empty() {
            // FAIL
            match self.symbol2number.get(symbol) {
                None => {
                    tracing::error!("No number for the empty symbol");
                }
                Some(second) => {
                    tracing::error!("The empty symbol corresdponds to number {}", second);
                }
            }
            assert!(false);
        }
        if let Some(second) = self.symbol2number.get(symbol) {
            return *second;
        }
        self.max_number += 1;
        let new_max = self.max_number;
        self.symbol2number.insert(symbol.to_string(), new_max);
        self.number2symbol.push(symbol.to_string());
        new_max
    }

    pub fn get_harmonization_vector(&mut self, symbols: &[SymbolType]) -> Vec<u32> {
        let mut harmv: Vec<u32> = vec![0; symbols.len()];
        for i in 0..symbols.len() {
            if symbols[i] != "" {
                harmv[i] = self.get_number(&symbols[i]);
            }
        }
        harmv
    }

    pub fn get_reverse_harmonization_vector(
        &self,
        symbols: &BTreeMap<SymbolType, u32>,
    ) -> Vec<u32> {
        let mut harmv: Vec<u32> = vec![0; (self.max_number + 1) as usize];
        for i in 0..harmv.len() {
            let sym = self.get_symbol(i as u32);
            if let Some(second) = symbols.get(&sym) {
                harmv[i] = *second;
            }
        }
        harmv
    }

    /// Build a translator mapping `other`'s symbol numbers into *this* coder's
    /// number space, interning any of `other`'s symbols this coder lacks. The
    /// result is indexed by `other`'s number: `translator[n]` is the number in
    /// `self` of the symbol that is number `n` in `other`. This is the
    /// harmonization primitive the keystone's K3 uses to reconcile two graphs'
    /// numberings before a binary op (divvunspell's
    /// `TransducerAlphabet::create_translator_from`).
    pub fn create_translator_from(&mut self, other: &SymbolCoder) -> Vec<u32> {
        let mut translator: Vec<u32> = Vec::with_capacity(other.number2symbol.len());
        for symbol in &other.number2symbol {
            translator.push(self.get_number(symbol));
        }
        translator
    }
}

impl Default for SymbolCoder {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data]
#[derive(Clone, Debug)]
pub struct HfstTropicalTransducerTransitionData {
    /* The actual transition data */
    pub input_number: u32,
    pub output_number: u32,
    pub weight: WeightType,
}

impl HfstTropicalTransducerTransitionData {
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-epsilon-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-epsilon-fn]
    pub fn get_epsilon() -> SymbolType {
        SymbolType::from("@_EPSILON_SYMBOL_@")
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-unknown-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-unknown-fn]
    pub fn get_unknown() -> SymbolType {
        SymbolType::from("@_UNKNOWN_SYMBOL_@")
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-identity-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-identity-fn]
    pub fn get_identity() -> SymbolType {
        SymbolType::from("@_IDENTITY_SYMBOL_@")
    }

    // The former class-static accessors 'get_max_number' / 'get_harmonization_vector'
    // / 'get_reverse_harmonization_vector' / 'get_symbol' / 'get_number' delegated to
    // a process-global coder; they are gone. The equivalent operations are now
    // instance methods on ['SymbolCoder'], invoked on the owning graph's
    // 'self.coder' (idiom5 de-globalization).
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-max-number-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-max-number-fn]
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-harmonization-vector-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-harmonization-vector-fn]
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-reverse-harmonization-vector-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-reverse-harmonization-vector-fn]
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-number-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-number-fn]

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.print-transition-data-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.print-transition-data-fn]
    pub fn print_transition_data(&self) {
        tracing::debug!(
            "{}:{} {}",
            self.input_number,
            self.output_number,
            self.weight
        );
    }

    pub fn new() -> Self {
        HfstTropicalTransducerTransitionData {
            input_number: 0,
            output_number: 0,
            weight: 0.0,
        }
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.hfst-tropical-transducer-transition-data-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.hfst-tropical-transducer-transition-data-fn]
    pub fn new_symbols(
        isymbol: SymbolType,
        osymbol: SymbolType,
        weight: WeightType,
        coder: &mut SymbolCoder,
    ) -> Self {
        if isymbol.is_empty() || osymbol.is_empty() {
            crate::HFST_THROW_MESSAGE!(
                EmptyStringException,
                "HfstTropicalTransducerTransitionData(SymbolType, SymbolType, WeightType)"
            );
        }

        HfstTropicalTransducerTransitionData {
            input_number: coder.get_number(&isymbol),
            output_number: coder.get_number(&osymbol),
            weight,
        }
    }

    pub fn new_numbers(inumber: u32, onumber: u32, weight: WeightType) -> Self {
        HfstTropicalTransducerTransitionData {
            input_number: inumber,
            output_number: onumber,
            weight,
        }
    }

    pub fn get_input_symbol(&self, coder: &SymbolCoder) -> SymbolType {
        coder.get_symbol(self.input_number)
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-input-symbol-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-input-symbol-fn]
    pub fn set_input_symbol(&mut self, symbol: &SymbolType, coder: &mut SymbolCoder) {
        self.input_number = coder.get_number(symbol);
    }

    pub fn get_output_symbol(&self, coder: &SymbolCoder) -> SymbolType {
        coder.get_symbol(self.output_number)
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-output-symbol-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-output-symbol-fn]
    pub fn set_output_symbol(&mut self, symbol: &SymbolType, coder: &mut SymbolCoder) {
        self.output_number = coder.get_number(symbol);
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-input-number-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-input-number-fn]
    pub fn get_input_number(&self) -> u32 {
        self.input_number
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-output-number-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-output-number-fn]
    pub fn get_output_number(&self) -> u32 {
        self.output_number
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-weight-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-weight-fn]
    pub fn get_weight(&self) -> WeightType {
        self.weight
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-weight-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-weight-fn]
    pub fn set_weight(&mut self, w: WeightType) {
        self.weight = w;
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-epsilon-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-epsilon-fn]
    pub fn is_epsilon(symbol: &SymbolType) -> bool {
        symbol == "@_EPSILON_SYMBOL_@"
    }
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-unknown-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-unknown-fn]
    pub fn is_unknown(symbol: &SymbolType) -> bool {
        symbol == "@_UNKNOWN_SYMBOL_@"
    }
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-identity-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-identity-fn]
    pub fn is_identity(symbol: &SymbolType) -> bool {
        symbol == "@_IDENTITY_SYMBOL_@"
    }
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-valid-symbol-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-valid-symbol-fn]
    pub fn is_valid_symbol(symbol: &SymbolType) -> bool {
        if symbol.is_empty() {
            return false;
        }
        true
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-marker-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-marker-fn]
    pub fn get_marker(_sts: &SymbolTypeSet) -> SymbolType {
        SymbolType::from("@_MARKER_SYMBOL_@")
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.operator-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.operator-fn]
    pub fn operator_lt(&self, another: &HfstTropicalTransducerTransitionData) -> bool {
        if self.input_number < another.input_number {
            return true;
        }
        if self.input_number > another.input_number {
            return false;
        }
        if self.output_number < another.output_number {
            return true;
        }
        if self.output_number > another.output_number {
            return false;
        }
        self.weight < another.weight
    }

    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.less-than-ignore-weight-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.less-than-ignore-weight-fn]
    pub fn less_than_ignore_weight(&self, another: &HfstTropicalTransducerTransitionData) -> bool {
        if self.input_number < another.input_number {
            return true;
        }
        if self.input_number > another.input_number {
            return false;
        }
        if self.output_number < another.output_number {
            return true;
        }
        if self.output_number > another.output_number {
            return false;
        }
        false
    }
}

// 'bool operator<' is the canonical ordering; 'Ord'/'PartialOrd' make the type
// usable in ordered containers, using 'total_cmp' for the weight to give a total
// order that agrees with 'operator<' for non-NaN weights.
impl PartialEq for HfstTropicalTransducerTransitionData {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}
impl Eq for HfstTropicalTransducerTransitionData {}
impl PartialOrd for HfstTropicalTransducerTransitionData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HfstTropicalTransducerTransitionData {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.input_number
            .cmp(&other.input_number)
            .then(self.output_number.cmp(&other.output_number))
            .then(self.weight.total_cmp(&other.weight))
    }
}

impl Default for HfstTropicalTransducerTransitionData {
    fn default() -> Self {
        Self::new()
    }
}

// Initialization of static members in class HfstTropicalTransducerTransitionData.
// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.number2-symbol-vector-initializer]
pub struct Number2SymbolVectorInitializer;

impl Number2SymbolVectorInitializer {
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.number2-symbol-vector-initializer.number2-symbol-vector-initializer-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.number2-symbol-vector-initializer.number2-symbol-vector-initializer-fn]
    pub fn new(vect: &mut Number2SymbolVector) -> Self {
        vect.push(String::from("@_EPSILON_SYMBOL_@"));
        vect.push(String::from("@_UNKNOWN_SYMBOL_@"));
        vect.push(String::from("@_IDENTITY_SYMBOL_@"));
        Number2SymbolVectorInitializer
    }
}

// [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.symbol2-number-map-initializer]
pub struct Symbol2NumberMapInitializer;

impl Symbol2NumberMapInitializer {
    // [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.symbol2-number-map-initializer.symbol2-number-map-initializer-fn]
    // [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.symbol2-number-map-initializer.symbol2-number-map-initializer-fn]
    pub fn new(map: &mut Symbol2NumberMap) -> Self {
        map.insert(String::from("@_EPSILON_SYMBOL_@"), 0);
        map.insert(String::from("@_UNKNOWN_SYMBOL_@"), 1);
        map.insert(String::from("@_IDENTITY_SYMBOL_@"), 2);
        Symbol2NumberMapInitializer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_translator_from_maps_other_numbers_into_self() {
        // Two independent coders where the same string gets different numbers.
        let mut other = SymbolCoder::new();
        let a_in_other = other.get_number("a"); // 3
        let b_in_other = other.get_number("b"); // 4

        let mut me = SymbolCoder::new();
        let b_in_self = me.get_number("b"); // 3
        me.get_number("c"); // 4

        let translator = me.create_translator_from(&other);

        // The three special symbols map straight through (0/1/2).
        assert_eq!(&translator[0..3], &[0, 1, 2]);
        // 'b' already existed in self -> same number, no new interning.
        assert_eq!(translator[b_in_other as usize], b_in_self);
        // 'a' was absent from self -> interned at the next free number (5).
        assert_eq!(translator[a_in_other as usize], 5);
        assert_eq!(me.get_number("a"), 5);
        // self's existing 'c' is untouched.
        assert_eq!(me.get_number("c"), 4);
    }
}
