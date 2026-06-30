//! Port of 'libhfst/src/implementations/HfstOlTransducer.{h,cc}' — the
//! optimized-lookup backend bridge between the HFST API and
//! 'hfst_ol::Transducer' (= ['crate::transducer::Transducer']).
//!
//! In C++ 'HfstOlTransducer' is a class of STATIC methods — a stateless
//! operations wrapper over 'hfst_ol::Transducer'. It is modelled here as a
//! unit struct ['HfstOlTransducer'] with an 'impl' block of associated
//! functions, exactly analogous to
//! ['crate::tropical_weight_transducer::TropicalWeightTransducer'] over
//! 'StdVectorFst'. The two stream helper classes
//! (['HfstOlInputStream'] / ['HfstOlOutputStream']) become their own structs.
//!
//! Ownership mapping for the C++ 'hfst_ol::Transducer*' signatures:
//! - 'create_empty_transducer(bool)' — the C++ 'new's and returns a
//!   'Transducer*' -> returns an owned ['Transducer'].
//! - methods that take a 'Transducer*' and read it ('is_cyclic',
//!   'extract_paths', 'get_flag_diacritics', 'get_alphabet') -> '&Transducer'.
//! - 'HfstOlInputStream::read_transducer' 'new's a 'Transducer' -> owned.
//! - 'HfstOlOutputStream::write_transducer(Transducer*)' reads it -> '&Transducer'.
//!
//! Stream modelling (per the porting convention): the C++ 'HfstOlInputStream'
//! holds an 'std::ifstream i_stream' plus an 'std::istream &input_stream'
//! reference that aliases either 'i_stream' or 'std::cin'; this is modelled as
//! a single binary input stream 'crate::transducer::IStream', which borrows its
//! reader, hence the ''a' lifetime (mirroring 'TropicalWeightInputStream<'a>').
//! 'HfstOlOutputStream' holds 'std::ofstream o_stream' + 'std::ostream
//! &output_stream' aliasing it or 'std::cout'; modelled as a single owned
//! writer. The C 'FILE*' overload of 'is_fst' is ported over '&mut dyn
//! std::io::BufRead' so no raw C file handle is needed.

#![allow(non_snake_case)]
#![allow(dead_code)] // many ported ops are only reached once the facade lands

use std::collections::BTreeSet;

use crate::hfst_extract_strings::ExtractStringsCb;
use crate::transducer::IStream;

/// 'typedef std::set<std::string> StringSet' (used by 'get_alphabet').
pub type StringSet = BTreeSet<String>;

// [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream]
pub struct HfstOlInputStream<'a> {
    filename: String,
    /// C++ holds an 'std::ifstream i_stream' plus an 'std::istream
    /// &input_stream' reference that aliases either 'i_stream' or 'std::cin'.
    /// Modelled here as a single binary input stream (per the porting
    /// convention, 'std::istream' (binary) -> 'crate::transducer::IStream').
    input_stream: IStream<'a>,
    weighted: bool,
}

// (no Default: HfstOlInputStream borrows its reader and cannot be constructed
// without one; the no-source ctor 'HfstOlInputStream(bool)' is deferred.)

// [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream]
pub struct HfstOlOutputStream {
    filename: String,
    /// C++ holds 'std::ofstream o_stream' + 'std::ostream &output_stream' that
    /// aliases either it or 'std::cout'. Modelled as a single owned writer.
    output_stream: Box<dyn std::io::Write>,
    weighted: bool,
}

// [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer]
pub struct HfstOlTransducer;

// ===== ol_construction_io (workflow body) =====
mod ol_construction_io {
    #![allow(unused_imports)]
    use super::*;
    use crate::hfst_exception_defs::StreamIsClosedException;
    use crate::hfst_flag_diacritics::FdTable;
    use crate::hfst_symbol_defs::StringSet;
    use crate::transducer::{HeaderFlag, IStream, SymbolNumber, SymbolTable, Transducer};

