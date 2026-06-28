//! Port of 'libhfst/src/HfstInputStream.{h,cc}' — 'hfst::HfstInputStream', the
//! stream for reading HFST binary transducers.
//!
//! @FIXME (from the C++): the structure of this class and its functions is
//! disorganised; the port mirrors it 1:1.
//!
//! ## Ownership redesign (owned reader)
//! The C++ holds a raw 'std::istream * input_stream' pointing at 'std::cin' / an
//! 'std::ifstream' / a caller 'std::istream', probes the first transducer's
//! header through it, then constructs a backend stream from the SAME source. The
//! original Rust skeleton tried to model 'input_stream' with ['IStream']'<'a>',
//! which only BORROWS '&'a mut dyn Read' and offers no putback — so it could
//! neither own 'std::cin'/a file nor support the heavy 'stream_unget' the header
//! probing needs. Both made the constructors and 'read_transducer'
//! 'unimplemented!'.
//!
//! This port fixes that: 'HfstInputStream' OWNS its reader via ['PushbackReader'],
//! a heap-pinned buffered reader (a 'Box<dyn Read>' plus an unget stack) that
//! models 'std::istream''s 'get' / 'putback' / 'peek' / 'eof'. The reader is held
//! behind a raw pointer ('reader') so the in-scope backend streams can borrow the
//! very same source via an ['IStream'] built from that pointer (the C++
//! shares one underlying stream; we share one owned reader). 'reader' is freed in
//! ['Drop'].
//!
//! ## Backend union modelling
//! The C++ holds a 'union StreamImplementation' of raw backend-stream pointers;
//! exactly one member is live, selected by 'type'. Mirrored here as
//! ['StreamImplementation'] (a struct of per-backend 'Option<Box<...>>'), kept for
//! fidelity. In this single-owned-reader port the in-scope backend stream is built
//! transiently inside 'read_transducer' (it borrows the owned reader), so the
//! field stays empty; the stream-state queries 'is_eof'/'is_bad'/'is_good' read
//! the owned reader directly (the reader IS the shared stream). 'sfst'/'foma'/
//! 'xfsm' are unported collaborators (deferred placeholders).
//!
//! ## In-scope reads
//!   * TROPICAL_OPENFST / LOG_OPENFST: after the HFST header is consumed, the
//!     remaining bytes are the OpenFst/rustfst 'VectorFst' payload that
//!     'HfstOutputStream' wrote via 'store()'; we slurp them and rebuild with
//!     'SerializableFst::load'.
//!   * HFST_OL / HFST_OLW: built through the implemented
//!     'HfstOlInputStream::read_transducer' / 'Transducer::new_istream'.
//! SFST/FOMA/XFSM stay deferred (no backend).

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};

use hfst_openfst::StdVectorFst;

use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_data_types::{ImplementationType, StringPairVector};
use crate::hfst_exception_defs::{
    EndOfStreamException, FileIsInGZFormatException, HfstFatalException,
    ImplementationTypeNotAvailableException, NotTransducerStreamException,
    TransducerHeaderException, TransducerTypeMismatchException,
};
use crate::hfst_ol_transducer::HfstOlInputStream as HfstOlBackendInputStream;
use crate::hfst_transducer::HfstTransducer;
use crate::log_weight_transducer::{LogFst, LogWeightInputStream, LogWeightTransducer};
use crate::transducer::IStream;
use crate::tropical_weight_transducer::{TropicalWeightInputStream, TropicalWeightTransducer};

/// Unported backend input-stream collaborators. Each is a placeholder unit type:
/// the corresponding ['StreamImplementation'] field exists for fidelity with the
/// C++ union, but no constructor is provided (the '#if HAVE_SFST' / 'HAVE_FOMA' /
/// 'HAVE_XFSM' paths are deferred).
pub struct SfstInputStream;
pub struct FomaInputStream;
pub struct XfsmInputStream;

