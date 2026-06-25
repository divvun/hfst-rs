//! Port of `libhfst/src/HfstInputStream.{h,cc}` — `hfst::HfstInputStream`, the
//! stream for reading HFST binary transducers.
//!
//! @FIXME (from the C++): the structure of this class and its functions is
//! disorganised; the port mirrors it 1:1.
//!
//! ## Backend union modelling
//! The C++ holds a `union StreamImplementation` of raw backend-stream pointers
//! (`sfst`, `tropical_ofst`, `log_ofst`, `foma`, `xfsm`, `hfst_ol`); exactly one
//! member is live and the live member is selected by the `type`
//! (`ImplementationType`) discriminant. Mirrored here as a struct of per-backend
//! `Option<Box<...>>` fields ([`StreamImplementation`]); the `type_` field is the
//! discriminant, and every accessor `switch (type)`es on it exactly as the C++
//! does. The tropical/log backend streams borrow their reader through
//! [`IStream`]`<'a>`, so the whole `HfstInputStream<'a>` carries that lifetime.
//! `sfst`/`foma`/`xfsm` backend streams are unported collaborators — modelled as
//! `unimplemented!("deferred: …")` placeholder unit types so the field is present
//! but cannot be constructed. `HfstOlInputStream` is likewise not yet ported.
//!
//! ## `std::istream * input_stream`
//! The raw `std::istream*` (NULL once a backend impl exists; non-NULL while the
//! first transducer's type is still unknown) becomes `Option<IStream<'a>>`:
//! `Some` mirrors a non-NULL pointer (reads go to the raw stream), `None` mirrors
//! NULL (reads route through the backend implementation).

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use std::collections::BTreeMap;

use crate::hfst_data_types::{ImplementationType, StringPairVector};
use crate::hfst_exception_defs::{
    EndOfStreamException, FileIsInGZFormatException, HfstFatalException,
    NotTransducerStreamException, TransducerHeaderException,
};
use crate::log_weight_transducer::LogWeightInputStream;
use crate::transducer::IStream;
use crate::tropical_weight_transducer::TropicalWeightInputStream;

/// Unported backend input-stream collaborators. Each is a placeholder unit type:
/// the corresponding [`StreamImplementation`] field exists for fidelity with the
/// C++ union, but no constructor is provided (the `#if HAVE_SFST` / `HAVE_FOMA` /
/// `HAVE_XFSM` paths and `HfstOlInputStream` are deferred).
pub struct SfstInputStream;
pub struct FomaInputStream;
pub struct XfsmInputStream;
pub struct HfstOlInputStream;

/// Port of the C++ `union StreamImplementation` (the backend implementation).
/// Exactly one field is `Some`, selected by `HfstInputStream::type_`. The
/// tropical/log members borrow their reader (`IStream<'a>`); the rest are
/// deferred placeholders.
// [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-implementation]
#[derive(Default)]
pub struct StreamImplementation<'a> {
    pub sfst: Option<Box<SfstInputStream>>,
    pub tropical_ofst: Option<Box<TropicalWeightInputStream<'a>>>,
    pub log_ofst: Option<Box<LogWeightInputStream<'a>>>,
    pub foma: Option<Box<FomaInputStream>>,
    pub xfsm: Option<Box<XfsmInputStream>>,
    pub hfst_ol: Option<Box<HfstOlInputStream>>,
}

/// The type of a transducer not supported directly by HFST version 3.0 but which
/// can occur in conversion functions. (C++ nested `enum HfstInputStream::TransducerType`.)
// [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.transducer-type]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransducerType {
    /// See `hfst_version_2_weighted_transducer`.
    HFST_VERSION_2_WEIGHTED,
    /// An SFST transducer with no alphabet, not supported.
    HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET,
    /// Old header + ordinary SFST transducer.
    HFST_VERSION_2_UNWEIGHTED,
    /// An OpenFst transducer, can cause problems if it does not have symbol tables.
    OPENFST_TROPICAL_,
    OPENFST_LOG_,
    /// An SFST transducer.
    SFST_,
    /// A foma transducer in unzipped format. A zipped file is handled by throwing
    /// a `FileIsInGZFormatException`.
    FOMA_,
    /// An xfsm transducer.
    XFSM_,
    /// Transducer type not recognized.
    ERROR_TYPE_,
}