    // ===========================================================================
    // HfstOlInputStream
    // ===========================================================================
    #[allow(dead_code)]
    impl<'a> HfstOlInputStream<'a> {
        /// 'HfstOlInputStream(bool weighted)' — reads from 'std::cin'.
        pub fn new(weighted: bool) -> Self {
            // C++ reads from std::cin; own a stdin reader.
            HfstOlInputStream {
                filename: String::new(),
                input_stream: IStream::new_owned(std::io::stdin()),
                weighted,
            }
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.hfst-ol-input-stream-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.hfst-ol-input-stream-fn]
        pub fn new_filename(filename: &str, weighted: bool) -> Self {
            // C++ opens an ifstream in binary mode; own the opened file (a failed
            // open leaves the stream in the not-good state the C++ would have).
            let reader: Box<dyn std::io::Read> = match std::fs::File::open(filename) {
                Ok(f) => Box::new(f),
                Err(_) => Box::new(std::io::empty()),
            };
            HfstOlInputStream {
                filename: filename.to_string(),
                input_stream: IStream::new_owned(reader),
                weighted,
            }
        }

        /// 'HfstOlInputStream(std::istream &is, bool weighted)'.
        pub fn new_istream(is: IStream<'a>, weighted: bool) -> Self {
            HfstOlInputStream {
                filename: String::new(),
                input_stream: is,
                weighted,
            }
        }

