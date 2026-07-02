//! Port of 'libhfst/src/implementations/LogWeightTransducer.{h,cc}' — the
//! OpenFST log-weight backend bridge between the HFST API and rustfst's
//! 'VectorFst<LogWeight>' (= ['LogFst'] / ['LogVectorFst']).
//!
//! In C++ 'LogWeightTransducer' is a class of (almost entirely) STATIC
//! methods — a stateless operations wrapper over 'fst::LogFst'. It is
//! modelled here as a unit struct ['LogWeightTransducer'] with an 'impl'
//! block of associated functions. The two stream helper classes
//! (['LogWeightInputStream'] / ['LogWeightOutputStream']) become their
//! own structs, and the 'LogArcLessThan' comparator becomes a small struct.
//!
//! 'using namespace fst;' in the C++ header is mapped onto the
//! 'hfst-openfst' adapter: 'LogVectorFst', 'LogTransition' (= 'fst::LogArc'),
//! 'LogWeight', 'SymbolTable', 'StateId', and the 'algorithms::' module.
//!
//! Ownership mapping for the C++ 'LogFst*' signatures:
//! - factory / unary-op methods that the C++ 'new's a result and returns
//!   'LogFst*' -> return owned 'LogVectorFst'.
//! - methods that take a 'LogFst*' and read it -> '&LogVectorFst'.
//! - methods that take a 'LogFst*' and mutate it in place (state/arc
//!   builders, 'add_to_weights', symbol-table setters, ...) -> '&mut LogVectorFst'.
//! - 'delete_transducer(LogFst*)' -> dropping the owned 'LogVectorFst'.
//! The C++ 'int64' typedef is 'i64' here.
// [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.delete-transducer-fn]
// [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.delete-transducer-fn]

#![allow(non_snake_case)]
#![allow(dead_code)] // many ported ops are only reached once the facade lands

use std::collections::{BTreeMap, BTreeSet};

use hfst_openfst::algorithms;
use hfst_openfst::prelude::*;
use hfst_openfst::{LogTransition, LogVectorFst, LogWeight, SymbolTable};

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

// [spec:hfst:def:log-weight-transducer.int64]

// [spec:hfst:def:log-weight-transducer.hfst.implementations.state-id]
pub type StateId = u32;

// [spec:hfst:def:log-weight-transducer.hfst.implementations.log-fst]
pub type LogFst = LogVectorFst;

/// 'typedef std::set<std::string> StringSet' (used by the alphabet helpers).
pub type StringSet = BTreeSet<String>;

// [spec:hfst:def:log-weight-transducer.hfst.implementations.log-arc-vector]
pub type LogArcVector = Vec<LogTransition>;

// [spec:hfst:def:log-weight-transducer.hfst.implementations.log-arc-less-than]
pub struct LogArcLessThan;

#[allow(dead_code)]
impl LogArcLessThan {
    // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-arc-less-than.operator-fn]
    // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-arc-less-than.operator-fn]
    // Standard LogArc strict ordering (ilabel, then olabel, then weight, then
    // target state). The C++ declares this comparator but never defines or uses
    // it; a faithful total order keeps it correct rather than panicking.
    pub fn operator_call(&self, arc1: &LogTransition, arc2: &LogTransition) -> bool {
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

// [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream]
pub struct LogWeightInputStream<'a> {
    filename: String,
    /// C++ holds an 'std::ifstream i_stream' plus an 'std::istream &input_stream'
    /// reference that aliases either 'i_stream' or 'std::cin'. Modelled here as a
    /// single owned binary input stream (per the porting convention,
    /// 'std::istream' (binary) -> 'crate::transducer::IStream').
    input_stream: IStream<'a>,
}

// (no Default: LogWeightInputStream borrows its reader and cannot be
// constructed without one; the no-source ctors are deferred.)

// [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream]
pub struct LogWeightOutputStream {
    filename: String,
    /// C++ holds 'std::ofstream o_stream' + 'std::ostream &output_stream' that
    /// aliases either it or 'std::cout'. Modelled as a single owned writer.
    output_stream: Box<dyn std::io::Write>,
}

/* Maps state numbers in AT&T text format to state ids used by OpenFst. */
// [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.state-map]
pub type StateMap = BTreeMap<i32, StateId>;

// [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer]
pub struct LogWeightTransducer;

// ===== construction_io (workflow body) =====
mod construction_io {
    #![allow(unused_imports)]
    use super::*;
    // ===========================================================================
    // AREA: construction_io  (bodies for LogWeightTransducer.{h,cc})
    //
    // Extra imports needed beyond the skeleton header (integrator: merge/dedupe):
    use std::io::{BufRead, Read, Write};
    use std::sync::Arc;

    use crate::hfst_flag_diacritics::FdOperation;
    // 'HfstFatalException' is referenced by the (deferred) read_transducer path.
    #[allow(unused_imports)]
    // ---------------------------------------------------------------------------
    // File-static globals from the .cc
    // ---------------------------------------------------------------------------

    // 'std::ostream * LogWeightTransducer::warning_stream = NULL;'

    // ---------------------------------------------------------------------------
    // Private module helpers (introduced for the port; not in the C++ header).
    // ---------------------------------------------------------------------------

    /// A weight written with 6 decimals matches the C++ '%f' formatting;
    /// 'operator<<' (ostream) uses the default float formatting, which we
    /// approximate with 'Display'.
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
        t: &LogVectorFst,
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

