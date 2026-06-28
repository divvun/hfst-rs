//! Port of 'libhfst/src/implementations/TropicalWeightTransducer.{h,cc}' — the
//! OpenFST tropical-weight backend bridge between the HFST API and rustfst's
//! 'VectorFst<TropicalWeight>' (= ['StdVectorFst']).
//!
//! In C++ 'TropicalWeightTransducer' is a class of (almost entirely) STATIC
//! methods — a stateless operations wrapper over 'fst::StdVectorFst'. It is
//! modelled here as a unit struct ['TropicalWeightTransducer'] with an 'impl'
//! block of associated functions. The two stream helper classes
//! (['TropicalWeightInputStream'] / ['TropicalWeightOutputStream']) become their
//! own structs, and the 'StdArcLessThan' comparator becomes a small struct.
//!
//! 'using namespace fst;' in the C++ header is mapped onto the
//! 'hfst-openfst' adapter: 'StdVectorFst', 'StdTransition' (= 'fst::StdArc'),
//! 'TropicalWeight', 'SymbolTable', 'StateId', and the 'algorithms::' module.
//!
//! Ownership mapping for the C++ 'StdVectorFst*' signatures:
//! - factory / unary-op methods that the C++ 'new's a result and returns
//!   'StdVectorFst*' -> return owned 'StdVectorFst'.
//! - methods that take a 'StdVectorFst*' and read it -> '&StdVectorFst'.
//! - methods that take a 'StdVectorFst*' and mutate it in place (state/arc
//!   builders, 'add_to_weights', symbol-table setters, ...) -> '&mut StdVectorFst'.
//! - 'delete_transducer(StdVectorFst*)' -> takes 'StdVectorFst' by value (drops it).
//! The C++ 'int64' typedef is 'i64' here.

#![allow(non_snake_case)]
#![allow(dead_code)] // many ported ops are only reached once the facade lands

use std::collections::{BTreeMap, BTreeSet};

use hfst_openfst::algorithms;
use hfst_openfst::prelude::*;
use hfst_openfst::{StdTransition, StdVectorFst, SymbolTable, TropicalWeight};

use crate::hfst_data_types::{
    HfstTwoLevelPaths, StringPair, StringPairSet, StringPairVector, StringVector,
};
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_flag_diacritics::FdTable;
use crate::hfst_symbol_defs::{
    NumberNumberMap, NumberPair, NumberPairSet, NumberPairVector, internal_epsilon,
    internal_identity, internal_unknown,
};
use crate::transducer::IStream;

// [spec:hfst:def:tropical-weight-transducer.int64]
pub type i64_ = i64;

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.state-id]
pub type StateId = u32;

/// 'typedef std::set<std::string> StringSet' (used by the alphabet helpers).
pub type StringSet = BTreeSet<String>;

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-vector]
pub type StdArcVector = Vec<StdTransition>;

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-less-than]
pub struct StdArcLessThan;

#[allow(dead_code)]
impl StdArcLessThan {
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-less-than.operator-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.std-arc-less-than.operator-fn]
    // Standard StdArc strict ordering (ilabel, then olabel, then weight, then
    // target state). The C++ declares this comparator but never defines or uses
    // it; a faithful total order keeps it correct rather than panicking.
    pub fn operator_call(&self, arc1: &StdTransition, arc2: &StdTransition) -> bool {
        if arc1.ilabel != arc2.ilabel {
            return arc1.ilabel < arc2.ilabel;
        }
        if arc1.olabel != arc2.olabel {
            return arc1.olabel < arc2.olabel;
        }
        if arc1.weight.value() != arc2.weight.value() {
            return arc1.weight.value() < arc2.weight.value();
        }
        arc1.nextstate < arc2.nextstate
    }
}

/// 'void openfst_tropical_set_hopcroft(bool value);'

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream]
pub struct TropicalWeightInputStream<'a> {
    filename: String,
    /// C++ holds an 'std::ifstream i_stream' plus an 'std::istream &input_stream'
    /// reference that aliases either 'i_stream' or 'std::cin'. Modelled here as a
    /// single owned binary input stream (per the porting convention,
    /// 'std::istream' (binary) -> 'crate::transducer::IStream').
    input_stream: IStream<'a>,
}

// (no Default: TropicalWeightInputStream borrows its reader and cannot be
// constructed without one; the no-source ctors are deferred.)

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream]
pub struct TropicalWeightOutputStream {
    filename: String,
    /// C++ holds 'std::ofstream o_stream' + 'std::ostream &output_stream' that
    /// aliases either it or 'std::cout'. Modelled as a single owned writer.
    output_stream: Box<dyn std::io::Write>,
    hfst_format: bool,
}

/* Maps state numbers in AT&T text format to state ids used by OpenFst. */
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.state-map]
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.state-map]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.state-map]
pub type StateMap = BTreeMap<i32, StateId>;

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer]
pub struct TropicalWeightTransducer;

// ===== construction-io (workflow body) =====
mod construction_io {
    #![allow(unused_imports)]
    use super::*;
    // ===========================================================================
    // AREA: construction-io  (bodies for TropicalWeightTransducer.{h,cc})
    //
    // Extra imports needed beyond the skeleton header (integrator: merge/dedupe):
    use std::io::Write;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::hfst_exception_defs::{
        NotValidAttFormatException, StreamIsClosedException, SymbolNotFoundException,
        TransducerHasWrongTypeException,
    };
    use crate::hfst_flag_diacritics::FdOperation;
    // 'HfstFatalException' is referenced by the (deferred) read_transducer path.
    #[allow(unused_imports)]
    use crate::hfst_exception_defs::HfstFatalException;

    // ---------------------------------------------------------------------------
    // File-static globals from the .cc
    // ---------------------------------------------------------------------------

