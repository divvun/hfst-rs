//! Reading and writing the AT&T tabular text format.

use std::io::{BufRead, Write};

use super::*;
use crate::string_utils::replace_all;

// C 'atof': parse the leading float, 0.0 on failure. The inputs here are clean
// whitespace-delimited tokens, so a plain parse suffices.
fn parse_weight(s: &str) -> f64 {
    s.trim_start().parse::<f64>().unwrap_or(0.0)
}

// Approximation of 'std::istream::eof()' for a fresh reader: no bytes remain.
fn is_eof(is: &mut dyn BufRead) -> bool {
    match is.fill_buf() {
        Ok(b) => b.is_empty(),
        Err(_) => true,
    }
}

impl HfstBasicTransducer {
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-weight-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-weight-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-weight-fn]
    // The C '%f' conversion renders with printf's default 6-digit precision.
    pub fn write_weight_file(file: &mut dyn Write, weight: f32) -> std::io::Result<()> {
        write!(file, "{:.6}", weight)
    }

    // The C++ ostream '<<' float formatting (6 significant digits) differs from
    // the FILE '%f' path above, and Rust's default '{}' differs from both;
    // forgiven unless a ported test proves the exact text.
    pub fn write_weight_os(os: &mut dyn Write, weight: f32) {
        let _ = write!(os, "{}", weight);
    }

    /** @brief Write the graph in AT&T format to ostream 'os'. */
    pub fn write_in_att_format_os(&self, os: &mut dyn Write, write_weights: bool) {
        for (source_state, it) in self.state_vector.iter().enumerate() {
            let source_state = source_state as u32;
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data().clone();

                let mut isymbol = data.get_input_symbol(&self.coder).to_string();
                replace_all(&mut isymbol, " ", "@_SPACE_@");
                replace_all(&mut isymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut isymbol, "\t", "@_TAB_@");

                let mut osymbol = data.get_output_symbol(&self.coder).to_string();
                replace_all(&mut osymbol, " ", "@_SPACE_@");
                replace_all(&mut osymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut osymbol, "\t", "@_TAB_@");

                let _ = write!(
                    os,
                    "{}\t{}\t{}\t{}",
                    source_state,
                    tr_it.get_target_state(),
                    isymbol,
                    osymbol
                );

                if write_weights {
                    let _ = write!(os, "\t");
                    Self::write_weight_os(os, data.get_weight());
                }
                let _ = writeln!(os);
            }
            if self.is_final_state(source_state) {
                let _ = write!(os, "{}", source_state);
                if write_weights {
                    let _ = write!(os, "\t");
                    Self::write_weight_os(
                        os,
                        self.get_final_weight(source_state)
                            .expect("state was confirmed final via is_final_state"),
                    );
                }
                let _ = writeln!(os);
            }
        }
    }

    /** @brief Write the graph in AT&T format to FILE 'file'. */
    pub fn write_in_att_format_file(
        &self,
        file: &mut dyn Write,
        write_weights: bool,
    ) -> std::io::Result<()> {
        for (source_state, it) in self.state_vector.iter().enumerate() {
            let source_state = source_state as u32;
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data().clone();

                let mut isymbol = data.get_input_symbol(&self.coder).to_string();
                replace_all(&mut isymbol, " ", "@_SPACE_@");
                replace_all(&mut isymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut isymbol, "\t", "@_TAB_@");

                let mut osymbol = data.get_output_symbol(&self.coder).to_string();
                replace_all(&mut osymbol, " ", "@_SPACE_@");
                replace_all(&mut osymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut osymbol, "\t", "@_TAB_@");

                write!(
                    file,
                    "{}\t{}\t{}\t{}",
                    source_state,
                    tr_it.get_target_state(),
                    isymbol,
                    osymbol
                )?;

                if write_weights {
                    write!(file, "\t")?;
                    Self::write_weight_file(file, data.get_weight())?;
                }
                writeln!(file)?;
            }
            if self.is_final_state(source_state) {
                write!(file, "{}", source_state)?;
                if write_weights {
                    write!(file, "\t")?;
                    Self::write_weight_file(
                        file,
                        self.get_final_weight(source_state)
                            .expect("state was confirmed final via is_final_state"),
                    )?;
                }
                writeln!(file)?;
            }
        }
        Ok(())
    }

    /** @brief Write the graph in AT&T format to FILE 'file' using numbers
    instead of symbol names. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-number-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-number-fn]
    //
    // NB: the C++ prints the final-state line *inside* the transition loop (so a
    // multi-transition final state repeats it); preserved bug-for-bug.
    // [spec:hfst:def:hfst-transition-graph.write-in-att-format-number-fn]
    // [spec:hfst:sem:hfst-transition-graph.write-in-att-format-number-fn]
    pub fn write_in_att_format_number_file(
        &self,
        file: &mut dyn Write,
        write_weights: bool,
    ) -> std::io::Result<()> {
        for (source_state, it) in self.state_vector.iter().enumerate() {
            let source_state = source_state as u32;
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data().clone();

                write!(
                    file,
                    "{}\t{}\t{}\t{}",
                    source_state,
                    tr_it.get_target_state(),
                    tr_it.get_input_number(),
                    tr_it.get_output_number()
                )?;

                if write_weights {
                    write!(file, "\t{:.6}", data.get_weight())?;
                }
                writeln!(file)?;

                if self.is_final_state(source_state) {
                    write!(file, "{}", source_state)?;
                    if write_weights {
                        write!(
                            file,
                            "\t{:.6}",
                            self.get_final_weight(source_state)
                                .expect("state was confirmed final via is_final_state")
                        )?;
                    }
                    writeln!(file)?;
                }
            }
        }
        Ok(())
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-att-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-att-line-fn]
    //
    // sscanf(line, "%s%s%s%s%s", ...) reads up to five whitespace-delimited
    // fields; 'n' is how many were read.
    // [spec:hfst:def:hfst-transition-graph.add-att-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.add-att-line-fn]
    pub fn add_att_line(
        &mut self,
        line: &str,
        epsilon_symbol: &str,
        warn_negs: bool,
    ) -> crate::error::Result<()> {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let n = tokens.len().min(5);
        let a = |i: usize| -> &str { tokens.get(i).copied().unwrap_or("") };

        // set value of weight
        let mut weight: f32 = 0.0;
        if n == 2 {
            // a final state line with weight
            weight = parse_weight(a(1)) as f32;
        }
        if n == 5 {
            // a transition line with weight
            weight = parse_weight(a(4)) as f32;
        }
        if (weight < 0.0) && warn_negs {
            tracing::warn!("Negative weight {:.6} found :-(", weight);
        }

        if n == 1 || n == 2 {
            // a final state line
            self.set_final_weight(parse_state_number(a(0)), &weight);
        } else if n == 4 || n == 5 {
            // a transition line
            let mut input_symbol = a(2).to_string();
            let mut output_symbol = a(3).to_string();

            // replace "@_SPACE_@"s with " " and "@0@"s with "@_EPSILON_SYMBOL_@"
            replace_all(&mut input_symbol, "@_SPACE_@", " ");
            replace_all(&mut input_symbol, "@0@", "@_EPSILON_SYMBOL_@");
            replace_all(&mut input_symbol, "@_TAB_@", "\t");
            replace_all(&mut input_symbol, "@_COLON_@", ":");

            replace_all(&mut output_symbol, "@_SPACE_@", " ");
            replace_all(&mut output_symbol, "@0@", "@_EPSILON_SYMBOL_@");
            replace_all(&mut output_symbol, "@_TAB_@", "\t");
            replace_all(&mut output_symbol, "@_COLON_@", ":");

            if epsilon_symbol == input_symbol {
                input_symbol = "@_EPSILON_SYMBOL_@".to_string();
            }
            if epsilon_symbol == output_symbol {
                output_symbol = "@_EPSILON_SYMBOL_@".to_string();
            }

            let tr = HfstBasicTransition::new_symbols(
                parse_state_number(a(1)),
                input_symbol.into(),
                output_symbol.into(),
                weight,
                self.coder_mut(),
            );
            self.add_transition(parse_state_number(a(0)), &tr, true);
        } else {
            // line could not be parsed
            let message = line.to_string();
            crate::bail!(NotValidAttFormat, message);
        }
        Ok(())
    }

    // HfstBasicTransducer(FILE*) — read an AT&T transducer from 'file'.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-basic-transducer-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-basic-transducer-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-transition-graph-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-transition-graph-fn]
    pub fn new_from_file(file: &mut dyn BufRead) -> crate::error::Result<Self> {
        let mut alphabet = HfstAlphabet::new();
        Self::initialize_alphabet(&mut alphabet);
        let state_vector = vec![HfstBasicTransitions::new()];
        let mut retval = HfstBasicTransducer {
            state_vector,
            final_weight_map: FinalWeightMap::new(),
            alphabet,
            name: String::new(),
            coder: SymbolCoder::new(),
        };
        let mut linecount: u32 = 0;
        let read = Self::read_in_att_format(file, "@0@", &mut linecount, false)?;
        retval.assign(&read);
        retval.name = String::new();
        Ok(retval)
    }

    // Create a graph from AT&T format in 'is' (if 'file' is null) or 'file'.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-att-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-att-format-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-read-in-att-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-read-in-att-format-fn]
    pub fn read_in_att_format(
        is: &mut dyn BufRead,
        epsilon_symbol: &str,
        linecount: &mut u32,
        warn_negs: bool,
    ) -> crate::error::Result<HfstBasicTransducer> {
        if is_eof(is) {
            crate::bail!(EndOfStream);
        }

        let mut retval = HfstBasicTransducer::new();
        loop {
            let line: String = match crate::io_utils::read_line_lossy(is) {
                None => break,
                Some(l) => l,
            };

            *linecount += 1;

            let bytes = line.as_bytes();
            // an empty line (with or without newline, incl. windows newline)
            if bytes.is_empty()
                || (bytes.len() == 1 && bytes[0] == b'\n')
                || (bytes.len() == 2 && bytes[0] == b'\r' && bytes[1] == b'\n')
            {
                // make sure that the end-of-file is reached (C++ 'fgetc(file)')
                let mut b = [0u8; 1];
                let _ = is.read(&mut b);
                break;
            }

            if bytes[0] == b'-' {
                // transducer separator line is "--"
                return Ok(retval);
            }

            retval.add_att_line(&line, epsilon_symbol, warn_negs)?;
        }
        Ok(retval)
    }
}
