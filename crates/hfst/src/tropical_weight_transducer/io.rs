//! Binary stream I/O and the AT&T text format.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;

use super::*;

// ---------------------------------------------------------------------------
// Private module helpers (introduced for the port; not in the C++ header).
// ---------------------------------------------------------------------------

/// A weight written with 6 decimals matches the C++ '%f' formatting;
/// 'operator<<' (ostream) uses the default float formatting, which we
/// approximate with 'Display'.
fn fmt_w(w: f32, fixed_decimals: bool) -> String {
    if fixed_decimals {
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
    fixed_decimals: bool,
    initial_state: StateId,
    zero_print: StateId,
) {
    let origin = att_origin(s, initial_state, zero_print);
    for arc in t.get_trs(s).expect("s is a valid state of this fst").trs() {
        let target = att_origin(arc.nextstate, initial_state, zero_print);
        let w = *arc.weight.value();
        if number {
            let _ = writeln!(
                os,
                "{}\t{}\t\\{}\t\\{}\t{}",
                origin,
                target,
                arc.ilabel,
                arc.olabel,
                fmt_w(w, fixed_decimals)
            );
        } else {
            let st = t
                .input_symbols()
                .expect("input symbols present: asserted before symbolic write");
            let isym = st.get_symbol(arc.ilabel).unwrap_or("");
            let osym = st.get_symbol(arc.olabel).unwrap_or("");
            let _ = writeln!(
                os,
                "{}\t{}\t{}\t{}\t{}",
                origin,
                target,
                isym,
                osym,
                fmt_w(w, fixed_decimals)
            );
        }
    }
    if t.is_final(s).expect("s is a valid state of this fst") {
        let fw = *t
            .final_weight(s)
            .expect("s is a valid state of this fst")
            .expect("state is final so weight is present")
            .value();
        let _ = writeln!(os, "{}\t{}", origin, fmt_w(fw, fixed_decimals));
    }
}

fn write_in_att_format_core(
    t: &StdVectorFst,
    os: &mut dyn Write,
    number: bool,
    fixed_decimals: bool,
) {
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
            write_att_state(t, os, s, number, fixed_decimals, initial_state, zero_print);
            break;
        }
    }
    // pass 2: the rest
    for s in t.states_iter() {
        if s != initial_state {
            write_att_state(t, os, s, number, fixed_decimals, initial_state, zero_print);
        }
    }
}

/// C 'atof': parse a leading float, 0.0 on failure (Rust 'parse' is stricter —
/// trailing garbage is not tolerated, a faithfulness gap).
fn parse_att_weight(s: &str) -> f32 {
    s.trim().parse::<f32>().unwrap_or(0.0)
}