/// 'std::istream'-like owned reader used to probe the HFST header. It owns the
/// underlying byte source ('Box<dyn Read>': a file, stdin, ...) and an unget
/// stack, so it supports 'get' / 'unget' / 'peek' / 'eof' — exactly what the
/// header probing requires (the borrowing ['IStream'] cannot). It also implements
/// ['Read'] (draining the unget stack first) so a backend ['IStream'] can keep
/// reading the same source after the header.
pub struct PushbackReader<'a> {
    inner: Box<dyn Read + 'a>,
    /// Unget stack: bytes are pushed by 'unget' and popped LIFO. The probing code
    /// ungets in reverse order, so popping restores the original order.
    pushback: Vec<u8>,
    /// 'std::istream' eofbit: set once a read tried to go past end.
    eof: bool,
    /// 'std::istream' failbit/badbit (collapsed): set on a failed read.
    fail: bool,
}

impl<'a> PushbackReader<'a> {
    fn new(inner: Box<dyn Read + 'a>) -> Self {
        PushbackReader {
            inner,
            pushback: Vec::new(),
            eof: false,
            fail: false,
        }
    }

    /// Models '(char) std::istream::get()': returns the next byte, or '0xFF' (the
    /// truncation of 'EOF' == -1) plus eofbit/failbit on end of stream. Callers
    /// distinguish a real '0xFF' byte from EOF via 'eof', exactly as the C++ tests
    /// 'stream_eof()' rather than the returned value.
    fn get(&mut self) -> u8 {
        if let Some(b) = self.pushback.pop() {
            return b;
        }
        let mut one = [0u8; 1];
        match self.inner.read(&mut one) {
            Ok(0) => {
                self.eof = true;
                self.fail = true;
                0xFF
            }
            Ok(_) => one[0],
            Err(_) => {
                self.fail = true;
                0xFF
            }
        }
    }

    /// Models 'std::istream::putback(c)': pushes a byte back and clears the
    /// eof/fail bits (a successful putback makes the stream good again).
    fn unget(&mut self, b: u8) {
        self.pushback.push(b);
        self.eof = false;
        self.fail = false;
    }

    /// Push a run of bytes back so that subsequent reads return them in their
    /// original order. The unget stack is LIFO ('get' pops the end), so the run
    /// is pushed reversed. Used to restore the bytes of the following
    /// transducer(s) after one FST payload has been parsed out of a slurped
    /// multi-transducer stream.
    fn unget_all(&mut self, bytes: &[u8]) {
        self.pushback.extend(bytes.iter().rev());
        self.eof = false;
        self.fail = false;
    }

    /// Models 'std::istream::ignore(n)': discard 'n' bytes.
    fn ignore(&mut self, n: u32) {
        for _ in 0..n {
            let _ = self.get();
        }
    }
}

impl<'a> Read for PushbackReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut n = 0;
        while n < buf.len() {
            match self.pushback.pop() {
                Some(b) => {
                    buf[n] = b;
                    n += 1;
                }
                None => break,
            }
        }
        if n == buf.len() {
            return Ok(n);
        }
        match self.inner.read(&mut buf[n..]) {
            Ok(m) => Ok(n + m),
            Err(e) => {
                if n > 0 {
                    Ok(n)
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Port of the C++ 'union StreamImplementation' (the backend implementation).
/// Kept for fidelity with the C++ union; in this single-owned-reader port the
/// in-scope backend stream is built transiently in 'read_transducer', so these
/// fields stay 'None'. The tropical/log/hfst_ol members carry the reader lifetime
/// ('IStream<'a>'); the rest are deferred placeholders.
// [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-implementation]
#[derive(Default)]
pub struct StreamImplementation<'a> {
    pub sfst: Option<Box<SfstInputStream>>,
    pub tropical_ofst: Option<Box<TropicalWeightInputStream<'a>>>,
    pub log_ofst: Option<Box<LogWeightInputStream<'a>>>,
    pub foma: Option<Box<FomaInputStream>>,
    pub xfsm: Option<Box<XfsmInputStream>>,
    pub hfst_ol: Option<Box<HfstOlBackendInputStream<'a>>>,
}

/// The type of a transducer not supported directly by HFST version 3.0 but which
/// can occur in conversion functions. (C++ nested 'enum HfstInputStream::TransducerType'.)
// [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.transducer-type]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransducerType {
    /// See 'hfst_version_2_weighted_transducer'.
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
    /// a 'FileIsInGZFormatException'.
    FOMA_,
    /// An xfsm transducer.
    XFSM_,
    /// Transducer type not recognized.
    ERROR_TYPE_,
}

/// A stream for reading HFST binary transducers.
///
/// @see 'HfstTransducer::HfstTransducer(HfstInputStream &in)'
// [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream]
pub struct HfstInputStream<'a> {
    /// The backend implementation (C++ 'StreamImplementation implementation').
    /// Fidelity placeholder; see ['StreamImplementation'].
    implementation: StreamImplementation<'a>,
    /// Implementation type (the discriminant selecting the live 'implementation' member).
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
    /// The owned reader used to probe the first transducer's header (and shared,
    /// via raw pointer, with the in-scope backend stream). Heap-pinned behind a
    /// raw pointer; freed in 'Drop'. Mirrors C++ 'std::istream * input_stream' in
    /// that it backs all the 'stream_*' primitives.
    reader: *mut PushbackReader<'a>,
    /// Mirrors C++ 'input_stream != NULL': 'true' while the first transducer's
    /// type is still being established (reads behave like the raw 'input_stream'),
    /// 'false' once 'read_transducer' has taken over.
    input_stream_active: bool,
}