/// A stream for reading HFST binary transducers.
///
/// @see `HfstTransducer::HfstTransducer(HfstInputStream &in)`
// [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream]
pub struct HfstInputStream<'a> {
    /// The backend implementation (C++ `StreamImplementation implementation`).
    implementation: StreamImplementation<'a>,
    /// Implementation type (the discriminant selecting the live `implementation` member).
    type_: ImplementationType,
    /// Name of next transducer, given in the hfst header.
    name: String,
    props: BTreeMap<String, String>,
    /// How many bytes have already been read by the function when processing the
    /// hfst header.
    bytes_to_skip: u32,
    /// The name of the file; if stdin, name is "".
    filename: String,
    /// Whether the current transducer has an hfst header.
    has_hfst_header: bool,
    /// A special case where an OpenFst transducer has no symbol tables but an SFST
    /// alphabet is appended at the end. Should not occur very often, but possible
    /// when converting old transducers into version 3.0. transducers.
    hfst_version_2_weighted_transducer: bool,
    /// The stream that the reading operations use when reading the first
    /// transducer. Then the type of the transducer is not known so there is no
    /// backend implementation whose reading functions could be used. C++ raw
    /// `std::istream * input_stream`: `None` == NULL (use backend implementation),
    /// `Some` == non-NULL (read the raw stream directly).
    input_stream: Option<IStream<'a>>,
}

// ===== input_impl (workflow body) =====
mod input_impl {
    #![allow(unused_imports)]
    use super::*;
    // [spec:hfst:def:hfst-input-stream.hfst.debug-error-fn]
    // [spec:hfst:sem:hfst-input-stream.hfst.debug-error-fn]
    pub fn debug_error(_msg: &str) {
        // #if PRINT_DEBUG_MESSAGES -> fprintf(stderr, ...); #else (void)msg; #endif
        // PRINT_DEBUG_MESSAGES is off, so this is a no-op (mirrors `(void)msg;`).
    }

    impl<'a> HfstInputStream<'a> {
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.ignore-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.ignore-fn]
        fn ignore(&mut self, n: u32) {
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    // this->implementation.sfst->ignore(n);
                    unimplemented!("deferred: SfstInputStream::ignore (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    self.implementation
                        .tropical_ofst
                        .as_mut()
                        .unwrap()
                        .ignore(n);
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation.log_ofst.as_mut().unwrap().ignore(n);
                }
                ImplementationType::FOMA_TYPE => {
                    // this->implementation.foma->ignore(n);
                    unimplemented!("deferred: FomaInputStream::ignore (no foma backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    // this->implementation.hfst_ol->ignore(n);
                    unimplemented!("deferred: HfstOlInputStream::ignore (no hfst_ol backend)")
                }
                _ => {
                    assert!(false);
                }
            }
        }

        fn stream_get_char_ref(&mut self, c: &mut char) -> char {
            if self.input_stream.is_some() {
                let mut b = [0u8; 1];
                self.input_stream.as_mut().unwrap().read(&mut b);
                *c = b[0] as char;
                return *c;
            }
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    // return c = this->implementation.sfst->stream_get();
                    unimplemented!("deferred: SfstInputStream::stream_get (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    *c = self
                        .implementation
                        .tropical_ofst
                        .as_mut()
                        .unwrap()
                        .stream_get();
                    return *c;
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    *c = self.implementation.log_ofst.as_mut().unwrap().stream_get();
                    return *c;
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::stream_get (no foma backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    unimplemented!("deferred: HfstOlInputStream::stream_get (no hfst_ol backend)")
                }
                _ => {
                    assert!(false);
                }
            }
            #[allow(unreachable_code)]
            {
                crate::HFST_THROW_MESSAGE!(HfstFatalException, "stream_get(char &) failed")
            }
        }

        fn stream_get_short_ref(&mut self, i: &mut i16) -> i16 {
            if self.input_stream.is_some() {
                let mut b = [0u8; 2];
                self.input_stream.as_mut().unwrap().read(&mut b);
                *i = i16::from_ne_bytes(b);
                return *i;
            }
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    unimplemented!("deferred: SfstInputStream::stream_get_short (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    *i = self
                        .implementation
                        .tropical_ofst
                        .as_mut()
                        .unwrap()
                        .stream_get_short();
                    return *i;
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    *i = self
                        .implementation
                        .log_ofst
                        .as_mut()
                        .unwrap()
                        .stream_get_short();
                    return *i;
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::stream_get_short (no foma backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    unimplemented!(
                        "deferred: HfstOlInputStream::stream_get_short (no hfst_ol backend)"
                    )
                }
                _ => {
                    assert!(false);
                }
            }
            #[allow(unreachable_code)]
            {
                crate::HFST_THROW_MESSAGE!(HfstFatalException, "stream_get(short &) failed")
            }
        }