        /* Skip the identifier string "HFST_OL_TYPE" or "HFST_OLW_TYPE" */
        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.skip-identifier-version-3-0-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.skip-identifier-version-3-0-fn]
        fn skip_identifier_version_3_0(&mut self) {
            self.ignore(if self.weighted { 14 } else { 13 });
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.skip-hfst-header-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.skip-hfst-header-fn]
        fn skip_hfst_header(&mut self) {
            self.ignore(6);
            self.skip_identifier_version_3_0();
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.open-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.open-fn]
        pub fn open(&mut self) {}

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.close-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.close-fn]
        pub fn close(&mut self) {
            if !self.filename.is_empty() {
                // C++ 'i_stream.close()': the IStream borrows its reader (owned by the
                // caller); there is nothing to close on our side.
            }
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-open-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-open-fn]
        pub fn is_open(&self) -> bool {
            if !self.filename.is_empty() {
                // C++ 'i_stream.is_open()': the IStream owns a valid reader once
                // constructed; modelled as always open.
                true
            } else {
                true
            }
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-eof-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-eof-fn]
        pub fn is_eof(&self) -> bool {
            // C++ tests 'input_stream.peek() == EOF'; 'IStream' has no peek, so we
            // approximate with the good/fail flag (set once a read hits EOF).
            !self.input_stream.good()
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-bad-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-bad-fn]
        pub fn is_bad(&self) -> bool {
            if self.filename.is_empty() {
                // std::cin.bad(): IStream has no badbit; approximated with !good().
                !self.input_stream.good()
            } else {
                // input_stream.bad(): same approximation.
                !self.input_stream.good()
            }
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-good-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-good-fn]
        pub fn is_good(&self) -> bool {
            if self.is_eof() {
                return false;
            }
            if self.filename.is_empty() {
                // std::cin.good()
                self.input_stream.good()
            } else {
                self.input_stream.good()
            }
        }

        /// 'bool is_fst(void) const;' — routes to the static 'is_fst(istream&)'.
        pub fn is_fst_self(&mut self) -> bool {
            // C++ 'is_fst(void) const' -> 'is_fst(input_stream) != 0'.
            Self::is_fst_istream(&mut self.input_stream) != 0
        }

        /// 'static int is_fst(FILE * f);' — 1=unweighted, 2=weighted.
        ///
        /// Ported from the C 'FILE*' overload to '&mut dyn BufRead'. The C
        /// 'ungetc'-everything-back behaviour is reproduced by *peeking* (no bytes
        /// are consumed from 'f').
        pub fn is_fst_file(f: &mut dyn std::io::BufRead) -> i32 {
            let mut buffer = [0u8; 24];
            // NOTE (preserved C++ bug): 'fread(buffer, 24, 1, f)' reads 1 element of
            // size 24, so 'num_read' is the element count (0 or 1), never 24. The
            // 'num_read != 24' test below is therefore always true, so this function
            // always returns 0 (and reads 'buffer+20' whether or not it was filled).
            // We peek without consuming so nothing is removed from the stream.
            let num_read: usize = match f.fill_buf() {
                Ok(buf) => {
                    let n = std::cmp::min(buf.len(), 24);
                    buffer[..n].copy_from_slice(&buf[..n]);
                    // element count of size-24 reads: 1 if a full 24 bytes are
                    // available, else 0 — matching 'fread(_, 24, 1, _)'.
                    if n == 24 { 1 } else { 0 }
                }
                Err(_) => 0,
            };
            let weighted: u32 =
                i32::from_ne_bytes([buffer[20], buffer[21], buffer[22], buffer[23]]) as u32;
            let res: i32 = if num_read != 24 {
                0
            } else if weighted == 0 {
                1
            } else if weighted == 1 {
                2
            } else {
                0
            };

            res
        }

        /// 'static int is_fst(std::istream &s);'
        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-fst-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-fst-fn]
        pub fn is_fst_istream(s: &mut IStream) -> i32 {
            // C++ reads 24 bytes, inspects buffer[20..24] for the weighted flag,
            // then puts every read byte back. gcount = bytes actually read.
            if !s.good() {
                return 0;
            }
            let mut buffer = [0u8; 24];
            let mut num_read = 0usize;
            while num_read < 24 {
                let c = s.get();
                if c < 0 {
                    break;
                }
                buffer[num_read] = c as u8;
                num_read += 1;
            }
            let weighted: u32 =
                i32::from_ne_bytes([buffer[20], buffer[21], buffer[22], buffer[23]]) as u32;
            let res: i32 = if num_read != 24 {
                0
            } else if weighted == 0 {
                1
            } else if weighted == 1 {
                2
            } else {
                0
            };
            if num_read > 0 {
                let mut i = num_read as isize - 1;
                while i >= 0 {
                    s.putback(buffer[i as usize]);
                    i -= 1;
                }
            }
            if num_read != 24 {
                s.clear();
            }
            res
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-get-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-get-fn]
        pub fn stream_get(&mut self) -> char {
            let mut b = [0u8; 1];
            self.input_stream.read(&mut b);
            b[0] as char
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-get-short-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-get-short-fn]
        pub fn stream_get_short(&mut self) -> i16 {
            let mut b = [0u8; 2];
            self.input_stream.read(&mut b);
            i16::from_ne_bytes(b)
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-unget-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-unget-fn]
        pub fn stream_unget(&mut self, c: char) {
            self.input_stream.putback(c as u8);
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.ignore-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.ignore-fn]
        pub fn ignore(&mut self, n: u32) {
            let mut buf = vec![0u8; n as usize];
            self.input_stream.read(&mut buf);
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.operator-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.operator-fn]
        pub fn operator_call(&self) -> bool {
            self.is_good()
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.read-transducer-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.read-transducer-fn]
        pub fn read_transducer(&mut self, has_header: bool) -> Transducer {
            if self.is_eof() {
                crate::HFST_THROW!(StreamIsClosedException);
            }
            // C++ wraps the body in 'try { ... } catch (const HfstException e) { throw e; }',
            // i.e. it merely rethrows; Rust panics propagate, so no wrapper is needed.
            if has_header {
                self.skip_hfst_header();
            }
            // 'new hfst_ol::Transducer(input_stream)' -> owned Transducer.
            Transducer::new_istream(&mut self.input_stream)
        }
    }

    // ===========================================================================
    // HfstOlOutputStream
    // ===========================================================================
    #[allow(dead_code)]
    impl HfstOlOutputStream {
        /// 'HfstOlOutputStream(bool weighted)' — writes to 'std::cout'.
        pub fn new(weighted: bool) -> Self {
            HfstOlOutputStream {
                filename: String::new(),
                output_stream: Box::new(std::io::stdout()),
                weighted,
            }
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.hfst-ol-output-stream-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.hfst-ol-output-stream-fn]
        pub fn new_filename(filename: &str, weighted: bool) -> Self {
            // C++ opens the file (out | binary); on failure it warns ('!output_stream')
            // and continues with a failed stream — modelled by falling back to a sink.
            let output_stream: Box<dyn std::io::Write> = match std::fs::File::create(filename) {
                Ok(f) => Box::new(f),
                Err(_) => {
                    tracing::error!("HfstOlOutputStream: failbit set (3).");
                    Box::new(std::io::sink())
                }
            };
            HfstOlOutputStream {
                filename: filename.to_string(),
                output_stream,
                weighted,
            }
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.write-transducer-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.write-transducer-fn]
        pub fn write_transducer(&mut self, transducer: &Transducer) {
            // C++ tests 'if (!output_stream)' (failbit) and warns; the boxed writer has
            // no failbit, so the check is elided.
            transducer.write(&mut *self.output_stream);
            let _ = self.output_stream.flush();
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.open-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.open-fn]
        pub fn open(&mut self) {}

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.close-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.close-fn]
        pub fn close(&mut self) {
            // Flush unconditionally: stdout is a buffered std::io::stdout() and the
            // tools exit via std::process::exit (no Drop), so it must be flushed too.
            let _ = self.output_stream.flush();
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.write-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.write-fn]
        pub fn write(&mut self, c: char) {
            let _ = self.output_stream.write_all(&[c as u8]);
        }
    }

    // ===========================================================================
    // HfstOlTransducer — construction / alphabet accessors / is_* predicate
    // ===========================================================================
    #[allow(dead_code)]
    impl HfstOlTransducer {
        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.create-empty-transducer-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.create-empty-transducer-fn]
        pub fn create_empty_transducer(weighted: bool) -> Transducer {
            Transducer::new_weighted(weighted)
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.is-cyclic-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.is-cyclic-fn]
        pub fn is_cyclic(t: &Transducer) -> bool {
            t.get_header().probe_flag(HeaderFlag::Cyclic)
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.get-flag-diacritics-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.get-flag-diacritics-fn]
        pub fn get_flag_diacritics(t: &Transducer) -> &FdTable<SymbolNumber> {
            t.get_alphabet().get_fd_table()
        }

        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.get-alphabet-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.get-alphabet-fn]
        pub fn get_alphabet(t: &Transducer) -> StringSet {
            let symbol_table: SymbolTable = t.get_alphabet().get_symbol_table().clone();
            symbol_table.iter().cloned().collect()
        }
    }
}

