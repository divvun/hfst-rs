//! Port of 'libhfst/src/implementations/ConvertTransducerFormat.{h,cc}'.
//!
//! This file is the *base* of the conversion machinery: the session-global
//! number↔string coding ('number_to_string_vector' / 'string_to_number_map',
//! seeded with the three special symbols by the global 'dummy3'/'dummy4'
//! objects) and the harmonization helpers built on it, plus the typedefs
//! ('StateId', 'String2NumberMap', 'NumberVector'). Ported exactly as the
//! tropical interning infra is — the two C++ 'static' members become
//! module-level 'LazyLock<Mutex<…>>' seeded by the initializer structs.
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

/* A number-to-string vector common to all transducers during a session. */
// [spec:hfst:def:convert-transducer-format.hfst.implementations.dummy3-fn]
// [spec:hfst:sem:convert-transducer-format.hfst.implementations.dummy3-fn]
static NUMBER_TO_STRING_VECTOR: LazyLock<Mutex<StringVector>> = LazyLock::new(|| {
    let mut vector = StringVector::new();
    StringVectorInitializer::new(&mut vector);
    Mutex::new(vector)
});

/* A string-to-number map common to all transducers during a session. */
// [spec:hfst:def:convert-transducer-format.hfst.implementations.dummy4-fn]
// [spec:hfst:sem:convert-transducer-format.hfst.implementations.dummy4-fn]
static STRING_TO_NUMBER_MAP: LazyLock<Mutex<String2NumberMap>> = LazyLock::new(|| {
    let mut map = String2NumberMap::new();
    String2NumberMapInitializer::new(&mut map);
    Mutex::new(map)
});

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
    ) -> *mut crate::hfst_basic_transducer::HfstBasicTransducer {
        use crate::hfst_data_types::ImplementationType::*;
        if t.type_ == TROPICAL_OPENFST_TYPE {
            let retval = Box::into_raw(Box::new(
                ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(
                    unsafe { &*t.implementation.tropical_ofst },
                    true,
                ),
            ));
            unsafe {
                (*retval).name = t.get_name();
            }
            return retval;
        }
        if t.type_ == LOG_OPENFST_TYPE {
            let retval = Box::into_raw(Box::new(
                ConversionFunctions::log_ofst_to_hfst_basic_transducer(
                    unsafe { &*t.implementation.log_ofst },
                    true,
                ),
            ));
            unsafe {
                (*retval).name = t.get_name();
            }
            return retval;
        }
        if t.type_ == HFST_OL_TYPE || t.type_ == HFST_OLW_TYPE {
            let retval = Box::into_raw(Box::new(
                ConversionFunctions::hfst_ol_to_hfst_basic_transducer(unsafe {
                    &*t.implementation.hfst_ol
                }),
            ));
            unsafe {
                (*retval).name = t.get_name();
            }
            return retval;
        }
        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    /* Get the string that is represented by 'number' in the number-to-string
    vector. If `number` is not found, return the empty string. */
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-string-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-string-fn]
    pub fn get_string(number: u32) -> String {
        let number_to_string_vector = NUMBER_TO_STRING_VECTOR.lock().unwrap();
        if number as usize >= number_to_string_vector.len() {
            return String::from("");
        } // number not found
        number_to_string_vector[number as usize].clone()
    }

    /* Get the number that represents 'str' in the string-to-number map.
    If `str` is not found, add it to the next free index. */
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-number-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-number-fn]
    pub fn get_number(str: &str) -> u32 {
        let mut string_to_number_map = STRING_TO_NUMBER_MAP.lock().unwrap();
        match string_to_number_map.get(str) {
            None => {
                // string not found
                let mut number_to_string_vector = NUMBER_TO_STRING_VECTOR.lock().unwrap();
                number_to_string_vector.push(str.to_string());
                let new_index = size_t_to_uint(number_to_string_vector.len() - 1);
                string_to_number_map.insert(str.to_string(), new_index);
                new_index
            }
            Some(second) => *second,
        }
    }

    /* Get a vector that tells how a transducer that follows the
    number-to-symbol encoding of `coding` should be harmonized so that it will
    follow the one of number_to_string_vector. */
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-harmonization-vector-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-harmonization-vector-fn]
    pub fn get_harmonization_vector(coding_vector: &StringVector) -> NumberVector {
        let mut retval = NumberVector::new();
        retval.reserve(coding_vector.len());
        for it in coding_vector.iter() {
            if *it != "" {
                retval.push(Self::get_number(it));
            } else {
                // a gap in indexing
                retval.push(0);
            }
        }
        retval
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