    fn write_in_att_format_core(t: &LogVectorFst, os: &mut dyn Write, number: bool, c_style: bool) {
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

    // [spec:hfst:def:log-weight-transducer.hfst.implementations.print-att-number-fn]
    // [spec:hfst:sem:log-weight-transducer.hfst.implementations.print-att-number-fn]
    #[allow(dead_code)]
    pub fn print_att_number(t: &LogVectorFst, os: &mut dyn std::io::Write) {
        let _ = write!(
            os,
            "initial state: {}\n",
            t.start().map(|s| s as i64).unwrap_or(-1)
        );
        for s in t.states_iter() {
            if t.is_final(s).unwrap() {
                let fw = *t.final_weight(s).unwrap().unwrap().value();
                let _ = write!(os, "{}\t{:.6}\n", s, fw);
            }
            for arc in t.get_trs(s).unwrap().trs() {
                let _ = write!(
                    os,
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
    // LogWeightInputStream
    // ===========================================================================

    #[allow(dead_code)]
    impl<'a> LogWeightInputStream<'a> {
        /// 'LogWeightInputStream(void)' — reads from stdin.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.log-weight-input-stream-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.log-weight-input-stream-fn]
        pub fn new() -> Self {
            // C++ reads from std::cin; own a stdin reader.
            LogWeightInputStream {
                filename: String::new(),
                input_stream: IStream::new_owned(std::io::stdin()),
            }
        }

        /// 'LogWeightInputStream(const std::string &filename)'.
        pub fn new_filename(filename: &str) -> Self {
            // C++ opens an ifstream in binary mode; own the opened file (a failed
            // open leaves the stream in the not-good state the C++ would have).
            let reader: Box<dyn std::io::Read> = match std::fs::File::open(filename) {
                Ok(f) => Box::new(f),
                Err(_) => Box::new(std::io::empty()),
            };
            LogWeightInputStream {
                filename: filename.to_string(),
                input_stream: IStream::new_owned(reader),
            }
        }

        /// 'LogWeightInputStream(std::istream &is)'.
        pub fn new_istream(is: IStream<'a>) -> Self {
            LogWeightInputStream {
                filename: String::new(),
                input_stream: is,
            }
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.skip-identifier-version-3-0-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.skip-identifier-version-3-0-fn]
        fn skip_identifier_version_3_0(&mut self) {
            // C++: input_stream.ignore(14); (Log skips the "LOG_OFST_TYPE" identifier)
            self.ignore(14);
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.skip-hfst-header-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.skip-hfst-header-fn]
        fn skip_hfst_header(&mut self) {
            self.ignore(6);
            self.skip_identifier_version_3_0();
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.close-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.close-fn]
        pub fn close(&mut self) {
            if !self.filename.is_empty() {
                // The underlying reader is borrowed (owned by the caller); there is
                // nothing to close on our side.
            }
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-eof-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-eof-fn]
        pub fn is_eof(&self) -> bool {
            // C++ tests 'input_stream.peek() == EOF'; 'IStream' has no peek, so we
            // approximate with the good/fail flag (set once a read hits EOF).
            !self.input_stream.good()
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-bad-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-bad-fn]
        pub fn is_bad(&self) -> bool {
            !self.input_stream.good()
        }

        // Also 'bool operator() (void) const' — the stream-good predicate.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-good-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-good-fn]
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.operator-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.operator-fn]
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.ignore-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.ignore-fn]
        pub fn ignore(&mut self, n: u32) {
            let mut buf = vec![0u8; n as usize];
            self.input_stream.read(&mut buf);
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.read-transducer-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.read-transducer-fn]
        pub fn read_transducer(&mut self) -> crate::error::Result<LogVectorFst> {
            if self.is_eof() {
                crate::bail!(StreamIsClosed);
            }
            // rustfst has no streaming istream read, so read the remaining bytes
            // and 'load_prefix' one FST from the front (it reports how many bytes
            // it consumed); put the unused remainder back for the next read.
            let bytes = self.input_stream.read_to_end();
            let (fst, consumed) = match LogVectorFst::load_prefix(&bytes) {
                Ok(x) => x,
                Err(_) => {
                    crate::bail!(
                        TransducerHasWrongType,
                        "could not read LOG_OPENFST transducer payload"
                    )
                }
            };
            for &b in bytes[consumed..].iter().rev() {
                self.input_stream.putback(b);
            }
            Ok(fst)
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-get-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-get-fn]
        pub fn stream_get(&mut self) -> char {
            let mut b = [0u8; 1];
            self.input_stream.read(&mut b);
            b[0] as char
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-get-short-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-get-short-fn]
        pub fn stream_get_short(&mut self) -> i16 {
            let mut b = [0u8; 2];
            self.input_stream.read(&mut b);
            i16::from_ne_bytes(b)
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-unget-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-unget-fn]
        pub fn stream_unget(&mut self, c: char) {
            self.input_stream.putback(c as u8);
        }

        /// 'static bool is_fst(...)' — peek the reader's first byte without consuming it.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-fst-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-fst-fn]
        pub fn is_fst_file(is: &mut dyn std::io::BufRead) -> bool {
            is.fill_buf().ok().and_then(|b| b.first().copied()) == Some(0xd6)
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
    // LogWeightOutputStream
    // ===========================================================================

    #[allow(dead_code)]
    impl LogWeightOutputStream {
        /// 'LogWeightOutputStream(void)' — writes to stdout.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream.log-weight-output-stream-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-output-stream.log-weight-output-stream-fn]
        pub fn new() -> Self {
            // C++ also does 'if (!output_stream) fprintf(stderr, "...failbit set (3)")',
            // but a 'Box<dyn Write>' over stdout cannot report a fail state here.
            LogWeightOutputStream {
                filename: String::new(),
                output_stream: Box::new(std::io::stdout()),
            }
        }

        /// 'LogWeightOutputStream(const std::string &filename)'.
        pub fn new_filename(filename: &str) -> Self {
            let file =
                std::fs::File::create(filename).expect("LogWeightOutputStream: cannot open file");
            LogWeightOutputStream {
                filename: filename.to_string(),
                output_stream: Box::new(file),
            }
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream.close-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-output-stream.close-fn]
        pub fn close(&mut self) {
            // Flush unconditionally: stdout is a buffered std::io::stdout() and the
            // tools exit via std::process::exit (no Drop), so it must be flushed too.
            let _ = self.output_stream.flush();
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream.write-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-output-stream.write-fn]
        pub fn write(&mut self, c: char) {
            let _ = self.output_stream.write_all(&[c as u8]);
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream.write-transducer-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-output-stream.write-transducer-fn]
        pub fn write_transducer(&mut self, transducer: &LogVectorFst) {
            // C++ also does 'if (!output_stream) fprintf(stderr, "...failbit set (1)")';
            // a 'Box<dyn Write>' cannot report a fail state, so that check is dropped.
            //
            // When writing a transducer, both input and output symbol tables are
            // included; the C++ sets the output table = input table on the caller's
            // transducer. The skeleton hands us '&LogVectorFst', so we do it on a clone
            // (NOTE: caller's transducer is not mutated, unlike C++). Unlike Tropical,
            // the Log .cc has NO 'hfst_format' branch — it always re-symbols.
            let mut t = transducer.clone();
            let output_st = transducer.input_symbols().unwrap().as_ref().clone();
            t.set_output_symbols(Arc::new(output_st));
            let _ = t.store(&mut self.output_stream);
            let _ = self.output_stream.flush();
        }
    }

    // ===========================================================================
    // LogWeightTransducer — construction / IO / symbol-table / accessors
    // ===========================================================================

    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    impl LogWeightTransducer {
        // ---- profiling / warning-stream globals ----

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-profile-seconds-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-profile-seconds-fn]
        pub fn get_profile_seconds() -> f32 {
            // 'log_seconds_in_harmonize' is 0 unless PROFILE_OPENFST (compiled out).
            0.0
        }

        // ---- private symbol-table helpers ----

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.create-symbol-table-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.create-symbol-table-fn]
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.initialize-symbol-tables-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.initialize-symbol-tables-fn]
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
        fn initialize_symbol_tables(t: &mut LogVectorFst) {
            let st = Self::create_symbol_table(String::new());
            t.set_input_symbols(Arc::new(st));
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-symbol-table-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-symbol-table-fn]
        fn remove_symbol_table(t: &mut LogVectorFst) {
            let _ = t.take_input_symbols();
        }

        // ---- factories ----

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.create-empty-transducer-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.create-empty-transducer-fn]
        pub fn create_empty_transducer() -> LogVectorFst {
            let mut t = LogVectorFst::new();
            Self::initialize_symbol_tables(&mut t);
            let s = t.add_state();
            t.set_start(s).unwrap();
            t
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.create-epsilon-transducer-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.create-epsilon-transducer-fn]
        pub fn create_epsilon_transducer() -> LogVectorFst {
            let mut t = LogVectorFst::new();
            Self::initialize_symbol_tables(&mut t);
            let s = t.add_state();
            t.set_start(s).unwrap();
            t.set_final(s, 0.0f32).unwrap();
            t
        }

        // ---- string versions of define_transducer (no asserts in the Log .cc) ----

        pub fn define_transducer_symbol(symbol: &str) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let s1 = t.add_state();
            let s2 = t.add_state();
            t.set_start(s1).unwrap();
            t.set_final(s2, 0.0f32).unwrap();
            let il = st.add_symbol(symbol);
            let ol = st.add_symbol(symbol);
            t.add_tr(s1, LogTransition::new(il, ol, 0.0f32, s2))
                .unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        pub fn define_transducer_symbol_pair(isymbol: &str, osymbol: &str) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let s1 = t.add_state();
            let s2 = t.add_state();
            t.set_start(s1).unwrap();
            t.set_final(s2, 0.0f32).unwrap();
            let il = st.add_symbol(isymbol);
            let ol = st.add_symbol(osymbol);
            t.add_tr(s1, LogTransition::new(il, ol, 0.0f32, s2))
                .unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        pub fn define_transducer_spv(spv: &StringPairVector) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let mut s1 = t.add_state();
            t.set_start(s1).unwrap();
            for it in spv {
                let s2 = t.add_state();
                let il = st.add_symbol(it.0.as_str());
                let ol = st.add_symbol(it.1.as_str());
                t.add_tr(s1, LogTransition::new(il, ol, 0.0f32, s2))
                    .unwrap();
                s1 = s2;
            }
            t.set_final(s1, 0.0f32).unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.define-transducer-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.define-transducer-fn]
        pub fn define_transducer_sps(sps: &StringPairSet, cyclic: bool) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let s1 = t.add_state(); // start state
            t.set_start(s1).unwrap();
            let mut s2 = s1; // final state
            if !sps.is_empty() {
                if !cyclic {
                    s2 = t.add_state();
                }
                for it in sps {
                    let il = st.add_symbol(it.0.as_str());
                    let ol = st.add_symbol(it.1.as_str());
                    t.add_tr(s1, LogTransition::new(il, ol, 0.0f32, s2))
                        .unwrap();
                }
            }
            t.set_final(s2, 0.0f32).unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        pub fn define_transducer_spsv(spsv: &[StringPairSet]) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());
            let mut s1 = t.add_state();
            t.set_start(s1).unwrap();
            for spset in spsv {
                let s2 = t.add_state();
                for it2 in spset {
                    let il = st.add_symbol(it2.0.as_str());
                    let ol = st.add_symbol(it2.1.as_str());
                    t.add_tr(s1, LogTransition::new(il, ol, 0.0f32, s2))
                        .unwrap();
                }
                s1 = s2;
            }
            t.set_final(s1, 0.0f32).unwrap();
            t.set_input_symbols(Arc::new(st));
            t
        }