        fn stream_get_ushort_ref(&mut self, i: &mut u16) -> u16 {
            let byte_1 = self.stream_get();
            let byte_2 = self.stream_get();
            *i = (((byte_2 as u8) as u16) << 8).wrapping_add((byte_1 as u8) as u16);
            *i
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-get-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-get-fn]
        fn stream_get(&mut self) -> char {
            if self.input_stream.is_some() {
                let mut b = [0u8; 1];
                self.input_stream.as_mut().unwrap().read(&mut b);
                return b[0] as char;
            }
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    unimplemented!("deferred: SfstInputStream::stream_get (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    return self
                        .implementation
                        .tropical_ofst
                        .as_mut()
                        .unwrap()
                        .stream_get();
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    return self.implementation.log_ofst.as_mut().unwrap().stream_get();
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::stream_get (no foma backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    unimplemented!("deferred: HfstOlInputStream::stream_get (no hfst_ol backend)")
                }
                _ => {
                    assert!(false);
                }
            }
            #[allow(unreachable_code)]
            {
                crate::HFST_THROW_MESSAGE!(HfstFatalException, "stream_get() failed")
            }
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-unget-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-unget-fn]
        fn stream_unget(&mut self, c: char) {
            if self.input_stream.is_some() {
                // input_stream->putback(c); -- IStream has no putback/unget.
                unimplemented!("deferred: input_stream putback — IStream has no putback/unget")
            }
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    unimplemented!("deferred: SfstInputStream::stream_unget (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    self.implementation
                        .tropical_ofst
                        .as_mut()
                        .unwrap()
                        .stream_unget(c);
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation
                        .log_ofst
                        .as_mut()
                        .unwrap()
                        .stream_unget(c);
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::stream_unget (no foma backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    unimplemented!("deferred: HfstOlInputStream::stream_unget (no hfst_ol backend)")
                }
                _ => {
                    assert!(false);
                }
            }
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-peek-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-peek-fn]
        fn stream_peek(&mut self) -> char {
            let c = self.stream_get();
            self.stream_unget(c);
            c
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-getstring-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-getstring-fn]
        fn stream_getstring(&mut self) -> String {
            let mut retval = String::new();
            loop {
                let c = self.stream_get();
                if self.stream_eof() {
                    crate::HFST_THROW!(EndOfStreamException);
                }
                if c == '\0' {
                    break;
                }
                retval.push(c);
            }
            retval
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-eof-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-eof-fn]
        fn stream_eof(&mut self) -> bool {
            if self.input_stream.is_some() {
                // input_stream->eof(); IStream exposes its EOF/fail state via good().
                return !self.input_stream.as_ref().unwrap().good();
            }
            self.is_eof()
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.set-implementation-specific-header-data-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.set-implementation-specific-header-data-fn]
        fn set_implementation_specific_header_data(
            &mut self,
            _data: &mut StringPairVector,
            _index: u32,
        ) -> bool {
            // #if HAVE_SFST || HAVE_LEAN_SFST
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    //return this->implementation.sfst->
                    //set_implementation_specific_header_data(data, index);
                }
                _ => {}
            }
            // #endif
            false
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-transducer-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-transducer-fn]
        // The C++ `read_transducer(HfstTransducer &t)` builds a HfstTransducer and
        // touches `t.implementation.*`, the OpenFst interfaces and ConversionFunctions.
        // `HfstTransducer` is not yet ported (facade layer), so the whole dispatch is
        // deferred.
        fn read_transducer(&mut self) {
            unimplemented!(
                "deferred: HfstInputStream::read_transducer — needs HfstTransducer facade"
            )
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.guess-fst-type-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.guess-fst-type-fn]
        fn guess_fst_type(&mut self, bytes_read: &mut i32) -> TransducerType {
            *bytes_read = 0;

            let c = self.stream_peek();

            match c as u8 {
                0xd6 => {
                    // OpenFst
                    let mut chars_read = [0 as char; 26];
                    for i in 0..26usize {
                        chars_read[i] = self.stream_get();
                        if self.stream_eof() {
                            crate::HFST_THROW!(EndOfStreamException);
                        }
                    }
                    let mut i: i32 = 25;
                    while i >= 0 {
                        self.stream_unget(chars_read[i as usize]);
                        i -= 1;
                    }

                    if chars_read[18] == 's' {
                        // standard
                        TransducerType::OPENFST_TROPICAL_
                    } else if chars_read[18] == 'l' {
                        // log
                        TransducerType::OPENFST_LOG_
                    } else {
                        crate::HFST_THROW!(NotTransducerStreamException)
                    }
                }
                b'#' => {
                    // foma
                    TransducerType::FOMA_
                }
                0x1f => {
                    // native foma (gz magic number is 1F 8B 08)
                    let c0 = self.stream_get();
                    if self.stream_eof() {
                        crate::HFST_THROW!(EndOfStreamException);
                    }
                    let c1 = self.stream_get();
                    if self.stream_eof() {
                        crate::HFST_THROW!(EndOfStreamException);
                    }
                    let c2 = self.stream_get();
                    if self.stream_eof() {
                        crate::HFST_THROW!(EndOfStreamException);
                    }
                    self.stream_unget(c2);
                    self.stream_unget(c1);
                    self.stream_unget(c0);
                    if (c0 as u8) == 0x1f && (c1 as u8) == 0x8b && (c2 as u8) == 0x08 {
                        crate::HFST_THROW!(FileIsInGZFormatException)
                    } else {
                        crate::HFST_THROW!(NotTransducerStreamException)
                    }
                }
                b'a' => {
                    // SFST
                    TransducerType::SFST_
                }
                b'P' => {
                    self.has_hfst_header = false;
                    // extract HFST version 2 header
                    let _ = self.stream_get();
                    let _ = self.stream_get();
                    let _ = self.stream_get();
                    let _ = self.stream_get();
                    *bytes_read = 4;
                    let c5 = self.stream_get();
                    if c5 == 'A' {
                        return TransducerType::HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET;
                    }
                    if c5 == 'a' {
                        self.stream_unget(c5);
                        return TransducerType::HFST_VERSION_2_UNWEIGHTED;
                    } else {
                        debug_error("#3");
                        crate::HFST_THROW!(NotTransducerStreamException)
                    }
                }
                b'A' => {
                    self.has_hfst_header = true;
                    let _ = self.stream_get();
                    *bytes_read = 1;
                    let c2 = self.stream_peek();
                    if c2 == 'a' {
                        return TransducerType::HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET;
                    }
                    if (c2 as u8) == 0xd6 {
                        TransducerType::HFST_VERSION_2_WEIGHTED
                    } else {
                        TransducerType::ERROR_TYPE_
                    }
                }
                0 => TransducerType::XFSM_,
                _ => TransducerType::ERROR_TYPE_,
            }
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.process-header-data-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.process-header-data-fn]
        fn process_header_data(&mut self, header_data: &mut StringPairVector, _warnings: bool) {
            if header_data.len() < 2 {
                crate::HFST_THROW_MESSAGE!(
                    TransducerHeaderException,
                    "Hfst header has too few attributes"
                );
            }

            // (1) first pair "version", "3.0"
            if !(("version" == header_data[0].0.as_str())
                && (("3.0" == header_data[0].1.as_str()) || ("3.3" == header_data[0].1.as_str())))
            {
                crate::HFST_THROW_MESSAGE!(
                    TransducerHeaderException,
                    "Hfst header: transducer version not recognised"
                );
            }

            // (2) second pair "type", (valid type field)
            if !("type" == header_data[1].0.as_str()) {
                crate::HFST_THROW_MESSAGE!(
                    TransducerHeaderException,
                    "Hfst header: transducer type not given"
                );
            }

            if "SFST" == header_data[1].1.as_str() {
                self.type_ = ImplementationType::SFST_TYPE;
            } else if "FOMA" == header_data[1].1.as_str() {
                self.type_ = ImplementationType::FOMA_TYPE;
            } else if "TROPICAL_OPENFST" == header_data[1].1.as_str()
                || "TROPICAL_OFST" == header_data[1].1.as_str()
            {
                self.type_ = ImplementationType::TROPICAL_OPENFST_TYPE;
            } else if "LOG_OPENFST" == header_data[1].1.as_str()
                || "LOG_OFST" == header_data[1].1.as_str()
            {
                self.type_ = ImplementationType::LOG_OPENFST_TYPE;
            } else if "HFST_OL" == header_data[1].1.as_str() {
                self.type_ = ImplementationType::HFST_OL_TYPE;
            } else if "HFST_OLW" == header_data[1].1.as_str() {
                self.type_ = ImplementationType::HFST_OLW_TYPE;
            } else {
                crate::HFST_THROW_MESSAGE!(
                    TransducerHeaderException,
                    "Hfst header: transducer type not recognised"
                );
            }

            if header_data.len() == 2 {
                return;
            }

            // (3) third optional pair "name", (string)
            if header_data[2].0 == "name" {
                self.name = header_data[2].1.clone();
            }
            for prop in header_data.iter() {
                self.props.insert(prop.0.clone(), prop.1.clone());
            }
        }

        /* Try to read a hfst header. If successful, return true and the number
        of bytes read. If not, return false and 0. Throw a
        NotTransducerStreamException if the header cannot
        be parsed after a field "HFST3" or "HFST".
        Throw a TransducerHeaderException if the header data cannot be parsed. */
        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.read-hfst-header-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-hfst-header-fn]
        fn read_hfst_header(&mut self, bytes_read: &mut i32) -> bool {
            let c = self.stream_peek();

            if c != 'H' {
                *bytes_read = 0;
                return false;
            }
            let mut header_bytes: i32 = 0;
            // try to read an HFST version 3.0 header
            if self.read_library_header(&mut header_bytes) {
                let mut size_bytes: i32 = 0;
                let header_size = self.get_header_size(&mut size_bytes); // throws error
                let mut header_info = self.get_header_data(header_size);
                self.process_header_data(&mut header_info, false); // throws error

                *bytes_read = header_bytes + size_bytes + header_size;
                return true;
            }
            header_bytes = 0;
            // try to read a pre-release HFST version 3.0 header
            if self.read_library_header_old(&mut header_bytes) {
                let mut type_bytes: i32 = 0;
                self.type_ = self.get_fst_type_old(&mut type_bytes); // throws error
                if self.type_ == ImplementationType::ERROR_TYPE {
                    crate::HFST_THROW!(NotTransducerStreamException);
                }
                *bytes_read = header_bytes + type_bytes;

                return true;
            }
            false
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-fst-type-old-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-fst-type-old-fn]
        fn get_fst_type_old(&mut self, bytes_read: &mut i32) -> ImplementationType {
            let fst_type = self.stream_getstring();
            if self.stream_eof() {
                debug_error("#5");
                //HFST_THROW(NotTransducerStreamException);
                crate::HFST_THROW!(EndOfStreamException);
            }
            if fst_type == "SFST_TYPE" {
                *bytes_read = 10;
                return ImplementationType::SFST_TYPE;
            }
            if fst_type == "FOMA_TYPE" {
                *bytes_read = 10;
                return ImplementationType::FOMA_TYPE;
            }
            if fst_type == "TROPICAL_OPENFST_TYPE" {
                *bytes_read = 19;
                return ImplementationType::TROPICAL_OPENFST_TYPE;
            }
            if fst_type == "LOG_OPENFST_TYPE" {
                *bytes_read = 14;
                return ImplementationType::LOG_OPENFST_TYPE;
            }
            if fst_type == "HFST_OL_TYPE" {
                *bytes_read = 13;
                return ImplementationType::HFST_OL_TYPE;
            }
            if fst_type == "HFST_OLW_TYPE" {
                *bytes_read = 14;
                return ImplementationType::HFST_OLW_TYPE;
            }
            ImplementationType::ERROR_TYPE
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.read-library-header-old-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-library-header-old-fn]
        fn read_library_header_old(&mut self, bytes_read: &mut i32) -> bool {
            let id = b"HFST3";

            for i in 0..6usize {
                let c = self.stream_get();
                // C++ indexes id[i] of a 6-char buffer "HFST3" (5 chars + NUL); i==5
                // reads the terminating '\0'.
                let id_c = if i < 5 { id[i] as char } else { '\0' };
                if c != id_c {
                    /* No match */
                    self.stream_unget(c);
                    if i > 0 {
                        let mut j: i32 = (i as i32) - 1;
                        while j >= 0 {
                            let jc = if (j as usize) < 5 {
                                id[j as usize] as char
                            } else {
                                '\0'
                            };
                            self.stream_unget(jc);
                            j -= 1;
                        }
                    }
                    *bytes_read = 0;
                    return false;
                }
            }
            *bytes_read = 6;
            true
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.read-library-header-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-library-header-fn]
        fn read_library_header(&mut self, bytes_read: &mut i32) -> bool {
            let id = b"HFST";

            for i in 0..5usize {
                let c = self.stream_get();
                // "HFST" is 4 chars + NUL; i==4 reads the terminating '\0'.
                let id_c = if i < 4 { id[i] as char } else { '\0' };
                if c != id_c {
                    /* No match */
                    self.stream_unget(c);
                    if i > 0 {
                        let mut j: i32 = (i as i32) - 1;
                        while j >= 0 {
                            let jc = if (j as usize) < 4 {
                                id[j as usize] as char
                            } else {
                                '\0'
                            };
                            self.stream_unget(jc);
                            j -= 1;
                        }
                    }
                    *bytes_read = 0;
                    return false;
                }
            }
            *bytes_read = 5;
            true
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-header-size-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-header-size-fn]
        fn get_header_size(&mut self, bytes_read: &mut i32) -> i32 {
            let mut header_size: u16 = 0;
            self.stream_get_ushort_ref(&mut header_size);
            let c = self.stream_get();
            if c != (0 as char) {
                debug_error("#6");
                crate::HFST_THROW_MESSAGE!(
                    NotTransducerStreamException,
                    "HFST header: header size could not be read"
                );
            }
            *bytes_read = 3;

            header_size as i32
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-header-data-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-header-data-fn]
        fn get_header_data(&mut self, header_size: i32) -> StringPairVector {
            let mut retval: StringPairVector = StringPairVector::new();
            let mut bytes_read: i32 = 0;

            loop {
                let str1 = self.stream_getstring();
                let str2 = self.stream_getstring();

                bytes_read = bytes_read + (str1.len() as i32) + (str2.len() as i32) + 2;

                if bytes_read > header_size {
                    debug_error("#7");

                    crate::HFST_THROW_MESSAGE!(
                        NotTransducerStreamException,
                        "HFST header: FATAL: more bytes read than the header contains"
                    );
                }
                if self.stream_eof() {
                    debug_error("#8");
                    crate::HFST_THROW_MESSAGE!(
                        NotTransducerStreamException,
                        "HFST header: FATAL: stream ended before the header could be read"
                    );
                }

                retval.push((str1, str2));
                if bytes_read == header_size {
                    break;
                }
            }
            retval
        }

        /* The implementation type of the first transducer in the stream. */
        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.stream-fst-type-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.stream-fst-type-fn]
        fn stream_fst_type(&mut self) -> ImplementationType {
            let mut bytes_read: i32 = 0;

            // whether the stream contains an HFST version 3.0 transducer
            if self.read_hfst_header(&mut bytes_read) {
                self.has_hfst_header = true;
                self.bytes_to_skip = bytes_read as u32;
                return self.type_;
            }

            // whether the stream contains an HFST version <3.0 transducer
            // or a native SFST, OpenFst, foma or xfsm transducer
            let transducer_type = self.guess_fst_type(&mut bytes_read);
            self.bytes_to_skip = bytes_read as u32;

            match transducer_type {
                TransducerType::HFST_VERSION_2_WEIGHTED => {
                    self.hfst_version_2_weighted_transducer = true;
                    ImplementationType::TROPICAL_OPENFST_TYPE
                }
                TransducerType::HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET => {
                    eprintln!(
                        "ERROR: version 2 HFST transducer with no alphabet  cannot be processed\nAdd an alphabet with HFST version 2 tool hfst-symbols"
                    );
                    ImplementationType::ERROR_TYPE
                }
                TransducerType::HFST_VERSION_2_UNWEIGHTED => ImplementationType::SFST_TYPE,
                TransducerType::OPENFST_TROPICAL_ => ImplementationType::TROPICAL_OPENFST_TYPE,
                TransducerType::OPENFST_LOG_ => ImplementationType::LOG_OPENFST_TYPE,
                TransducerType::SFST_ => ImplementationType::SFST_TYPE,
                TransducerType::FOMA_ => ImplementationType::FOMA_TYPE,
                TransducerType::XFSM_ => ImplementationType::XFSM_TYPE,
                TransducerType::ERROR_TYPE_ => ImplementationType::ERROR_TYPE,
            }
        }

        /* Open a transducer stream to stdout.
        The implementation type of the stream is defined by
        the type of the first transducer in the stream. */
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.hfst-input-stream-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.hfst-input-stream-fn]
        pub fn new() -> Self {
            // input_stream = &std::cin; -- IStream cannot own/borrow std::cin in this
            // skeleton (no source available), and the type-probing + backend dispatch
            // reads from it before constructing the backend stream. Deferred.
            unimplemented!(
                "deferred: HfstInputStream::new — IStream cannot own a std::cin reader (lifetime)"
            )
        }

        // FIXME: HfstOutputStream takes a string parameter,
        //        HfstInputStream a const char*
        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.hfst-input-stream-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.hfst-input-stream-fn]
        pub fn new_filename(_filename: &str) -> Self {
            // The C++ opens an `ifstream`, probes it, then constructs a backend stream
            // from the same filename. `IStream` cannot own an `ifstream` (lifetime),
            // and the SFST/foma/xfsm/hfst_ol backends are absent. Deferred.
            unimplemented!(
                "deferred: HfstInputStream::new_filename — IStream cannot own a file reader (lifetime)"
            )
        }

        // HfstInputStream(std::istream &is)
        pub fn new_istream(_is: IStream<'a>) -> Self {
            // The C++ probes `is` (stream_fst_type) then, for OpenFst/HFST_OL types,
            // constructs a backend stream from the SAME `is`. After type probing the
            // `IStream` would have to be moved into the backend, but probing borrows
            // `self` (which owns the IStream) and the SFST/foma/xfsm/hfst_ol backends
            // are absent. Deferred.
            unimplemented!(
                "deferred: HfstInputStream::new_istream — needs IStream re-homing into backend + absent backends"
            )
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.close-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.close-fn]
        pub fn close(&mut self) {
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    unimplemented!("deferred: SfstInputStream::close (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    self.implementation.tropical_ofst.as_mut().unwrap().close();
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation.log_ofst.as_mut().unwrap().close();
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::close (no foma backend)")
                }
                ImplementationType::XFSM_TYPE => {
                    unimplemented!("deferred: XfsmInputStream::close (no xfsm backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    unimplemented!("deferred: HfstOlInputStream::close (no hfst_ol backend)")
                }
                _ => {
                    assert!(false);
                }
            }
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-eof-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-eof-fn]
        pub fn is_eof(&mut self) -> bool {
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    unimplemented!("deferred: SfstInputStream::is_eof (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    self.implementation.tropical_ofst.as_ref().unwrap().is_eof()
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation.log_ofst.as_ref().unwrap().is_eof()
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::is_eof (no foma backend)")
                }
                ImplementationType::XFSM_TYPE => {
                    unimplemented!("deferred: XfsmInputStream::is_eof (no xfsm backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    unimplemented!("deferred: HfstOlInputStream::is_eof (no hfst_ol backend)")
                }
                _ => {
                    assert!(false);
                    false
                }
            }
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-bad-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-bad-fn]
        pub fn is_bad(&mut self) -> bool {
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    unimplemented!("deferred: SfstInputStream::is_bad (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    self.implementation.tropical_ofst.as_ref().unwrap().is_bad()
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation.log_ofst.as_ref().unwrap().is_bad()
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::is_bad (no foma backend)")
                }
                ImplementationType::XFSM_TYPE => {
                    unimplemented!("deferred: XfsmInputStream::is_bad (no xfsm backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    unimplemented!("deferred: HfstOlInputStream::is_bad (no hfst_ol backend)")
                }
                _ => {
                    assert!(false);
                    false
                }
            }
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-good-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-good-fn]
        pub fn is_good(&mut self) -> bool {
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    unimplemented!("deferred: SfstInputStream::is_good (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => self
                    .implementation
                    .tropical_ofst
                    .as_ref()
                    .unwrap()
                    .is_good(),
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation.log_ofst.as_ref().unwrap().is_good()
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::is_good (no foma backend)")
                }
                ImplementationType::XFSM_TYPE => {
                    unimplemented!("deferred: XfsmInputStream::is_good (no xfsm backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    unimplemented!("deferred: HfstOlInputStream::is_good (no hfst_ol backend)")
                }
                _ => {
                    assert!(false);
                    false
                }
            }
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-type-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-type-fn]
        pub fn get_type(&self) -> ImplementationType {
            self.type_
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-hfst-header-included-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-hfst-header-included-fn]
        pub fn is_hfst_header_included(&self) -> bool {
            self.has_hfst_header
        }
    }
}
