//! Port of 'libhfst/src/implementations/ConvertTransducerFormat.{h,cc}'.
//!
//! This file is the *base* of the conversion machinery: the session-global
//! number↔string coding ('number_to_string_vector' / 'string_to_number_map',
//! seeded with the three special symbols by the global 'dummy3'/'dummy4'
//! objects) and the harmonization helpers built on it, plus the typedefs
//! ('StateId', 'String2NumberMap', 'NumberVector'). The two C++ 'static' members
//! are encapsulated in an owned ['FormatCoder'] (the same K1 treatment as the
//! tropical 'SymbolCoder'); a single process-global instance preserves the
//! shared session numbering until a later stage moves it onto the conversion.
//!
//! Deferred to higher layers (facade + backends):
//!   * 'hfst_transducer_to_hfst_basic_transducer' (the type-dispatch) needs the
//!     facade 'HfstTransducer' with its 'type' field + 'implementation' union;
//!   * every per-backend converter ('sfst_*', 'foma_*', 'xfsm_*',
//!     'tropical_ofst_*', 'log_ofst_*', 'hfst_ol_*') lives in a separate
//!     'Convert*Transducer.cc' and needs its backend (rustfst / 'hfst-ol');
//!   * the 'MAIN_TEST' 'main'.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

use crate::hfst_data_types::{StringVector, size_t_to_uint};
use crate::hfst_exception_defs::FunctionNotImplementedException;

// 'fst::StdArc::StateId', i.e. 'unsigned int'. (Gated by 'HAVE_OPENFST'; the
// OpenFST converters that use it are deferred to the rustfst backend.)
// [spec:hfst:def:convert-transducer-format.hfst.implementations.state-id]
pub type StateId = u32;

// [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.string2-number-map]
pub type String2NumberMap = BTreeMap<String, u32>;
// [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.number-vector]
pub type NumberVector = Vec<u32>;

// Static members of 'ConversionFunctions', seeded by the initializer structs
// below (the C++ global 'dummy3'/'dummy4').
//
// 'get_number' is the only function that touches both statics; it always locks
// the map first and the vector second, and nothing locks them the other way
// round, so the pair is deadlock-free.

/* The number↔string coding common to all transducers during a session. The two
C++ statics (number_to_string_vector / string_to_number_map, seeded by the
'dummy3'/'dummy4' globals) are encapsulated in an owned 'FormatCoder' — the same
K1 treatment the tropical coding gets in SymbolCoder. A single process-global
instance preserves the shared session numbering exactly; later stages move it
onto the conversion path. */
// [spec:hfst:def:convert-transducer-format.hfst.implementations.dummy3-fn]
// [spec:hfst:sem:convert-transducer-format.hfst.implementations.dummy3-fn]
// [spec:hfst:def:convert-transducer-format.hfst.implementations.dummy4-fn]
// [spec:hfst:sem:convert-transducer-format.hfst.implementations.dummy4-fn]
static GLOBAL_FORMAT_CODER: LazyLock<Mutex<FormatCoder>> =
    LazyLock::new(|| Mutex::new(FormatCoder::new()));

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
    pub fn get_string(&self, number: u32) -> String {
        if number as usize >= self.number_to_string.len() {
            return String::from("");
        }
        self.number_to_string[number as usize].clone()
    }

    /// Map 'str' to its number, appending it at the next free index if unseen.
    pub fn get_number(&mut self, str: &str) -> u32 {
        match self.string_to_number.get(str) {
            None => {
                self.number_to_string.push(str.to_string());
                let new_index = size_t_to_uint(self.number_to_string.len() - 1);
                self.string_to_number.insert(str.to_string(), new_index);
                new_index
            }
            Some(second) => *second,
        }
    }

    pub fn get_harmonization_vector(&mut self, coding_vector: &StringVector) -> NumberVector {
        let mut retval = NumberVector::new();
        retval.reserve(coding_vector.len());
        for it in coding_vector.iter() {
            if *it != "" {
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
    // Dispatch on the backend type; the SFST/FOMA/XFSM/My arms are #if'd out.
    // The C++ sets 'retval->name = t.get_name()' on every arm.
    pub fn hfst_transducer_to_hfst_basic_transducer(
        t: &crate::hfst_transducer::HfstTransducer,
    ) -> crate::hfst_basic_transducer::HfstBasicTransducer {
        use crate::hfst_data_types::ImplementationType::*;
        if t.type_ == TROPICAL_OPENFST_TYPE {
            let mut retval = ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(
                t.implementation.as_tropical(),
                true,
            );
            retval.name = t.get_name();
            return retval;
        }
        if t.type_ == LOG_OPENFST_TYPE {
            let mut retval = ConversionFunctions::log_ofst_to_hfst_basic_transducer(
                t.implementation.as_log(),
                true,
            );
            retval.name = t.get_name();
            return retval;
        }
        if t.type_ == HFST_OL_TYPE || t.type_ == HFST_OLW_TYPE {
            let mut retval = ConversionFunctions::hfst_ol_to_hfst_basic_transducer(
                t.implementation.as_hfst_ol(),
            );
            retval.name = t.get_name();
            return retval;
        }
        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    /* Get the string that is represented by 'number' in the number-to-string
    vector. If `number` is not found, return the empty string. */
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-string-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-string-fn]
    pub fn get_string(number: u32) -> String {
        GLOBAL_FORMAT_CODER.lock().unwrap().get_string(number)
    }

    /* Get the number that represents 'str' in the string-to-number map.
    If `str` is not found, add it to the next free index. */
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-number-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-number-fn]
    pub fn get_number(str: &str) -> u32 {
        GLOBAL_FORMAT_CODER.lock().unwrap().get_number(str)
    }

    /* Get a vector that tells how a transducer that follows the
    number-to-symbol encoding of `coding` should be harmonized so that it will
    follow the one of number_to_string_vector. */
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-harmonization-vector-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-harmonization-vector-fn]
    pub fn get_harmonization_vector(coding_vector: &StringVector) -> NumberVector {
        GLOBAL_FORMAT_CODER
            .lock()
            .unwrap()
            .get_harmonization_vector(coding_vector)
    }
}

// Initialization of static members in class ConversionFunctions.
// [spec:hfst:def:convert-transducer-format.hfst.implementations.string-vector-initializer]
pub struct StringVectorInitializer;

impl StringVectorInitializer {
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.string-vector-initializer.string-vector-initializer-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.string-vector-initializer.string-vector-initializer-fn]
    pub fn new(vector: &mut StringVector) -> Self {
        vector.push(String::from("@_EPSILON_SYMBOL_@"));
        vector.push(String::from("@_UNKNOWN_SYMBOL_@"));
        vector.push(String::from("@_IDENTITY_SYMBOL_@"));
        StringVectorInitializer
    }
}

// [spec:hfst:def:convert-transducer-format.hfst.implementations.string2-number-map-initializer]
pub struct String2NumberMapInitializer;

impl String2NumberMapInitializer {
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.string2-number-map-initializer.string2-number-map-initializer-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.string2-number-map-initializer.string2-number-map-initializer-fn]
    pub fn new(map: &mut String2NumberMap) -> Self {
        map.insert(String::from("@_EPSILON_SYMBOL_@"), 0);
        map.insert(String::from("@_UNKNOWN_SYMBOL_@"), 1);
        map.insert(String::from("@_IDENTITY_SYMBOL_@"), 2);
        String2NumberMapInitializer
    }
}