        // ---- number versions of define_transducer ----

        pub fn define_transducer_number(number: u32) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            Self::initialize_symbol_tables(&mut t);
            let s1 = t.add_state();
            let s2 = t.add_state();
            t.set_start(s1).unwrap();
            t.set_final(s2, 0.0f32).unwrap();
            t.add_tr(s1, LogTransition::new(number, number, 0.0f32, s2))
                .unwrap();
            t
        }

        pub fn define_transducer_number_pair(inumber: u32, onumber: u32) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            Self::initialize_symbol_tables(&mut t);
            let s1 = t.add_state();
            let s2 = t.add_state();
            t.set_start(s1).unwrap();
            t.set_final(s2, 0.0f32).unwrap();
            t.add_tr(s1, LogTransition::new(inumber, onumber, 0.0f32, s2))
                .unwrap();
            t
        }

        pub fn define_transducer_npv(npv: &NumberPairVector) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            let mut s1 = t.add_state();
            t.set_start(s1).unwrap();
            for it in npv {
                let s2 = t.add_state();
                t.add_tr(s1, LogTransition::new(it.0, it.1, 0.0f32, s2))
                    .unwrap();
                s1 = s2;
            }
            t.set_final(s1, 0.0f32).unwrap();
            t
        }

        pub fn define_transducer_nps(nps: &NumberPairSet, cyclic: bool) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            let s1 = t.add_state(); // start state
            t.set_start(s1).unwrap();
            let mut s2 = s1; // final state
            if !nps.is_empty() {
                if !cyclic {
                    s2 = t.add_state();
                }
                for it in nps {
                    t.add_tr(s1, LogTransition::new(it.0, it.1, 0.0f32, s2))
                        .unwrap();
                }
            }
            t.set_final(s2, 0.0f32).unwrap();
            t
        }

        pub fn define_transducer_npsv(npsv: &[NumberPairSet]) -> LogVectorFst {
            let mut t = LogVectorFst::new();
            let mut s1 = t.add_state();
            t.set_start(s1).unwrap();
            for npset in npsv {
                let s2 = t.add_state();
                for it2 in npset {
                    t.add_tr(s1, LogTransition::new(it2.0, it2.1, 0.0f32, s2))
                        .unwrap();
                }
                s1 = s2;
            }
            t.set_final(s1, 0.0f32).unwrap();
            t
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.copy-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.copy-fn]
        pub fn copy(t: &LogVectorFst) -> LogVectorFst {
            t.clone()
        }

        // ---- weight properties / setters ----
        //
        // NOTE: 'add_to_weights', 'get_smallest_weight' and 'has_weights' are NOT in
        // the Log .cc — mirrored from the Tropical port for API parity (the Log
        // determinize/minimize do not actually call them).

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-to-weights-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-to-weights-fn]
        pub fn add_to_weights(t: &mut LogVectorFst, w: f32) {
            let states: Vec<StateId> = t.states_iter().collect();
            for s in states {
                // (no in-place arc mutator in rustfst — pop & re-add, order preserved)
                let trs = t.pop_trs(s).unwrap();
                for arc in trs {
                    let nw = *arc.weight.value() + w;
                    t.add_tr(
                        s,
                        LogTransition::new(arc.ilabel, arc.olabel, nw, arc.nextstate),
                    )
                    .unwrap();
                }
                if t.is_final(s).unwrap() {
                    let old_weight = *t.final_weight(s).unwrap().unwrap().value();
                    t.set_final(s, old_weight + w).unwrap();
                }
            }
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-smallest-weight-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-smallest-weight-fn]
        pub fn get_smallest_weight(t: &LogVectorFst) -> f32 {
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.has-weights-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.has-weights-fn]
        pub fn has_weights(t: &LogVectorFst) -> bool {
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.set-final-weights-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.set-final-weights-fn]
        pub fn set_final_weights(t: &LogVectorFst, weight: f32) -> LogVectorFst {
            // NOTE: the Log .cc has NO 'increment' parameter (unlike Tropical); it
            // always overwrites. C++ mutates 't' in place and returns it; the skeleton
            // hands us '&LogVectorFst', so we work on a clone (caller not mutated).
            let mut t = t.clone();
            let states: Vec<StateId> = t.states_iter().collect();
            for s in states {
                if t.is_final(s).unwrap() {
                    t.set_final(s, weight).unwrap();
                }
            }
            t
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.transform-weights-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.transform-weights-fn]
        pub fn transform_weights(t: &LogVectorFst, func: fn(f32) -> f32) -> LogVectorFst {
            // C++ mutates in place (final first, then arcs via MutableArcIterator) and
            // returns 't'; we operate on a clone, pop/re-add to preserve arc order.
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
                        LogTransition::new(arc.ilabel, arc.olabel, nw, arc.nextstate),
                    )
                    .unwrap();
                }
            }
            t
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.set-weight-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.set-weight-fn]
        pub fn set_weight(t: &LogVectorFst, f: f32) -> LogVectorFst {
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

        /// 'write_in_att_format(LogFst*, std::ostream &os)'.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.write-in-att-format-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.write-in-att-format-fn]
        pub fn write_in_att_format_ostream(t: &LogVectorFst, os: &mut dyn std::io::Write) {
            write_in_att_format_core(t, os, false, false);
        }

        /// 'write_in_att_format_number(LogFst*, std::ostream &os)'.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.write-in-att-format-number-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.write-in-att-format-number-fn]
        pub fn write_in_att_format_number_ostream(t: &LogVectorFst, os: &mut dyn std::io::Write) {
            write_in_att_format_core(t, os, true, false);
        }

        // ---- AT&T read ----

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-and-map-state-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-and-map-state-fn]
        fn add_and_map_state(
            t: &mut LogVectorFst,
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.read-in-att-format-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.read-in-att-format-fn]
        pub fn read_in_att_format(
            ifile: &mut dyn std::io::BufRead,
        ) -> crate::error::Result<LogVectorFst> {
            let mut t = LogVectorFst::new();
            let mut st = Self::create_symbol_table(String::new());

            let mut state_map: StateMap = StateMap::new();

            // Add initial state that is numbered as zero.
            let initial_state = Self::add_and_map_state(&mut t, 0, &mut state_map);
            t.set_start(initial_state).unwrap();

            loop {
                let line_str = match crate::io_utils::read_line_lossy(ifile) {
                    None => break,
                    Some(l) => l,
                };
                let bytes = line_str.as_bytes();

                if !bytes.is_empty() && bytes[0] == b'-' {
                    // transducer separator
                    return Ok(t);
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
                        LogTransition::new(input_number, output_number, weight, target_state),
                    )
                    .unwrap();
                } else {
                    // line could not be parsed
                    let message = line_str.to_string();
                    crate::bail!(NotValidAttFormat, message);
                }
            }

            t.set_input_symbols(Arc::new(st));
            Ok(t)
        }

        // ---- alphabet / symbol-table handling ----

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.insert-to-alphabet-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.insert-to-alphabet-fn]
        pub fn insert_to_alphabet(t: &mut LogVectorFst, symbol: &str) {
            assert!(t.input_symbols().is_some());
            let mut st = t.input_symbols().unwrap().as_ref().clone();
            st.add_symbol(symbol);
            t.set_input_symbols(Arc::new(st));
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-from-alphabet-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-from-alphabet-fn]
        pub fn remove_from_alphabet(t: &mut LogVectorFst, symbol: &str) {
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-alphabet-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-alphabet-fn]
        pub fn get_alphabet(t: &LogVectorFst) -> StringSet {
            assert!(t.input_symbols().is_some());
            let mut s = StringSet::new();
            let st = t.input_symbols().unwrap();
            for (_l, sym) in st.iter() {
                s.insert(sym.to_string());
            }
            s
        }

        // NOTE: 'get_initial_input_symbols(_rec)' / 'get_first_input_symbols(_rec)' are
        // NOT in the Log .cc — mirrored from the Tropical port for API parity.

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-initial-input-symbols-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-initial-input-symbols-fn]
        pub fn get_initial_input_symbols_rec(
            t: &LogVectorFst,
            s: StateId,
            visited_states: &mut BTreeSet<StateId>,
            symbols: &mut StringSet,
        ) {
            visited_states.insert(s);
            let trs: Vec<LogTransition> = t.get_trs(s).unwrap().trs().to_vec();
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

        pub fn get_initial_input_symbols(t: &LogVectorFst) -> StringSet {
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-first-input-symbols-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-first-input-symbols-fn]
        pub fn get_first_input_symbols_rec(
            t: &LogVectorFst,
            s: StateId,
            visited_states: &mut BTreeSet<StateId>,
            symbols: &mut StringSet,
        ) {
            visited_states.insert(s);
            let trs: Vec<LogTransition> = t.get_trs(s).unwrap().trs().to_vec();
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

        pub fn get_first_input_symbols(t: &LogVectorFst) -> StringSet {
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-symbol-number-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-symbol-number-fn]
        pub fn get_symbol_number(t: &LogVectorFst, symbol: &str) -> crate::error::Result<u32> {
            assert!(t.input_symbols().is_some());
            match t.input_symbols().unwrap().get_label(symbol) {
                None => crate::bail!(SymbolNotFound),
                Some(i) => Ok(i),
            }
        }

        // NOTE: 'get_biggest_symbol_number' / 'get_symbol_vector' / 'set_symbol_table'
        // / 'print_alphabet' are NOT in the Log .cc — mirrored from Tropical.

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-biggest-symbol-number-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-biggest-symbol-number-fn]
        pub fn get_biggest_symbol_number(t: &LogVectorFst) -> u32 {
            let mut biggest_number = 0u32;
            for (label, _sym) in t.input_symbols().unwrap().iter() {
                if label > biggest_number {
                    biggest_number = label;
                }
            }
            biggest_number
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-symbol-vector-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-symbol-vector-fn]
        pub fn get_symbol_vector(t: &LogVectorFst) -> StringVector {
            let biggest_symbol_number = Self::get_biggest_symbol_number(t);
            let mut symbol_vector: StringVector =
                vec![String::new(); (biggest_symbol_number + 1) as usize];

            let alphabet = Self::get_alphabet(t);
            for it in alphabet.iter() {
                let symbol_number = Self::get_symbol_number(t, it).expect(
                    "symbol enumerated from this transducer's own symbol table is present in it",
                );
                symbol_vector[symbol_number as usize] = it.clone();
            }
            symbol_vector
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.create-mapping-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.create-mapping-fn]
        pub fn create_mapping(t1: &LogVectorFst, t2: &LogVectorFst) -> NumberNumberMap {
            let mut km = NumberNumberMap::new();
            let st1 = t1.input_symbols().unwrap();
            let st2 = t2.input_symbols().unwrap();
            for (label, sym) in st1.iter() {
                let mapped = st2.get_label(sym).unwrap();
                km.insert(label, mapped);
            }
            km
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.recode-symbol-numbers-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.recode-symbol-numbers-fn]
        pub fn recode_symbol_numbers(t: &mut LogVectorFst, km: &mut NumberNumberMap) {
            let states: Vec<StateId> = t.states_iter().collect();
            for s in states {
                let trs = t.pop_trs(s).unwrap();
                for arc in trs {
                    // C++ 'km[label]' inserts 0 for a missing key.
                    let il = *km.entry(arc.ilabel).or_insert(0);
                    let ol = *km.entry(arc.olabel).or_insert(0);
                    t.add_tr(s, LogTransition::new(il, ol, arc.weight, arc.nextstate))
                        .unwrap();
                }
            }
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.set-symbol-table-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.set-symbol-table-fn]
        pub fn set_symbol_table(t: &mut LogVectorFst, symbol_mappings: Vec<(u16, String)>) {
            let mut st = Self::create_symbol_table(String::new());
            for (num, sym) in &symbol_mappings {
                // NOTE: C++ 'AddSymbol(sym, num)' honours the explicit label; rustfst
                // has no add-at-explicit-label, so 'num' is ignored (gap).
                let _ = num;
                st.add_symbol(sym.as_str());
            }
            t.set_input_symbols(Arc::new(st));
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.print-alphabet-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.print-alphabet-fn]
        pub fn print_alphabet(t: &LogVectorFst) {
            let line: String = t
                .input_symbols()
                .unwrap()
                .iter()
                .map(|(_l, sym)| format!("'{}', ", sym))
                .collect();
            tracing::debug!("{}", line);
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-flag-diacritics-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-flag-diacritics-fn]
        pub fn get_flag_diacritics(t: &LogVectorFst) -> FdTable<i64> {
            let mut table: FdTable<i64> = FdTable::new();
            let symbols = t.input_symbols().unwrap();
            for (label, sym) in symbols.iter() {
                if FdOperation::is_diacritic(sym) {
                    table.define_diacritic(label as i64, sym);
                }
            }
            table
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.expand-arcs-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.expand-arcs-fn]
        pub fn expand_arcs(
            t: &LogVectorFst,
            unknown: &mut StringSet,
            unknown_symbols_in_use: bool,
        ) -> LogVectorFst {
            // NOTE: unlike the Tropical port, the Log .cc does NOT filter 'unknown'
            // entries through 'FdOperation::is_diacritic' — every unknown symbol is
            // expanded unconditionally. We follow the Log source.
            let mut result = LogVectorFst::new();

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

                let trs: Vec<LogTransition> = t.get_trs(s).unwrap().trs().to_vec();
                for arc in &trs {
                    let result_nextstate = arc.nextstate;

                    if unknown_symbols_in_use {
                        let is = t.input_symbols().unwrap();

                        if arc.ilabel == 1 && arc.olabel == 1 {
                            // cross-product "?:?"
                            for it1 in unknown.iter() {
                                let inumber: i64 =
                                    is.get_label(it1).map(|l| l as i64).unwrap_or(-1);
                                for it2 in unknown.iter() {
                                    let onumber: i64 =
                                        is.get_label(it2).map(|l| l as i64).unwrap_or(-1);
                                    if inumber != onumber {
                                        result
                                            .add_tr(
                                                result_s,
                                                LogTransition::new(
                                                    inumber as u32,
                                                    onumber as u32,
                                                    arc.weight,
                                                    result_nextstate,
                                                ),
                                            )
                                            .unwrap();
                                    }
                                }
                                result
                                    .add_tr(
                                        result_s,
                                        LogTransition::new(
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
                                        LogTransition::new(
                                            1,
                                            inumber as u32,
                                            arc.weight,
                                            result_nextstate,
                                        ),
                                    )
                                    .unwrap();
                            }
                        } else if arc.ilabel == 2 || arc.olabel == 2 {
                            // identity "?:?"
                            for it in unknown.iter() {
                                let number: i64 = is.get_label(it).map(|l| l as i64).unwrap_or(-1);
                                result
                                    .add_tr(
                                        result_s,
                                        LogTransition::new(
                                            number as u32,
                                            number as u32,
                                            arc.weight,
                                            result_nextstate,
                                        ),
                                    )
                                    .unwrap();
                            }
                        } else if arc.ilabel == 1 {
                            // "?:x"
                            for it in unknown.iter() {
                                let number: i64 = is.get_label(it).map(|l| l as i64).unwrap_or(-1);
                                result
                                    .add_tr(
                                        result_s,
                                        LogTransition::new(
                                            number as u32,
                                            arc.olabel,
                                            arc.weight,
                                            result_nextstate,
                                        ),
                                    )
                                    .unwrap();
                            }
                        } else if arc.olabel == 1 {
                            // "x:?"
                            for it in unknown.iter() {
                                let number: i64 = is.get_label(it).map(|l| l as i64).unwrap_or(-1);
                                result
                                    .add_tr(
                                        result_s,
                                        LogTransition::new(
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

                    // the original transition is copied in all cases
                    result
                        .add_tr(
                            result_s,
                            LogTransition::new(
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.harmonize-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.harmonize-fn]
        pub fn harmonize(
            t1: &LogVectorFst,
            t2: &LogVectorFst,
            unknown_symbols_in_use: bool,
        ) -> (LogVectorFst, LogVectorFst) {
            // NOTE: C++ takes 'LogFst*' and mutates the inputs in place; the skeleton
            // hands us '&LogVectorFst', so we work on clones — the caller's transducers
            // are NOT mutated (a divergence from the C++ side effect).
            let mut t1 = t1.clone();
            let mut t2 = t2.clone();

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

            // 2. add new symbols from t1 to t2's symbol table...
            // (the Log .cc has NO '< 3' sanity check that the Tropical port carries)
            let mut st2 = t2.input_symbols().unwrap().as_ref().clone();
            for it in unknown_t2.iter() {
                st2.add_symbol(it.as_str());
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.is-automaton-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.is-automaton-fn]
        pub fn is_automaton(t: &LogVectorFst) -> bool {
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.number-of-states-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.number-of-states-fn]
        pub fn number_of_states(t: &LogVectorFst) -> u32 {
            let mut retval = 0u32;
            for _s in t.states_iter() {
                retval += 1;
            }
            retval
        }

        // 'number_of_arcs' is NOT in the Log .cc — mirrored from Tropical for parity.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.number-of-arcs-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.number-of-arcs-fn]
        pub fn number_of_arcs(t: &LogVectorFst) -> u32 {
            let mut retval = 0u32;
            for s in t.states_iter() {
                retval += t.num_trs(s).unwrap() as u32;
            }
            retval
        }

        // ---- public low-level builders ----

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-state-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-state-fn]
        pub fn add_state(t: &mut LogVectorFst) -> StateId {
            let s = t.add_state();
            if s == 0 {
                t.set_start(s).unwrap();
            }
            s
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.set-final-weight-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.set-final-weight-fn]
        pub fn set_final_weight(t: &mut LogVectorFst, s: StateId, w: f32) {
            t.set_final(s, w).unwrap();
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-transition-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-transition-fn]
        pub fn add_transition(
            t: &mut LogVectorFst,
            source: StateId,
            isymbol: &str,
            osymbol: &str,
            w: f32,
            target: StateId,
        ) {
            let mut st = t.input_symbols().unwrap().as_ref().clone();
            let ilabel = st.add_symbol(isymbol);
            let olabel = st.add_symbol(osymbol);
            t.add_tr(source, LogTransition::new(ilabel, olabel, w, target))
                .unwrap();
            t.set_input_symbols(Arc::new(st));
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-final-weight-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-final-weight-fn]
        pub fn get_final_weight(t: &LogVectorFst, s: StateId) -> f32 {
            // C++ 't->Final(s).Value()' — Zero().Value() is +inf for a non-final state.
            t.final_weight(s)
                .unwrap()
                .map(|w| *w.value())
                .unwrap_or(f32::INFINITY)
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.is-final-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.is-final-fn]
        pub fn is_final(t: &LogVectorFst, s: StateId) -> f32 {
            // C++ returns '(t->Final(s) != Zero())' implicitly converted to float.
            if t.is_final(s).unwrap() { 1.0 } else { 0.0 }
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-initial-state-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-initial-state-fn]
        pub fn get_initial_state(t: &LogVectorFst) -> StateId {
            t.start().unwrap_or(NO_STATE_ID)
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
        pub fn represent_empty_transducer_as_having_one_state(t: &mut LogVectorFst) {
            if t.start().is_none() || t.num_states() == 0 {
                // BUG PRESERVED: the C++ does 'delete t; t = create_empty_transducer();',
                // assigning a LOCAL pointer — the caller's transducer is unchanged.
                // We replicate the no-op (mutating *t here would change the caller).
                let _ = &t;
            }
        }
    }
}

// ===== operations (workflow body) =====
mod operations {
    #![allow(unused_imports)]
    use super::*;
    // ===========================================================================
    // area: operations — algebraic operations and OpenFST-algorithm wrappers.
    //
    // Method bodies for the 'impl LogWeightTransducer' block defined in the
    // skeleton. One private module-level free helper is also provided.
    //
    // Ports of 'libhfst/src/implementations/LogWeightTransducer.cc'. The Log .cc is
    // a simpler/older sibling of the Tropical backend: it has NO CHECK_EPSILON_CYCLES,
    // NO get_smallest_weight/add_to_weights weight-shifting, NO encode-weights toggle,
    // and NO 'prune'. 'n_best' is unimplemented (throws), 'are_equivalent' is built on
    // 'minimize', and 'intersect'/'subtract' determinize *both* operands.
    // ===========================================================================

    // 'FunctionNotImplementedException' is needed by 'n_best''s HFST_THROW; the
    // skeleton does not import it, so pull it in here.

    /// 'dst->SetInputSymbols(src->InputSymbols())' — copy 'src''s input symbol table
    /// (as a shared 'Arc') onto 'dst'. No-op when 'src' has no input symbols.
    #[allow(dead_code)]
    fn copy_input_symbol_table(src: &LogVectorFst, dst: &mut LogVectorFst) {
        if let Some(symt) = src.input_symbols().map(|s| std::sync::Arc::clone(s)) {
            dst.set_input_symbols(symt);
        }
    }

    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    impl LogWeightTransducer {
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.push-labels-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.push-labels-fn]
        pub fn push_labels(t: &LogVectorFst, to_initial_state: bool) -> LogVectorFst {
            assert!(t.input_symbols().is_some());

            let mut retval = LogVectorFst::new();
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.push-weights-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.push-weights-fn]
        pub fn push_weights(t: &LogVectorFst, to_initial_state: bool) -> LogVectorFst {
            assert!(t.input_symbols().is_some());

            let mut retval = LogVectorFst::new();
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.determinize-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.determinize-fn]
        pub fn determinize(t: &LogVectorFst) -> LogVectorFst {
            // C++ mutates 't' in place; operate on a local clone.
            let mut t = t.clone();

            algorithms::RmEpsilon(&mut t);
            // EncodeMapper<LogArc> encode_mapper(kEncodeLabels | kEncodeWeights, ENCODE);
            let encode_mapper =
                algorithms::Encode(&mut t, algorithms::EncodeType::EncodeWeightsAndLabels);
            let mut det = LogVectorFst::new();
            algorithms::Determinize(&t, &mut det);
            algorithms::Decode(&mut det, encode_mapper);
            det
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.minimize-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.minimize-fn]
        pub fn minimize(t: &LogVectorFst) -> LogVectorFst {
            // C++ mutates 't' in place; operate on a local clone.
            let mut t = t.clone();

            algorithms::RmEpsilon(&mut t);
            // EncodeMapper<LogArc> encode_mapper(kEncodeLabels /*|kEncodeWeights*/, ENCODE);
            let encode_mapper = algorithms::Encode(&mut t, algorithms::EncodeType::EncodeLabels);
            let mut det = LogVectorFst::new();
            algorithms::Determinize(&t, &mut det);
            algorithms::Minimize(&mut det);
            algorithms::Decode(&mut det, encode_mapper);
            det
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-epsilons-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-epsilons-fn]
        pub fn remove_epsilons(t: &LogVectorFst) -> LogVectorFst {
            // C++: return new LogFst(RmEpsilonFst<LogArc>(*t));
            let mut retval = t.clone();
            algorithms::RmEpsilon(&mut retval);
            retval
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.n-best-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.n-best-fn]
        pub fn n_best(_t: &LogVectorFst, _n: u32) -> LogVectorFst {
            /* LogFst *n_best_fst = new LogFst();
            fst::ShortestPath(*t, n_best_fst, (size_t)n);
            return n_best_fst; */
            // in openfst 1.8 LogFst does not have necessary algebra for shortest paths
            unimplemented!("n_best: not implemented for this transducer type")
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-star-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-star-fn]
        pub fn repeat_star(t: &LogVectorFst) -> LogVectorFst {
            // C++: return new LogFst(ClosureFst<LogArc>(*t, CLOSURE_STAR));
            let mut t = t.clone();
            hfst_openfst::rustfst::algorithms::closure::closure(
                &mut t,
                hfst_openfst::rustfst::algorithms::closure::ClosureType::ClosureStar,
            );
            t
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-plus-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-plus-fn]
        pub fn repeat_plus(t: &LogVectorFst) -> LogVectorFst {
            // C++: return new LogFst(ClosureFst<LogArc>(*t, CLOSURE_PLUS));
            let mut t = t.clone();
            hfst_openfst::rustfst::algorithms::closure::closure(
                &mut t,
                hfst_openfst::rustfst::algorithms::closure::ClosureType::ClosurePlus,
            );
            t
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-n-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-n-fn]
        pub fn repeat_n(t: &LogVectorFst, n: u32) -> LogVectorFst {
            if n == 0 {
                return LogWeightTransducer::create_epsilon_transducer();
            }

            let mut repetition = LogWeightTransducer::create_epsilon_transducer();
            for _ in 0..n {
                algorithms::Concat(&mut repetition, t);
            }
            copy_input_symbol_table(t, &mut repetition);
            repetition
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-le-n-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-le-n-fn]
        pub fn repeat_le_n(t: &LogVectorFst, n: u32) -> LogVectorFst {
            if n == 0 {
                return LogWeightTransducer::create_epsilon_transducer();
            }

            let mut repetition = LogWeightTransducer::create_epsilon_transducer();
            for _ in 0..n {
                let optional_t = LogWeightTransducer::optionalize(t);
                algorithms::Concat(&mut repetition, &optional_t);
            }
            copy_input_symbol_table(t, &mut repetition);
            repetition
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.optionalize-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.optionalize-fn]
        pub fn optionalize(t: &LogVectorFst) -> LogVectorFst {
            let mut eps = LogWeightTransducer::create_epsilon_transducer();
            algorithms::Union(&mut eps, t);
            copy_input_symbol_table(t, &mut eps);
            eps
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.invert-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.invert-fn]
        pub fn invert(t: &LogVectorFst) -> LogVectorFst {
            let mut inverse = LogWeightTransducer::copy(t);
            algorithms::Invert(&mut inverse);
            copy_input_symbol_table(t, &mut inverse);
            inverse
        }

        /* Makes valgrind angry... */
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.reverse-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.reverse-fn]
        pub fn reverse(t: &LogVectorFst) -> LogVectorFst {
            let mut reversed = LogVectorFst::new();
            algorithms::Reverse(t, &mut reversed);
            copy_input_symbol_table(t, &mut reversed);
            reversed
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-input-language-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-input-language-fn]
        pub fn extract_input_language(t: &LogVectorFst) -> LogVectorFst {
            // C++: new LogFst(ProjectFst<LogArc>(*t, ProjectType::INPUT));
            let mut retval = t.clone();
            algorithms::ProjectInput(&mut retval);
            copy_input_symbol_table(t, &mut retval);
            retval
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-output-language-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-output-language-fn]
        pub fn extract_output_language(t: &LogVectorFst) -> LogVectorFst {
            // C++: new LogFst(ProjectFst<LogArc>(*t, ProjectType::OUTPUT));
            let mut retval = t.clone();
            algorithms::ProjectOutput(&mut retval);
            copy_input_symbol_table(t, &mut retval);
            retval
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.compose-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.compose-fn]
        pub fn compose(t1: &LogVectorFst, t2: &LogVectorFst) -> LogVectorFst {
            let mut foo: StringSet = StringSet::new();
            // a copy of t2 is created so that its symbol table check sum is
            // the same as t1's
            // (else OpenFst complains about non-matching check sums... )
            let mut t2_copy = LogWeightTransducer::expand_arcs(t2, &mut foo, false);

            // C++ mutates t1 (sets/sorts); operate on a local clone.
            let mut t1_copy = t1.clone();

            // t2_->SetInputSymbols(t1->InputSymbols());
            let in_syms = t1_copy.input_symbols().map(|s| std::sync::Arc::clone(s));
            if let Some(a) = in_syms.clone() {
                t2_copy.set_input_symbols(a);
            }
            // t1->SetOutputSymbols(t1->InputSymbols());
            if let Some(a) = in_syms {
                t1_copy.set_output_symbols(a);
            }

            algorithms::ArcSortOutput(&mut t1_copy);
            algorithms::ArcSortInput(&mut t2_copy);

            let mut result = LogVectorFst::new();
            algorithms::Compose(&t1_copy, &t2_copy, &mut result);

            // result->SetInputSymbols(t1->InputSymbols());
            copy_input_symbol_table(t1, &mut result);
            result
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.concatenate-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.concatenate-fn]
        pub fn concatenate(t1: &LogVectorFst, t2: &LogVectorFst) -> LogVectorFst {
            let mut result = t1.clone();
            algorithms::Concat(&mut result, t2);
            copy_input_symbol_table(t1, &mut result);
            result
        }

        pub fn disjunct(t1: &LogVectorFst, t2: &LogVectorFst) -> LogVectorFst {
            let mut result = t1.clone();
            algorithms::Union(&mut result, t2);
            copy_input_symbol_table(t1, &mut result);
            result
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.disjunct-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.disjunct-fn]
        pub fn disjunct_spv<'a>(
            t: &'a mut LogVectorFst,
            spv: &StringPairVector,
        ) -> &'a mut LogVectorFst {
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
                        LogTransition::new(inumber, onumber, LogWeight::new(0.0), new_state),
                    )
                    .unwrap();
                    s = new_state;
                }
            }

            t.set_final(s, LogWeight::new(0.0)).unwrap();

            t.set_input_symbols(std::sync::Arc::new(st));
            t
        }

        pub fn disjunct_npv<'a>(
            t: &'a mut LogVectorFst,
            npv: &NumberPairVector,
        ) -> &'a mut LogVectorFst {
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
                        LogTransition::new(inumber, onumber, LogWeight::new(0.0), new_state),
                    )
                    .unwrap();
                    s = new_state;
                }
            }

            t.set_final(s, LogWeight::new(0.0)).unwrap();
            t
        }

        /// 'static LogFst * disjunct_as_tries(LogFst * t1, const LogFst * t2)' —
        /// public trie-disjunction entry point.
        pub fn disjunct_as_tries_pub<'a>(
            t1: &'a mut LogVectorFst,
            t2: &LogVectorFst,
        ) -> &'a mut LogVectorFst {
            let t1_state = t1.start().unwrap();
            let t2_state = t2.start().unwrap();
            LogWeightTransducer::disjunct_as_tries(t1, t1_state, t2, t2_state);
            t1
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.intersect-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.intersect-fn]
        pub fn intersect(t1: &LogVectorFst, t2: &LogVectorFst) -> LogVectorFst {
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

            algorithms::ArcSortOutput(&mut t1);
            algorithms::ArcSortInput(&mut t2);

            algorithms::RmEpsilon(&mut t1);
            algorithms::RmEpsilon(&mut t2);

            // EncodeMapper<LogArc> encoder(0x0001, ENCODE);
            let encoder = algorithms::Encode(&mut t1, algorithms::EncodeType::EncodeLabels);
            let _encoder2 = algorithms::Encode(&mut t2, algorithms::EncodeType::EncodeLabels);

            // DeterminizeFst<LogArc> det1(enc1); det2(enc2);
            let mut det1 = LogVectorFst::new();
            let mut det2 = LogVectorFst::new();
            algorithms::Determinize(&t1, &mut det1);
            algorithms::Determinize(&t2, &mut det2);

            // IntersectFst<LogArc> intersect(det1, det2); foo = new LogFst(intersect);
            let mut foo = LogVectorFst::new();
            algorithms::Intersect(&det1, &det2, &mut foo);

            // DecodeFst<LogArc> decode(*foo, encoder); result = new LogFst(decode);
            algorithms::Decode(&mut foo, encoder);
            let mut result = foo;

            // result->SetInputSymbols(t1->InputSymbols());
            copy_input_symbol_table(&t1, &mut result);
            result
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.subtract-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.subtract-fn]
        pub fn subtract(t1: &LogVectorFst, t2: &LogVectorFst) -> LogVectorFst {
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

            algorithms::ArcSortOutput(&mut t1);
            algorithms::ArcSortInput(&mut t2);

            algorithms::RmEpsilon(&mut t1);
            algorithms::RmEpsilon(&mut t2);

            // EncodeMapper<LogArc> encoder(0x0003, ENCODE); // t2 must be unweighted
            let encoder =
                algorithms::Encode(&mut t1, algorithms::EncodeType::EncodeWeightsAndLabels);
            let _encoder2 =
                algorithms::Encode(&mut t2, algorithms::EncodeType::EncodeWeightsAndLabels);

            // DeterminizeFst<LogArc> det1(enc1); det2(enc2);
            let mut det1 = LogVectorFst::new();
            let mut det2 = LogVectorFst::new();
            algorithms::Determinize(&t1, &mut det1);
            algorithms::Determinize(&t2, &mut det2);

            // LogFst *difference = new LogFst(); Difference(det1, det2, difference);
            let mut difference = LogVectorFst::new();
            algorithms::Difference(&det1, &det2, &mut difference);

            // DecodeFst<LogArc> subtract(*difference, encoder);
            algorithms::Decode(&mut difference, encoder);

            // result = new LogFst(subtract); result->SetInputSymbols(t1->InputSymbols());
            let mut result = difference;
            copy_input_symbol_table(&t1, &mut result);
            result
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.are-equivalent-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.are-equivalent-fn]
        pub fn are_equivalent(a: &LogVectorFst, b: &LogVectorFst) -> bool {
            let mut mina = LogWeightTransducer::minimize(a);
            let mut minb = LogWeightTransducer::minimize(b);
            // EncodeMapper<LogArc> encode_mapper(0x0001, ENCODE);
            // EncodeFst<LogArc> enca(*mina, &encode_mapper);
            // EncodeFst<LogArc> encb(*minb, &encode_mapper);
            let _encode_mapper =
                algorithms::Encode(&mut mina, algorithms::EncodeType::EncodeLabels);
            let _encode_mapper2 =
                algorithms::Encode(&mut minb, algorithms::EncodeType::EncodeLabels);
            // LogFst A(enca); LogFst B(encb); return Equivalent(A, B);
            algorithms::Equivalent(&mina, &minb)
        }

        // ----- TRIE FUNCTIONS BEGINS -----

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.has-arc-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.has-arc-fn]
        fn has_arc(t: &LogVectorFst, sourcestate: i32, ilabel: i32, olabel: i32) -> i32 {
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

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.disjunct-as-tries-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.disjunct-as-tries-fn]
        fn disjunct_as_tries(
            t1: &mut LogVectorFst,
            t1_state: StateId,
            t2: &LogVectorFst,
            t2_state: StateId,
        ) {
            if t2.is_final(t2_state).unwrap() {
                let t1_final = t1
                    .final_weight(t1_state)
                    .unwrap()
                    .unwrap_or_else(LogWeight::zero);
                let t2_final = t2.final_weight(t2_state).unwrap().unwrap();
                t1.set_final(t1_state, t1_final.plus(&t2_final).unwrap())
                    .unwrap();
            }
            let trs = t2.get_trs(t2_state).unwrap().trs().to_vec();
            for arc in &trs {
                let arc_index = LogWeightTransducer::has_arc(
                    t1,
                    t1_state as i32,
                    arc.ilabel as i32,
                    arc.olabel as i32,
                );
                if arc_index == -1 {
                    let new_state = t1.add_state();
                    t1.add_tr(
                        t1_state,
                        LogTransition::new(arc.ilabel, arc.olabel, arc.weight.clone(), new_state),
                    )
                    .unwrap();
                    LogWeightTransducer::add_sub_trie(t1, new_state, t2, arc.nextstate);
                } else {
                    // MutableArcIterator ajter(&t1, t1_state); ajter.Seek(arc_index);
                    let next = t1.get_trs(t1_state).unwrap().trs()[arc_index as usize].nextstate;
                    LogWeightTransducer::disjunct_as_tries(t1, next, t2, arc.nextstate);
                }
            }
        }

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-sub-trie-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-sub-trie-fn]
        fn add_sub_trie(
            t1: &mut LogVectorFst,
            t1_state: StateId,
            t2: &LogVectorFst,
            t2_state: StateId,
        ) {
            if t2.is_final(t2_state).unwrap() {
                let t1_final = t1
                    .final_weight(t1_state)
                    .unwrap()
                    .unwrap_or_else(LogWeight::zero);
                let t2_final = t2.final_weight(t2_state).unwrap().unwrap();
                t1.set_final(t1_state, t1_final.plus(&t2_final).unwrap())
                    .unwrap();
            }
            let trs = t2.get_trs(t2_state).unwrap().trs().to_vec();
            for arc in &trs {
                let new_state = t1.add_state();
                t1.add_tr(
                    t1_state,
                    LogTransition::new(arc.ilabel, arc.olabel, arc.weight.clone(), new_state),
                )
                .unwrap();
                LogWeightTransducer::add_sub_trie(t1, new_state, t2, arc.nextstate);
            }
        }

        // ----- TRIE FUNCTIONS END -----
    }
}