impl<'a> Drop for HfstInputStream<'a> {
    fn drop(&mut self) {
        if !self.reader.is_null() {
            // Frees the owned reader. The backend collaborators in
            // 'implementation' are 'None' in this port, so no dangling borrow.
            drop(unsafe { Box::from_raw(self.reader) });
            self.reader = std::ptr::null_mut();
        }
    }
}

// ===== input_impl (workflow body) =====
mod input_impl {
    #![allow(unused_imports)]
    use super::*;
    // [spec:hfst:def:hfst-input-stream.hfst.debug-error-fn]
    // [spec:hfst:sem:hfst-input-stream.hfst.debug-error-fn]
    pub fn debug_error(_msg: &str) {
        // #if PRINT_DEBUG_MESSAGES -> fprintf(stderr, ...); #else (void)msg; #endif
        // PRINT_DEBUG_MESSAGES is off, so this is a no-op (mirrors '(void)msg;').
    }

    impl<'a> HfstInputStream<'a> {
        /// Borrow the owned reader. Sound for the single-threaded, non-aliasing way
        /// the 'stream_*' primitives use it (one call at a time).
        #[inline]
        fn pbr(&mut self) -> &mut PushbackReader<'a> {
            unsafe { &mut *self.reader }
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.ignore-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.ignore-fn]
        fn ignore(&mut self, n: u32) {
            // C++ 'switch (type)' dispatches to the active backend's 'ignore'. In
            // this single-owned-reader port the reader IS the shared stream.
            self.pbr().ignore(n);
        }

        fn stream_get_char_ref(&mut self, c: &mut char) -> char {
            *c = self.pbr().get() as char;
            *c
        }

