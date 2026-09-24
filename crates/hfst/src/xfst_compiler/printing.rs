//! The print commands that describe networks and session state: sigma,
//! labels, lists, definitions, names and the network itself.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // @brief Print parts of automaton with epsilon loops
    // @todo unimplemented yet
    pub fn collect_epsilon_loops(&mut self) -> &mut Self {
        self.diag_warning("collect epsilon-loops is not implemented; nothing was printed");
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print arc count for @a level
    pub fn print_arc_count_level(
        &mut self,
        level: &str,
        oss: &mut dyn std::io::Write,
    ) -> &mut Self {
        let _ = writeln!(oss, "missing {} arc count", level);
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print arc count
    pub fn print_arc_count(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing arc count");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print file info
    pub fn print_file_info(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        self.diag_warning("print file-info is not implemented; nothing was printed");
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print flag diacritics
    pub fn print_flags(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing print flags");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print label mappings
    pub fn print_labelmaps(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing label-maps");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print properties of top network
    pub fn print_properties(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing print properties");
        self.flush();
        self
    }

    // @brief Print properties of network named @a name
    pub fn print_properties_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        self.diag_warning("print properties is not implemented; nothing was printed");
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print nnumber of symbols in network
    pub fn print_sigma_count(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing print sigma count");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print number of paths with all symbols on @a level
    pub fn print_sigma_word_count_level(
        &mut self,
        level: &str,
        oss: &mut dyn std::io::Write,
    ) -> &mut Self {
        let _ = writeln!(oss, "missing {} sigma word count", level);
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print number of paths with all symbols
    pub fn print_sigma_word_count(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing sigma word count");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print size of network named @a name
    pub fn print_size_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "{:>10}", name);
        let _ = writeln!(oss, ": ? bytes. ? states, ? arcs, ? paths.");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print size of top network
    pub fn print_size(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "? bytes. ? states, ? arcs, ? paths.");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Print aliases
    pub fn print_aliases(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let aliases: Vec<(String, String)> = self
            .aliases
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (first, second) in aliases.iter() {
            let _ = write!(oss, "{:>10}", "alias ");
            let _ = write!(oss, "{} {}", first, second);
        }
        self.flush();
        self.prompt();
        self
    }

    // @brief Print definition
    pub fn print_defined(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let mut definitions = false;
        let defs: Vec<(String, String)> = self
            .original_definitions
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (first, second) in defs.iter() {
            definitions = true;
            let _ = write!(oss, "{:>10}", first);
            let _ = writeln!(oss, " {}", second);
        }
        if !definitions {
            let _ = writeln!(oss, "No defined symbols.");
        }

        definitions = false;
        let funcs: Vec<(String, String)> = self
            .original_function_definitions
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (first, second) in funcs.iter() {
            definitions = true;
            let _ = write!(oss, "{:>10}", first);
            let _ = writeln!(oss, " {}", second);
        }
        if !definitions {
            let _ = writeln!(oss, "No function definitions.");
        }

        self.flush();
        self.prompt();
        self
    }

    // @brief Print directory contents
    pub fn print_dir(&mut self, glob: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        match glob::glob(glob) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    let _ = writeln!(oss, "{}", entry.display());
                }
            }
            Err(e) => {
                let _ = writeln!(oss, "glob({}) = {}", glob, e);
            }
        }
        self.prompt();
        self
    }

    pub fn print_labels_tr(
        &mut self,
        oss: &mut dyn std::io::Write,
        tr: &HfstTransducer<B>,
    ) -> &mut Self {
        let mut label_set: BTreeSet<(Symbol, Symbol)> = BTreeSet::new();
        let fsm = HfstBasicTransducer::from_transducer(tr);

        for it in fsm.iter() {
            for tr_it in it.iter() {
                label_set.insert((
                    tr_it.get_input_symbol(fsm.coder()),
                    tr_it.get_output_symbol(fsm.coder()),
                ));
            }
        }

        let _ = write!(oss, "Labels: ");
        let first_elem = label_set.iter().next().cloned();
        for it in label_set.iter() {
            if Some(it) != first_elem.as_ref() {
                let _ = write!(oss, ", ");
            }
            let _ = write!(oss, "{}", it.0);
            if it.0 != it.1 {
                let _ = write!(oss, ":{}", it.1);
            }
        }
        let _ = writeln!(oss);
        let _ = writeln!(oss, "Size: {}", label_set.len() as i32);

        self.flush();
        self.prompt();
        self
    }

    // @brief Print labels in network @a name
    pub fn print_labels_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        match self.definitions.get(name).copied() {
            None => {
                let _ = writeln!(oss, "no such definition '{}'", name);
            }
            Some(tr) => {
                let net = self.net(tr).clone();
                return self.print_labels_tr(oss, &net);
            }
        }
        self.flush();
        self.prompt();
        self
    }

    // @brief Print labels
    pub fn print_labels(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        let net = self.net(topmost).clone();
        self.print_labels_tr(oss, &net)
    }

    // @brief Print label count
    pub fn print_label_count(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };

        let mut label_map: BTreeMap<(Symbol, Symbol), u32> = BTreeMap::new();
        let fsm = HfstBasicTransducer::from_transducer(self.net(topmost));

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
                let _ = write!(oss, "   ");
            }
            let _ = write!(oss, "{}. ", index);
            let _ = write!(oss, "{}", key.0);
            if key.0 != key.1 {
                let _ = write!(oss, ":{}", key.1);
            }
            let _ = write!(oss, " {}", value);
        }
        let _ = writeln!(oss);

        self.flush();
        self.prompt();
        self
    }

    // @brief Print list named @a name
    pub fn print_list_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        if !self.lists.contains_key(name) {
            let _ = writeln!(oss, "No such list defined: {}", name);
            self.flush();
            self.prompt();
            return self;
        }
        let l = self.lists[name].clone();
        let _ = write!(oss, "{:>10}", name);
        let _ = write!(oss, ": ");
        for s in l.iter() {
            let _ = write!(oss, "{} ", s);
        }
        let _ = writeln!(oss);
        self.flush();
        self.prompt();
        self
    }

    // @brief Print all lists
    pub fn print_list(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        if self.lists.is_empty() {
            let _ = writeln!(oss, "No lists defined.");
            self.flush();
            self.prompt();
            return self;
        }
        let lists: Vec<(String, BTreeSet<Symbol>)> = self
            .lists
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (first, second) in lists.iter() {
            // HERE
            let _ = write!(oss, "{:>10}", first);
            let _ = write!(oss, " ");
            for s in second.iter() {
                let _ = write!(oss, "{} ", s);
            }
            let _ = writeln!(oss);
        }
        self.flush();
        self.prompt();
        self
    }

    // @brief Print name of top network
    pub fn print_name(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };

        let entries: Vec<(String, NetId)> = self
            .names
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        for (first, second) in entries.iter() {
            if tmp == *second {
                let _ = writeln!(oss, "Name {}", first);
                self.flush();
                self.prompt();
                return self;
            }
        }

        let _ = writeln!(oss, "No name.");
        self.flush();
        self.prompt();
        self
    }

    // @brief Print network
    pub fn print_net(&mut self, oss: &mut dyn std::io::Write) -> crate::error::Result<&mut Self> {
        if self.variables["print-sigma"] == "ON" {
            self.print_sigma(oss, false /*do not prompt*/)?;
        }
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let basic = HfstBasicTransducer::from_transducer(self.net(tmp));
        basic.write_in_xfst_format(oss, self.variables["print-weight"] == "ON");
        self.flush();
        self.prompt();
        Ok(self)
    }

    // @brief Print network named @a name
    pub fn print_net_name(
        &mut self,
        name: &str,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        match self.definitions.get(name).copied() {
            None => {
                self.diag_unknown_definition(name);
                self.prompt();
                Ok(self)
            }
            Some(it) => {
                if self.variables["print-sigma"] == "ON" {
                    self.stack.push(it);
                    self.print_sigma(oss, false /*do not prompt*/)?;
                    self.stack.pop();
                }
                let basic = HfstBasicTransducer::from_transducer(self.net(it));
                basic.write_in_xfst_format(oss, self.variables["print-weight"] == "ON");
                self.flush();
                self.prompt();
                Ok(self)
            }
        }
    }

    // @brief Print all symbols of network
    pub fn print_sigma(
        &mut self,
        oss: &mut dyn std::io::Write,
        prompt: bool,
    ) -> crate::error::Result<&mut Self> {
        let Some(t) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let alpha = self.net(t).get_alphabet()?;

        // find out whether unknown or identity is used in transitions
        let (unknown, identity) = uses_unknown_or_identity(self.net(t));

        self.print_alphabet(&alpha, unknown, identity, oss);
        if prompt {
            self.prompt();
        }
        self.flush();
        Ok(self)
    }

    // @brief Print all symbols of network named @a name
    pub fn print_sigma_name(&mut self, _name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing print sigma");
        self.flush();
        self.prompt();
        self
    }

    // @brief Print all networks in stack
    pub fn print_stack(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        for i in 0..self.stack.len() {
            let _ = write!(
                oss,
                "{:>10}",
                format!("{}: ? bytes. ? states, ? arcs, ? paths.", i)
            );
            let _ = writeln!(oss);
        }
        self.flush();
        self.prompt();
        self
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
    ) {
        let mut sigma_count: u32 = 0;
        let _ = write!(oss, "Sigma: ");
        if self.variables["print-foma-sigma"] == "ON" {
            if unknown {
                let _ = write!(oss, "?");
            }
            if identity {
                if unknown {
                    let _ = write!(oss, ", ");
                }
                let _ = write!(oss, "@");
            }
        } else
        // xfst-style sigma print
        {
            if unknown || identity {
                let _ = write!(oss, "?");
            }
        }

        let mut first_symbol = true;
        for it in alpha.iter() {
            if !is_special_symbol(it) {
                if !first_symbol || unknown || identity {
                    let _ = write!(oss, ", ");
                }
                if it == "?" {
                    let _ = write!(oss, "\"?\"");
                } else if it == "@" && self.variables["print-foma-sigma"] == "ON" {
                    let _ = write!(oss, "\"@\"");
                } else {
                    let _ = write!(oss, "{}", it);
                }
                sigma_count += 1;
                first_symbol = false;
            }
        }
        let _ = writeln!(oss);
        let _ = writeln!(oss, "Size: {}.", sigma_count);
        self.flush();
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

    let fsm = HfstBasicTransducer::from_transducer(t);
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
