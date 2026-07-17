//! Port of 'libhfst/src/implementations/ConvertTransducerFormat.{h,cc}'.
//!
//! This file is the *base* of the conversion machinery: the number↔string coding
//! ('number_to_string_vector' / 'string_to_number_map', seeded with the three
//! special symbols by the global 'dummy3'/'dummy4' objects) and the harmonization
//! helpers built on it, plus the typedefs ('StateId', 'String2NumberMap',
//! 'NumberVector'). The two C++ 'static' members are an owned ['FormatCoder'] (the
//! same shape as the tropical 'SymbolCoder'); the de-globalization keystone
//! removed the process-global instance, so a 'FormatCoder' is constructed where
//! one is needed rather than shared process-wide.
//!
//! Deferred to higher layers (facade + backends):
//!   * 'hfst_transducer_to_hfst_basic_transducer' (the type-dispatch) needs the
//!     facade 'HfstTransducer' with its 'type' field + 'implementation' union;
//!   * every per-backend converter ('sfst_*', 'foma_*', 'xfsm_*',
//!     'tropical_ofst_*', 'log_ofst_*', 'hfst_ol_*') lives in a separate
//!     'Convert*Transducer.cc' and needs its backend (rustfst / 'hfst-ol');
//!   * the 'MAIN_TEST' 'main'.

use std::collections::BTreeMap;

use crate::hfst_data_types::{StringVector, Symbol};

// 'fst::StdArc::StateId', i.e. 'unsigned int'. (Gated by 'HAVE_OPENFST'; the
// OpenFST converters that use it are deferred to the rustfst backend.)
// [spec:hfst:def:convert-transducer-format.hfst.implementations.state-id]
pub type StateId = u32;

// [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.string2-number-map]
pub type String2NumberMap = BTreeMap<Symbol, u32>;
// [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.number-vector]
pub type NumberVector = Vec<u32>;

// The C++ static members of 'ConversionFunctions' are seeded by the initializer
// structs below (the C++ global 'dummy3'/'dummy4').

/* The C++ 'number_to_string_vector' / 'string_to_number_map' statics (the
'dummy3'/'dummy4' session globals) are gone: their former 'ConversionFunctions'
static accessors had no remaining callers once the de-globalization keystone
routed the tropical conversion through each graph's own coder. The coding lives
on as the owned 'FormatCoder' below, constructed where a format coder is needed
rather than shared process-wide. */
// [spec:hfst:def:convert-transducer-format.hfst.implementations.dummy3-fn]
// [spec:hfst:sem:convert-transducer-format.hfst.implementations.dummy3-fn]
// [spec:hfst:def:convert-transducer-format.hfst.implementations.dummy4-fn]
// [spec:hfst:sem:convert-transducer-format.hfst.implementations.dummy4-fn]

/// Owned number↔string coding for format conversion: 'number_to_string' is the
/// key table (index = number) and 'string_to_number' its inverse. Seeded with
/// the three special symbols at 0/1/2.
#[derive(Clone, Debug)]
pub struct FormatCoder {
    number_to_string: StringVector,
    string_to_number: String2NumberMap,
}

impl FormatCoder {
    pub fn new() -> Self {
        let mut number_to_string = StringVector::new();
        StringVectorInitializer::new(&mut number_to_string);
        let mut string_to_number = String2NumberMap::new();
        String2NumberMapInitializer::new(&mut string_to_number);
        FormatCoder {
            number_to_string,
            string_to_number,
        }
    }

    /// Map 'number' to its string, or "" if out of range (as the C++ does).
    pub fn get_string(&self, number: u32) -> Symbol {
        if number as usize >= self.number_to_string.len() {
            return Symbol::default();
        }
        self.number_to_string[number as usize].clone()
    }

    /// Map 'str' to its number, appending it at the next free index if unseen.
    pub fn get_number(&mut self, str: &str) -> u32 {
        match self.string_to_number.get(str) {
            None => {
                let symbol = Symbol::new(str);
                self.number_to_string.push(symbol.clone());
                let new_index =
                    u32::try_from(self.number_to_string.len() - 1).expect("value out of u32 range");
                self.string_to_number.insert(symbol, new_index);
                new_index
            }
            Some(second) => *second,
        }
    }

    pub fn get_harmonization_vector(&mut self, coding_vector: &StringVector) -> NumberVector {
        let mut retval = NumberVector::with_capacity(coding_vector.len());
        for it in coding_vector.iter() {
            if !it.is_empty() {
                retval.push(self.get_number(it));
            } else {
                // a gap in indexing
                retval.push(0);
            }
        }
        retval
    }
}

impl Default for FormatCoder {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions]
pub struct ConversionFunctions;

impl ConversionFunctions {
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-transducer-to-hfst-basic-transducer-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-transducer-to-hfst-basic-transducer-fn]
    //
    // The C++ dispatched on the backend type tag (the SFST/FOMA/XFSM/My arms
    // were #if'd out); the dispatch is the type parameter now
    // ([dec:hfst:monomorphic-backends]) and each former arm's body is the
    // backend's 'Backend::to_basic'. The C++ sets 'retval->name =
    // t.get_name()' on every arm.
    pub fn hfst_transducer_to_hfst_basic_transducer<B: crate::backend::Backend>(
        t: &crate::hfst_transducer::HfstTransducer<B>,
    ) -> crate::error::Result<crate::hfst_basic_transducer::HfstBasicTransducer> {
        let mut retval = t.get_basic_transducer()?;
        retval.name = t.get_name();
        Ok(retval)
    }
}

// Initialization of static members in class ConversionFunctions.
// [spec:hfst:def:convert-transducer-format.hfst.implementations.string-vector-initializer]
pub struct StringVectorInitializer;

impl StringVectorInitializer {
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.string-vector-initializer.string-vector-initializer-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.string-vector-initializer.string-vector-initializer-fn]
    pub fn new(vector: &mut StringVector) -> Self {
        vector.push(Symbol::new_static("@_EPSILON_SYMBOL_@"));
        vector.push(Symbol::new_static("@_UNKNOWN_SYMBOL_@"));
        vector.push(Symbol::new_static("@_IDENTITY_SYMBOL_@"));
        StringVectorInitializer
    }
}

// [spec:hfst:def:convert-transducer-format.hfst.implementations.string2-number-map-initializer]
pub struct String2NumberMapInitializer;

impl String2NumberMapInitializer {
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.string2-number-map-initializer.string2-number-map-initializer-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.string2-number-map-initializer.string2-number-map-initializer-fn]
    pub fn new(map: &mut String2NumberMap) -> Self {
        map.insert(Symbol::new_static("@_EPSILON_SYMBOL_@"), 0);
        map.insert(Symbol::new_static("@_UNKNOWN_SYMBOL_@"), 1);
        map.insert(Symbol::new_static("@_IDENTITY_SYMBOL_@"), 2);
        String2NumberMapInitializer
    }
}