// ===== ol_lookup_ops (workflow body) =====
mod ol_lookup_ops {
    #![allow(unused_imports)]
    use super::*;
    use std::collections::BTreeMap;

    use crate::hfst_data_types::{HfstTwoLevelPath, StringPairVector};
    use crate::hfst_extract_strings::ExtractStringsCb;
    use crate::hfst_flag_diacritics::{FdState, FdTable};
    use crate::transducer::{
        HeaderFlag, SymbolNumber, Transducer, TransitionTableIndex, indexes_transition_index_table,
    };

    /* The recursive path-extraction worker (C++ file-'static' free function in
    `namespace hfst::implementations`). Kept module-private, like the C++. Note the
    faithful quirk that `all_visitations` / `path_visitations` are passed *by value*
    (each recursive call gets its own copy), while `spv` and `fd_state_stack` are
    shared (passed by reference / pointer). */
    // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.extract-paths-fn]
    // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.extract-paths-fn]
    #[allow(clippy::too_many_arguments)]
    fn extract_paths(
        t: &Transducer,
        s: TransitionTableIndex,
        mut all_visitations: BTreeMap<TransitionTableIndex, u16>,
        mut path_visitations: BTreeMap<TransitionTableIndex, u16>,
        /*std::vector<char>& lbuffer, int lpos,
        std::vector<char>& ubuffer, int upos,*/
        weight_sum: f32,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        mut fd_state_stack: Option<&mut Vec<FdState<SymbolNumber>>>,
        filter_fd: bool,
        spv: &mut StringPairVector,
    ) -> bool {
        if cycles >= 0 && (*path_visitations.entry(s).or_insert(0) as i32) > cycles {
            return true;
        }
        *all_visitations.entry(s).or_insert(0) += 1;
        *path_visitations.entry(s).or_insert(0) += 1;

        if !spv.is_empty() {
            // check for finality
            let mut final_ = false;
            let mut final_weight = 0.0f32;
            if indexes_transition_index_table(s) {
                if t.get_index(s).final_() {
                    final_ = true;
                    final_weight = if t.get_header().probe_flag(HeaderFlag::Weighted) {
                        t.get_index(s).final_weight()
                    } else {
                        0.0f32
                    };
                }
            } else if t.get_transition(s).final_() {
                final_ = true;
                final_weight = if t.get_header().probe_flag(HeaderFlag::Weighted) {
                    t.get_transition(s).get_weight()
                } else {
                    0.0f32
                };
            }

            let mut path = HfstTwoLevelPath {
                first: weight_sum + final_weight,
                second: spv.clone(),
            };

            let ret = callback.operator_call(&mut path, final_);
            if !ret.continueSearch || !ret.continuePath {
                *path_visitations.entry(s).or_insert(0) -= 1;
                return ret.continueSearch;
            }
        }

        // sort arcs by number of visitations
        let transitions = t.get_transitions_from_state(s);
        let mut sorted_transitions: Vec<TransitionTableIndex> = Vec::new();
        for it in transitions.iter() {
            let target = t.get_transition(*it).get_target();
            let mut i: usize = 0;
            while i < sorted_transitions.len() {
                let av_t = *all_visitations.get(&target).unwrap_or(&0);
                let av_i = *all_visitations
                    .get(&t.get_transition(sorted_transitions[i]).get_target())
                    .unwrap_or(&0);
                if av_t < av_i {
                    break;
                }
                i += 1;
            }
            sorted_transitions.insert(i, *it);
        }

        let mut res = true;
        let mut i: usize = 0;
        while i < sorted_transitions.len() && res {
            let input = t.get_transition(sorted_transitions[i]).get_input_symbol();
            let output = t.get_transition(sorted_transitions[i]).get_output_symbol();
            let target = t.get_transition(sorted_transitions[i]).get_target();

            let mut added_fd_state = false;

            if let Some(stack) = fd_state_stack.as_deref_mut() {
                if stack
                    .last()
                    .unwrap()
                    .get_table()
                    .get_operation(input)
                    .is_some()
                {
                    let top = stack.last().unwrap().clone();
                    stack.push(top);
                    if stack.last_mut().unwrap().apply_operation_symbol(input) {
                        added_fd_state = true;
                    } else {
                        stack.pop();
                        i += 1;
                        continue; // don't follow the transition
                    }
                }
            }

            /* Handle spv here. Special symbols (flags, epsilons)
            are always inserted. */
            let mut istring = String::new();
            let mut ostring = String::new();

            debug_assert!(fd_state_stack.is_some() || !filter_fd);
            if !filter_fd
                || fd_state_stack
                    .as_deref()
                    .unwrap()
                    .last()
                    .unwrap()
                    .get_table()
                    .get_operation(input)
                    .is_none()
            {
                istring = t.get_alphabet().get_symbol_table()[input as usize].clone();
            }

            if !filter_fd
                || fd_state_stack
                    .as_deref()
                    .unwrap()
                    .last()
                    .unwrap()
                    .get_table()
                    .get_operation(output)
                    .is_none()
            {
                ostring = t.get_alphabet().get_symbol_table()[output as usize].clone();
            }

            spv.push((istring, ostring));

            res = extract_paths(
                t,
                target,
                all_visitations.clone(),
                path_visitations.clone(),
                /*lbuffer,lp, ubuffer,up,*/
                weight_sum
                    + if t.get_header().probe_flag(HeaderFlag::Weighted) {
                        t.get_transition(sorted_transitions[i]).get_weight()
                    } else {
                        0.0f32
                    },
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
            i += 1;
        }

        *path_visitations.entry(s).or_insert(0) -= 1;
        res
    }

    #[allow(dead_code)]
    const BUFFER_START_SIZE: i32 = 64;

    impl HfstOlTransducer {
        // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.extract-paths-fn]
        // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.extract-paths-fn]
        pub fn extract_paths(
            t: &Transducer,
            callback: &mut dyn ExtractStringsCb,
            cycles: i32,
            fd: *const FdTable<SymbolNumber>,
            filter_fd: bool,
        ) {
            //std::vector<char> lbuffer(BUFFER_START_SIZE, 0);
            //std::vector<char> ubuffer(BUFFER_START_SIZE, 0);
            let all_visitations: BTreeMap<TransitionTableIndex, u16> = BTreeMap::new();
            let path_visitations: BTreeMap<TransitionTableIndex, u16> = BTreeMap::new();
            let mut fd_state_stack: Option<Vec<FdState<SymbolNumber>>> = if fd.is_null() {
                None
            } else {
                Some(vec![FdState::new(unsafe { &*fd })])
            };

            let mut spv = StringPairVector::new();

            extract_paths(
                t,
                0,
                all_visitations,
                path_visitations,
                /*lbuffer,0,ubuffer,0,*/
                0.0f32,
                callback,
                cycles,
                fd_state_stack.as_mut(),
                filter_fd,
                &mut spv,
            );
        }
    }
}
