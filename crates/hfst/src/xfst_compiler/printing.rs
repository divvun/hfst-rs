//! The print commands that describe networks and session state: sigma,
//! labels, lists, definitions, names and the network itself.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // @brief Print parts of automaton with epsilon loops
    pub fn collect_epsilon_loops(&mut self) -> CmdResult {
        Err(CommandError::not_supported("collect epsilon-loops"))
    }

    // @brief Print arc count
    // [spec:hfst:sem:xfst-cmd.print-counts]
    /// 'print arc-tally': the number of arcs in the top network.
    pub fn print_arc_count(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let top = self.top()?;
        writeln!(oss, "{}", self.net(top).number_of_arcs())?;
        self.prompt();
        Ok(())
    }

    // @brief Print file info
    pub fn print_file_info(&mut self, _oss: &mut dyn std::io::Write) -> CmdResult {
        Err(CommandError::not_supported("print file-info"))
    }

    // @brief Print flag diacritics
    // [spec:hfst:sem:xfst-cmd.print-counts]
    /// 'print flags': the flag diacritics in the top network's alphabet.
    pub fn print_flags(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let top = self.top()?;
        for symbol in self.net(top).get_alphabet()?.iter() {
            if crate::hfst_flag_diacritics::FdOperation::is_diacritic(symbol) {
                writeln!(oss, "{}", symbol)?;
            }
        }
        self.prompt();
        Ok(())
    }

    // @brief Print label mappings
    pub fn print_labelmaps(&mut self, _oss: &mut dyn std::io::Write) -> CmdResult {
        Err(CommandError::not_supported("print label-maps"))
    }

    // @brief Print properties of top network
    // [spec:hfst:sem:xfst-cmd.print-counts]
    /// 'print props': each property of the top network.
    pub fn print_properties(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let top = self.top()?;
        for (name, value) in self.net(top).get_properties() {
            writeln!(oss, "{}: {}", name, value)?;
        }
        self.prompt();
        Ok(())
    }

    // @brief Print nnumber of symbols in network
    // [spec:hfst:sem:xfst-cmd.print-counts]
    /// 'print sigma-tally': how many arcs each sigma symbol labels, on
    /// either side.
    pub fn print_sigma_count(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let top = self.top()?;
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(top))?;
        let mut tally: BTreeMap<Symbol, usize> = self
            .net(top)
            .get_alphabet()?
            .into_iter()
            .filter(|s| !is_special_symbol(s))
            .map(|s| (s, 0))
            .collect();
        for state in fsm.iter() {
            for arc in state.iter() {
                let input = arc.get_input_symbol(fsm.coder());
                let output = arc.get_output_symbol(fsm.coder());
                if let Some(n) = tally.get_mut(&input) {
                    *n += 1;
                }
                if output != input
                    && let Some(n) = tally.get_mut(&output)
                {
                    *n += 1;
                }
            }
        }
        for (symbol, count) in &tally {
            writeln!(oss, "{}: {}", symbol, count)?;
        }
        self.prompt();
        Ok(())
    }

    // @brief Print number of paths with all symbols
    pub fn print_sigma_word_count(&mut self, _oss: &mut dyn std::io::Write) -> CmdResult {
        Err(CommandError::not_supported("print sigma-word-tally"))
    }

    // @brief Print size of top network
    // [spec:hfst:sem:xfst-cmd.print-counts]
    /// 'print size': the top network's state and arc counts.
    pub fn print_size(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let top = self.top()?;
        writeln!(oss, "{}", self.size_line(top))?;
        self.prompt();
        Ok(())
    }

    /// The state and arc counts of a network, as 'print size' shows them.
    pub(super) fn size_line(&self, id: NetId) -> String {
        let t = self.net(id);
        format!(
            "{} states, {} arcs",
            t.number_of_states(),
            t.number_of_arcs()
        )
    }

    // @brief Print aliases
    pub fn print_aliases(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        for (name, commands) in &self.aliases {
            writeln!(oss, "alias {} {}", name, commands)?;
        }
        self.flush();
        self.prompt();
        Ok(())
    }

    // @brief Print definition
    pub fn print_defined(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let mut definitions = false;
        let defs: Vec<(String, String)> = self
            .original_definitions
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (first, second) in defs.iter() {
            definitions = true;
            write!(oss, "{:>10}", first)?;
            writeln!(oss, " {}", second)?;
        }
        if !definitions {
            writeln!(oss, "No defined symbols.")?;
        }

        definitions = false;
        let funcs: Vec<(String, String)> = self
            .original_function_definitions
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (first, second) in funcs.iter() {
            definitions = true;
            write!(oss, "{:>10}", first)?;
            writeln!(oss, " {}", second)?;
        }
        if !definitions {
            writeln!(oss, "No function definitions.")?;
        }

        self.flush();
        self.prompt();
        Ok(())
    }

    // @brief Print directory contents
    pub fn print_dir(&mut self, glob: &str, oss: &mut dyn std::io::Write) -> CmdResult {
        match glob::glob(glob) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    writeln!(oss, "{}", entry.display())?;
                }
            }
            Err(e) => {
                writeln!(oss, "glob({}) = {}", glob, e)?;
            }
        }
        self.prompt();
        Ok(())
    }

    pub fn print_labels_tr(
        &mut self,
        oss: &mut dyn std::io::Write,
        tr: &HfstTransducer<B>,
    ) -> CmdResult {
        let mut label_set: BTreeSet<(Symbol, Symbol)> = BTreeSet::new();
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(tr)?;

        for it in fsm.iter() {
            for tr_it in it.iter() {
                label_set.insert((
                    tr_it.get_input_symbol(fsm.coder()),
                    tr_it.get_output_symbol(fsm.coder()),
                ));
            }
        }

        write!(oss, "Labels: ")?;
        let first_elem = label_set.iter().next().cloned();
        for it in label_set.iter() {
            if Some(it) != first_elem.as_ref() {
                write!(oss, ", ")?;
            }
            write!(oss, "{}", it.0)?;
            if it.0 != it.1 {
                write!(oss, ":{}", it.1)?;
            }
        }
        writeln!(oss)?;
        writeln!(oss, "Size: {}", label_set.len() as i32)?;

        self.flush();
        self.prompt();
        Ok(())
    }

    // @brief Print labels in network @a name
    pub fn print_labels_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> CmdResult {
        let Some(&tr) = self.definitions.get(name) else {
            return Err(self.unknown_definition(name));
        };
        let net = self.net(tr).clone();
        self.print_labels_tr(oss, &net)
    }

    // @brief Print labels
    pub fn print_labels(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let topmost = self.top()?;
        let net = self.net(topmost).clone();
        self.print_labels_tr(oss, &net)
    }

    // @brief Print label count
    pub fn print_label_count(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let topmost = self.top()?;

        let mut label_map: BTreeMap<(Symbol, Symbol), u32> = BTreeMap::new();
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(topmost))?;

        for it in fsm.iter() {
            for tr_it in it.iter() {
                *label_map
                    .entry((
                        tr_it.get_input_symbol(fsm.coder()),
                        tr_it.get_output_symbol(fsm.coder()),
                    ))
                    .or_insert(0) += 1;
            }
        }

        for (i, (key, value)) in label_map.iter().enumerate() {
            let index = (i as u32) + 1;
            if i != 0 {
                write!(oss, "   ")?;
            }
            write!(oss, "{}. ", index)?;
            write!(oss, "{}", key.0)?;
            if key.0 != key.1 {
                write!(oss, ":{}", key.1)?;
            }
            write!(oss, " {}", value)?;
        }
        writeln!(oss)?;

        self.flush();
        self.prompt();
        Ok(())
    }

    // @brief Print list named @a name
    pub fn print_list_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> CmdResult {
        if !self.lists.contains_key(name) {
            return Err(CommandError::new(format!("no such list: '{}'", name)));
        }
        let l = self.lists[name].clone();
        write!(oss, "{:>10}", name)?;
        write!(oss, ": ")?;
        for s in l.iter() {
            write!(oss, "{} ", s)?;
        }
        writeln!(oss)?;
        self.flush();
        self.prompt();
        Ok(())
    }

    // @brief Print all lists
    pub fn print_list(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        if self.lists.is_empty() {
            writeln!(oss, "No lists defined.")?;
            self.flush();
            self.prompt();
            return Ok(());
        }
        let lists: Vec<(String, BTreeSet<Symbol>)> = self
            .lists
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (first, second) in lists.iter() {
            // HERE
            write!(oss, "{:>10}", first)?;
            write!(oss, " ")?;
            for s in second.iter() {
                write!(oss, "{} ", s)?;
            }
            writeln!(oss)?;
        }
        self.flush();
        self.prompt();
        Ok(())
    }

    // @brief Print name of top network
    pub fn print_name(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let tmp = self.top()?;

        let entries: Vec<(String, NetId)> = self
            .names
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        for (first, second) in entries.iter() {
            if tmp == *second {
                writeln!(oss, "Name {}", first)?;
                self.flush();
                self.prompt();
                return Ok(());
            }
        }

        writeln!(oss, "No name.")?;
        self.flush();
        self.prompt();
        Ok(())
    }

    // @brief Print network
    pub fn print_net(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        if self.variables["print-sigma"] == "ON" {
            self.print_sigma(oss, false /*do not prompt*/)?;
        }
        let tmp = self.top()?;
        let basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(tmp))?;
        basic.write_in_xfst_format(oss, self.variables["print-weight"] == "ON");
        self.flush();
        self.prompt();
        Ok(())
    }

    // @brief Print network named @a name
    pub fn print_net_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> CmdResult {
        match self.definitions.get(name).copied() {
            None => Err(self.unknown_definition(name)),
            Some(it) => {
                if self.variables["print-sigma"] == "ON" {
                    self.stack.push(it);
                    self.print_sigma(oss, false /*do not prompt*/)?;
                    self.stack.pop();
                }
                let basic =
                    ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(it))?;
                basic.write_in_xfst_format(oss, self.variables["print-weight"] == "ON");
                self.flush();
                self.prompt();
                Ok(())
            }
        }
    }

    // @brief Print all symbols of network
    pub fn print_sigma(&mut self, oss: &mut dyn std::io::Write, prompt: bool) -> CmdResult {
        let t = self.top()?;
        let alpha = self.net(t).get_alphabet()?;

        // find out whether unknown or identity is used in transitions
        let (unknown, identity) = uses_unknown_or_identity(self.net(t));

        self.print_alphabet(&alpha, unknown, identity, oss)?;
        if prompt {
            self.prompt();
        }
        self.flush();
        Ok(())
    }

    // @brief Print all networks in stack
    // [spec:hfst:sem:xfst-cmd.print-counts]
    /// 'print stack': the size of every network on the stack, from the
    /// bottom.
    pub fn print_stack(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        for (i, &id) in self.stack.iter().enumerate() {
            writeln!(oss, "{}: {}", i, self.size_line(id))?;
        }
        self.prompt();
        Ok(())
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-alphabet-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-alphabet-fn]
    // @brief Print alphabet \a alpha to \a outfile. \a unknown and \a identity
    // define whether these symbols occur in the transitions of the transducer
    // whose alphabet we are printing.
    fn print_alphabet(
        &mut self,
        alpha: &StringSet,
        unknown: bool,
        identity: bool,
        oss: &mut dyn std::io::Write,
    ) -> CmdResult {
        let mut sigma_count: u32 = 0;
        write!(oss, "Sigma: ")?;
        if self.variables["print-foma-sigma"] == "ON" {
            if unknown {
                write!(oss, "?")?;
            }
            if identity {
                if unknown {
                    write!(oss, ", ")?;
                }
                write!(oss, "@")?;
            }
        } else
        // xfst-style sigma print
        {
            if unknown || identity {
                write!(oss, "?")?;
            }
        }

        let mut first_symbol = true;
        for it in alpha.iter() {
            if !is_special_symbol(it) {
                if !first_symbol || unknown || identity {
                    write!(oss, ", ")?;
                }
                if it == "?" {
                    write!(oss, "\"?\"")?;
                } else if it == "@" && self.variables["print-foma-sigma"] == "ON" {
                    write!(oss, "\"@\"")?;
                } else {
                    write!(oss, "{}", it)?;
                }
                sigma_count += 1;
                first_symbol = false;
            }
        }
        writeln!(oss)?;
        writeln!(oss, "Size: {}.", sigma_count)?;
        self.flush();
        Ok(())
    }
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.is-special-symbol-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.is-special-symbol-fn]
fn is_special_symbol(s: &str) -> bool {
    if s == crate::hfst_symbol_defs::internal_epsilon
        || s == crate::hfst_symbol_defs::internal_unknown
        || s == crate::hfst_symbol_defs::internal_identity
    {
        return true;
    }
    false
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.is-unknown-or-identity-used-in-transducer-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.is-unknown-or-identity-used-in-transducer-fn]
// Returns (unknown used, identity used).
fn uses_unknown_or_identity<B: Backend>(t: &HfstTransducer<B>) -> (bool, bool) {
    let mut unknown = false;
    let mut identity = false;

    let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(t)
        .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
    for it in fsm.iter() {
        for tr_it in it.iter() {
            let istr = tr_it.get_input_symbol(fsm.coder());
            let ostr = tr_it.get_input_symbol(fsm.coder());
            if istr == crate::hfst_symbol_defs::internal_unknown
                || ostr == crate::hfst_symbol_defs::internal_unknown
            {
                unknown = true;
            } else if istr == crate::hfst_symbol_defs::internal_identity
                || ostr == crate::hfst_symbol_defs::internal_identity
            // should not happen
            {
                identity = true;
            } else {
                // ;
            }
            if unknown && identity {
                return (unknown, identity);
            }
        }
    }
    (unknown, identity)
}