/// C 'atoi': parse a leading int, 0 on failure.
fn parse_att_int(s: &str) -> i32 {
    s.trim().parse::<i32>().unwrap_or(0)
}

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.print-att-number-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.print-att-number-fn]
#[allow(dead_code)]
pub fn print_att_number(t: &StdVectorFst, os: &mut dyn std::io::Write) {
    let _ = writeln!(
        os,
        "initial state: {}",
        t.start().map(|s| s as i64).unwrap_or(-1)
    );
    for s in t.states_iter() {
        if t.is_final(s).expect("s is a valid state of this fst") {
            let fw = *t
                .final_weight(s)
                .expect("s is a valid state of this fst")
                .expect("state is final so weight is present")
                .value();
            let _ = writeln!(os, "{}\t{:.6}", s, fw);
        }
        for arc in t.get_trs(s).expect("s is a valid state of this fst").trs() {
            let _ = writeln!(
                os,
                "{}\t{}\t{}\t{}\t{:.6}",
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

impl<'a> Default for TropicalWeightInputStream<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl<'a> TropicalWeightInputStream<'a> {
    /// 'TropicalWeightInputStream(void)' — reads from stdin.
    pub fn new() -> Self {
        // C++ reads from std::cin; own a stdin reader.
        TropicalWeightInputStream {
            filename: String::new(),
            input_stream: Box::new(BufReader::new(std::io::stdin())),
        }
    }

    /// 'TropicalWeightInputStream(const std::string &filename)'.
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.tropical-weight-input-stream-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.tropical-weight-input-stream-fn]
    pub fn new_filename(filename: &str) -> Self {
        // C++ opens an ifstream in binary mode; own the opened file. A failed
        // open yields an empty reader, so every read reports end of stream
        // exactly as the C++ not-good stream would.
        let reader: Box<dyn BufRead + 'a> = match std::fs::File::open(filename) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(_) => Box::new(std::io::empty()),
        };
        TropicalWeightInputStream {
            filename: filename.to_string(),
            input_stream: reader,
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
            // The owned reader is closed when this stream is dropped, so
            // there is nothing to do here.
        }
    }

    pub fn is_fst(&mut self) -> bool {
        // C++ 'is_fst()' routes to the static 'is_fst(input_stream)'.
        Self::is_fst_stream(&mut *self.input_stream)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.ignore-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.ignore-fn]
    pub fn ignore(&mut self, n: u32) {
        let mut sink = std::io::sink();
        let mut head = Read::take(&mut self.input_stream, n as u64);
        let _ = std::io::copy(&mut head, &mut sink);
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.read-transducer-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.read-transducer-fn]
    pub fn read_transducer(&mut self) -> crate::error::Result<StdVectorFst> {
        // C++ 'if (is_eof()) HFST_THROW(StreamIsClosedException)', where
        // 'is_eof' peeks for EOF; an empty fill is that same peek.
        if self
            .input_stream
            .fill_buf()
            .map(|b| b.is_empty())
            .unwrap_or(true)
        {
            crate::bail!(StreamIsClosed);
        }
        // rustfst has no streaming istream read, so read the remaining bytes
        // and 'load_prefix' one FST from the front (it reports how many bytes
        // it consumed); the unused remainder goes back in front of the reader
        // for the next read.
        let mut bytes: Vec<u8> = Vec::new();
        if self.input_stream.read_to_end(&mut bytes).is_err() {
            crate::bail!(
                TransducerHasWrongType,
                "could not read TROPICAL_OPENFST transducer payload"
            );
        }
        let (fst, consumed) = match StdVectorFst::load_prefix(&bytes) {
            Ok(x) => x,
            Err(_) => {
                crate::bail!(
                    TransducerHasWrongType,
                    "could not read TROPICAL_OPENFST transducer payload"
                )
            }
        };
        if consumed < bytes.len() {
            let rest = bytes.split_off(consumed);
            let drained = std::mem::replace(
                &mut self.input_stream,
                Box::new(std::io::empty()) as Box<dyn BufRead + 'a>,
            );
            self.input_stream = Box::new(BufReader::new(std::io::Cursor::new(rest).chain(drained)));
        }
        Ok(fst)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-fn]
    pub fn stream_get(&mut self) -> char {
        let mut b = [0u8; 1];
        let _ = Read::read_exact(&mut self.input_stream, &mut b);
        b[0] as char
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-short-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-short-fn]
    pub fn stream_get_short(&mut self) -> i16 {
        let mut b = [0u8; 2];
        let _ = Read::read_exact(&mut self.input_stream, &mut b);
        i16::from_ne_bytes(b)
    }

    /// 'static bool is_fst(...)' — peek the reader's first byte without consuming it.
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-fst-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-fst-fn]
    pub fn is_fst_stream(is: &mut dyn BufRead) -> bool {
        is.fill_buf().ok().and_then(|b| b.first().copied()) == Some(0xd6)
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
        let file =
            std::fs::File::create(filename).expect("TropicalWeightOutputStream: cannot open file");
        TropicalWeightOutputStream {
            filename: filename.to_string(),
            output_stream: Box::new(file),
            hfst_format,
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.close-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.close-fn]
    pub fn close(&mut self) {
        // Flush unconditionally: stdout (empty filename) is a buffered
        // std::io::stdout() that must be flushed too, since the tools exit via
        // std::process::exit, which does not run Drop.
        let _ = self.output_stream.flush();
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-fn]
    pub fn write(&mut self, c: char) {
        let _ = self.output_stream.write_all(&[c as u8]);
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-transducer-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-transducer-fn]
    pub fn write_transducer(&mut self, transducer: &StdVectorFst) {
        if TropicalWeightTransducer::write_transducer_to(
            transducer,
            &mut self.output_stream,
            self.hfst_format,
        )
        .is_err()
        {
            tracing::error!("TropicalWeightOutputStream: could not write transducer");
        }
        let _ = self.output_stream.flush();
    }
}

impl TropicalWeightTransducer {
    /// The payload-serialization body of
    /// 'TropicalWeightOutputStream::write_transducer' over a caller-supplied
    /// writer — the tropical arm of 'Backend::write'
    /// ([dec:hfst:monomorphic-backends]).
    pub fn write_transducer_to(
        transducer: &StdVectorFst,
        os: &mut dyn Write,
        hfst_format: bool,
    ) -> crate::error::Result<()> {
        if transducer.input_symbols().is_none() {
            tracing::warn!("### Missing Input Symbol Table when writing! ###");
        }
        // When not writing the HFST framing, OpenFST includes both input and
        // output symbol tables; the C++ sets the output table = input table on
        // the caller's transducer. The skeleton hands us '&StdVectorFst', so we
        // do it on a clone (NOTE: caller's transducer is not mutated, unlike C++).
        if !hfst_format {
            let mut t = transducer.clone();
            let st = transducer
                .input_symbols()
                .expect("input symbol table present when writing")
                .as_ref()
                .clone();
            t.set_output_symbols(Arc::new(st));
            if t.store(os).is_err() {
                crate::bail!(StreamCannotBeWritten, "could not write transducer payload");
            }
        } else if transducer.store(os).is_err() {
            crate::bail!(StreamCannotBeWritten, "could not write transducer payload");
        }
        Ok(())
    }
}

impl TropicalWeightTransducer {
    // ---- AT&T write ----

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
    pub fn read_in_att_format(
        ifile: &mut dyn std::io::BufRead,
    ) -> crate::error::Result<StdVectorFst> {
        let mut t = StdVectorFst::new();
        let mut st = Self::create_symbol_table(String::new());

        let mut state_map: StateMap = StateMap::new();

        // Add initial state that is numbered as zero.
        let initial_state = Self::add_and_map_state(&mut t, 0, &mut state_map);
        t.set_start(initial_state)
            .expect("state just created by add_and_map_state");

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
                weight = parse_att_weight(toks[1]);
            }
            if n == 5 {
                weight = parse_att_weight(toks[4]);
            }

            if n == 1 || n == 2 {
                // final state line
                let final_number = parse_att_int(toks[0]);
                let final_state = Self::add_and_map_state(&mut t, final_number, &mut state_map);
                t.set_final(final_state, weight)
                    .expect("state just created by add_and_map_state");
            } else if n == 4 || n == 5 {
                // transition line
                let origin_number = parse_att_int(toks[0]);
                let target_number = parse_att_int(toks[1]);
                let origin_state = Self::add_and_map_state(&mut t, origin_number, &mut state_map);
                let target_state = Self::add_and_map_state(&mut t, target_number, &mut state_map);

                let input_number = st.add_symbol(toks[2]);
                let output_number = st.add_symbol(toks[3]);

                t.add_tr(
                    origin_state,
                    StdTransition::new(input_number, output_number, weight, target_state),
                )
                .expect("transition added between states mapped by add_and_map_state");
            } else {
                // line could not be parsed
                let message = line_str.to_string();
                crate::bail!(NotValidAttFormat, message);
            }
        }

        t.set_input_symbols(Arc::new(st));
        Ok(t)
    }
}
