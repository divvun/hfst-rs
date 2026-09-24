//! Writing the xfst text listing of a graph.

use std::io::Write;

use super::*;

impl HfstBasicTransducer {
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-fn]
    //
    // Iterates bytes (C++ 'for (char pos : symbol)' over a byte string); the
    // escaped chars are ASCII and never appear as UTF-8 continuation bytes, so
    // multibyte symbols are reconstructed byte-for-byte.
    pub fn xfstize(symbol: &mut String) {
        let mut escaped_symbol: Vec<u8> = Vec::new();
        for pos in symbol.bytes() {
            if pos == b'%' {
                escaped_symbol.extend_from_slice(b"\"%\"");
            } else if pos == b'"' {
                escaped_symbol.extend_from_slice(b"%\"");
            } else if pos == b'?' {
                escaped_symbol.extend_from_slice(b"\"?\"");
            } else {
                escaped_symbol.push(pos);
            }
        }
        *symbol = String::from_utf8(escaped_symbol)
            .expect("escaped bytes are valid UTF-8 by construction");
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-symbol-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-symbol-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-symbol-fn]
    pub fn xfstize_symbol(symbol: &mut String) {
        Self::xfstize(symbol);
        crate::string_utils::replace_all(symbol, "@_EPSILON_SYMBOL_@", "0");
        crate::string_utils::replace_all(symbol, "@_UNKNOWN_SYMBOL_@", "?");
        crate::string_utils::replace_all(symbol, "@_IDENTITY_SYMBOL_@", "?");
        crate::string_utils::replace_all(symbol, "\t", "@_TAB_@");
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-state-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-state-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-state-fn]
    pub fn print_xfst_state_os(&self, os: &mut dyn Write, state: HfstState) {
        if state == Self::INITIAL_STATE {
            let _ = write!(os, "S");
        }
        if self.is_final_state(state) {
            let _ = write!(os, "f");
        }
        let _ = write!(os, "s{}", state);
    }

    pub fn print_xfst_state_file(
        &self,
        file: &mut dyn Write,
        state: HfstState,
    ) -> std::io::Result<()> {
        if state == Self::INITIAL_STATE {
            write!(file, "S")?;
        }
        if self.is_final_state(state) {
            write!(file, "f")?;
        }
        write!(file, "s{}", state)
    }

    pub fn print_xfst_arc_os(
        &self,
        os: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
    ) {
        let isym = data.get_input_symbol(&self.coder);
        let osym = data.get_output_symbol(&self.coder);
        // replace all spaces, epsilons and tabs
        if isym != osym {
            let _ = write!(os, "<");
        }
        let mut s = isym.to_string();
        Self::xfstize_symbol(&mut s);
        let _ = write!(os, "{}", s);
        if isym != osym || osym == "@_UNKNOWN_SYMBOL_@" {
            s = osym.to_string();
            Self::xfstize_symbol(&mut s);
            let _ = write!(os, ":{}", s);
        }
        if isym != osym {
            let _ = write!(os, ">");
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-arc-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-arc-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-arc-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-arc-fn]
    pub fn print_xfst_arc_file(
        &self,
        file: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
    ) -> std::io::Result<()> {
        let isym = data.get_input_symbol(&self.coder);
        let osym = data.get_output_symbol(&self.coder);
        if isym != osym {
            write!(file, "<")?;
        }
        // replace all spaces, epsilons and tabs
        let mut s = isym.to_string();
        Self::xfstize_symbol(&mut s);
        write!(file, "{}", s)?;
        if isym != osym || osym == "@_UNKNOWN_SYMBOL_@" {
            s = osym.to_string();
            Self::xfstize_symbol(&mut s);
            write!(file, ":{}", s)?;
        }
        if isym != osym {
            write!(file, ">")?;
        }
        Ok(())
    }

    /** @brief Write the graph in xfst text format to ostream 'os'. */
    // [spec:hfst:def:hfst-transition-graph.write-in-xfst-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.write-in-xfst-format-fn]
    pub fn write_in_xfst_format(&self, os: &mut dyn Write, write_weights: bool) {
        let _ = write_weights; // todo
        for (source_state, it) in self.state_vector.iter().enumerate() {
            let source_state = source_state as u32;
            self.print_xfst_state_os(os, source_state);
            let _ = write!(os, ":\t");

            if it.is_empty() {
                let _ = write!(os, "(no arcs)");
            } else {
                for (i, tr_it) in it.iter().enumerate() {
                    if i != 0 {
                        let _ = write!(os, ", ");
                    }
                    let data = tr_it.get_transition_data();
                    self.print_xfst_arc_os(os, data);

                    let _ = write!(os, " -> ");
                    self.print_xfst_state_os(os, tr_it.get_target_state());
                }
            }
            let _ = writeln!(os, ".");
        }
    }

    /** @brief Write the graph in xfst text format to FILE 'file'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-xfst-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-xfst-format-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-xfst-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-xfst-format-fn]
    pub fn write_in_xfst_format_file(
        &self,
        file: &mut dyn Write,
        write_weights: bool,
    ) -> std::io::Result<()> {
        let _ = write_weights;
        for (source_state, it) in self.state_vector.iter().enumerate() {
            let source_state = source_state as u32;
            self.print_xfst_state_file(file, source_state)?;
            write!(file, ":\t")?;

            if it.is_empty() {
                write!(file, "(no arcs)")?;
            } else {
                for (i, tr_it) in it.iter().enumerate() {
                    if i != 0 {
                        write!(file, ", ")?;
                    }
                    let data = tr_it.get_transition_data();
                    self.print_xfst_arc_file(file, data)?;

                    write!(file, " -> ")?;
                    self.print_xfst_state_file(file, tr_it.get_target_state())?;
                }
            }
            writeln!(file, ".")?;
        }
        Ok(())
    }
}