    // 'float tropical_seconds = 0;' — only ever non-zero under PROFILE_OPENFST,
    // which is compiled out here.
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-profile-seconds-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-profile-seconds-fn]
    // (the getter is an associated fn on TropicalWeightTransducer below)

    // 'bool openfst_tropical_use_hopcroft = false;'
    static OPENFST_TROPICAL_USE_HOPCROFT: AtomicBool = AtomicBool::new(false);

    // 'std::ostream * TropicalWeightTransducer::warning_stream = NULL;'
    static mut WARNING_STREAM: *mut Box<dyn std::io::Write> = std::ptr::null_mut();

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.openfst-tropical-set-hopcroft-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.openfst-tropical-set-hopcroft-fn]
    pub fn openfst_tropical_set_hopcroft(value: bool) {
        OPENFST_TROPICAL_USE_HOPCROFT.store(value, Ordering::Relaxed);
    }

    /// Reader for the 'openfst_tropical_use_hopcroft' global (added so the
    /// operations-area 'minimize' can consult the flag — not in the C++ header).
    pub(crate) fn openfst_tropical_get_hopcroft() -> bool {
        OPENFST_TROPICAL_USE_HOPCROFT.load(Ordering::Relaxed)
    }

    // ---------------------------------------------------------------------------
    // Private module helpers (introduced for the port; not in the C++ header).
    // ---------------------------------------------------------------------------

    /// 'std::ostream'-style sink wrapping a C 'FILE *' (used by the 'FILE *'
    /// AT&T writers and 'print_att_number').
    struct CFileWriter(*mut libc::FILE);

    impl std::io::Write for CFileWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let n =
                unsafe { libc::fwrite(buf.as_ptr() as *const libc::c_void, 1, buf.len(), self.0) };
            Ok(n)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// '%f' (FILE* 'fprintf') prints 6 decimals; 'operator<<' (ostream) uses the
    /// default float formatting. We approximate the latter with 'Display'.
    fn fmt_w(w: f32, c_style: bool) -> String {
        if c_style {
            format!("{:.6}", w)
        } else {
            format!("{}", w)
        }
    }

    /// State-number printing rule shared by the AT&T writers: the initial state is
    /// always printed as 0, and (real) state 0 is printed as the initial state's
    /// number (the two are swapped).
    fn att_origin(s: StateId, initial_state: StateId, zero_print: StateId) -> i64 {
        if s == 0 {
            zero_print as i64
        } else if s == initial_state {
            0
        } else {
            s as i64
        }
    }

    fn write_att_state(
        t: &StdVectorFst,
        os: &mut dyn Write,
        s: StateId,
        number: bool,
        c_style: bool,
        initial_state: StateId,
        zero_print: StateId,
    ) {
        let origin = att_origin(s, initial_state, zero_print);
        for arc in t.get_trs(s).unwrap().trs() {
            let target = att_origin(arc.nextstate, initial_state, zero_print);
            let w = *arc.weight.value();
            if number {
                let _ = write!(
                    os,
                    "{}\t{}\t\\{}\t\\{}\t{}\n",
                    origin,
                    target,
                    arc.ilabel,
                    arc.olabel,
                    fmt_w(w, c_style)
                );
            } else {
                let st = t.input_symbols().unwrap();
                let isym = st.get_symbol(arc.ilabel).unwrap_or("");
                let osym = st.get_symbol(arc.olabel).unwrap_or("");
                let _ = write!(
                    os,
                    "{}\t{}\t{}\t{}\t{}\n",
                    origin,
                    target,
                    isym,
                    osym,
                    fmt_w(w, c_style)
                );
            }
        }
        if t.is_final(s).unwrap() {
            let fw = *t.final_weight(s).unwrap().unwrap().value();
            let _ = write!(os, "{}\t{}\n", origin, fmt_w(fw, c_style));
        }
    }

    fn write_in_att_format_core(t: &StdVectorFst, os: &mut dyn Write, number: bool, c_style: bool) {
        if !number {
            assert!(t.input_symbols().is_some());
        }
        let initial_state = t.start().unwrap_or(NO_STATE_ID);
        let mut zero_print: StateId = 0;
        if initial_state != 0 {
            zero_print = initial_state;
        }

        // pass 1: the initial state only
        for s in t.states_iter() {
            if s == initial_state {
                write_att_state(t, os, s, number, c_style, initial_state, zero_print);
                break;
            }
        }
        // pass 2: the rest
        for s in t.states_iter() {
            if s != initial_state {
                write_att_state(t, os, s, number, c_style, initial_state, zero_print);
            }
        }
    }

    /// C 'atof': parse a leading float, 0.0 on failure (Rust 'parse' is stricter —
    /// trailing garbage is not tolerated, a faithfulness gap).
    fn att_atof(s: &str) -> f32 {
        s.trim().parse::<f32>().unwrap_or(0.0)
    }

    /// C 'atoi': parse a leading int, 0 on failure.
    fn att_atoi(s: &str) -> i32 {
        s.trim().parse::<i32>().unwrap_or(0)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.print-att-number-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.print-att-number-fn]
    #[allow(dead_code)]
    pub fn print_att_number(t: &StdVectorFst, ofile: *mut libc::FILE) {
        let mut w = CFileWriter(ofile);
        let _ = write!(
            w,
            "initial state: {}\n",
            t.start().map(|s| s as i64).unwrap_or(-1)
        );
        for s in t.states_iter() {
            if t.is_final(s).unwrap() {
                let fw = *t.final_weight(s).unwrap().unwrap().value();
                let _ = write!(w, "{}\t{:.6}\n", s, fw);
            }
            for arc in t.get_trs(s).unwrap().trs() {
                let _ = write!(
                    w,
                    "{}\t{}\t{}\t{}\t{:.6}\n",
                    s,
                    arc.nextstate,
                    arc.ilabel,
                    arc.olabel,
                    *arc.weight.value()
                );
            }
        }
    }

    // ===========================================================================
    // TropicalWeightInputStream
    // ===========================================================================

    #[allow(dead_code)]
    impl<'a> TropicalWeightInputStream<'a> {
        /// 'TropicalWeightInputStream(void)' — reads from stdin.
        pub fn new() -> Self {
            // C++ reads from std::cin; own a stdin reader.
            TropicalWeightInputStream {
                filename: String::new(),
                input_stream: IStream::new_owned(std::io::stdin()),
            }
        }

        /// 'TropicalWeightInputStream(const std::string &filename)'.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.tropical-weight-input-stream-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.tropical-weight-input-stream-fn]
        pub fn new_filename(filename: &str) -> Self {
            // C++ opens an ifstream in binary mode; own the opened file. A failed
            // open yields an empty reader, leaving the stream in the not-good
            // state the C++ would also be in.
            let reader: Box<dyn std::io::Read> = match std::fs::File::open(filename) {
                Ok(f) => Box::new(f),
                Err(_) => Box::new(std::io::empty()),
            };
            TropicalWeightInputStream {
                filename: filename.to_string(),
                input_stream: IStream::new_owned(reader),
            }
        }

        /// 'TropicalWeightInputStream(std::istream &is)'.
        pub fn new_istream(is: IStream<'a>) -> Self {
            TropicalWeightInputStream {
                filename: String::new(),
                input_stream: is,
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.skip-identifier-version-3-0-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.skip-identifier-version-3-0-fn]
        fn skip_identifier_version_3_0(&mut self) {
            self.ignore(19);
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.skip-hfst-header-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.skip-hfst-header-fn]
        fn skip_hfst_header(&mut self) {
            self.ignore(6);
            self.skip_identifier_version_3_0();
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.close-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.close-fn]
        pub fn close(&mut self) {
            if !self.filename.is_empty() {
                // The underlying reader is borrowed (owned by the caller); there is
                // nothing to close on our side.
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-eof-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-eof-fn]
        pub fn is_eof(&self) -> bool {
            // C++ tests 'input_stream.peek() == EOF'; 'IStream' has no peek, so we
            // approximate with the good/fail flag (set once a read hits EOF).
            !self.input_stream.good()
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-bad-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-bad-fn]
        pub fn is_bad(&self) -> bool {
            !self.input_stream.good()
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-good-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-good-fn]
        pub fn is_good(&self) -> bool {
            if self.is_eof() {
                return false;
            }
            self.input_stream.good()
        }

        pub fn is_fst(&mut self) -> bool {
            // C++ 'is_fst()' routes to the static 'is_fst(input_stream)'.
            Self::is_fst_istream(&mut self.input_stream)
        }

        /// 'bool operator() (void) const;' — stream-good predicate.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.operator-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.operator-fn]
        pub fn operator_call(&self) -> bool {
            self.is_good()
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.ignore-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.ignore-fn]
        pub fn ignore(&mut self, n: u32) {
            let mut buf = vec![0u8; n as usize];
            self.input_stream.read(&mut buf);
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.read-transducer-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.read-transducer-fn]
        pub fn read_transducer(&mut self) -> StdVectorFst {
            if self.is_eof() {
                crate::HFST_THROW!(StreamIsClosedException);
            }
            // DEFERRED: OpenFST streaming read ('FstHeader::Read' + 'StdVectorFst::Read'
            // from an istream) has no rustfst equivalent — rustfst exposes only
            // 'SerializableFst::load(&[u8])', and 'IStream' cannot yield the
            // remaining-bytes buffer (no read-to-end, framing handled elsewhere).
            let _ = TransducerHasWrongTypeException::new(String::new(), String::new(), 0);
            unimplemented!(
                "deferred: read_transducer — rustfst has no streaming OpenFST Fst read (only load(&[u8]))"
            )
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-fn]
        pub fn stream_get(&mut self) -> char {
            let mut b = [0u8; 1];
            self.input_stream.read(&mut b);
            b[0] as char
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-short-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-short-fn]
        pub fn stream_get_short(&mut self) -> i16 {
            let mut b = [0u8; 2];
            self.input_stream.read(&mut b);
            i16::from_ne_bytes(b)
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-unget-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-unget-fn]
        pub fn stream_unget(&mut self, c: char) {
            self.input_stream.putback(c as u8);
        }

        /// 'static bool is_fst(FILE * f);'
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-fst-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-fst-fn]
        pub fn is_fst_file(f: *mut libc::FILE) -> bool {
            if f.is_null() {
                return false;
            }
            let c = unsafe { libc::fgetc(f) };
            unsafe {
                libc::ungetc(c, f);
            }
            c == 0xd6
        }

        /// 'static bool is_fst(std::istream &s);'
        pub fn is_fst_istream(s: &mut IStream) -> bool {
            // C++ 's.good() && s.peek() == 0xd6'. peek = get then put the byte back.
            if !s.good() {
                return false;
            }
            let c = s.get();
            if c >= 0 {
                s.putback(c as u8);
            }
            c == 0xd6
        }
    }

    // ===========================================================================
    // TropicalWeightOutputStream
    // ===========================================================================

    #[allow(dead_code)]
    impl TropicalWeightOutputStream {
        /// 'TropicalWeightOutputStream(bool hfst_format=true)' — writes to stdout.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.tropical-weight-output-stream-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.tropical-weight-output-stream-fn]
        pub fn new(hfst_format: bool) -> Self {
            TropicalWeightOutputStream {
                filename: String::new(),
                output_stream: Box::new(std::io::stdout()),
                hfst_format,
            }
        }

        /// 'TropicalWeightOutputStream(const std::string &filename, bool hfst_format=false)'.
        pub fn new_filename(filename: &str, hfst_format: bool) -> Self {
            let file = std::fs::File::create(filename)
                .expect("TropicalWeightOutputStream: cannot open file");
            TropicalWeightOutputStream {
                filename: filename.to_string(),
                output_stream: Box::new(file),
                hfst_format,
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.close-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.close-fn]
        pub fn close(&mut self) {
            if !self.filename.is_empty() {
                let _ = self.output_stream.flush();
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-fn]
        pub fn write(&mut self, c: char) {
            let _ = self.output_stream.write_all(&[c as u8]);
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-transducer-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-transducer-fn]
        pub fn write_transducer(&mut self, transducer: &StdVectorFst) {
            if transducer.input_symbols().is_none() {
                eprintln!("### Missing Input Symbol Table when writing! ###");
            }
            // When not writing the HFST framing, OpenFST includes both input and
            // output symbol tables; the C++ sets the output table = input table on
            // the caller's transducer. The skeleton hands us '&StdVectorFst', so we
            // do it on a clone (NOTE: caller's transducer is not mutated, unlike C++).
            if !self.hfst_format {
                let mut t = transducer.clone();
                let st = transducer.input_symbols().unwrap().as_ref().clone();
                t.set_output_symbols(Arc::new(st));
                let _ = t.store(&mut self.output_stream);
            } else {
                let _ = transducer.store(&mut self.output_stream);
            }
        }
    }

    // ===========================================================================
    // TropicalWeightTransducer — construction / IO / symbol-table / accessors
    // ===========================================================================

    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    impl TropicalWeightTransducer {
        // ---- profiling / warning-stream globals ----

        pub fn get_profile_seconds() -> f32 {
            // 'tropical_seconds' is 0 unless PROFILE_OPENFST (compiled out).
            0.0
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-warning-stream-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-warning-stream-fn]
        pub fn get_warning_stream() -> *mut Box<dyn std::io::Write> {
            unsafe { WARNING_STREAM }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-warning-stream-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-warning-stream-fn]
        pub fn set_warning_stream(os: *mut Box<dyn std::io::Write>) {
            unsafe {
                WARNING_STREAM = os;
            }
        }

        // ---- private symbol-table helpers ----

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-symbol-table-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-symbol-table-fn]
        fn create_symbol_table(_name: String) -> SymbolTable {
            // rustfst 'SymbolTable' has no name; the 'name' arg is dropped. Start
            // from 'empty()' so the internal symbols land at exactly 0/1/2 (an
            // 'add_symbol' on a fresh 'new()' table would already hold <eps> at 0).
            let mut st = SymbolTable::empty();
            st.add_symbol(internal_epsilon); // 0
            st.add_symbol(internal_unknown); // 1
            st.add_symbol(internal_identity); // 2
            st
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.initialize-symbol-tables-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.initialize-symbol-tables-fn]
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
        fn initialize_symbol_tables(t: &mut StdVectorFst) {
            let st = Self::create_symbol_table(String::new());
            t.set_input_symbols(Arc::new(st));
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-symbol-table-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-symbol-table-fn]
        fn remove_symbol_table(t: &mut StdVectorFst) {
            let _ = t.take_input_symbols();
        }

        // ---- factories ----

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-empty-transducer-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-empty-transducer-fn]
        pub fn create_empty_transducer() -> StdVectorFst {
            let mut t = StdVectorFst::new();
            Self::initialize_symbol_tables(&mut t);
            let s = t.add_state();
            t.set_start(s).unwrap();
            t
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-epsilon-transducer-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-epsilon-transducer-fn]
        pub fn create_epsilon_transducer() -> StdVectorFst {
            let mut t = StdVectorFst::new();
            Self::initialize_symbol_tables(&mut t);
            let s = t.add_state();
            t.set_start(s).unwrap();
            t.set_final(s, 0.0f32).unwrap();
            t
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.delete-transducer-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.delete-transducer-fn]
        pub fn delete_transducer(t: StdVectorFst) {
            drop(t);
        }

        // ---- string versions of define_transducer ----

        pub fn define_transducer_symbol(symbol: &str) -> StdVectorFst {
            assert!(!symbol.is_empty());
            let mut t = StdVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let s1 = t.add_state();
            let s2 = t.add_state();
            t.set_start(s1).unwrap();
            t.set_final(s2, 0.0f32).unwrap();
            let il = st.add_symbol(symbol);
            let ol = st.add_symbol(symbol);
            t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
                .unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        pub fn define_transducer_symbol_pair(isymbol: &str, osymbol: &str) -> StdVectorFst {
            assert!(!isymbol.is_empty());
            assert!(!osymbol.is_empty());
            let mut t = StdVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let s1 = t.add_state();
            let s2 = t.add_state();
            t.set_start(s1).unwrap();
            t.set_final(s2, 0.0f32).unwrap();
            let il = st.add_symbol(isymbol);
            let ol = st.add_symbol(osymbol);
            t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
                .unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        pub fn define_transducer_spv(spv: &StringPairVector) -> StdVectorFst {
            let mut t = StdVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let mut s1 = t.add_state();
            t.set_start(s1).unwrap();
            for it in spv {
                let s2 = t.add_state();
                assert!(!it.0.is_empty());
                assert!(!it.1.is_empty());
                let il = st.add_symbol(it.0.as_str());
                let ol = st.add_symbol(it.1.as_str());
                t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
                    .unwrap();
                s1 = s2;
            }
            t.set_final(s1, 0.0f32).unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.define-transducer-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.define-transducer-fn]
        pub fn define_transducer_sps(sps: &StringPairSet, cyclic: bool) -> StdVectorFst {
            let mut t = StdVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let s1 = t.add_state(); // start state
            t.set_start(s1).unwrap();
            let mut s2 = s1; // final state
            if !sps.is_empty() {
                if !cyclic {
                    s2 = t.add_state();
                }
                for it in sps {
                    assert!(!it.0.is_empty());
                    assert!(!it.1.is_empty());
                    let il = st.add_symbol(it.0.as_str());
                    let ol = st.add_symbol(it.1.as_str());
                    t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
                        .unwrap();
                }
            }
            t.set_final(s2, 0.0f32).unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        pub fn define_transducer_spsv(spsv: &[StringPairSet]) -> StdVectorFst {
            let mut t = StdVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let mut s1 = t.add_state();
            t.set_start(s1).unwrap();
            for spset in spsv {
                let s2 = t.add_state();
                for it2 in spset {
                    assert!(!it2.0.is_empty());
                    assert!(!it2.1.is_empty());
                    let il = st.add_symbol(it2.0.as_str());
                    let ol = st.add_symbol(it2.1.as_str());
                    t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
                        .unwrap();
                }
                s1 = s2;
            }
            t.set_final(s1, 0.0f32).unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        // ---- number versions of define_transducer (no symbol table, per C++) ----

        pub fn define_transducer_number(number: u32) -> StdVectorFst {
            let mut t = StdVectorFst::new();
            Self::initialize_symbol_tables(&mut t);
            let s1 = t.add_state();
            let s2 = t.add_state();
            t.set_start(s1).unwrap();
            t.set_final(s2, 0.0f32).unwrap();
            t.add_tr(s1, StdTransition::new(number, number, 0.0f32, s2))
                .unwrap();
            t
        }

        pub fn define_transducer_number_pair(inumber: u32, onumber: u32) -> StdVectorFst {
            let mut t = StdVectorFst::new();
            Self::initialize_symbol_tables(&mut t);
            let s1 = t.add_state();
            let s2 = t.add_state();
            t.set_start(s1).unwrap();
            t.set_final(s2, 0.0f32).unwrap();
            t.add_tr(s1, StdTransition::new(inumber, onumber, 0.0f32, s2))
                .unwrap();
            t
        }

        pub fn define_transducer_npv(npv: &NumberPairVector) -> StdVectorFst {
            let mut t = StdVectorFst::new();
            let mut s1 = t.add_state();
            t.set_start(s1).unwrap();
            for it in npv {
                let s2 = t.add_state();
                t.add_tr(s1, StdTransition::new(it.0, it.1, 0.0f32, s2))
                    .unwrap();
                s1 = s2;
            }
            t.set_final(s1, 0.0f32).unwrap();
            t
        }

        pub fn define_transducer_nps(nps: &NumberPairSet, cyclic: bool) -> StdVectorFst {
            let mut t = StdVectorFst::new();
            let s1 = t.add_state(); // start state
            t.set_start(s1).unwrap();
            let mut s2 = s1; // final state
            if !nps.is_empty() {
                if !cyclic {
                    s2 = t.add_state();
                }
                for it in nps {
                    t.add_tr(s1, StdTransition::new(it.0, it.1, 0.0f32, s2))
                        .unwrap();
                }
            }
            t.set_final(s2, 0.0f32).unwrap();
            t
        }

        pub fn define_transducer_npsv(npsv: &[NumberPairSet]) -> StdVectorFst {
            let mut t = StdVectorFst::new();
            let mut s1 = t.add_state();
            t.set_start(s1).unwrap();
            for npset in npsv {
                let s2 = t.add_state();
                for it2 in npset {
                    t.add_tr(s1, StdTransition::new(it2.0, it2.1, 0.0f32, s2))
                        .unwrap();
                }
                s1 = s2;
            }
            t.set_final(s1, 0.0f32).unwrap();
            t
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.copy-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.copy-fn]
        pub fn copy(t: &StdVectorFst) -> StdVectorFst {
            t.clone()
        }

        // ---- weight properties / setters ----

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-to-weights-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-to-weights-fn]
        pub fn add_to_weights(t: &mut StdVectorFst, w: f32) {
            let states: Vec<StateId> = t.states_iter().collect();
            for s in states {
                // (no in-place arc mutator in rustfst — pop & re-add, order preserved)
                let trs = t.pop_trs(s).unwrap();
                for arc in trs {
                    let nw = *arc.weight.value() + w;
                    t.add_tr(
                        s,
                        StdTransition::new(arc.ilabel, arc.olabel, nw, arc.nextstate),
                    )
                    .unwrap();
                }
                if t.is_final(s).unwrap() {
                    let old_weight = *t.final_weight(s).unwrap().unwrap().value();
                    t.set_final(s, old_weight + w).unwrap();
                }
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-smallest-weight-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-smallest-weight-fn]
        pub fn get_smallest_weight(t: &StdVectorFst) -> f32 {
            let mut retval = f32::INFINITY;
            for s in t.states_iter() {
                for arc in t.get_trs(s).unwrap().trs() {
                    let w = *arc.weight.value();
                    if w < retval {
                        retval = w;
                    }
                }
                if t.is_final(s).unwrap() {
                    let w = *t.final_weight(s).unwrap().unwrap().value();
                    if w < retval {
                        retval = w;
                    }
                }
            }
            retval
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-weights-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-weights-fn]
        pub fn has_weights(t: &StdVectorFst) -> bool {
            for s in t.states_iter() {
                for arc in t.get_trs(s).unwrap().trs() {
                    if *arc.weight.value() != 0.0 {
                        return true;
                    }
                }
                if t.is_final(s).unwrap() && *t.final_weight(s).unwrap().unwrap().value() != 0.0 {
                    return true;
                }
            }
            false
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weights-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weights-fn]
        pub fn set_final_weights(t: &StdVectorFst, weight: f32, increment: bool) -> StdVectorFst {
            let mut t = t.clone();
            let states: Vec<StateId> = t.states_iter().collect();
            for s in states {
                if t.is_final(s).unwrap() {
                    if increment {
                        let old_weight = *t.final_weight(s).unwrap().unwrap().value();
                        t.set_final(s, weight + old_weight).unwrap();
                    } else {
                        t.set_final(s, weight).unwrap();
                    }
                }
            }
            t
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.transform-weights-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.transform-weights-fn]
        pub fn transform_weights(t: &StdVectorFst, func: fn(f32) -> f32) -> StdVectorFst {
            let mut t = t.clone();
            let states: Vec<StateId> = t.states_iter().collect();
            for s in states {
                if t.is_final(s).unwrap() {
                    let v = *t.final_weight(s).unwrap().unwrap().value();
                    t.set_final(s, func(v)).unwrap();
                }
                let trs = t.pop_trs(s).unwrap();
                for arc in trs {
                    let nw = func(*arc.weight.value());
                    t.add_tr(
                        s,
                        StdTransition::new(arc.ilabel, arc.olabel, nw, arc.nextstate),
                    )
                    .unwrap();
                }
            }
            t
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-weight-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-weight-fn]
        pub fn set_weight(t: &StdVectorFst, f: f32) -> StdVectorFst {
            let mut t_copy = t.clone();
            let states: Vec<StateId> = t_copy.states_iter().collect();
            for s in states {
                if t_copy.is_final(s).unwrap() {
                    t_copy.set_final(s, f).unwrap();
                }
            }
            t_copy
        }

        // ---- AT&T write ----

        /// 'write_in_att_format(StdVectorFst*, FILE *ofile)'.
        pub fn write_in_att_format_file(t: &StdVectorFst, ofile: *mut libc::FILE) {
            let mut w = CFileWriter(ofile);
            write_in_att_format_core(t, &mut w, false, true);
        }

        /// 'write_in_att_format_number(StdVectorFst*, FILE *ofile)'.
        pub fn write_in_att_format_number_file(t: &StdVectorFst, ofile: *mut libc::FILE) {
            let mut w = CFileWriter(ofile);
            write_in_att_format_core(t, &mut w, true, true);
        }

        /// 'write_in_att_format(StdVectorFst*, std::ostream &os)'.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.write-in-att-format-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.write-in-att-format-fn]
        pub fn write_in_att_format_ostream(t: &StdVectorFst, os: &mut dyn std::io::Write) {
            write_in_att_format_core(t, os, false, false);
        }

        /// 'write_in_att_format_number(StdVectorFst*, std::ostream &os)'.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.write-in-att-format-number-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.write-in-att-format-number-fn]
        pub fn write_in_att_format_number_ostream(t: &StdVectorFst, os: &mut dyn std::io::Write) {
            write_in_att_format_core(t, os, true, false);
        }

        // ---- AT&T read ----

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-and-map-state-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-and-map-state-fn]
        fn add_and_map_state(
            t: &mut StdVectorFst,
            state_number: i32,
            state_map: &mut StateMap,
        ) -> StateId {
            match state_map.get(&state_number) {
                None => {
                    let retval = t.add_state();
                    state_map.insert(state_number, retval);
                    retval
                }
                Some(&v) => v,
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.read-in-att-format-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.read-in-att-format-fn]
        pub fn read_in_att_format(ifile: *mut libc::FILE) -> StdVectorFst {
            use std::ffi::CStr;

            let mut t = StdVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());

            let mut line = [0 as libc::c_char; 255];
            let mut state_map: StateMap = StateMap::new();

            // Add initial state that is numbered as zero.
            let initial_state = Self::add_and_map_state(&mut t, 0, &mut state_map);
            t.set_start(initial_state).unwrap();

            loop {
                let r = unsafe { libc::fgets(line.as_mut_ptr(), 255, ifile) };
                if r.is_null() {
                    break;
                }
                let line_str = unsafe { CStr::from_ptr(line.as_ptr()) }.to_string_lossy();
                let bytes = line_str.as_bytes();

                if !bytes.is_empty() && bytes[0] == b'-' {
                    // transducer separator
                    return t;
                }

                // sscanf("%s\t%s\t%s\t%s\t%s", ...) — %s splits on whitespace.
                let toks: Vec<&str> = line_str.split_whitespace().collect();
                let n = toks.len().min(5);

                // set value of weight
                let mut weight: f32 = 0.0;
                if n == 2 {
                    weight = att_atof(toks[1]);
                }
                if n == 5 {
                    weight = att_atof(toks[4]);
                }

                if n == 1 || n == 2 {
                    // final state line
                    let final_number = att_atoi(toks[0]);
                    let final_state = Self::add_and_map_state(&mut t, final_number, &mut state_map);
                    t.set_final(final_state, weight).unwrap();
                } else if n == 4 || n == 5 {
                    // transition line
                    let origin_number = att_atoi(toks[0]);
                    let target_number = att_atoi(toks[1]);
                    let origin_state =
                        Self::add_and_map_state(&mut t, origin_number, &mut state_map);
                    let target_state =
                        Self::add_and_map_state(&mut t, target_number, &mut state_map);

                    let input_number = st.add_symbol(toks[2]);
                    let output_number = st.add_symbol(toks[3]);

                    t.add_tr(
                        origin_state,
                        StdTransition::new(input_number, output_number, weight, target_state),
                    )
                    .unwrap();
                } else {
                    // line could not be parsed
                    let message = line_str.to_string();
                    crate::HFST_THROW_MESSAGE!(NotValidAttFormatException, message);
                }
            }

            t.set_input_symbols(Arc::new(st));
            t
        }

        // ---- alphabet / symbol-table handling ----

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-to-alphabet-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-to-alphabet-fn]
        pub fn insert_to_alphabet(t: &mut StdVectorFst, symbol: &str) {
            assert!(t.input_symbols().is_some());
            let mut st = t.input_symbols().unwrap().as_ref().clone();
            st.add_symbol(symbol);
            t.set_input_symbols(Arc::new(st));
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-from-alphabet-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-from-alphabet-fn]
        pub fn remove_from_alphabet(t: &mut StdVectorFst, symbol: &str) {
            assert!(t.input_symbols().is_some());
            let old = t.input_symbols().unwrap().as_ref().clone();
            let mut st = SymbolTable::empty();
            for (_label, sym) in old.iter() {
                if sym != symbol {
                    // NOTE: rustfst's SymbolTable has no add-at-explicit-label, so
                    // the original label of 'sym' cannot be preserved (the C++ does
                    // 'AddSymbol(sym, label)'). Symbols after the removed one shift.
                    st.add_symbol(sym);
                }
            }
            t.set_input_symbols(Arc::new(st));
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-alphabet-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-alphabet-fn]
        pub fn get_alphabet(t: &StdVectorFst) -> StringSet {
            assert!(t.input_symbols().is_some());
            let mut s = StringSet::new();
            let st = t.input_symbols().unwrap();
            for (_l, sym) in st.iter() {
                s.insert(sym.to_string());
            }
            s
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-input-symbols-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-input-symbols-fn]
        pub fn get_initial_input_symbols_rec(
            t: &StdVectorFst,
            s: StateId,
            visited_states: &mut BTreeSet<StateId>,
            symbols: &mut StringSet,
        ) {
            visited_states.insert(s);
            let trs: Vec<StdTransition> = t.get_trs(s).unwrap().trs().to_vec();
            for arc in &trs {
                assert!(t.input_symbols().is_some());
                let sym = t
                    .input_symbols()
                    .unwrap()
                    .get_symbol(arc.ilabel)
                    .unwrap_or("")
                    .to_string();
                assert!(!sym.is_empty());

                if !FdOperation::is_diacritic(&sym) && arc.ilabel != 0 {
                    symbols.insert(sym);
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

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-first-input-symbols-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-first-input-symbols-fn]
        pub fn get_first_input_symbols_rec(
            t: &StdVectorFst,
            s: StateId,
            visited_states: &mut BTreeSet<StateId>,
            symbols: &mut StringSet,
        ) {
            visited_states.insert(s);
            let trs: Vec<StdTransition> = t.get_trs(s).unwrap().trs().to_vec();
            for arc in &trs {
                assert!(t.input_symbols().is_some());
                let sym = t
                    .input_symbols()
                    .unwrap()
                    .get_symbol(arc.ilabel)
                    .unwrap_or("")
                    .to_string();
                assert!(!sym.is_empty());

                if !FdOperation::is_diacritic(&sym) && arc.ilabel != 0 {
                    symbols.insert(
                        t.input_symbols()
                            .unwrap()
                            .get_symbol(arc.ilabel)
                            .unwrap_or("")
                            .to_string(),
                    );
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
            let s = t.start().unwrap();
            let mut visited_states: BTreeSet<StateId> = BTreeSet::new();
            Self::get_first_input_symbols_rec(t, s, &mut visited_states, &mut symbols);
            symbols
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-number-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-number-fn]
        pub fn get_symbol_number(t: &StdVectorFst, symbol: &str) -> u32 {
            assert!(t.input_symbols().is_some());
            match t.input_symbols().unwrap().get_label(symbol) {
                None => crate::HFST_THROW!(SymbolNotFoundException),
                Some(i) => i,
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-biggest-symbol-number-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-biggest-symbol-number-fn]
        pub fn get_biggest_symbol_number(t: &StdVectorFst) -> u32 {
            let mut biggest_number = 0u32;
            for (label, _sym) in t.input_symbols().unwrap().iter() {
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
                vec![String::new(); (biggest_symbol_number + 1) as usize];

            let alphabet = Self::get_alphabet(t);
            for it in alphabet.iter() {
                let symbol_number = Self::get_symbol_number(t, it);
                symbol_vector[symbol_number as usize] = it.clone();
            }
            symbol_vector
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-mapping-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-mapping-fn]
        pub fn create_mapping(t1: &StdVectorFst, t2: &StdVectorFst) -> NumberNumberMap {
            let mut km = NumberNumberMap::new();
            let st1 = t1.input_symbols().unwrap();
            let st2 = t2.input_symbols().unwrap();
            for (label, sym) in st1.iter() {
                let mapped = st2.get_label(sym).unwrap();
                km.insert(label, mapped);
            }
            km
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.recode-symbol-numbers-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.recode-symbol-numbers-fn]
        pub fn recode_symbol_numbers(t: &mut StdVectorFst, km: &mut NumberNumberMap) {
            let states: Vec<StateId> = t.states_iter().collect();
            for s in states {
                let trs = t.pop_trs(s).unwrap();
                for arc in trs {
                    // C++ 'km[label]' inserts 0 for a missing key.
                    let il = *km.entry(arc.ilabel).or_insert(0);
                    let ol = *km.entry(arc.olabel).or_insert(0);
                    t.add_tr(s, StdTransition::new(il, ol, arc.weight, arc.nextstate))
                        .unwrap();
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
            for (_l, sym) in t.input_symbols().unwrap().iter() {
                eprint!("'{}', ", sym);
            }
            eprintln!();
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-flag-diacritics-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-flag-diacritics-fn]
        pub fn get_flag_diacritics(t: &StdVectorFst) -> FdTable<i64> {
            let mut table: FdTable<i64> = FdTable::new();
            let symbols = t.input_symbols().unwrap();
            for (label, sym) in symbols.iter() {
                if FdOperation::is_diacritic(sym) {
                    table.define_diacritic(label as i64, sym);
                }
            }
            table
        }

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
                    result.set_start(result_s).unwrap();
                }
                if t.is_final(s).unwrap() {
                    let fw = *t.final_weight(s).unwrap().unwrap().value();
                    result.set_final(result_s, fw).unwrap();
                }

                let trs: Vec<StdTransition> = t.get_trs(s).unwrap().trs().to_vec();
                for arc in &trs {
                    let result_nextstate = arc.nextstate;

                    if unknown_symbols_in_use {
                        let is_ = t.input_symbols().unwrap();

                        if arc.ilabel == 1 && arc.olabel == 1 {
                            // cross-product "?:?"
                            for it1 in unknown.iter() {
                                if !FdOperation::is_diacritic(it1) {
                                    let inumber: i64 =
                                        is_.get_label(it1).map(|l| l as i64).unwrap_or(-1);
                                    for it2 in unknown.iter() {
                                        if !FdOperation::is_diacritic(it2) {
                                            let onumber: i64 =
                                                is_.get_label(it2).map(|l| l as i64).unwrap_or(-1);
                                            if inumber != onumber {
                                                result
                                                    .add_tr(
                                                        result_s,
                                                        StdTransition::new(
                                                            inumber as u32,
                                                            onumber as u32,
                                                            arc.weight,
                                                            result_nextstate,
                                                        ),
                                                    )
                                                    .unwrap();
                                            }
                                        }
                                    }
                                    result
                                        .add_tr(
                                            result_s,
                                            StdTransition::new(
                                                inumber as u32,
                                                1,
                                                arc.weight,
                                                result_nextstate,
                                            ),
                                        )
                                        .unwrap();
                                    result
                                        .add_tr(
                                            result_s,
                                            StdTransition::new(
                                                1,
                                                inumber as u32,
                                                arc.weight,
                                                result_nextstate,
                                            ),
                                        )
                                        .unwrap();
                                }
                            }
                        } else if arc.ilabel == 2 || arc.olabel == 2 {
                            // identity "?:?"
                            for it in unknown.iter() {
                                if !FdOperation::is_diacritic(it) {
                                    let number: i64 =
                                        is_.get_label(it).map(|l| l as i64).unwrap_or(-1);
                                    result
                                        .add_tr(
                                            result_s,
                                            StdTransition::new(
                                                number as u32,
                                                number as u32,
                                                arc.weight,
                                                result_nextstate,
                                            ),
                                        )
                                        .unwrap();
                                }
                            }
                        } else if arc.ilabel == 1 {
                            // "?:x"
                            for it in unknown.iter() {
                                if !FdOperation::is_diacritic(it) {
                                    let number: i64 =
                                        is_.get_label(it).map(|l| l as i64).unwrap_or(-1);
                                    result
                                        .add_tr(
                                            result_s,
                                            StdTransition::new(
                                                number as u32,
                                                arc.olabel,
                                                arc.weight,
                                                result_nextstate,
                                            ),
                                        )
                                        .unwrap();
                                }
                            }
                        } else if arc.olabel == 1 {
                            // "x:?"
                            for it in unknown.iter() {
                                if !FdOperation::is_diacritic(it) {
                                    let number: i64 =
                                        is_.get_label(it).map(|l| l as i64).unwrap_or(-1);
                                    result
                                        .add_tr(
                                            result_s,
                                            StdTransition::new(
                                                arc.ilabel,
                                                number as u32,
                                                arc.weight,
                                                result_nextstate,
                                            ),
                                        )
                                        .unwrap();
                                }
                            }
                        }
                    }

                    // the original transition is copied in all cases
                    result
                        .add_tr(
                            result_s,
                            StdTransition::new(
                                arc.ilabel,
                                arc.olabel,
                                arc.weight,
                                result_nextstate,
                            ),
                        )
                        .unwrap();
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
            let mut unknown_t1 = StringSet::new();
            let mut unknown_t2 = StringSet::new();
            let t1_symbols = Self::get_alphabet(&t1);
            let t2_symbols = Self::get_alphabet(&t2);
            crate::hfst_symbol_defs::symbols::collect_unknown_sets(
                &t1_symbols,
                &mut unknown_t1,
                &t2_symbols,
                &mut unknown_t2,
            );

            if debug {
                eprint!("New symbols for t1: ");
                for it in unknown_t1.iter() {
                    eprint!("'{}', ", it);
                }
                eprintln!();
                eprint!("New symbols for t2: ");
                for it in unknown_t2.iter() {
                    eprint!("'{}', ", it);
                }
                eprintln!();
            }

            // 2. add new symbols from t1 to t2's symbol table...
            let mut st2 = t2.input_symbols().unwrap().as_ref().clone();
            for it in unknown_t2.iter() {
                if st2.add_symbol(it.as_str()) < 3 {
                    eprintln!("ERROR: string {} got strange number", it);
                    assert!(false);
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
                h.set_input_symbols(Arc::clone(t1.input_symbols().unwrap()));
                h
            };

            let harmonized_t2 = if !unknown_symbols_in_use {
                t2
            } else {
                let mut h = Self::expand_arcs(&t2, &mut unknown_t2, unknown_symbols_in_use);
                h.set_input_symbols(Arc::clone(t2.input_symbols().unwrap()));
                h
            };

            (harmonized_t1, harmonized_t2)
        }

        // ---- basic accessors ----

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-automaton-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-automaton-fn]
        pub fn is_automaton(t: &StdVectorFst) -> bool {
            for s in t.states_iter() {
                for arc in t.get_trs(s).unwrap().trs() {
                    if arc.ilabel != arc.olabel {
                        return false;
                    }
                    if arc.ilabel == 1 {
                        // ?:?
                        return false;
                    }
                }
            }
            true
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-states-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-states-fn]
        pub fn number_of_states(t: &StdVectorFst) -> u32 {
            let mut retval = 0u32;
            for _s in t.states_iter() {
                retval += 1;
            }
            retval
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-arcs-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-arcs-fn]
        pub fn number_of_arcs(t: &StdVectorFst) -> u32 {
            let mut retval = 0u32;
            for s in t.states_iter() {
                retval += t.num_trs(s).unwrap() as u32;
            }
            retval
        }

        // ---- public low-level builders ----

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-state-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-state-fn]
        pub fn add_state(t: &mut StdVectorFst) -> StateId {
            let s = t.add_state();
            if s == 0 {
                t.set_start(s).unwrap();
            }
            s
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weight-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weight-fn]
        pub fn set_final_weight(t: &mut StdVectorFst, s: StateId, w: f32) {
            t.set_final(s, w).unwrap();
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-transition-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-transition-fn]
        pub fn add_transition(
            t: &mut StdVectorFst,
            source: StateId,
            isymbol: &str,
            osymbol: &str,
            w: f32,
            target: StateId,
        ) {
            let mut st = t.input_symbols().unwrap().as_ref().clone();
            let ilabel = st.add_symbol(isymbol);
            let olabel = st.add_symbol(osymbol);
            t.add_tr(source, StdTransition::new(ilabel, olabel, w, target))
                .unwrap();
            t.set_input_symbols(Arc::new(st));
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-final-weight-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-final-weight-fn]
        pub fn get_final_weight(t: &StdVectorFst, s: StateId) -> f32 {
            // C++ 't->Final(s).Value()' — Zero().Value() is +inf for a non-final state.
            t.final_weight(s)
                .unwrap()
                .map(|w| *w.value())
                .unwrap_or(f32::INFINITY)
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-final-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-final-fn]
        pub fn is_final(t: &StdVectorFst, s: StateId) -> f32 {
            // C++ returns '(t->Final(s) != Zero())' implicitly converted to float.
            if t.is_final(s).unwrap() { 1.0 } else { 0.0 }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-state-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-state-fn]
        pub fn get_initial_state(t: &StdVectorFst) -> StateId {
            t.start().unwrap_or(NO_STATE_ID)
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
        pub fn represent_empty_transducer_as_having_one_state(t: &mut StdVectorFst) {
            if t.start().is_none() || t.num_states() == 0 {
                // BUG PRESERVED: the C++ does 'delete t; t = create_empty_transducer();',
                // assigning a LOCAL pointer — the caller's transducer is unchanged.
                // We replicate the no-op (mutating *t here would change the caller).
            }
        }
    }
}

// Re-export the 'hfst::implementations' free function consumed by the facade
// 'set_minimization_algorithm' (HfstTransducer.cc:246).
pub use construction_io::openfst_tropical_set_hopcroft;

// ===== operations (workflow body) =====
mod operations {
    #![allow(unused_imports)]
    use super::*;
    // ===========================================================================
    // area: operations — algebraic operations and OpenFST-algorithm wrappers.
    //
    // These are method bodies for the 'impl TropicalWeightTransducer' block defined
    // in the skeleton (paste them in, replacing the corresponding 'unimplemented! ()'
    // stubs). Two private module-level free helpers are also provided.
    //
    // Ports of 'libhfst/src/implementations/TropicalWeightTransducer.cc'.
    // ===========================================================================

    /// Port of the 'CHECK_EPSILON_CYCLES(x, y)' macro: convert 'x' to an
    /// 'HfstBasicTransducer', and if it has negative-weight epsilon cycles, emit a
    /// warning to the (file-static) 'warning_stream' if one is set.
    #[allow(dead_code)]
    fn check_epsilon_cycles(x: &StdVectorFst, y: &str) {
        let fsm = crate::convert_transducer_format::ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(x, true);
        if fsm.has_negative_epsilon_cycles() {
            let warning_stream = TropicalWeightTransducer::get_warning_stream();
            if !warning_stream.is_null() {
                use std::io::Write;
                unsafe {
                    let _ = writeln!(
                        &mut **warning_stream,
                        "{}: warning: transducer has epsilon cycles with a negative weight",
                        y
                    );
                }
            }
        }
    }

    /// 'dst->SetInputSymbols(src->InputSymbols())' — copy 'src''s input symbol table
    /// (as a shared 'Arc') onto 'dst'. No-op when 'src' has no input symbols.
    #[allow(dead_code)]
    fn copy_input_symbol_table(src: &StdVectorFst, dst: &mut StdVectorFst) {
        if let Some(symt) = src.input_symbols().map(|s| std::sync::Arc::clone(s)) {
            dst.set_input_symbols(symt);
        }
    }

    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    impl TropicalWeightTransducer {
        // This function can be moved to its own file if TropicalWeightTransducer.o
        // yields a 'File too big' error.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-labels-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-labels-fn]
        pub fn push_labels(t: &StdVectorFst, to_initial_state: bool) -> StdVectorFst {
            assert!(t.input_symbols().is_some());

            check_epsilon_cycles(t, "push_labels");

            let mut retval = StdVectorFst::new();
            if to_initial_state {
                algorithms::Push(
                    t,
                    &mut retval,
                    algorithms::FstReweightType::ReweightToInitial,
                    algorithms::PushType::PUSH_LABELS,
                );
            } else {
                algorithms::Push(
                    t,
                    &mut retval,
                    algorithms::FstReweightType::ReweightToFinal,
                    algorithms::PushType::PUSH_LABELS,
                );
            }
            copy_input_symbol_table(t, &mut retval);
            retval
        }

        // This function can be moved to its own file if TropicalWeightTransducer.o
        // yields a 'File too big' error.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-weights-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-weights-fn]
        pub fn push_weights(t: &StdVectorFst, to_initial_state: bool) -> StdVectorFst {
            assert!(t.input_symbols().is_some());

            check_epsilon_cycles(t, "push_weights");

            let mut retval = StdVectorFst::new();
            if to_initial_state {
                algorithms::Push(
                    t,
                    &mut retval,
                    algorithms::FstReweightType::ReweightToInitial,
                    algorithms::PushType::PUSH_WEIGHTS,
                );
            } else {
                algorithms::Push(
                    t,
                    &mut retval,
                    algorithms::FstReweightType::ReweightToFinal,
                    algorithms::PushType::PUSH_WEIGHTS,
                );
            }
            copy_input_symbol_table(t, &mut retval);
            retval
        }

        // This function can be moved to its own file if TropicalWeightTransducer.o
        // yields a 'File too big' error.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn]
        pub fn minimize(t: &StdVectorFst) -> StdVectorFst {
            // C++ mutates 't' in place; the skeleton hands us '&StdVectorFst', so we
            // operate on a local clone (the caller-side mutation side effect is lost).
            let mut t = t.clone();

            check_epsilon_cycles(&t, "minimize");

            // (USE_FOMA_EPSILON_REMOVAL && HAVE_FOMA) path is not configured here.
            algorithms::RmEpsilon(&mut t);

            let w = TropicalWeightTransducer::get_smallest_weight(&t);
            if w < 0.0 {
                TropicalWeightTransducer::add_to_weights(&mut t, -w);
            }

            let encode_type = if crate::get_encode_weights() {
                algorithms::EncodeType::EncodeWeightsAndLabels
            } else {
                algorithms::EncodeType::EncodeLabels
            };
            let encode_mapper = algorithms::Encode(&mut t, encode_type);
            let mut det = StdVectorFst::new();

            algorithms::Determinize(&t, &mut det);
            algorithms::Minimize(&mut det);
            algorithms::Decode(&mut det, encode_mapper);

            if w < 0.0 {
                TropicalWeightTransducer::add_to_weights(&mut det, w);
            }

            det
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
        pub fn determinize(t: &StdVectorFst) -> StdVectorFst {
            // C++ mutates 't' in place; operate on a local clone.
            let mut t = t.clone();

            check_epsilon_cycles(&t, "determinize");

            algorithms::RmEpsilon(&mut t);

            let w = TropicalWeightTransducer::get_smallest_weight(&t);
            if w < 0.0 {
                TropicalWeightTransducer::add_to_weights(&mut t, -w);
            }

            let encode_type = if crate::get_encode_weights() {
                algorithms::EncodeType::EncodeWeightsAndLabels
            } else {
                algorithms::EncodeType::EncodeLabels
            };
            let encode_mapper = algorithms::Encode(&mut t, encode_type);
            let mut det = StdVectorFst::new();
            algorithms::Determinize(&t, &mut det);
            algorithms::Decode(&mut det, encode_mapper);

            if w < 0.0 {
                TropicalWeightTransducer::add_to_weights(&mut det, w);
            }

            det
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-epsilons-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-epsilons-fn]
        pub fn remove_epsilons(t: &StdVectorFst) -> StdVectorFst {
            check_epsilon_cycles(t, "remove_epsilons");
            // C++: return new StdVectorFst(RmEpsilonFst<StdArc>(*t));
            let mut retval = t.clone();
            algorithms::RmEpsilon(&mut retval);
            retval
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.prune-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.prune-fn]
        pub fn prune(t: &StdVectorFst) -> StdVectorFst {
            // C++: fst::Prune(*t, retval, TropicalWeight::One());
            // The hfst-openfst adapter's Prune is in-place (rustfst gap), so we prune
            // a clone with threshold One().
            let mut retval = t.clone();
            algorithms::Prune(&mut retval, TropicalWeight::one());
            retval
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.n-best-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.n-best-fn]
        pub fn n_best(t: &StdVectorFst, n: u32) -> StdVectorFst {
            check_epsilon_cycles(t, "n_best");

            let mut scaled = t.clone();
            algorithms::RmEpsilon(&mut scaled);
            let w = TropicalWeightTransducer::get_smallest_weight(&scaled);
            if w < 0.0 {
                TropicalWeightTransducer::add_to_weights(&mut scaled, -w);
            }
            // fst::ShortestPath(*scaled, n_best_fst, (size_t)n); the C++ bad_alloc
            // catch -> HfstFatalException is dropped (Rust aborts on OOM).
            let config = hfst_openfst::rustfst::algorithms::ShortestPathConfig::default()
                .with_nshortest(n as usize);
            let mut n_best_fst: StdVectorFst =
                hfst_openfst::rustfst::algorithms::shortest_path_with_config(&scaled, config)
                    .expect("rustfst shortest_path");
            algorithms::RmEpsilon(&mut n_best_fst);
            if w < 0.0 {
                TropicalWeightTransducer::add_to_weights(&mut n_best_fst, w);
            }
            n_best_fst
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-star-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-star-fn]
        pub fn repeat_star(t: &StdVectorFst) -> StdVectorFst {
            // C++: return new StdVectorFst(ClosureFst<StdArc>(*t, CLOSURE_STAR));
            let mut t = t.clone();
            hfst_openfst::rustfst::algorithms::closure::closure(
                &mut t,
                hfst_openfst::rustfst::algorithms::closure::ClosureType::ClosureStar,
            );
            t
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-plus-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-plus-fn]
        pub fn repeat_plus(t: &StdVectorFst) -> StdVectorFst {
            // C++: return new StdVectorFst(ClosureFst<StdArc>(*t, CLOSURE_PLUS));
            let mut t = t.clone();
            hfst_openfst::rustfst::algorithms::closure::closure(
                &mut t,
                hfst_openfst::rustfst::algorithms::closure::ClosureType::ClosurePlus,
            );
            t
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-n-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-n-fn]
        pub fn repeat_n(t: &StdVectorFst, n: u32) -> StdVectorFst {
            if n == 0 {
                return TropicalWeightTransducer::create_epsilon_transducer();
            }

            let mut repetition = TropicalWeightTransducer::create_epsilon_transducer();
            copy_input_symbol_table(t, &mut repetition);
            for _ in 0..n {
                algorithms::Concat(&mut repetition, t);
            }
            repetition
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-le-n-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-le-n-fn]
        pub fn repeat_le_n(t: &StdVectorFst, n: u32) -> StdVectorFst {
            if n == 0 {
                return TropicalWeightTransducer::create_epsilon_transducer();
            }

            let mut repetition = TropicalWeightTransducer::create_epsilon_transducer();
            copy_input_symbol_table(t, &mut repetition);

            for _ in 0..n {
                let mut optional_t = TropicalWeightTransducer::optionalize(t);
                copy_input_symbol_table(t, &mut optional_t);
                algorithms::Concat(&mut repetition, &optional_t);
            }
            repetition
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.optionalize-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.optionalize-fn]
        pub fn optionalize(t: &StdVectorFst) -> StdVectorFst {
            let mut eps = TropicalWeightTransducer::create_epsilon_transducer();
            if let Some(symt) = t.input_symbols().map(|s| std::sync::Arc::clone(s)) {
                eps.set_input_symbols(symt);
            }
            if let Some(symt) = t.output_symbols().map(|s| std::sync::Arc::clone(s)) {
                eps.set_output_symbols(symt);
            }
            algorithms::Union(&mut eps, t);
            eps
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.invert-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.invert-fn]
        pub fn invert(t: &StdVectorFst) -> StdVectorFst {
            let mut inverse = TropicalWeightTransducer::copy(t);
            algorithms::Invert(&mut inverse);
            copy_input_symbol_table(t, &mut inverse);
            inverse
        }

        /* Makes valgrind angry... */
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.reverse-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.reverse-fn]
        pub fn reverse(transducer: &StdVectorFst) -> StdVectorFst {
            let mut reversed = StdVectorFst::new();
            algorithms::Reverse(transducer, &mut reversed);
            copy_input_symbol_table(transducer, &mut reversed);
            reversed
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-input-language-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-input-language-fn]
        pub fn extract_input_language(t: &StdVectorFst) -> StdVectorFst {
            // C++: new StdVectorFst(ProjectFst<StdArc>(*t, ProjectType::INPUT));
            let mut proj = t.clone();
            algorithms::ProjectInput(&mut proj);
            // substitute unknown with identity
            let mut retval = TropicalWeightTransducer::substitute_number(&proj, 1, 2);
            copy_input_symbol_table(t, &mut retval);
            retval
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-output-language-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-output-language-fn]
        pub fn extract_output_language(t: &StdVectorFst) -> StdVectorFst {
            // C++: new StdVectorFst(ProjectFst<StdArc>(*t, ProjectType::OUTPUT));
            let mut proj = t.clone();
            algorithms::ProjectOutput(&mut proj);
            // substitute unknown with identity
            let mut retval = TropicalWeightTransducer::substitute_number(&proj, 1, 2);
            copy_input_symbol_table(t, &mut retval);
            retval
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-fn]
        pub fn compose(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
            let mut foo: StringSet = StringSet::new();
            // a copy of t2 is created so that its symbol table check sum
            // is the same as t1's
            // (else OpenFst complains about non-matching check sums... )
            let mut t2_ = TropicalWeightTransducer::expand_arcs(t2, &mut foo, false);

            // C++ mutates t1 (sets/sorts/unsets); operate on a local clone.
            let mut t1_ = t1.clone();

            // t1->SetOutputSymbols(t1->InputSymbols());
            let in_syms = t1_.input_symbols().map(|s| std::sync::Arc::clone(s));
            if let Some(a) = in_syms {
                t1_.set_output_symbols(a);
            }
            // t2_->SetInputSymbols(t1->OutputSymbols());
            let out_syms = t1_.output_symbols().map(|s| std::sync::Arc::clone(s));
            if let Some(a) = out_syms {
                t2_.set_input_symbols(a);
            }

            algorithms::ArcSortOutput(&mut t1_);
            algorithms::ArcSortInput(&mut t2_);

            let mut result = StdVectorFst::new();
            algorithms::Compose(&t1_, &t2_, &mut result);

            // t1->SetOutputSymbols(NULL); (only affected the caller's t1 in C++)

            // result->SetInputSymbols(t1->InputSymbols());
            copy_input_symbol_table(t1, &mut result);
            result
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.concatenate-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.concatenate-fn]
        pub fn concatenate(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
            let mut result = t1.clone();
            copy_input_symbol_table(t1, &mut result);
            algorithms::Concat(&mut result, t2);
            result
        }

        pub fn disjunct(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
            let mut result = t1.clone();
            copy_input_symbol_table(t1, &mut result);
            algorithms::Union(&mut result, t2);
            result
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-fn]
        pub fn disjunct_spv<'a>(
            t: &'a mut StdVectorFst,
            spv: &StringPairVector,
        ) -> &'a mut StdVectorFst {
            let mut st: SymbolTable = (**t.input_symbols().unwrap()).clone();

            let mut s = t.start().unwrap();

            for it in spv {
                let inumber = st.add_symbol(it.0.as_str());
                let onumber = st.add_symbol(it.1.as_str());

                let mut transition_found = false;
                let mut next: StateId = 0;
                for a in t.get_trs(s).unwrap().trs() {
                    if a.ilabel == inumber && a.olabel == onumber {
                        transition_found = true;
                        next = a.nextstate;
                        break;
                    }
                }

                if transition_found {
                    s = next;
                } else {
                    let new_state = t.add_state();
                    t.add_tr(
                        s,
                        StdTransition::new(inumber, onumber, TropicalWeight::new(0.0), new_state),
                    )
                    .unwrap();
                    s = new_state;
                }
            }

            t.set_final(s, TropicalWeight::new(0.0)).unwrap();

            t.set_input_symbols(std::sync::Arc::new(st));
            t
        }

        pub fn disjunct_npv<'a>(
            t: &'a mut StdVectorFst,
            npv: &NumberPairVector,
        ) -> &'a mut StdVectorFst {
            let mut s = t.start().unwrap();

            for it in npv {
                let inumber = it.0;
                let onumber = it.1;

                let mut transition_found = false;
                let mut next: StateId = 0;
                for a in t.get_trs(s).unwrap().trs() {
                    if a.ilabel == inumber && a.olabel == onumber {
                        transition_found = true;
                        next = a.nextstate;
                        break;
                    }
                }

                if transition_found {
                    s = next;
                } else {
                    let new_state = t.add_state();
                    t.add_tr(
                        s,
                        StdTransition::new(inumber, onumber, TropicalWeight::new(0.0), new_state),
                    )
                    .unwrap();
                    s = new_state;
                }
            }

            t.set_final(s, TropicalWeight::new(0.0)).unwrap();
            t
        }

        /// 'static fst::StdVectorFst * disjunct_as_tries(fst::StdVectorFst * t1,
        ///   const fst::StdVectorFst * t2)' — public trie-disjunction entry point.
        pub fn disjunct_as_tries_pub<'a>(
            t1: &'a mut StdVectorFst,
            t2: &StdVectorFst,
        ) -> &'a mut StdVectorFst {
            let t1_state = t1.start().unwrap();
            let t2_state = t2.start().unwrap();
            TropicalWeightTransducer::disjunct_as_tries(t1, t1_state, t2, t2_state);
            t1
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.intersect-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.intersect-fn]
        pub fn intersect(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
            check_epsilon_cycles(t1, "intersect");
            check_epsilon_cycles(t2, "intersect");

            // C++ mutates t1/t2 in place; operate on local clones.
            let mut t1 = t1.clone();
            let mut t2 = t2.clone();

            algorithms::RmEpsilon(&mut t1);
            algorithms::RmEpsilon(&mut t2);

            algorithms::ArcSortOutput(&mut t1);
            algorithms::ArcSortInput(&mut t2);

            // weights must not be encoded, else e.g. [a:b::1] & [a:b::2] will be empty
            // EncodeMapper<StdArc> encoder(0x0001, ENCODE); (0x0001 == kEncodeLabels)
            // One shared encoder for t1 AND t2 (and the Decode below).
            let encoder = algorithms::Encode(&mut t1, algorithms::EncodeType::EncodeLabels);
            let encoder = algorithms::EncodeInto(&mut t2, encoder);

            algorithms::ArcSortOutput(&mut t1);
            algorithms::ArcSortInput(&mut t2);

            // IntersectFst<StdArc> intersect(*t1, *t2); foo = new StdVectorFst(intersect);
            let mut foo = StdVectorFst::new();
            algorithms::Intersect(&t1, &t2, &mut foo);

            // DecodeFst<StdArc> decode(*foo, encoder); result = new StdVectorFst(decode);
            algorithms::Decode(&mut foo, encoder);
            let result = foo;

            // t1->SetOutputSymbols(NULL); t2->SetOutputSymbols(NULL); (caller-side only)
            result
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.subtract-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.subtract-fn]
        pub fn subtract(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
            // bool DEBUG = false; (debug printfs dropped)

            // C++ mutates t1/t2 in place; operate on local clones.
            let mut t1 = t1.clone();
            let mut t2 = t2.clone();

            if t1.output_symbols().is_none() {
                let a = t1.input_symbols().map(|s| std::sync::Arc::clone(s));
                if let Some(a) = a {
                    t1.set_output_symbols(a);
                }
            }
            if t2.output_symbols().is_none() {
                let a = t2.input_symbols().map(|s| std::sync::Arc::clone(s));
                if let Some(a) = a {
                    t2.set_output_symbols(a);
                }
            }

            check_epsilon_cycles(&t1, "subtract");
            check_epsilon_cycles(&t2, "subtract");

            algorithms::RmEpsilon(&mut t1);
            algorithms::RmEpsilon(&mut t2);

            algorithms::ArcSortOutput(&mut t1);
            algorithms::ArcSortInput(&mut t2);

            // Remove weights from t2, is this really needed?
            let mut t2_ = TropicalWeightTransducer::copy(&t2);

            for s in 0..t2_.num_states() as StateId {
                let ntrs = t2_.get_trs(s).unwrap().trs().len();
                {
                    let mut aiter = t2_.tr_iter_mut(s).unwrap();
                    for i in 0..ntrs {
                        aiter.set_weight(i, TropicalWeight::new(0.0)).unwrap();
                    }
                }
                if t2_.is_final(s).unwrap() {
                    t2_.set_final(s, TropicalWeight::new(0.0)).unwrap();
                }
            }

            // EncodeMapper<StdArc> encoder(kEncodeLabels, ENCODE); shared by t1 AND t2.
            let encoder = algorithms::Encode(&mut t1, algorithms::EncodeType::EncodeLabels);
            let encoder = algorithms::EncodeInto(&mut t2_, encoder);

            algorithms::ArcSortOutput(&mut t1);
            algorithms::ArcSortInput(&mut t2_);

            let mut det2 = StdVectorFst::new();
            algorithms::Determinize(&t2_, &mut det2);

            let mut difference = StdVectorFst::new();
            algorithms::Difference(&t1, &det2, &mut difference);

            // DecodeFst<StdArc> subtract(*difference, encoder);
            algorithms::Decode(&mut difference, encoder);

            // t1->SetOutputSymbols(NULL); t2->SetOutputSymbols(NULL); (caller-side only)
            difference
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.are-equivalent-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.are-equivalent-fn]
        pub fn are_equivalent(one: &StdVectorFst, another: &StdVectorFst) -> bool {
            let mut a = TropicalWeightTransducer::copy(one);
            let mut b = TropicalWeightTransducer::copy(another);

            check_epsilon_cycles(&a, "are_equivalent");
            check_epsilon_cycles(&b, "are_equivalent");

            algorithms::RmEpsilon(&mut a);
            algorithms::RmEpsilon(&mut b);

            let encode_type = if crate::get_encode_weights() {
                algorithms::EncodeType::EncodeWeightsAndLabels
            } else {
                algorithms::EncodeType::EncodeLabels
            };

            // Encode both fsts through ONE shared table (OpenFST's
            // Encode(fst, &encoder)): the same (ilabel, olabel) pair then maps to
            // the same encoded label in both, so the subsequent Equivalent does
            // not depend on the order the global symbol table numbered the labels.
            let table = algorithms::Encode(&mut a, encode_type);
            let _table = algorithms::EncodeInto(&mut b, table);

            let mut deta = StdVectorFst::new();
            let mut detb = StdVectorFst::new();

            algorithms::Determinize(&a, &mut deta);
            algorithms::Determinize(&b, &mut detb);

            algorithms::Equivalent(&deta, &detb)
        }

        // ----- TRIE FUNCTIONS BEGINS -----

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-arc-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-arc-fn]
        fn has_arc(t: &StdVectorFst, sourcestate: i32, ilabel: i32, olabel: i32) -> i32 {
            for (position, a) in t
                .get_trs(sourcestate as StateId)
                .unwrap()
                .trs()
                .iter()
                .enumerate()
            {
                if (a.ilabel as i32 == ilabel) && (a.olabel as i32 == olabel) {
                    return position as i32;
                }
            }

            -1
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-as-tries-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-as-tries-fn]
        fn disjunct_as_tries(
            t1: &mut StdVectorFst,
            t1_state: StateId,
            t2: &StdVectorFst,
            t2_state: StateId,
        ) {
            if t2.is_final(t2_state).unwrap() {
                let t1_final = t1
                    .final_weight(t1_state)
                    .unwrap()
                    .unwrap_or_else(TropicalWeight::zero);
                let t2_final = t2.final_weight(t2_state).unwrap().unwrap();
                t1.set_final(t1_state, t1_final.plus(&t2_final).unwrap())
                    .unwrap();
            }
            let trs = t2.get_trs(t2_state).unwrap().trs().to_vec();
            for arc in &trs {
                let arc_index = TropicalWeightTransducer::has_arc(
                    t1,
                    t1_state as i32,
                    arc.ilabel as i32,
                    arc.olabel as i32,
                );
                if arc_index == -1 {
                    let new_state = t1.add_state();
                    t1.add_tr(
                        t1_state,
                        StdTransition::new(arc.ilabel, arc.olabel, arc.weight.clone(), new_state),
                    )
                    .unwrap();
                    TropicalWeightTransducer::add_sub_trie(t1, new_state, t2, arc.nextstate);
                } else {
                    // MutableArcIterator ajter(&t1, t1_state); ajter.Seek(arc_index);
                    let next = t1.get_trs(t1_state).unwrap().trs()[arc_index as usize].nextstate;
                    TropicalWeightTransducer::disjunct_as_tries(t1, next, t2, arc.nextstate);
                }
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-sub-trie-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-sub-trie-fn]
        fn add_sub_trie(
            t1: &mut StdVectorFst,
            t1_state: StateId,
            t2: &StdVectorFst,
            t2_state: StateId,
        ) {
            if t2.is_final(t2_state).unwrap() {
                let t1_final = t1
                    .final_weight(t1_state)
                    .unwrap()
                    .unwrap_or_else(TropicalWeight::zero);
                let t2_final = t2.final_weight(t2_state).unwrap().unwrap();
                t1.set_final(t1_state, t1_final.plus(&t2_final).unwrap())
                    .unwrap();
            }
            let trs = t2.get_trs(t2_state).unwrap().trs().to_vec();
            for arc in &trs {
                let new_state = t1.add_state();
                t1.add_tr(
                    t1_state,
                    StdTransition::new(arc.ilabel, arc.olabel, arc.weight.clone(), new_state),
                )
                .unwrap();
                TropicalWeightTransducer::add_sub_trie(t1, new_state, t2, arc.nextstate);
            }
        }

        // ----- TRIE FUNCTIONS END -----
    }
}

// ===== lookup-extract-misc (workflow body) =====
mod lookup_extract_misc {
    #![allow(unused_imports)]
    use super::*;
    // ============================================================================
    // Additional imports needed by the lookup-extract-misc bodies.  These extend
    // the skeleton's import block; the integrator should fold them into the module
    // preamble (de-duplicating against what is already imported there).
    // ============================================================================
    use std::sync::Arc;

    use hfst_openfst::rustfst::fst_properties::{FstProperties, compute_fst_properties};

    use crate::hfst_data_types::HfstTwoLevelPath;
    use crate::hfst_flag_diacritics::{FdOperation, FdState};
    use crate::hfst_lookup_flag_diacritics::FlagDiacriticTable;
    use crate::hfst_symbol_defs::symbols::{
        collect_unknown_sets, remove_flags_two_level_path, to_string_vector_from_two_level_path,
    };

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.label-pair]
    pub type LabelPair = (i32, i32);
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.label-pair-vector]
    pub type LabelPairVector = Vec<LabelPair>;

    // ============================================================================
    // File-static free helpers (C++ 'static' functions in
    // 'namespace hfst::implementations').  Kept module-private, like the C++.
    // ============================================================================

    /* The recursive path-extraction worker.  Note the faithful C++ quirk that
    `all_visitations` / `path_visitations` are passed *by value* (each recursive
    call gets its own copy), while `spv` and `fd_state_stack` are shared
    (passed by reference / pointer). */
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.extract-paths-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.extract-paths-fn]
    #[allow(clippy::too_many_arguments)]
    fn extract_paths(
        t: &StdVectorFst,
        s: StateId,
        mut all_visitations: BTreeMap<StateId, u16>,
        mut path_visitations: BTreeMap<StateId, u16>,
        weight_sum: f32,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        mut fd_state_stack: Option<&mut Vec<FdState<i64>>>,
        filter_fd: bool,
        spv: &mut StringPairVector,
    ) -> bool {
        if cycles >= 0 && (*path_visitations.entry(s).or_insert(0) as i32) > cycles {
            return true;
        }
        *all_visitations.entry(s).or_insert(0) += 1;
        *path_visitations.entry(s).or_insert(0) += 1;

        if !spv.is_empty() {
            let final_ = t.is_final(s).unwrap();
            let fw = if final_ {
                *t.final_weight(s).unwrap().unwrap().value()
            } else {
                0.0
            };
            let mut path = HfstTwoLevelPath {
                first: weight_sum + fw,
                second: spv.clone(),
            };
            let ret = callback.operator_call(&mut path, final_);
            if !ret.continueSearch || !ret.continuePath {
                *path_visitations.entry(s).or_insert(0) -= 1;
                return ret.continueSearch;
            }
        }

        // sort arcs by number of visitations (stable insertion sort, ascending)
        let mut arcs: Vec<StdTransition> = Vec::new();
        for a in t.get_trs(s).unwrap().trs() {
            let mut i = 0usize;
            while i < arcs.len() {
                let av_a = *all_visitations.get(&a.nextstate).unwrap_or(&0);
                let av_i = *all_visitations.get(&arcs[i].nextstate).unwrap_or(&0);
                if av_a < av_i {
                    break;
                }
                i += 1;
            }
            arcs.insert(i, a.clone());
        }

        let mut res = true;
        let mut idx = 0usize;
        while idx < arcs.len() && res {
            let arc = arcs[idx].clone();
            let mut added_fd_state = false;

            if let Some(stack) = fd_state_stack.as_deref_mut() {
                if stack
                    .last()
                    .unwrap()
                    .get_table()
                    .get_operation(arc.ilabel as i64)
                    .is_some()
                {
                    let top = stack.last().unwrap().clone();
                    stack.push(top);
                    if stack
                        .last_mut()
                        .unwrap()
                        .apply_operation_symbol(arc.ilabel as i64)
                    {
                        added_fd_state = true;
                    } else {
                        stack.pop();
                        idx += 1;
                        continue; // don't follow the transition
                    }
                }
            }

            /* Handle spv here. Special symbols (flags, epsilons) are always
            inserted. */
            let mut istring = String::new();
            let mut ostring = String::new();

            if !filter_fd
                || fd_state_stack
                    .as_deref()
                    .unwrap()
                    .last()
                    .unwrap()
                    .get_table()
                    .get_operation(arc.ilabel as i64)
                    .is_none()
            {
                istring = t
                    .input_symbols()
                    .unwrap()
                    .get_symbol(arc.ilabel)
                    .unwrap_or("")
                    .to_string();
            }

            if !filter_fd
                || fd_state_stack
                    .as_deref()
                    .unwrap()
                    .last()
                    .unwrap()
                    .get_table()
                    .get_operation(arc.olabel as i64)
                    .is_none()
            {
                ostring = t
                    .input_symbols()
                    .unwrap()
                    .get_symbol(arc.olabel)
                    .unwrap_or("")
                    .to_string();
            }

            spv.push((istring, ostring));

            res = extract_paths(
                t,
                arc.nextstate,
                all_visitations.clone(),
                path_visitations.clone(),
                weight_sum + *arc.weight.value(),
                callback,
                cycles,
                fd_state_stack.as_deref_mut(),
                filter_fd,
                spv,
            );

            spv.pop();

            if added_fd_state {
                fd_state_stack.as_deref_mut().unwrap().pop();
            }
            idx += 1;
        }

        *path_visitations.entry(s).or_insert(0) -= 1;
        res
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.is-minimal-and-empty-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.is-minimal-and-empty-fn]
    fn is_minimal_and_empty(t: &StdVectorFst) -> bool {
        let start_state = match t.start() {
            None => return true, // C++: start_state < 0
            Some(s) => s,
        };
        for _arc in t.get_trs(start_state).unwrap().trs() {
            return false;
        }
        true
    }

    /* Get a random path from transducer 't'.  Faithful to the C++ it signals
    failure by throwing a C-string; here those become `panic_any(&'static str)`
    that `random_path` catches with `catch_unwind`. */
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.random-path-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.random-path-fn]
    fn random_path_(t: &StdVectorFst) -> HfstTwoLevelPath {
        /* If the transducer is empty, return. */
        if is_minimal_and_empty(t) {
            std::panic::panic_any("transducer is empty");
        }

        let mut path = HfstTwoLevelPath {
            first: 0.0,
            second: StringPairVector::new(),
        };
        let mut current_state = t.start().unwrap();

        let is_epsilon_path_accepted = t.is_final(t.start().unwrap()).unwrap();

        let mut last_index: i32 = 0;

        let num_states = t.num_states();
        let mut visited = vec![0i32; num_states];
        let mut broken = vec![0i32; num_states];

        loop {
            visited[current_state as usize] = 1;

            let mut t_transitions: Vec<StdTransition> =
                t.get_trs(current_state).unwrap().trs().to_vec();

            /* If we cannot proceed, return the longest path so far. */
            if t_transitions.is_empty() || broken[current_state as usize] != 0 {
                let mut i = path.second.len() as i32 - 1;
                while i >= last_index {
                    path.second.pop();
                    i -= 1;
                }
                if !is_epsilon_path_accepted && path.second.is_empty() {
                    std::panic::panic_any("cannot extract random path");
                }
                return path;
            }

            /* Go through all transitions in a random order. */
            while !t_transitions.is_empty() {
                let index = (unsafe { libc::rand() } as usize) % t_transitions.len();
                let arc = t_transitions[index].clone();
                t_transitions.remove(index);

                let t_target = arc.nextstate;

                path.second.push((
                    t.input_symbols()
                        .unwrap()
                        .get_symbol(arc.ilabel)
                        .unwrap_or("")
                        .to_string(),
                    t.input_symbols()
                        .unwrap()
                        .get_symbol(arc.olabel)
                        .unwrap_or("")
                        .to_string(),
                ));
                path.first += *arc.weight.value();

                /* If the target state is final, */
                if t.is_final(t_target).unwrap() {
                    if (unsafe { libc::rand() } % 4) == 0 {
                        // randomly return the path so far,
                        path.first += *t.final_weight(t_target).unwrap().unwrap().value();
                        if !is_epsilon_path_accepted && path.second.is_empty() {
                            std::panic::panic_any("cannot extract random path");
                        }
                        return path;
                    } // or continue.
                    last_index = path.second.len() as i32;
                }

                /* Give more probability for shorter paths. */
                if broken[t_target as usize] == 0 {
                    if visited[t_target as usize] == 1 {
                        if (unsafe { libc::rand() } % 4) == 0 {
                            broken[t_target as usize] = 1;
                        }
                    }
                }

                if visited[t_target as usize] == 1 {
                    if (unsafe { libc::rand() } % 4) == 0 {
                        broken[t_target as usize] = 1;
                    }
                }

                /* Proceed to the target state. */
                current_state = t_target;
                break;
            }
        }
    }

    /* Try to extract a random path from 't' at most 'max_times' times. */
    fn random_path(t: &StdVectorFst, mut max_times: u32) -> HfstTwoLevelPath {
        while max_times > 0 {
            max_times -= 1;
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| random_path_(t)));
            std::panic::set_hook(prev);
            match r {
                Ok(p) => return p,
                Err(e) => {
                    let msg = if let Some(s) = e.downcast_ref::<&str>() {
                        (*s).to_string()
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        std::panic::resume_unwind(e)
                    };
                    if msg == "transducer is empty" {
                        std::panic::panic_any("transducer is empty");
                    } else if msg == "cannot extract random path" {
                        continue;
                    } else {
                        std::panic::panic_any("cannot extract random path");
                    }
                }
            }
        }
        std::panic::panic_any("cannot extract random path");
    }

    // ============================================================================
    // 'impl TropicalWeightTransducer' — lookup-extract-misc bodies.
    // ============================================================================
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    impl TropicalWeightTransducer {
        // ---- extract_paths / extract_random_paths --------------------------------

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-paths-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-paths-fn]
        pub fn extract_paths(
            t: &StdVectorFst,
            callback: &mut dyn ExtractStringsCb,
            cycles: i32,
            fd: *mut FdTable<i64>,
            filter_fd: bool,
        ) {
            if t.start().is_none() {
                return;
            }

            let all_visitations: BTreeMap<StateId, u16> = BTreeMap::new();
            let path_visitations: BTreeMap<StateId, u16> = BTreeMap::new();
            let mut fd_state_stack: Option<Vec<FdState<i64>>> = if fd.is_null() {
                None
            } else {
                Some(vec![FdState::new(unsafe { &*fd })])
            };

            let start = t.start().unwrap();
            let mut spv = StringPairVector::new();
            extract_paths(
                t,
                start,
                all_visitations,
                path_visitations,
                0.0f32,
                callback,
                cycles,
                fd_state_stack.as_mut(),
                filter_fd,
                &mut spv,
            );

            // add epsilon path, if needed
            if t.start().is_some() && t.is_final(start).unwrap() {
                let mut epsilon_path = HfstTwoLevelPath {
                    first: *t.final_weight(start).unwrap().unwrap().value(),
                    second: StringPairVector::new(),
                };
                callback.operator_call(&mut epsilon_path, true /* final */);
            }
            // fd_state_stack dropped here (C++ 'delete fd_state_stack').
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fd-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fd-fn]
        pub fn extract_random_paths_fd(
            t: &StdVectorFst,
            results: &mut HfstTwoLevelPaths,
            max_num: i32,
            filter_fd: bool,
        ) {
            let mut fdt = FlagDiacriticTable::new();
            let alpha = Self::get_alphabet(t);
            for it in alpha.iter() {
                fdt.insert_symbol(it);
            }

            let mut fd_results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
            // We filter flags after extracting paths, so we request five times
            // more paths than wanted.
            Self::extract_random_paths(t, &mut fd_results, 5 * max_num);

            let mut max_num = max_num;
            for it in fd_results.iter() {
                if max_num <= 0 {
                    break;
                }
                let mut path = it.clone();
                let sv = to_string_vector_from_two_level_path(&path);

                if fdt.is_valid_string(&sv) {
                    if filter_fd {
                        path = remove_flags_two_level_path(&path);
                    }
                    results.insert(path);
                    max_num -= 1;
                }
            }
        }

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fn]
        pub fn extract_random_paths(
            t: &StdVectorFst,
            results: &mut HfstTwoLevelPaths,
            max_num: i32,
        ) {
            unsafe {
                libc::srand(libc::time(std::ptr::null_mut()) as libc::c_uint);
            }

            let mut max_num = max_num;
            while max_num > 0 {
                /* Try to extract one path at most 5 times. */
                max_num -= 1;
                let prev = std::panic::take_hook();
                std::panic::set_hook(Box::new(|_| {}));
                let r =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| random_path(t, 5)));
                std::panic::set_hook(prev);

                let mut path = match r {
                    Ok(p) => p,
                    Err(e) => {
                        let msg = if let Some(s) = e.downcast_ref::<&str>() {
                            (*s).to_string()
                        } else if let Some(s) = e.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            std::panic::resume_unwind(e)
                        };
                        if msg == "cannot extract random path" {
                            continue; // one trial used, keep on trying
                        }
                        return; // not even possible to extract paths
                    }
                };

                /* If we extract the same path again, try at most 5 times to
                extract another one. */
                let mut i = max_num;
                while results.contains(&path) && i > 0 {
                    i -= 1;
                    let prev = std::panic::take_hook();
                    std::panic::set_hook(Box::new(|_| {}));
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        random_path(t, 5)
                    }));
                    std::panic::set_hook(prev);
                    if let Ok(p) = r {
                        path = p;
                    } // keep on trying
                }

                /* Insert the path (another or the same). */
                results.insert(path);
            }
        }

        // ---- substitute ----------------------------------------------------------

        /// 'substitute(StdVectorFst*, unsigned int, unsigned int)' — relabels label
        /// 'old_number' to 'new_number' on both the input and output side (C++ uses
        /// 'RelabelFst<StdArc>(*t, v, v)'; modelled here as a direct rebuild since
        /// rustfst's 'relabel_pairs' module is private).
        pub fn substitute_number(
            t: &StdVectorFst,
            old_number: u32,
            new_number: u32,
        ) -> StdVectorFst {
            let mut result = t.clone();
            let states: Vec<StateId> = result.states_iter().collect();
            for s in states {
                let trs = result.pop_trs(s).unwrap();
                let mut nt: Vec<StdTransition> = Vec::with_capacity(trs.len());
                for mut a in trs {
                    if a.ilabel == old_number {
                        a.ilabel = new_number;
                    }
                    if a.olabel == old_number {
                        a.olabel = new_number;
                    }
                    nt.push(a);
                }
                for a in nt {
                    result.add_tr(s, a).unwrap();
                }
            }
            result
        }

        /// 'substitute(StdVectorFst*, NumberPair old, NumberPair new)'. The C++
        /// encodes label pairs ('kEncodeLabels'), substitutes the single encoded
        /// label, then decodes; the net effect (replace arcs whose '(ilabel,olabel)'
        /// equals 'old' with 'new') is reproduced here directly.
        pub fn substitute_number_pair(
            t: &StdVectorFst,
            old_number_pair: NumberPair,
            new_number_pair: NumberPair,
        ) -> StdVectorFst {
            let mut result = t.clone();
            let states: Vec<StateId> = result.states_iter().collect();
            for s in states {
                let trs = result.pop_trs(s).unwrap();
                let mut nt: Vec<StdTransition> = Vec::with_capacity(trs.len());
                for mut a in trs {
                    if a.ilabel == old_number_pair.0 && a.olabel == old_number_pair.1 {
                        a.ilabel = new_number_pair.0;
                        a.olabel = new_number_pair.1;
                    }
                    nt.push(a);
                }
                for a in nt {
                    result.add_tr(s, a).unwrap();
                }
            }
            result
        }

        /// 'substitute(StdVectorFst*, std::string old_symbol, std::string new_symbol)'.
        pub fn substitute_symbol(
            t: &StdVectorFst,
            old_symbol: String,
            new_symbol: String,
        ) -> StdVectorFst {
            // assert(t->InputSymbols() != NULL);
            let mut st = (**t.input_symbols().unwrap()).clone();
            let old_l = st.add_symbol(old_symbol.as_str());
            let new_l = st.add_symbol(new_symbol.as_str());
            let mut retval = Self::substitute_number(t, old_l, new_l);
            retval.set_input_symbols(Arc::new(st));
            retval
        }

        /// 'substitute(StdVectorFst*, StringPair old, StringPair new)'.
        pub fn substitute_string_pair(
            t: &StdVectorFst,
            old_symbol_pair: StringPair,
            new_symbol_pair: StringPair,
        ) -> StdVectorFst {
            // assert(t->InputSymbols() != NULL);
            let mut st = (**t.input_symbols().unwrap()).clone();
            let old_pair: NumberPair = (
                st.add_symbol(old_symbol_pair.0.as_str()),
                st.add_symbol(old_symbol_pair.1.as_str()),
            );
            let new_pair: NumberPair = (
                st.add_symbol(new_symbol_pair.0.as_str()),
                st.add_symbol(new_symbol_pair.1.as_str()),
            );
            let mut retval = Self::substitute_number_pair(t, old_pair, new_pair);
            retval.set_input_symbols(Arc::new(st));
            retval
        }

        /// 'substitute(StdVectorFst*, StringPair old, StringPairSet new)'.
        pub fn substitute_string_pair_set(
            t: &StdVectorFst,
            old_symbol_pair: StringPair,
            new_symbol_pair_set: StringPairSet,
        ) -> StdVectorFst {
            let mut tc = t.clone();
            let mut st = (**tc.input_symbols().unwrap()).clone();
            // assert(st != NULL);
            let states: Vec<StateId> = tc.states_iter().collect();
            for s in states {
                let trs = tc.pop_trs(s).unwrap();
                let mut nt: Vec<StdTransition> = Vec::new();
                for arc in trs {
                    let isym = st.get_symbol(arc.ilabel).unwrap_or("").to_string();
                    let osym = st.get_symbol(arc.olabel).unwrap_or("").to_string();
                    if isym == old_symbol_pair.0 && osym == old_symbol_pair.1 {
                        // C++ replaces this arc with one arc per pair in the set;
                        // an empty set leaves the original arc untouched (the C++
                        // 'SetValue' is never reached).
                        if new_symbol_pair_set.is_empty() {
                            nt.push(arc);
                        } else {
                            for it in new_symbol_pair_set.iter() {
                                let il = st.add_symbol(it.0.as_str());
                                let ol = st.add_symbol(it.1.as_str());
                                nt.push(StdTransition::new(
                                    il,
                                    ol,
                                    arc.weight.clone(),
                                    arc.nextstate,
                                ));
                            }
                        }
                    } else {
                        nt.push(arc);
                    }
                }
                for a in nt {
                    tc.add_tr(s, a).unwrap();
                }
            }
            tc.set_input_symbols(Arc::new(st));
            tc
        }

        /// 'substitute(StdVectorFst*, const StringPair old, StdVectorFst *transducer)'.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.substitute-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.substitute-fn]
        pub fn substitute_string_transducer(
            t: &StdVectorFst,
            old_symbol_pair: StringPair,
            transducer: &StdVectorFst,
        ) -> StdVectorFst {
            // assert(t->InputSymbols() != NULL);
            let mut result = t.clone();
            let mut st = (**result.input_symbols().unwrap()).clone();
            let old_il = st.add_symbol(old_symbol_pair.0.as_str());
            let old_ol = st.add_symbol(old_symbol_pair.1.as_str());

            let states = result.num_states() as u32;
            for i in 0..states {
                let trs = result.pop_trs(i).unwrap();
                let mut kept: Vec<StdTransition> = Vec::with_capacity(trs.len());
                for mut arc in trs {
                    // find arcs that must be replaced
                    if arc.ilabel == old_il && arc.olabel == old_ol {
                        let destination_state = arc.nextstate;
                        let start_state = result.add_state();

                        // change the label of the arc to epsilon and point the arc
                        // to a new state (weight remains the same)
                        arc.ilabel = 0;
                        arc.olabel = 0;
                        arc.nextstate = start_state;
                        kept.push(arc);

                        // add rest of the states to transducer t
                        let states_to_add = transducer.num_states();
                        for _ in 1..states_to_add {
                            result.add_state();
                        }

                        // go through all states and arcs in replace transducer tr
                        for tr_state_id in transducer.states_iter() {
                            // final states in tr correspond in t to a non-final
                            // state which has an epsilon transition to original
                            // destination state of arc that is being replaced
                            if transducer.is_final(tr_state_id).unwrap() {
                                let fw = transducer.final_weight(tr_state_id).unwrap().unwrap();
                                result
                                    .add_tr(
                                        tr_state_id + start_state,
                                        StdTransition::new(0, 0, fw, destination_state),
                                    )
                                    .unwrap();
                            }

                            for tr_arc in transducer.get_trs(tr_state_id).unwrap().trs() {
                                result
                                    .add_tr(
                                        tr_state_id + start_state,
                                        StdTransition::new(
                                            tr_arc.ilabel,
                                            tr_arc.olabel,
                                            tr_arc.weight.clone(),
                                            tr_arc.nextstate + start_state,
                                        ),
                                    )
                                    .unwrap();
                            }
                        }
                    } else {
                        kept.push(arc);
                    }
                }
                for a in kept {
                    result.add_tr(i, a).unwrap();
                }
            }

            result.set_input_symbols(Arc::new(st));
            result
        }

        /// 'substitute(StdVectorFst*, const NumberPair old, StdVectorFst *transducer)'.
        pub fn substitute_number_transducer(
            t: &StdVectorFst,
            old_number_pair: NumberPair,
            transducer: &StdVectorFst,
        ) -> StdVectorFst {
            let mut result = t.clone();

            let states = result.num_states() as u32;
            for i in 0..states {
                let trs = result.pop_trs(i).unwrap();
                let mut kept: Vec<StdTransition> = Vec::with_capacity(trs.len());
                for mut arc in trs {
                    // find arcs that must be replaced
                    if arc.ilabel == old_number_pair.0 && arc.olabel == old_number_pair.1 {
                        let destination_state = arc.nextstate;
                        let start_state = result.add_state();

                        arc.ilabel = 0;
                        arc.olabel = 0;
                        arc.nextstate = start_state;
                        kept.push(arc);

                        let states_to_add = transducer.num_states();
                        for _ in 1..states_to_add {
                            result.add_state();
                        }

                        for tr_state_id in transducer.states_iter() {
                            if transducer.is_final(tr_state_id).unwrap() {
                                let fw = transducer.final_weight(tr_state_id).unwrap().unwrap();
                                result
                                    .add_tr(
                                        tr_state_id + start_state,
                                        StdTransition::new(0, 0, fw, destination_state),
                                    )
                                    .unwrap();
                            }

                            for tr_arc in transducer.get_trs(tr_state_id).unwrap().trs() {
                                result
                                    .add_tr(
                                        tr_state_id + start_state,
                                        StdTransition::new(
                                            tr_arc.ilabel,
                                            tr_arc.olabel,
                                            tr_arc.weight.clone(),
                                            tr_arc.nextstate + start_state,
                                        ),
                                    )
                                    .unwrap();
                            }
                        }
                    } else {
                        kept.push(arc);
                    }
                }
                for a in kept {
                    result.add_tr(i, a).unwrap();
                }
            }

            result
        }

        // ---- insert_freely -------------------------------------------------------

        /// 'insert_freely(StdVectorFst*, const StringPair &symbol_pair)'.
        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-freely-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-freely-fn]
        pub fn insert_freely_string(t: &StdVectorFst, symbol_pair: &StringPair) -> StdVectorFst {
            let mut result = t.clone();
            let mut st = (**result.input_symbols().unwrap()).clone();
            // assert(st != NULL);
            let states: Vec<StateId> = result.states_iter().collect();
            for state_id in states {
                let il = st.add_symbol(symbol_pair.0.as_str());
                let ol = st.add_symbol(symbol_pair.1.as_str());
                result
                    .add_tr(
                        state_id,
                        StdTransition::new(il, ol, TropicalWeight::new(0.0), state_id),
                    )
                    .unwrap();
            }
            result.set_input_symbols(Arc::new(st));
            result
        }

        // ---- weight setters ------------------------------------------------------

        // ---- harmonize + symbol/number recoding ----------------------------------

        /* Find the number-to-number mappings needed to be performed to t1 so that
        it will follow the same symbol-to-number encoding as t2. */

        /* Recode the symbol numbers in this transducer as indicated in KeyMap km. */

        /* Expand "?:?", "?:x" and "x:?" transitions according to 'unknown'. */

        // ---- alphabet / symbol-table queries -------------------------------------

        /* recursive helper */

        /* recursive helper */

        // ---- predicates / counts -------------------------------------------------

        // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-cyclic-fn]
        // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-cyclic-fn]
        pub fn is_cyclic(t: &StdVectorFst) -> bool {
            // C++: return t->Properties(kCyclic, true) & kCyclic;
            let mut known = FstProperties::empty();
            let props = compute_fst_properties(t, FstProperties::CYCLIC, &mut known, true)
                .expect("rustfst compute_fst_properties");
            props.contains(FstProperties::CYCLIC)
        }
    }
}
