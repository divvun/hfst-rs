//! Reading and writing networks, properties and definitions from and to files.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.convert-to-common-format-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.convert-to-common-format-fn]
    // @brief Convert format of \a t read from file \a filename to common
    // format used by this xfst compiler and print a warning message
    // about loss of information during conversion, if needed.
    fn convert_to_common_format(
        &mut self,
        t: crate::hfst_transducer::AnyTransducer,
        filename: Option<&str>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // The lossiness warnings consult the stream sum's runtime tag BEFORE
        // the typed extraction below converts it away.
        let t_type = t.get_type();
        if t_type != B::TYPE {
            if t_type == ImplementationType::HFST_OL_TYPE
                || t_type == ImplementationType::HFST_OLW_TYPE
                || t_type == ImplementationType::THFST_TYPE
            {
                if self.verbose {
                    self.diag_warning(
                        "transducer is in optimized lookup format, 'apply up' is the only operation it supports",
                    );
                }
            } else {
                // C++ hfst-xfst THROWS TransducerTypeMismatchException for a
                // cross-backend read ("loading automata in different formats
                // (OpenFst, foma) is not supported in XFST scripts"); we are
                // more permissive and convert through the basic transducer.
                // That is not a problem, but the conversion should never be
                // silent: log it unconditionally at info level (the tracing
                // level filter, not xfst's own -v flag, decides visibility).
                let mut line = format!(
                    "converting transducer type from {} to {}",
                    crate::hfst_data_types::implementation_type_to_format(t_type),
                    crate::hfst_data_types::implementation_type_to_format(B::TYPE)
                );
                if filename.is_some() {
                    line.push_str(&format!(
                        " when reading from file '{}'",
                        to_filename(filename)
                    ));
                }
                if !crate::hfst_transducer::is_safe_conversion(t_type, B::TYPE) {
                    line.push_str(" (loss of information is possible)");
                }
                info!("{}", line);
            }
        }
        // The typed extraction ([dec:hfst:monomorphic-backends]): the matching
        // variant moves out unchanged, any other converts through the basic
        // transducer (this replaces the former in-place 'convert').
        t.into_typed::<B>()
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.open-hfst-input-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.open-hfst-input-stream-fn]
    // @brief Open HfstInputStream to file \a filename.
    // Print an error message and return NULL, if not succesful.
    fn open_hfst_input_stream(&mut self, filename: &str) -> CmdResult<HfstInputStream<'static>> {
        self.check_filename(filename)?;

        // Probe with a plain open first: the io error names the actual cause
        // (missing, unreadable, a directory), which 'could not open' alone
        // would leave the user guessing at.
        crate::hfst_data_types::open_file(filename, "r")
            .map_err(|e| CommandError::new(format!("could not open file '{}': {}", filename, e)))?;

        HfstInputStream::new_filename(filename).map_err(|_| {
            CommandError::new(format!(
                "unable to read transducers from {}",
                to_filename(Some(filename))
            ))
            .with_note("the file exists but is not in a transducer format this compiler reads")
        })
    }

    // @brief Read transducers from file \a infilename and either push
    // them to the stack (if \a definitions is false) or add them as definitions
    // (if \a definitions is true).
    fn load_stack_or_definitions(&mut self, infilename: &str, definitions: bool) -> CmdResult {
        let mut instream = self.open_hfst_input_stream(infilename)?;

        while instream.is_good() {
            let t = instream
                .read()
                .map_err(|e| CommandError::new(format!("reading '{}': {}", infilename, e)))?;
            let t = self
                .convert_to_common_format(t, Some(infilename))
                .map_err(|e| CommandError::new(format!("converting '{}': {}", infilename, e)))?;
            let t: NetId = self.alloc_net(t);

            if definitions {
                self.add_loaded_definition(t);
            } else {
                self.stack.push(t);
                self.print_transducer_info();
            }
        }

        instream.close();
        self.prompt();
        Ok(())
    }

    // @brief Add a transducer definition with name given by 't.get_name()'
    // and value \a t.
    fn add_loaded_definition(&mut self, t: NetId) {
        let def_name = self.net(t).get_name();
        if def_name.is_empty() {
            self.diag_warning("loaded transducer definition has no name, skipping it");
            return;
        }
        if self.definitions.contains_key(def_name.as_str()) {
            self.diag_warning(&format!(
                "a definition named '{}' already exists, overwriting it",
                def_name
            ));
        }
        self.definitions.insert(Symbol::from(def_name), t);
    }

    fn add_prop_line(&mut self, line: &str) -> CmdResult {
        let Some((name, value)) = line.split_once(':') else {
            return Err(CommandError::new(format!(
                "property line '{}' has no ':' between name and value",
                line
            )));
        };
        self.properties
            .insert(name.to_string(), value.trim_start().to_string());
        Ok(())
    }

    // @brief Write top transducer in att format to @a outfile
    pub fn write_att(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let tmp = self.top()?;
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(tmp))?;
        fsm.write_in_att_format_os(oss, self.variables["print-weight"] == "ON");
        oss.flush()?;
        self.prompt();
        Ok(())
    }

    // @brief Load regex macros from file
    // @todo Definition names cannot be stored in HFST automata binaries
    pub fn load_definitions(&mut self, infilename: &str) -> CmdResult {
        self.load_stack_or_definitions(infilename, true)
    }

    // @brief Load stack from file
    pub fn load_stack(&mut self, infilename: &str) -> CmdResult {
        self.load_stack_or_definitions(infilename, false)
    }

    // @brief Add properties from text, one property per line
    // @todo properties cannot be stored in HFST automata
    pub fn add_props(&mut self, indata: &str) -> CmdResult {
        for line in indata.split('\n').filter(|l| !l.is_empty()) {
            self.add_prop_line(line)?;
        }
        self.prompt();
        Ok(())
    }

    // @brief Save top networks dot form in @a outfile
    pub fn write_dot(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let tmp = self.top()?;
        crate::hfst_print_dot::print_dot_os(oss, self.net_mut(tmp));
        oss.flush()?;
        self.prompt();
        Ok(())
    }

    // @brief Save top networks prolog form in @a outfile
    pub fn write_prolog(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        self.top()?;
        let ids: Vec<NetId> = self.stack.iter().rev().copied().collect();
        for (i, tr) in ids.iter().enumerate() {
            let mut name = self.net(*tr).get_name();
            if name.is_empty() {
                name = "NO_NAME".to_string();
            }
            let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(*tr))?;
            let write_weights = self.variables["print-weight"] == "ON";
            fsm.write_in_prolog_format_os(oss, &name, write_weights)?;
            if i + 1 != self.stack.len() {
                writeln!(oss)?;
            }
        }
        oss.flush()?;
        self.prompt();
        Ok(())
    }

    // @brief Save top networks spaced paths form in @a outfile
    // [spec:hfst:sem:xfst-cmd.write-word-lists]
    /// 'write spaced-text': every path, symbols separated by spaces, a pair
    /// whose sides differ as 'upper:lower'.
    pub fn write_spaced(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        for path in self.top_paths("write spaced-text")? {
            let symbols: Vec<String> = path
                .iter()
                .filter(|(i, o)| !(is_epsilon(i) && is_epsilon(o)))
                .map(|(i, o)| {
                    if i == o {
                        escape_text_symbol(i)
                    } else {
                        format!("{}:{}", escape_text_symbol(i), escape_text_symbol(o))
                    }
                })
                .collect();
            writeln!(oss, "{}", symbols.join(" "))?;
        }
        oss.flush()?;
        self.prompt();
        Ok(())
    }

    /// Every path of the top network, in a stable order, failing on a cyclic
    /// network rather than listing part of it.
    fn top_paths(&self, command: &str) -> CmdResult<Vec<Vec<(Symbol, Symbol)>>> {
        let top = self.top()?;
        let mut results = HfstTwoLevelPaths::new();
        self.net(top)
            .extract_paths(&mut results, -1, -1)
            .map_err(|e| {
                if matches!(e.kind, crate::error::ErrorKind::TransducerIsCyclic) {
                    CommandError::new(format!(
                        "'{}' cannot list a cyclic network: it has infinitely many paths",
                        command
                    ))
                } else {
                    e.into()
                }
            })?;
        let paths: BTreeSet<Vec<(Symbol, Symbol)>> =
            results.into_iter().map(|p| p.second).collect();
        Ok(paths.into_iter().collect())
    }

    // @brief Save top networks paths form in @a outfile
    // [spec:hfst:sem:xfst-cmd.write-word-lists]
    /// 'write text': every string of the upper side, symbols joined.
    pub fn write_text(&mut self, oss: &mut dyn std::io::Write) -> CmdResult {
        let mut words = BTreeSet::new();
        for path in self.top_paths("write text")? {
            let word: String = path
                .iter()
                .filter(|(i, _)| !is_epsilon(i))
                .map(|(i, _)| escape_text_symbol(i))
                .collect();
            words.insert(word);
        }
        for word in &words {
            writeln!(oss, "{}", word)?;
        }
        oss.flush()?;
        self.prompt();
        Ok(())
    }

    // @brief Save definition @a name in @a outfile
    // @todo HFST does not support saving name of definition in file
    pub fn write_definition(&mut self, name: &str, outfilename: &str) -> CmdResult {
        let Some(&def_ptr) = self.definitions.get(name) else {
            return Err(self.unknown_definition(name));
        };

        let mut outstream = if !outfilename.is_empty() {
            HfstOutputStream::new_filename(outfilename, B::TYPE, true)?
        } else {
            HfstOutputStream::new(B::TYPE, true)?
        };
        let mut tmp = HfstTransducer::new_copy(self.net(def_ptr))?;
        if self.variables["name-nets"] == "ON" {
            tmp.set_name(name);
        }
        outstream.write(&mut tmp)?;
        outstream.close();
        self.prompt();
        Ok(())
    }

    // @brief Save all definitions in @a outfile
    // @todo HFST does not support saving name of definition in file
    pub fn write_definitions(&mut self, outfilename: &str) -> CmdResult {
        if self.definitions.is_empty() {
            return Err(CommandError::new(
                "no networks are defined, nothing to save",
            ));
        }

        let mut outstream = if !outfilename.is_empty() {
            HfstOutputStream::new_filename(outfilename, B::TYPE, true)?
        } else {
            HfstOutputStream::new(B::TYPE, true)?
        };
        let defs: Vec<(String, NetId)> = self
            .definitions
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        for (name, def) in defs.iter() {
            let mut tmp = HfstTransducer::new_copy(self.net(*def))?;
            tmp.set_name(name);
            outstream.write(&mut tmp)?;
        }
        outstream.close();
        self.prompt();
        Ok(())
    }

    // @brief Save all transducers in stack to @a outfile
    pub fn write_stack(&mut self, outfilename: &str) -> CmdResult {
        let top = self.top()?;
        self.check_filename(outfilename)?;

        let top_type = self.net(top).get_type();
        let mut outstream = if !outfilename.is_empty() {
            HfstOutputStream::new_filename(outfilename, top_type, true)?
        } else {
            HfstOutputStream::new(top_type, true)?
        };
        let ids: Vec<NetId> = self.stack.clone();
        for t in ids.iter() {
            outstream.write(self.net_mut(*t))?;
        }
        outstream.close();
        self.prompt();
        Ok(())
    }

    // @brief Read properties from @a indata, one per line
    // @todo HFST automata do not support properties
    pub fn read_props(&mut self, indata: &str) -> CmdResult {
        for line in indata.split('\n').filter(|l| !l.is_empty()) {
            self.add_prop_line(line)?;
        }
        self.prompt();
        Ok(())
    }

    // @brief Read prolog form transducer from @a indata
    pub fn read_prolog(&mut self, _indata: &str) -> CmdResult {
        Err(CommandError::not_supported("read prolog"))
    }

    // @brief Read spaced form transducer from @a infile
    pub fn read_spaced_from_file(&mut self, filename: &str) -> CmdResult {
        self.read_text_or_spaced(filename, true)
    }

    // @brief Read spaced form transducer from @a indata
    pub fn read_spaced(&mut self, _indata: &str) -> CmdResult {
        Err(CommandError::not_supported(
            "read spaced-text from inline text",
        ))
    }

    // @brief Read text form transducer from @a infile
    pub fn read_text_from_file(&mut self, filename: &str) -> CmdResult {
        self.read_text_or_spaced(filename, false)
    }

    // @brief Read text form transducer from @a indata
    pub fn read_text(&mut self, _indata: &str) -> CmdResult {
        Err(CommandError::not_supported("read text from inline text"))
    }

    // @brief Read lexicons from @a infile
    pub fn read_lexc_from_file(&mut self, filename: &str) -> CmdResult {
        self.check_filename(filename)?;

        if self.variables["lexc-with-flags"] == "ON" {
            self.lexc.set_with_flags(true);
            if self.variables["lexc-minimize-flags"] == "ON" {
                self.lexc.set_minimize_flags(true);
                if self.variables["lexc-rename-flags"] == "ON" {
                    self.lexc.set_rename_flags(true);
                }
            }
        }

        // [spec:hfst:req:xfst-cmd.io-errors]
        let indata = std::fs::read_to_string(filename).map_err(|e| {
            CommandError::new(format!("could not read lexc file '{}': {}", filename, e))
        })?;

        if self.has_lexc_been_read {
            self.lexc.reset();
        } else {
            self.has_lexc_been_read = true;
        }

        let Some(mut t) = self.lexc.compile(&indata) else {
            return Err(CommandError::new(format!(
                "could not compile '{}' as lexc",
                filename
            )));
        };

        t.optimize_with_config(&self.engine_config)?;
        let t = self.alloc_net(t);
        self.stack.push(t);
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    // @brief Read a transducer in att format from file @a filename
    pub fn read_att_from_file(&mut self, filename: &str) -> CmdResult {
        self.check_filename(filename)?;
        let infile = std::fs::File::open(filename).map_err(|e| {
            CommandError::new(format!("could not read att file '{}': {}", filename, e))
        })?;
        let mut reader = std::io::BufReader::new(infile);

        let att_eps = self.variables["att-epsilon"].clone();
        let epsilon = if att_eps == "@0@ | @_EPSILON_SYMBOL_@" {
            crate::hfst_symbol_defs::internal_epsilon
        } else {
            att_eps.as_str()
        };
        let r = HfstTransducer::<B>::read_in_att_format_file(&mut reader, epsilon, false).map_err(
            |e| CommandError::new(format!("'{}' is not valid att format: {}", filename, e)),
        )?;
        let net = self.alloc_net(r);
        let cfg = self.engine_config;
        self.net_mut(net).optimize_with_config(&cfg)?;
        self.stack.push(net);
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.check-filename-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.check-filename-fn]
    pub fn check_filename(&self, filename: &str) -> CmdResult {
        if self.restricted_mode && (filename.contains('/') || filename.contains('\\')) {
            return Err(CommandError::new(
                "restricted mode (--restricted-mode) allows reads and writes only in the current directory, so a filename cannot contain a path separator",
            ));
        }
        Ok(())
    }

    // @brief Read strings (with or without spaces between the symbols,
    // as defined by \a spaces) from \a infile, disjunct them into
    // a single transducer and push it to the stack.
    fn read_text_or_spaced(&mut self, filename: &str, spaces: bool) -> CmdResult {
        self.check_filename(filename)?;
        // [spec:hfst:req:xfst-cmd.io-errors]
        let infile = std::fs::File::open(filename)
            .map_err(|e| CommandError::new(format!("could not open file '{}': {}", filename, e)))?;

        let tmp: NetId = self.alloc_net(HfstTransducer::new());
        let mcs: Vec<Symbol> = Vec::new(); // no multichar symbols
        // [spec:hfst:def:xfst-compiler.hfst.xfst.tok-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.tok-fn]
        let tok = crate::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer::new(
            &mcs,
            crate::hfst_symbol_defs::internal_epsilon,
        )?;
        let reader = std::io::BufReader::new(infile);

        for line in reader.lines() {
            let line =
                line.map_err(|e| CommandError::new(format!("reading '{}': {}", filename, e)))?;
            let line = self.remove_newline(line);
            let spv = tok.tokenize_pair_string(&line, spaces)?;
            // [spec:hfst:def:xfst-compiler.hfst.xfst.line-tr-fn]
            // [spec:hfst:sem:xfst-compiler.hfst.xfst.line-tr-fn]
            let line_tr = HfstTransducer::new_string_pair_vector(&spv)?;
            self.net_mut(tmp).disjunct(&line_tr, true)?;
        }

        let cfg = self.engine_config;
        self.net_mut(tmp).minimize_with_config(&cfg)?; // a trie is easily minimizable
        self.stack.push(tmp);
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.to-filename-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.to-filename-fn]
fn to_filename(file: Option<&str>) -> &str {
    match file {
        None => "<stdin>",
        Some(f) => f,
    }
}

fn is_epsilon(s: &str) -> bool {
    s == crate::hfst_symbol_defs::internal_epsilon || s == "@0@"
}

/// A symbol as the text readers expect it: ':', space and backslash would
/// otherwise be read as a pair separator, a symbol break and an escape.
fn escape_text_symbol(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, ':' | ' ' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