// ===== lookup_extract_misc (workflow body) =====
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
    use crate::hfst_flag_diacritics::FdState;

    // [spec:hfst:def:log-weight-transducer.hfst.implementations.label-pair]
    pub type LabelPair = (i32, i32);
    // [spec:hfst:def:log-weight-transducer.hfst.implementations.label-pair-vector]
    pub type LabelPairVector = Vec<LabelPair>;

    // ============================================================================
    // File-static free helpers (C++ 'static' functions in
    // 'namespace hfst::implementations').  Kept module-private, like the C++.
    // ============================================================================

    /* The recursive path-extraction worker.  Note the faithful C++ quirk that
    `all_visitations` / `path_visitations` are passed *by value* (each recursive
    call gets its own copy), while `spv` and `fd_state_stack` are shared
    (passed by reference / pointer). */
    // [spec:hfst:def:log-weight-transducer.hfst.implementations.extract-paths-fn]
    // [spec:hfst:sem:log-weight-transducer.hfst.implementations.extract-paths-fn]
    #[allow(clippy::too_many_arguments)]
    fn extract_paths(
        t: &LogVectorFst,
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
            let is_final = t.is_final(s).unwrap();
            let fw = if is_final {
                *t.final_weight(s).unwrap().unwrap().value()
            } else {
                0.0
            };
            let mut path = HfstTwoLevelPath {
                first: weight_sum + fw,
                second: spv.clone(),
            };
            let ret = callback.operator_call(&mut path, is_final);
            if !ret.continueSearch || !ret.continuePath {
                *path_visitations.entry(s).or_insert(0) -= 1;
                return ret.continueSearch;
            }
        }

        // sort arcs by number of visitations (stable insertion sort, ascending)
        let mut arcs: Vec<LogTransition> = Vec::new();
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

    // ============================================================================
    // 'impl LogWeightTransducer' — lookup-extract-misc bodies.
    // ============================================================================
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    impl LogWeightTransducer {
        // ---- extract_paths / extract_random_paths --------------------------------

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-paths-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-paths-fn]
        pub fn extract_paths(
            t: &LogVectorFst,
            callback: &mut dyn ExtractStringsCb,
            cycles: i32,
            fd: Option<&FdTable<i64>>,
            filter_fd: bool,
        ) {
            if t.start().is_none() {
                return;
            }

            let all_visitations: BTreeMap<StateId, u16> = BTreeMap::new();
            let path_visitations: BTreeMap<StateId, u16> = BTreeMap::new();
            let mut fd_state_stack: Option<Vec<FdState<i64>>> = fd.map(|fd| vec![FdState::new(fd)]);

            let start = t.start().unwrap();
            let mut spv = StringPairVector::new();
            // NOTE: faithful to the Log .cc, which (unlike Tropical) does NOT add a
            // trailing epsilon path and does NOT 'delete fd_state_stack' afterwards.
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
            // fd_state_stack dropped here (the C++ leaks it; harmless in Rust).
        }

        /// 'extract_random_paths(const LogFst*, HfstTwoLevelPaths&, int)'.  Unlike the
        /// Tropical backend (which implements this), the Log .cc throws
        /// 'FunctionNotImplementedException'.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-random-paths-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-random-paths-fn]
        pub fn extract_random_paths(
            t: &LogVectorFst,
            results: &mut HfstTwoLevelPaths,
            max_num: i32,
        ) {
            let _ = (t, results, max_num);
            unimplemented!("extract_random_paths: not implemented for this transducer type");
        }

        // ---- substitute ----------------------------------------------------------

        /// 'substitute(LogFst*, unsigned int, unsigned int)' — relabels label
        /// 'old_number' to 'new_number' on both the input and output side (C++ uses
        /// 'RelabelFst<LogArc>(*t, v, v)'; modelled here as a direct rebuild since
        /// rustfst's 'relabel_pairs' module is private).
        pub fn substitute_number(
            t: &LogVectorFst,
            old_number: u32,
            new_number: u32,
        ) -> LogVectorFst {
            let mut result = t.clone();
            let states: Vec<StateId> = result.states_iter().collect();
            for s in states {
                let trs = result.pop_trs(s).unwrap();
                let mut nt: Vec<LogTransition> = Vec::with_capacity(trs.len());
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

        /// 'substitute(LogFst*, NumberPair old, NumberPair new)'. The C++ encodes
        /// label pairs ('kEncodeLabels'), substitutes the single encoded label, then
        /// decodes; the net effect (replace arcs whose '(ilabel,olabel)' equals 'old'
        /// with 'new') is reproduced here directly.
        pub fn substitute_number_pair(
            t: &LogVectorFst,
            old_number_pair: NumberPair,
            new_number_pair: NumberPair,
        ) -> LogVectorFst {
            let mut result = t.clone();
            let states: Vec<StateId> = result.states_iter().collect();
            for s in states {
                let trs = result.pop_trs(s).unwrap();
                let mut nt: Vec<LogTransition> = Vec::with_capacity(trs.len());
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

        /// 'substitute(LogFst*, std::string old_symbol, std::string new_symbol)'.
        pub fn substitute_symbol(
            t: &LogVectorFst,
            old_symbol: String,
            new_symbol: String,
        ) -> LogVectorFst {
            // assert(t->InputSymbols() != NULL);
            let mut st = (**t.input_symbols().unwrap()).clone();
            let old_l = st.add_symbol(old_symbol.as_str());
            let new_l = st.add_symbol(new_symbol.as_str());
            let mut retval = Self::substitute_number(t, old_l, new_l);
            retval.set_input_symbols(Arc::new(st));
            retval
        }

        /// 'substitute(LogFst*, StringPair old, StringPair new)'.
        pub fn substitute_string_pair(
            t: &LogVectorFst,
            old_symbol_pair: StringPair,
            new_symbol_pair: StringPair,
        ) -> LogVectorFst {
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

        /// 'substitute(LogFst*, StringPair old, StringPairSet new)'.
        pub fn substitute_string_pair_set(
            t: &LogVectorFst,
            old_symbol_pair: StringPair,
            new_symbol_pair_set: StringPairSet,
        ) -> LogVectorFst {
            let mut tc = t.clone();
            let mut st = (**tc.input_symbols().unwrap()).clone();
            // assert(st != NULL);
            let states: Vec<StateId> = tc.states_iter().collect();
            for s in states {
                let trs = tc.pop_trs(s).unwrap();
                let mut nt: Vec<LogTransition> = Vec::new();
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
                                nt.push(LogTransition::new(
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

        /// 'substitute(LogFst*, const StringPair old, LogFst *transducer)'.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.substitute-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.substitute-fn]
        pub fn substitute_string_transducer(
            t: &LogVectorFst,
            old_symbol_pair: StringPair,
            transducer: &LogVectorFst,
        ) -> LogVectorFst {
            // assert(t->InputSymbols() != NULL);
            let mut result = t.clone();
            let mut st = (**result.input_symbols().unwrap()).clone();
            let old_il = st.add_symbol(old_symbol_pair.0.as_str());
            let old_ol = st.add_symbol(old_symbol_pair.1.as_str());

            let states = result.num_states() as u32;
            for i in 0..states {
                let trs = result.pop_trs(i).unwrap();
                let mut kept: Vec<LogTransition> = Vec::with_capacity(trs.len());
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
                                        LogTransition::new(0, 0, fw, destination_state),
                                    )
                                    .unwrap();
                            }

                            for tr_arc in transducer.get_trs(tr_state_id).unwrap().trs() {
                                result
                                    .add_tr(
                                        tr_state_id + start_state,
                                        LogTransition::new(
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

        /// 'substitute(LogFst*, const NumberPair old, LogFst *transducer)'.
        pub fn substitute_number_transducer(
            t: &LogVectorFst,
            old_number_pair: NumberPair,
            transducer: &LogVectorFst,
        ) -> LogVectorFst {
            let mut result = t.clone();

            let states = result.num_states() as u32;
            for i in 0..states {
                let trs = result.pop_trs(i).unwrap();
                let mut kept: Vec<LogTransition> = Vec::with_capacity(trs.len());
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
                                        LogTransition::new(0, 0, fw, destination_state),
                                    )
                                    .unwrap();
                            }

                            for tr_arc in transducer.get_trs(tr_state_id).unwrap().trs() {
                                result
                                    .add_tr(
                                        tr_state_id + start_state,
                                        LogTransition::new(
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

        /// 'insert_freely(LogFst*, const StringPair &symbol_pair)'.
        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.insert-freely-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.insert-freely-fn]
        pub fn insert_freely_string(t: &LogVectorFst, symbol_pair: &StringPair) -> LogVectorFst {
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
                        LogTransition::new(il, ol, LogWeight::new(0.0), state_id),
                    )
                    .unwrap();
            }
            result.set_input_symbols(Arc::new(st));
            result
        }

        // ---- predicates / counts -------------------------------------------------

        // [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.is-cyclic-fn]
        // [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.is-cyclic-fn]
        pub fn is_cyclic(t: &LogVectorFst) -> bool {
            // C++: return t->Properties(kCyclic, true) & kCyclic;
            let mut known = FstProperties::empty();
            let props = compute_fst_properties(t, FstProperties::CYCLIC, &mut known, true)
                .expect("rustfst compute_fst_properties");
            props.contains(FstProperties::CYCLIC)
        }
    }
}