        fn stream_get_short_ref(&mut self, i: &mut i16) -> i16 {
            let mut b = [0u8; 2];
            b[0] = self.pbr().get();
            b[1] = self.pbr().get();
            *i = i16::from_ne_bytes(b);
            *i
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
            // C++: if (input_stream != NULL) return (char) input_stream->get();
            // else dispatch to the backend. Here the owned reader serves both.
            self.pbr().get() as char
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-unget-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-unget-fn]
        fn stream_unget(&mut self, c: char) {
            self.pbr().unget(c as u32 as u8);
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
            // The C++ stream is byte-oriented (std::string of bytes); stream_get
            // hands back one byte per call as a 0-255 char. Collect the raw bytes
            // up to the NUL terminator and decode once as UTF-8. Pushing each
            // byte-valued char straight into a Rust String would re-encode every
            // byte >= 0x80 as multi-byte UTF-8, corrupting both the value and its
            // length (str.len() would exceed the bytes consumed, overrunning the
            // header-length accounting in get_header_data).
            let mut bytes: Vec<u8> = Vec::new();
            loop {
                let c = self.stream_get();
                if self.stream_eof() {
                    crate::HFST_THROW!(EndOfStreamException);
                }
                if c == '\0' {
                    break;
                }
                bytes.push(c as u8);
            }
            String::from_utf8_lossy(&bytes).into_owned()
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-eof-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-eof-fn]
        fn stream_eof(&mut self) -> bool {
            if self.input_stream_active {
                // C++ 'input_stream->eof()': the eofbit, set only once a read went
                // past the end.
                return self.pbr().eof;
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

        /// Slurp every remaining byte from the owned reader (unget stack first,
        /// then the underlying source). Used to hand the OpenFst/rustfst
        /// 'VectorFst' payload to 'SerializableFst::load'.
        fn read_remaining_bytes(&mut self) -> Vec<u8> {
            let reader = self.pbr();
            let mut buf: Vec<u8> = Vec::new();
            let _ = Read::read_to_end(reader, &mut buf);
            buf
        }

        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-transducer-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-transducer-fn]
        // Reads the next transducer from the stream and stores it in 't'. The C++
        // calls 'implementation.X->read_transducer()'; the tropical/log backend
        // 'read_transducer' is deferred (rustfst exposes only the whole-buffer
        // 'load'), so for those types we read the payload directly off the owned
        // reader and rebuild the fst here.
        pub fn read_transducer(&mut self, t: &mut HfstTransducer) {
            if self.type_ != ImplementationType::XFSM_TYPE {
                if self.input_stream_active {
                    // first transducer in the stream
                    self.input_stream_active = false;
                    if self.stream_eof() {
                        crate::HFST_THROW!(EndOfStreamException);
                    }
                    // C++ re-opens the backend stream by filename and skips the
                    // already-read header bytes here ('ignore(bytes_to_skip)'). In
                    // this single-owned-reader port the probe already consumed the
                    // header from the one reader, so there is nothing to skip.
                } else {
                    if self.stream_eof() {
                        crate::HFST_THROW!(EndOfStreamException);
                    }
                    let current_type = self.get_type();
                    let stype = self.stream_fst_type();
                    if stype != current_type {
                        crate::HFST_THROW_MESSAGE!(
                            TransducerTypeMismatchException,
                            "HfstInputStream contains HfstTransducers whose type is not the same"
                        );
                    }
                }
            }

            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    // implementation.sfst->read_transducer() — no SFST backend.
                    unimplemented!("deferred: SfstInputStream::read_transducer (no SFST backend)")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    // Slurp the rest of the stream, parse exactly ONE FST off the
                    // front, and unget the leftover (any following transducers in a
                    // multi-transducer HFST stream) so the next 'read_transducer'
                    // re-reads them. The C++ backend stream reads one OpenFst record
                    // and leaves the istream positioned after it; load_prefix mirrors
                    // that by reporting how many bytes the single FST consumed.
                    let bytes = self.read_remaining_bytes();
                    let (fst, consumed) = match StdVectorFst::load_prefix(&bytes) {
                        Ok(fc) => fc,
                        Err(_) => crate::HFST_THROW_MESSAGE!(
                            NotTransducerStreamException,
                            "could not read TROPICAL_OPENFST transducer payload"
                        ),
                    };
                    if consumed < bytes.len() {
                        self.pbr().unget_all(&bytes[consumed..]);
                    }
                    t.implementation.tropical_ofst = Box::into_raw(Box::new(fst));

                    /* If we were reading an OpenFst transducer with no HFST header,
                    round-trip it through HfstBasicTransducer to normalise its
                    symbol tables / epsilon-unknown-identity coding. */
                    if !self.has_hfst_header {
                        let net = ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(
                            unsafe { &*t.implementation.tropical_ofst },
                            false,
                        );
                        TropicalWeightTransducer::delete_transducer(unsafe {
                            *Box::from_raw(t.implementation.tropical_ofst)
                        });
                        t.implementation.tropical_ofst = Box::into_raw(Box::new(
                            ConversionFunctions::hfst_basic_transducer_to_tropical_ofst(&net),
                        ));
                    }

                    // A special case: HFST version 2 transducer with an appended
                    // SFST alphabet. It needs the backend 'stream_get' loop, which
                    // is not available in this port.
                    if self.hfst_version_2_weighted_transducer {
                        unimplemented!(
                            "deferred: HFST version 2 weighted transducer (appended SFST alphabet)"
                        );
                    }
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    let bytes = self.read_remaining_bytes();
                    let (fst, consumed) = match LogFst::load_prefix(&bytes) {
                        Ok(fc) => fc,
                        Err(_) => crate::HFST_THROW_MESSAGE!(
                            NotTransducerStreamException,
                            "could not read LOG_OPENFST transducer payload"
                        ),
                    };
                    if consumed < bytes.len() {
                        self.pbr().unget_all(&bytes[consumed..]);
                    }
                    t.implementation.log_ofst = Box::into_raw(Box::new(fst));

                    if !self.has_hfst_header {
                        let net = ConversionFunctions::log_ofst_to_hfst_basic_transducer(
                            unsafe { &*t.implementation.log_ofst },
                            false,
                        );
                        LogWeightTransducer::delete_transducer(unsafe {
                            *Box::from_raw(t.implementation.log_ofst)
                        });
                        t.implementation.log_ofst = Box::into_raw(Box::new(
                            ConversionFunctions::hfst_basic_transducer_to_log_ofst(&net),
                        ));
                    }

                    if self.hfst_version_2_weighted_transducer {
                        // this should not happen
                        crate::HFST_THROW_MESSAGE!(HfstFatalException, "not transducer stream");
                    }
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream::read_transducer (no foma backend)")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    let weighted = self.type_ == ImplementationType::HFST_OLW_TYPE;
                    // Build a transient backend stream that borrows the owned
                    // reader (positioned just after the header that probing
                    // consumed), then read with has_header = false.
                    let reader: &'a mut PushbackReader<'a> = unsafe { &mut *self.reader };
                    let is = IStream::new(reader);
                    let mut ol_in = HfstOlBackendInputStream::new_istream(is, weighted);
                    let tr = ol_in.read_transducer(false);
                    t.implementation.hfst_ol = Box::into_raw(Box::new(tr));
                    if t.get_type() != self.type_ {
                        // weights need to be added or removed
                        t.convert(self.type_, String::new());
                    }
                }
                // case ERROR_TYPE: default:
                _ => {
                    debug_error("#1");
                    crate::HFST_THROW!(NotTransducerStreamException);
                }
            }

