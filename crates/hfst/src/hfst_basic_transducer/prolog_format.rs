//! Reading and writing the prolog text format, and the line reader it uses.

use std::io::{BufRead, Write};

use super::*;

// 'get_stripped_line' wrapped in the C++ try/catch: returns None when it would
// throw 'EndOfStreamException'. The panic hook is silenced so the caught
// exception (a 'panic_any') does not print.
fn catch_get_stripped_line(
    is: &mut dyn BufRead,
    linecount: &mut u32,
) -> crate::error::Result<Option<String>> {
    match HfstBasicTransducer::get_stripped_line(is, linecount) {
        Ok(v) => Ok(Some(v)),
        Err(e) if matches!(e.kind, crate::error::ErrorKind::EndOfStream) => Ok(None),
        Err(e) => Err(e),
    }
}

impl HfstBasicTransducer {
    // note: unknown and identity are both '?'
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prologize-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prologize-symbol-fn]
    pub fn prologize_symbol(symbol: &str) -> String {
        if symbol == "0" {
            return "%0".to_string();
        }
        if symbol == "?" {
            return "%?".to_string();
        }
        if symbol == "@_EPSILON_SYMBOL_@" {
            return "0".to_string();
        }
        if symbol == "@_UNKNOWN_SYMBOL_@" {
            return "?".to_string();
        }
        if symbol == "@_IDENTITY_SYMBOL_@" {
            return "?".to_string();
        }
        // prepend a backslash to a double quote and to a backslash
        let mut retval = symbol.to_string();
        crate::string_utils::replace_all(&mut retval, "\\", "\\\\");
        crate::string_utils::replace_all(&mut retval, "\"", "\\\"");
        retval
    }

    // caveat: '?' is always unknown
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.deprologize-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.deprologize-symbol-fn]
    pub fn deprologize_symbol(symbol: &str) -> String {
        if symbol == "%0" {
            return "0".to_string();
        }
        if symbol == "%?" {
            return "?".to_string();
        }
        if symbol == "0" {
            return "@_EPSILON_SYMBOL_@".to_string();
        }
        if symbol == "?" {
            return "@_UNKNOWN_SYMBOL_@".to_string();
        }
        // remove the escaping backslash in front of a double quote and a backslash
        let mut retval = symbol.to_string();
        crate::string_utils::replace_all(&mut retval, "\\\"", "\"");
        crate::string_utils::replace_all(&mut retval, "\\\\", "\\");
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-prolog-arc-symbols-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-prolog-arc-symbols-fn]
    pub fn print_prolog_arc_symbols_file(
        file: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
        coder: &SymbolCoder,
    ) -> std::io::Result<()> {
        let isym = data.get_input_symbol(coder);
        let osym = data.get_output_symbol(coder);
        let symbol = Self::prologize_symbol(&isym);
        write!(file, "\"{}\"", symbol)?;

        if isym != osym || isym == "@_UNKNOWN_SYMBOL_@" {
            let symbol = Self::prologize_symbol(&osym);
            write!(file, ":\"{}\"", symbol)?;
        }
        Ok(())
    }

    pub fn print_prolog_arc_symbols_os(
        os: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
        coder: &SymbolCoder,
    ) {
        let isym = data.get_input_symbol(coder);
        let osym = data.get_output_symbol(coder);
        let symbol = Self::prologize_symbol(&isym);
        let _ = write!(os, "\"{}\"", symbol);

        if isym != osym || isym == "@_UNKNOWN_SYMBOL_@" {
            let symbol = Self::prologize_symbol(&osym);
            let _ = write!(os, ":\"{}\"", symbol);
        }
    }

    /** @brief Write the graph in prolog format to FILE 'file'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-prolog-format-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-prolog-format-fn]
    pub fn write_in_prolog_format_file(
        &self,
        file: &mut dyn Write,
        name: &str,
        write_weights: bool,
    ) -> crate::error::Result<()> {
        // Print the name.
        if name.contains(',') {
            let msg = "no commas allowed in the name of prolog networks".to_string();
            crate::bail!(Hfst, msg);
        }
        self.write_in_prolog_format_file_body(file, name, write_weights)
            .map_err(|e| crate::err!(StreamCannotBeWritten, e.to_string()))
    }

    fn write_in_prolog_format_file_body(
        &self,
        file: &mut dyn Write,
        identifier: &str,
        write_weights: bool,
    ) -> std::io::Result<()> {
        writeln!(file, "network({}).", identifier)?;

        // Print symbols that are in the alphabet but not used in arcs.
        let mut symbols_used = self.symbols_used();
        Self::initialize_alphabet(&mut symbols_used); // exclude special symbols
        for it in self.alphabet.iter() {
            if !symbols_used.contains(it) {
                writeln!(
                    file,
                    "symbol({}, \"{}\").",
                    identifier,
                    Self::prologize_symbol(it)
                )?;
            }
        }

        // Print arcs.
        for (source_state, it) in self.state_vector.iter().enumerate() {
            for tr_it in it.iter() {
                write!(
                    file,
                    "arc({}, {}, {}, ",
                    identifier,
                    source_state,
                    tr_it.get_target_state()
                )?;
                let data = tr_it.get_transition_data();
                Self::print_prolog_arc_symbols_file(file, data, &self.coder)?;
                if write_weights {
                    write!(file, ", ")?;
                    Self::write_weight_file(file, data.get_weight())?;
                }
                writeln!(file, ").")?;
            }
        }

        // Print final states.
        for (k, v) in self.final_weight_map.iter() {
            write!(file, "final({}, {}", identifier, k)?;
            if write_weights {
                write!(file, ", ")?;
                Self::write_weight_file(file, *v)?;
            }
            writeln!(file, ").")?;
        }
        Ok(())
    }

    /** @brief Write the graph in prolog format to ostream 'os'. */
    pub fn write_in_prolog_format_os(
        &self,
        os: &mut dyn Write,
        name: &str,
        write_weights: bool,
    ) -> crate::error::Result<()> {
        // Print the name.
        if name.contains(',') {
            let msg = "no commas allowed in the name of prolog networks".to_string();
            crate::bail!(Hfst, msg);
        }
        let _ = writeln!(os, "network({}).", name);

        // Print symbols that are in the alphabet but not used in arcs.
        let mut symbols_used = self.symbols_used();
        Self::initialize_alphabet(&mut symbols_used); // exclude special symbols
        for it in self.alphabet.iter() {
            if !symbols_used.contains(it) {
                let _ = writeln!(os, "symbol({}, \"{}\").", name, Self::prologize_symbol(it));
            }
        }

        // Print arcs.
        for (source_state, it) in self.state_vector.iter().enumerate() {
            for tr_it in it.iter() {
                let _ = write!(
                    os,
                    "arc({}, {}, {}, ",
                    name,
                    source_state,
                    tr_it.get_target_state()
                );
                let data = tr_it.get_transition_data();
                Self::print_prolog_arc_symbols_os(os, data, &self.coder);
                if write_weights {
                    let _ = write!(os, ", ");
                    Self::write_weight_os(os, data.get_weight());
                }
                let _ = writeln!(os, ").");
            }
        }

        // Print final states.
        for (k, v) in self.final_weight_map.iter() {
            let _ = write!(os, "final({}, {}", name, k);
            if write_weights {
                let _ = write!(os, ", ");
                Self::write_weight_os(os, *v);
            }
            let _ = writeln!(os, ").");
        }
        Ok(())
    }

    // If 'str' is of format ".+", return .+ (quotes stripped). Else None.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-quotes-from-both-sides-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-quotes-from-both-sides-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-quotes-from-both-sides-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-quotes-from-both-sides-fn]
    pub fn strip_quotes_from_both_sides(str: &str) -> Option<&str> {
        if str.len() < 3 {
            return None;
        }
        let bytes = str.as_bytes();
        if bytes[0] != b'"' || bytes[str.len() - 1] != b'"' {
            return None;
        }
        Some(&str[1..str.len() - 1])
    }

    // If 'str' is of format .+)\." return .+ (suffix stripped). Else None.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-ending-parenthesis-and-comma-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-ending-parenthesis-and-comma-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-ending-parenthesis-and-comma-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-ending-parenthesis-and-comma-fn]
    pub fn strip_ending_parenthesis_and_comma(str: &str) -> Option<&str> {
        if str.len() < 3 {
            return None;
        }
        let bytes = str.as_bytes();
        if bytes[str.len() - 2] != b')' || bytes[str.len() - 1] != b'.' {
            return None;
        }
        Some(&str[..str.len() - 2])
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-network-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-network-line-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.parse-prolog-network-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.parse-prolog-network-line-fn]
    //
    // sscanf(line, "network(%s", namearr): match the literal prefix, then '%s'
    // (skip leading whitespace, read one non-whitespace token). Returns the
    // network name on success.
    pub fn parse_prolog_network_line(line: &str) -> Option<String> {
        // 'network(NAME).'
        let rest = line.strip_prefix("network(")?;
        let tok: String = rest
            .trim_start()
            .chars()
            .take_while(|c| !c.is_whitespace())
            .collect();
        if tok.is_empty() {
            return None;
        }

        // strip the ending ")." from the name
        let namestr = Self::strip_ending_parenthesis_and_comma(&tok)?;
        Some(namestr.to_string())
    }

    // Get positions of 'c' in 'str'. If 'esc' precedes 'c', 'c' is not included.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-positions-of-unescaped-char-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-positions-of-unescaped-char-fn]
    pub fn get_positions_of_unescaped_char(str: &str, c: char, esc: char) -> Vec<u32> {
        let mut retval: Vec<u32> = Vec::new();
        let bytes = str.as_bytes();
        for i in 0..str.len() {
            if bytes[i] == c as u8 {
                if i == 0 {
                    retval.push(i as u32);
                } else if bytes[i - 1] == esc as u8 {
                    // skip escaped chars
                } else {
                    retval.push(i as u32);
                }
            }
        }
        retval
    }

    // Extract input/output symbols from prolog arc 'str' of format "foo":"bar"
    // or "foo". Returns (isymbol, osymbol) on success.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-prolog-arc-symbols-fn]
    // [spec:hfst:def:hfst-transition-graph.get-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-transition-graph.get-prolog-arc-symbols-fn]
    pub fn get_prolog_arc_symbols(str: &str) -> Option<(String, String)> {
        // find positions of non-escaped double quotes
        let quote_positions = Self::get_positions_of_unescaped_char(str, '"', '\\');

        // "foo"
        if quote_positions.len() == 2 {
            if quote_positions[0] != 0 || quote_positions[1] != (str.len() - 1) as u32 {
                return None; // extra characters outside quotes
            }
        }
        // "foo":"bar"
        else if quote_positions.len() == 4 {
            if quote_positions[0] != 0 || quote_positions[3] != (str.len() - 1) as u32 {
                return None; // extra characters outside quotes
            }
            if quote_positions[2] - quote_positions[1] != 2 {
                return None; // missing colon between inner quotes
            }
            if str.as_bytes()[(quote_positions[1] + 1) as usize] != b':' {
                return None; // else than colon between inner quotes
            }
        }
        // not valid prolog arc
        else {
            return None;
        }

        // "foo"
        if quote_positions.len() == 2 {
            // "foo" -> foo
            let start = (quote_positions[0] + 1) as usize;
            let len = (quote_positions[1] - quote_positions[0] - 1) as usize;
            let symbol = str[start..start + len].to_string();
            let mut isymbol = Self::deprologize_symbol(&symbol);
            if isymbol == "@_UNKNOWN_SYMBOL_@" {
                // single unknown -> identity
                isymbol = "@_IDENTITY_SYMBOL_@".to_string();
            }
            let osymbol = isymbol.clone();
            Some((isymbol, osymbol))
        }
        // "foo":"bar"
        else {
            let s1 = (quote_positions[0] + 1) as usize;
            let l1 = (quote_positions[1] - quote_positions[0] - 1) as usize;
            let insymbol = str[s1..s1 + l1].to_string();
            let s2 = (quote_positions[2] + 1) as usize;
            let l2 = (quote_positions[3] - quote_positions[2] - 1) as usize;
            let outsymbol = str[s2..s2 + l2].to_string();
            Some((
                Self::deprologize_symbol(&insymbol),
                Self::deprologize_symbol(&outsymbol),
            ))
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.extract-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.extract-weight-fn]
    // [spec:hfst:def:hfst-transition-graph.extract-weight-fn]
    // [spec:hfst:sem:hfst-transition-graph.extract-weight-fn]
    // Returns (symbol with any ", W" suffix removed, weight or 0.0).
    pub fn extract_weight(symbol: &str) -> Option<(String, f32)> {
        // at least one double quote should be found
        let ldq = symbol.rfind('"')?;

        let mut sym = symbol.to_string();
        let mut weight: f32 = 0.0;
        match symbol.rfind(' ') {
            None => {
                // no weight
            }
            Some(ls) => {
                if ldq > ls {
                    // no weight, last space is part of a symbol
                } else if ldq + 2 == ls && ls < symbol.len() - 1 {
                    // + 2 because of the comma
                    let buffer = &symbol[ls + 1..];
                    match buffer.parse::<f32>() {
                        Ok(w) => weight = w,
                        Err(_) => return None, // a float could not be read
                    }
                    sym.truncate(ls - 1); // get rid of the comma and weight
                } else {
                    return None; // not valid symbol and weight
                }
            }
        }
        Some((sym, weight))
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-arc-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-arc-line-fn]
    // [spec:hfst:def:hfst-transition-graph.parse-prolog-arc-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.parse-prolog-arc-line-fn]
    // Returns (source, target, isymbol, osymbol, weight) on success.
    pub fn parse_prolog_arc_line(
        line: &str,
        graph_name: &str,
    ) -> Option<(HfstState, HfstState, String, String, f32)> {
        // sscanf(line, "arc(%[^,], %[^,], %[^,], %[^\t\n]", ...): four scanset
        // fields separated by a literal comma plus optional whitespace.
        let mut n = 0;
        let mut namestr = String::new();
        let mut sourcestr = String::new();
        let mut targetstr = String::new();
        let mut symbolstr = String::new();
        if let Some(mut rest) = line.strip_prefix("arc(") {
            let f1: String = rest.chars().take_while(|&c| c != ',').collect();
            if !f1.is_empty() {
                namestr = f1.clone();
                n = 1;
                rest = &rest[f1.len()..];
                if let Some(r) = rest.strip_prefix(',') {
                    rest = r.trim_start();
                    let f2: String = rest.chars().take_while(|&c| c != ',').collect();
                    if !f2.is_empty() {
                        sourcestr = f2.clone();
                        n = 2;
                        rest = &rest[f2.len()..];
                        if let Some(r) = rest.strip_prefix(',') {
                            rest = r.trim_start();
                            let f3: String = rest.chars().take_while(|&c| c != ',').collect();
                            if !f3.is_empty() {
                                targetstr = f3.clone();
                                n = 3;
                                rest = &rest[f3.len()..];
                                if let Some(r) = rest.strip_prefix(',') {
                                    rest = r.trim_start();
                                    let f4: String = rest
                                        .chars()
                                        .take_while(|&c| c != '\t' && c != '\n')
                                        .collect();
                                    if !f4.is_empty() {
                                        symbolstr = f4;
                                        n = 4;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // strip the ending ")." from symbolstr
        let symbol = Self::strip_ending_parenthesis_and_comma(&symbolstr)?;

        if n != 4 {
            return None;
        }
        if namestr != graph_name {
            return None;
        }

        let source: u32 = parse_state_number(&sourcestr);
        let target: u32 = parse_state_number(&targetstr);

        // handle the weight that might be included in symbol string
        let (symbol, weight) = Self::extract_weight(symbol)?;

        let (isymbol, osymbol) = Self::get_prolog_arc_symbols(&symbol)?;

        Some((source, target, isymbol, osymbol, weight))
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-final-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-final-line-fn]
    // [spec:hfst:def:hfst-transition-graph.parse-prolog-final-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.parse-prolog-final-line-fn]
    // Returns (final state, weight) on success.
    pub fn parse_prolog_final_line(line: &str, graph_name: &str) -> Option<(HfstState, f32)> {
        // 'final(NAME, number).' or 'final(NAME, number, weight).'
        let mut weight: f32 = 0.0;
        let number_of_commas = line.chars().filter(|&c| c == ',').count();

        let namestr: String;
        let finalstr: String;

        if number_of_commas == 1 {
            // sscanf(line, "final(%[^,], %[^)]).", namestr, finalstr)
            let rest = line.strip_prefix("final(")?;
            let name: String = rest.chars().take_while(|&c| c != ',').collect();
            if name.is_empty() {
                return None;
            }
            let after = &rest[name.len()..];
            let r = after.strip_prefix(',')?.trim_start();
            let fin: String = r.chars().take_while(|&c| c != ')').collect();
            if fin.is_empty() {
                return None;
            }
            namestr = name;
            finalstr = fin;
        } else if number_of_commas == 2 {
            // sscanf(line, "final(%[^,], %[^,], %[^)]).", namestr, finalstr, weightstr)
            let rest = line.strip_prefix("final(")?;
            let name: String = rest.chars().take_while(|&c| c != ',').collect();
            if name.is_empty() {
                return None;
            }
            let after = &rest[name.len()..];
            let r = after.strip_prefix(',')?.trim_start();
            let fin: String = r.chars().take_while(|&c| c != ',').collect();
            if fin.is_empty() {
                return None;
            }
            let after2 = &r[fin.len()..];
            let r2 = after2.strip_prefix(',')?.trim_start();
            let weightstr: String = r2.chars().take_while(|&c| c != ')').collect();
            if weightstr.is_empty() {
                return None;
            }
            match weightstr.parse::<f32>() {
                Ok(w) => weight = w,
                Err(_) => return None, // a float could not be read
            }
            namestr = name;
            finalstr = fin;
        } else {
            return None;
        }

        if namestr != graph_name {
            return None;
        }

        Some((parse_state_number(&finalstr), weight))
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-symbol-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-symbol-line-fn]
    // [spec:hfst:def:hfst-transition-graph.parse-prolog-symbol-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.parse-prolog-symbol-line-fn]
    // Returns the deprologized alphabet symbol on success.
    pub fn parse_prolog_symbol_line(line: &str, graph_name: &str) -> Option<String> {
        // sscanf(line, "symbol(%[^,], %s", namearr, symbolarr)
        let mut n = 0;
        let mut namearr = String::new();
        let mut symbolarr = String::new();
        if let Some(rest) = line.strip_prefix("symbol(") {
            let name: String = rest.chars().take_while(|&c| c != ',').collect();
            if !name.is_empty() {
                namearr = name.clone();
                n = 1;
                let after = &rest[name.len()..];
                if let Some(after_comma) = after.strip_prefix(',') {
                    let sym: String = after_comma
                        .trim_start()
                        .chars()
                        .take_while(|c| !c.is_whitespace())
                        .collect();
                    if !sym.is_empty() {
                        symbolarr = sym;
                        n = 2;
                    }
                }
            }
        }

        if n != 2 {
            return None;
        }

        let namestr = namearr;

        if namestr != graph_name {
            return None;
        }

        let symbolstr = Self::strip_ending_parenthesis_and_comma(&symbolarr)?;
        let symbolstr = Self::strip_quotes_from_both_sides(symbolstr)?;

        Some(Self::deprologize_symbol(symbolstr))
    }

    // Erase newlines from the end of 'str' and return 'str'.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-newlines-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-newlines-fn]
    // [spec:hfst:def:hfst-transition-graph.std.string-strip-newlines-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.string-strip-newlines-fn]
    pub fn strip_newlines(str: &mut String) -> String {
        let mut i: i64 = str.len() as i64 - 1;
        while i >= 0 {
            let b = str.as_bytes()[i as usize];
            if b == b'\n' || b == b'\r' {
                str.remove(i as usize);
            } else {
                break;
            }
            i -= 1;
        }
        str.clone()
    }

    // Try to get a line from 'is' (if 'file' is null) or 'file'. On success,
    // strip newlines, increment 'linecount', and return the line; else throw
    // EndOfStreamException.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-stripped-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-stripped-line-fn]
    // [spec:hfst:def:hfst-transition-graph.std.string-get-stripped-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.string-get-stripped-line-fn]
    pub fn get_stripped_line(
        is: &mut dyn BufRead,
        linecount: &mut u32,
    ) -> crate::error::Result<String> {
        let linestr = match crate::io_utils::read_line_lossy(is) {
            None => crate::bail!(EndOfStream),
            Some(l) => l,
        };
        *linecount += 1;

        let mut s = linestr;
        Ok(Self::strip_newlines(&mut s))
    }

    // Create a graph from prolog format in 'is' (if 'file' is null) or 'file'.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-prolog-format-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-read-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-read-in-prolog-format-fn]
    pub fn read_in_prolog_format(
        is: &mut dyn BufRead,
        linecount: &mut u32,
    ) -> crate::error::Result<HfstBasicTransducer> {
        let mut retval = HfstBasicTransducer::new();
        let mut linestr: String;

        loop {
            match catch_get_stripped_line(is, linecount)? {
                Some(l) => linestr = l,
                None => crate::bail!(NotValidPrologFormat),
            }

            if !linestr.is_empty() && linestr.as_bytes()[0] == b'#' {
                continue; // comment line
            } else {
                break; // first non-comment line
            }
        }

        match Self::parse_prolog_network_line(&linestr) {
            Some(name) => retval.name = name,
            None => {
                let message = format!("first line not valid prolog: {linestr}");
                crate::bail!(NotValidPrologFormat, message);
            }
        }

        loop {
            match catch_get_stripped_line(is, linecount)? {
                Some(l) => {
                    linestr = l;
                    if linestr.is_empty() {
                        // prolog separator
                        return Ok(retval);
                    }
                }
                None => return Ok(retval),
            }

            if let Some((source, target, isymbol, osymbol, weight)) =
                Self::parse_prolog_arc_line(&linestr, &retval.name)
            {
                let tr = HfstBasicTransition::new_symbols(
                    target,
                    isymbol.into(),
                    osymbol.into(),
                    weight,
                    retval.coder_mut(),
                );
                retval.add_transition(source, &tr, true);
            } else if let Some((state, weight)) =
                Self::parse_prolog_final_line(&linestr, &retval.name)
            {
                retval.set_final_weight(state, &weight);
            } else if let Some(symbol) = Self::parse_prolog_symbol_line(&linestr, &retval.name) {
                retval.add_symbol_to_alphabet(&symbol.into());
            } else {
                let message = format!("line not valid prolog: {linestr}");
                crate::bail!(NotValidPrologFormat, message);
            }
        }
    }

    pub fn read_in_prolog_format_is(
        is: &mut dyn BufRead,
        linecount: &mut u32,
    ) -> crate::error::Result<HfstBasicTransducer> {
        Self::read_in_prolog_format(is, linecount)
    }

    pub fn read_in_prolog_format_file(
        file: &mut dyn BufRead,
        linecount: &mut u32,
    ) -> crate::error::Result<HfstBasicTransducer> {
        Self::read_in_prolog_format(file, linecount)
    }
}