            if self.type_ != ImplementationType::XFSM_TYPE {
                let nm = self.name.clone();
                t.set_name(&nm);
                let props: Vec<(String, String)> = self
                    .props
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                for (k, v) in props {
                    t.set_property(&k, &v);
                }
            }
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
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-hfst-header-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-hfst-header-fn]
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
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.get-fst-type-old-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.get-fst-type-old-fn]
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
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-library-header-old-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-library-header-old-fn]
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
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-library-header-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-library-header-fn]
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
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.get-header-size-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.get-header-size-fn]
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
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.get-header-data-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.get-header-data-fn]
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
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-fst-type-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-fst-type-fn]
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

        /// Shared constructor body: own 'inner', probe the header, validate the
        /// type. (The C++ constructors differ only in how 'input_stream' is seeded
        /// and in re-constructing a per-type backend stream, which this port builds
        /// transiently at read time.)
        fn new_with_reader(inner: Box<dyn Read + 'a>, filename: String) -> Self {
            let mut this = HfstInputStream {
                implementation: StreamImplementation::default(),
                type_: ImplementationType::ERROR_TYPE,
                name: String::new(),
                props: BTreeMap::new(),
                bytes_to_skip: 0,
                filename,
                has_hfst_header: false,
                hfst_version_2_weighted_transducer: false,
                reader: Box::into_raw(Box::new(PushbackReader::new(inner))),
                input_stream_active: true,
            };

            if this.stream_eof() {
                crate::HFST_THROW!(EndOfStreamException);
            }
            this.type_ = this.stream_fst_type();

            if !HfstTransducer::is_lean_implementation_type_available(this.type_) {
                std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                    "ImplementationTypeNotAvailableException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                    this.type_,
                ));
            }

            // C++ 'switch (type)' constructs the per-type backend stream here. We
            // build it transiently in 'read_transducer'; this switch only rejects
            // the unsupported / unrecognised types up front.
            match this.type_ {
                ImplementationType::TROPICAL_OPENFST_TYPE
                | ImplementationType::LOG_OPENFST_TYPE
                | ImplementationType::HFST_OL_TYPE
                | ImplementationType::HFST_OLW_TYPE => {}
                ImplementationType::SFST_TYPE => {
                    unimplemented!("deferred: SfstInputStream (no SFST backend)")
                }
                ImplementationType::FOMA_TYPE => {
                    unimplemented!("deferred: FomaInputStream (no foma backend)")
                }
                ImplementationType::XFSM_TYPE => {
                    unimplemented!("deferred: XfsmInputStream (no xfsm backend)")
                }
                _ => {
                    debug_error("#10");
                    crate::HFST_THROW_MESSAGE!(
                        NotTransducerStreamException,
                        "transducer type not recognised"
                    );
                }
            }

            this
        }

        /* Open a transducer stream to stdin.
        The implementation type of the stream is defined by
        the type of the first transducer in the stream. */
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.hfst-input-stream-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.hfst-input-stream-fn]
        pub fn new() -> Self {
            // C++ 'input_stream = &std::cin;'
            Self::new_with_reader(Box::new(std::io::stdin()), String::new())
        }

        // FIXME: HfstOutputStream takes a string parameter,
        //        HfstInputStream a const char*
        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.hfst-input-stream-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.hfst-input-stream-fn]
        pub fn new_filename(filename: &str) -> Self {
            if !filename.is_empty() {
                let f = match File::open(filename) {
                    Ok(f) => f,
                    Err(_) => crate::HFST_THROW_MESSAGE!(
                        NotTransducerStreamException,
                        "file could not be opened"
                    ),
                };
                Self::new_with_reader(Box::new(BufReader::new(f)), filename.to_string())
            } else {
                Self::new_with_reader(Box::new(std::io::stdin()), String::new())
            }
        }

        // HfstInputStream(std::istream &is)
        pub fn new_istream(is: IStream<'a>) -> Self {
            // C++ 'input_stream = &is;' — adopt the (possibly borrowed) stream as
            // this 'HfstInputStream's source, then probe its first transducer's
            // header in 'new_with_reader' exactly like the other constructors. The
            // 'PushbackReader'/'HfstInputStream' lifetime parameter lets a borrowed
            // 'IStream<'a>' back the stream for 'a.
            Self::new_with_reader(is.into_reader(), String::new())
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.close-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.close-fn]
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.close-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.close-fn]
        pub fn close(&mut self) {
            // C++ 'switch (type)' dispatches to the active backend's 'close' (for a
            // file it closes the handle). The owned reader is freed in 'Drop'; for
            // stdin nothing is done. Either way there is nothing to do here.
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-eof-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-eof-fn]
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.is-eof-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.is-eof-fn]
        pub fn is_eof(&mut self) -> bool {
            // C++ dispatches to the active backend's 'is_eof', which peeks the
            // shared stream ('peek() == EOF'). The owned reader IS that stream.
            let c = self.pbr().get();
            if self.pbr().eof {
                return true;
            }
            self.pbr().unget(c);
            false
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-bad-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-bad-fn]
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.is-bad-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.is-bad-fn]
        pub fn is_bad(&mut self) -> bool {
            // C++ backend 'is_bad' ~ '!stream.good()'.
            self.pbr().fail
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-good-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-good-fn]
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.is-good-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.is-good-fn]
        pub fn is_good(&mut self) -> bool {
            // C++ backend 'is_good': false at eof, else 'stream.good()'.
            if self.is_eof() {
                return false;
            }
            !self.pbr().fail
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-type-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-type-fn]
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.get-type-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.get-type-fn]
        pub fn get_type(&self) -> ImplementationType {
            self.type_
        }

        // [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-hfst-header-included-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-hfst-header-included-fn]
        // [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.is-hfst-header-included-fn]
        // [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.is-hfst-header-included-fn]
        pub fn is_hfst_header_included(&self) -> bool {
            self.has_hfst_header
        }
    }
}
