//! Reading and writing networks, properties and definitions from and to files.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // @brief Read lexicons from @a indata
    pub fn read_lexc(&mut self, indata: &str) -> &mut Self {
        // The C++ declares read_lexc(const char* indata) but provides no
        // definition (it is never invoked; the parser uses read_lexc_from_file).
        // Mirror that by doing nothing and returning self.
        self
    }

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
        // CHECK_FILENAME equivalent: if (!check_filename(filename)) return;
        if !self.check_filename(filename.unwrap_or("")) {
            return t.into_typed::<B>();
        }

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
    fn open_hfst_input_stream(&mut self, filename: &str) -> Option<HfstInputStream<'static>> {
        // assert(infilename != NULL): filename is always a valid &str here.
        if !self.check_filename(filename) {
            return None;
        }

        match crate::hfst_data_types::open_file(filename, "r") {
            Err(e) => {
                // The io error names the actual cause (missing, unreadable, a
                // directory); 'could not open' alone leaves the user guessing.
                self.diag_error(&format!("could not open file '{}': {}", filename, e));
                self.flush();
                self.xfst_fail();
                return None;
            }
            Ok(infile) => {
                // close the probe handle (the real read goes through
                // HfstInputStream below); dropping cannot fail.
                drop(infile);
            }
        }

        // C++: try { new HfstInputStream(infilename) } catch (NotTransducerStreamException)
        match HfstInputStream::new_filename(filename) {
            Ok(instream) => Some(instream),
            Err(_) => {
                self.diag_error_with_notes(
                    &format!(
                        "unable to read transducers from {}",
                        to_filename(Some(filename))
                    ),
                    &[String::from(
                        "the file exists but is not in a transducer format this compiler reads",
                    )],
                );
                self.flush();
                self.xfst_fail();
                None
            }
        }
    }

    // @brief Read transducers from file \a infilename and either push
    // them to the stack (if \a definitions is false) or add them as definitions
    // (if \a definitions is true).
    fn load_stack_or_definitions(&mut self, infilename: &str, definitions: bool) -> &mut Self {
        // CHECK_FILENAME(infilename)
        if !self.check_filename(infilename) {
            return self;
        }
        // Try to open the stream to file infilename
        // IF_NULL_PROMPT_AND_RETURN_THIS(instream)
        let Some(mut instream) = self.open_hfst_input_stream(infilename) else {
            if self.variables["quit-on-fail"] == "ON" {
                self.fail_flag = true;
            }
            self.prompt();
            return self;
        };

        // Read transducers from stream
        while instream.is_good() {
            let t = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    self.diag_error(&format!("reading '{}': {}", infilename, e));
                    if self.variables["quit-on-fail"] == "ON" {
                        self.fail_flag = true;
                    }
                    break;
                }
            };

            // Convert transducer format, if needed
            let t = match self.convert_to_common_format(t, Some(infilename)) {
                Ok(t) => t,
                Err(e) => {
                    self.diag_error(&format!("converting '{}': {}", infilename, e));
                    if self.variables["quit-on-fail"] == "ON" {
                        self.fail_flag = true;
                    }
                    break;
                }
            };
            let t: NetId = self.alloc_net(t);

            // Add transducer as definition..
            if definitions {
                let t_type = self.net(t).get_type();
                if t_type == ImplementationType::HFST_OL_TYPE
                    || t_type == ImplementationType::HFST_OLW_TYPE
                    || t_type == ImplementationType::THFST_TYPE
                {
                    self.diag_error("cannot load optimized lookup transducers as definitions");
                    self.flush();
                    break;
                }
                self.add_loaded_definition(t);
            }
            // ..or push it to stack.
            else {
                self.stack.push(t);
                self.print_transducer_info();
            }
        }

        instream.close();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Add a transducer definition with name given by 't.get_name()'
    // and value \a t.
    fn add_loaded_definition(&mut self, t: NetId) -> &mut Self {
        let def_name = self.net(t).get_name();
        if def_name.is_empty() {
            self.diag_warning("loaded transducer definition has no name, skipping it");
            return self;
        }
        if self.definitions.contains_key(def_name.as_str()) {
            self.diag_warning(&format!(
                "a definition named '{}' already exists, overwriting it",
                def_name
            ));
            // the previous net stays in the arena; the map entry is replaced.
            self.definitions.remove(def_name.as_str());
        }
        self.definitions.insert(Symbol::from(def_name), t);
        self
    }

    fn add_prop_line(&mut self, line: &str) -> &mut Self {
        // Split the line into name (up to the first ':') and value.
        let bytes = line.as_bytes();
        let mut p = 0usize;
        while p < bytes.len() && bytes[p] != b':' {
            p += 1;
        }
        let name = line[..p].to_string();
        if p >= bytes.len() {
            // *p == '\0': no colon in line (assert(*p != '\0') is a no-op in release)
            error!("no colon in line");
        }
        // skip the colon, then skip leading whitespace of the value
        let mut q = if p < bytes.len() { p + 1 } else { p };
        while q < bytes.len()
            && (bytes[q] == b' '
                || bytes[q] == b'\t'
                || bytes[q] == b'\n'
                || bytes[q] == 0x0b
                || bytes[q] == 0x0c
                || bytes[q] == b'\r')
        {
            q += 1;
        }
        let value = line[q..].to_string();
        self.properties.insert(name, value);
        self
    }

    // @brief Write top transducer in att format to @a outfile
    pub fn write_att(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(tmp))
            .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        fsm.write_in_att_format_os(oss, self.variables["print-weight"] == "ON");
        self.flush();
        self.prompt();
        self
    }

    // @brief Load regex macros from file
    // @todo Definition names cannot be stored in HFST automata binaries
    pub fn load_definitions(&mut self, infilename: &str) -> &mut Self {
        // CHECK_FILENAME(infilename)
        if !self.check_filename(infilename) {
            return self;
        }
        self.load_stack_or_definitions(infilename, true /* definitions*/)
    }

    // @brief Load stack from file
    pub fn load_stack(&mut self, infilename: &str) -> &mut Self {
        // CHECK_FILENAME(infilename)
        if !self.check_filename(infilename) {
            return self;
        }
        self.load_stack_or_definitions(infilename, false)
    }

    // @brief Add properties from text, one property per line
    // @todo properties cannot be stored in HFST automata
    pub fn add_props(&mut self, indata: &str) -> &mut Self {
        for line in indata.split('\n').filter(|l| !l.is_empty()) {
            self.add_prop_line(line);
        }
        self.prompt();
        self
    }

    // @brief Save @a name network in dot form in @a outfile
    pub fn write_dot_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = oss;
        if self.stack.is_empty() {
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }
        let mut outfile = match std::fs::File::create(name) {
            Ok(f) => f,
            Err(e) => {
                self.diag_error(&format!("could not open file '{}': {}", name, e));
                self.xfst_fail();
                self.prompt();
                return self;
            }
        };
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        crate::hfst_print_dot::print_dot_os(&mut outfile, self.net_mut(tmp));
        self.prompt();
        self
    }

    // @brief Save top networks dot form in @a outfile
    pub fn write_dot(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        if self.stack.is_empty() {
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        crate::hfst_print_dot::print_dot_os(oss, self.net_mut(tmp));
        let _ = oss.flush();
        self.prompt();
        self
    }

    // @brief Save top networks prolog form in @a outfile
    pub fn write_prolog(
        &mut self,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        if self.stack.is_empty() {
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }
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
                // separator
                let _ = writeln!(oss);
            }
        }
        let _ = oss.flush();
        self.prompt();
        Ok(self)
    }

    // @brief Save top networks spaced paths form in @a outfile
    pub fn write_spaced(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing write spaced");
        let _ = oss.flush();
        self.prompt();
        self
    }

    // @brief Save top networks paths form in @a outfile
    pub fn write_text(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing write text");
        let _ = oss.flush();
        self.prompt();
        self
    }

    // @brief Save function @a name in @a outfile
    // @todo HFST does not support function macros in automata
    pub fn write_function(&mut self, name: &str, outfilename: &str) -> &mut Self {
        /*if (outfile == 0)
        {
          hfst_fprintf(outstream_, "%10s: %p\n", name, functions_[name]);
          }*/
        let _ = (name, outfilename);
        self.prompt();
        self
    }

    // @brief Save definition @a name in @a outfile
    // @todo HFST does not support saving name of definition in file
    pub fn write_definition(
        &mut self,
        name: &str,
        outfilename: &str,
    ) -> crate::error::Result<&mut Self> {
        if !self.definitions.contains_key(name) {
            self.diag_unknown_definition(name);
            self.prompt();
            return Ok(self);
        }

        let mut outstream = if !outfilename.is_empty() {
            HfstOutputStream::new_filename(outfilename, B::TYPE, true)?
        } else {
            HfstOutputStream::new(B::TYPE, true)?
        };
        let def_ptr = self.definitions[name];
        let mut tmp = HfstTransducer::new_copy(self.net(def_ptr))?;
        if self.variables["name-nets"] == "ON" {
            tmp.set_name(name);
        }
        outstream.write(&mut tmp)?;
        outstream.close();
        self.prompt();
        Ok(self)
    }

    // @brief Save all definitions in @a outfile
    // @todo HFST does not support saving name of definition in file
    pub fn write_definitions(&mut self, outfilename: &str) -> crate::error::Result<&mut Self> {
        if self.definitions.is_empty() {
            self.diag_warning("no networks are defined, nothing to save");
            self.prompt();
            return Ok(self);
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
        Ok(self)
    }

    // @brief Save all transducers in stack to @a outfile
    pub fn write_stack(&mut self, outfilename: &str) -> crate::error::Result<&mut Self> {
        if self.stack.is_empty() {
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            return Ok(self);
        }

        if !self.check_filename(outfilename) {
            return Ok(self);
        }

        let top_type = self
            .net(*self.stack.last().expect("stack non-empty, checked above"))
            .get_type();
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
        Ok(self)
    }

    // @brief Read properties from @a indata, one per line
    // @todo HFST automata do not support properties
    pub fn read_props(&mut self, indata: &str) -> &mut Self {
        for line in indata.split('\n').filter(|l| !l.is_empty()) {
            self.add_prop_line(line);
        }
        self.prompt();
        self
    }

    // @brief Read prolog form transducer from @a indata
    pub fn read_prolog(&mut self, indata: &str) -> &mut Self {
        let _ = indata;
        self.diag_warning("read prolog is not implemented; nothing was pushed");
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Read spaced form transducer from @a infile
    pub fn read_spaced_from_file(&mut self, filename: &str) -> crate::error::Result<&mut Self> {
        if !self.check_filename(filename) {
            return Ok(self);
        }
        self.read_text_or_spaced(filename, true) // spaces are used
    }

    // @brief Read spaced form transducer from @a indata
    pub fn read_spaced(&mut self, indata: &str) -> &mut Self {
        let _ = indata;
        self.diag_warning("read spaced-text is not implemented; nothing was pushed");
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Read text form transducer from @a infile
    pub fn read_text_from_file(&mut self, filename: &str) -> crate::error::Result<&mut Self> {
        if !self.check_filename(filename) {
            return Ok(self);
        }
        self.read_text_or_spaced(filename, false) // spaces are not used
    }

    // @brief Read text form transducer from @a indata
    pub fn read_text(&mut self, indata: &str) -> &mut Self {
        let _ = indata;
        self.diag_warning("read text is not implemented; nothing was pushed");
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Read lexicons from @a infile
    pub fn read_lexc_from_file(&mut self, filename: &str) -> crate::error::Result<&mut Self> {
        if !self.check_filename(filename) {
            return Ok(self);
        }

        if self.variables["lexc-with-flags"] == "ON" {
            self.lexc.set_with_flags(true);
            if self.variables["lexc-minimize-flags"] == "ON" {
                self.lexc.set_minimize_flags(true);
                if self.variables["lexc-rename-flags"] == "ON" {
                    self.lexc.set_rename_flags(true);
                }
            }
        }

        // The C++ 'lexc.parse(FILE*)' path is replaced by the AST-walk
        // 'lexc.compile(&str)', so the file contents are read into a string.
        let indata = match std::fs::read_to_string(filename) {
            Ok(s) => s,
            Err(e) => {
                self.diag_error(&format!("could not read lexc file '{}': {}", filename, e));
                self.xfst_fail();
                self.prompt();
                return Ok(self);
            }
        };

        if self.has_lexc_been_read {
            self.lexc.reset();
        } else {
            self.has_lexc_been_read = true;
        }

        let Some(mut t) = self.lexc.compile(&indata) else {
            self.diag_error(&format!("could not compile '{}' as lexc", filename));
            self.xfst_fail();
            self.prompt();
            return Ok(self);
        };

        t.optimize_with_config(&self.engine_config)?;
        let t = self.alloc_net(t);
        self.stack.push(t);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Read a transducer in att format from file @a filename
    pub fn read_att_from_file(&mut self, filename: &str) -> crate::error::Result<&mut Self> {
        if !self.check_filename(filename) {
            return Ok(self);
        }
        let infile = match std::fs::File::open(filename) {
            Ok(f) => f,
            Err(e) => {
                self.diag_error(&format!("could not read att file '{}': {}", filename, e));
                self.xfst_fail();
                self.prompt();
                return Ok(self);
            }
        };
        let mut reader = std::io::BufReader::new(infile);

        let att_eps = self.variables["att-epsilon"].clone();
        let att_eps_default = att_eps == "@0@ | @_EPSILON_SYMBOL_@";
        let result = if att_eps_default {
            HfstTransducer::<B>::read_in_att_format_file(
                &mut reader,
                crate::hfst_symbol_defs::internal_epsilon,
                false,
            )
        } else {
            HfstTransducer::<B>::read_in_att_format_file(&mut reader, &att_eps, false)
        };
        match result {
            Ok(r) => {
                let net = self.alloc_net(r);
                let cfg = self.engine_config;
                self.net_mut(net).optimize_with_config(&cfg)?;
                self.stack.push(net);
                self.print_transducer_info();
            }
            Err(e) => {
                self.diag_error(&format!("'{}' is not valid att format: {}", filename, e));
                self.xfst_fail();
            }
        }
        self.prompt();
        Ok(self)
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.check-filename-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.check-filename-fn]
    pub fn check_filename(&mut self, filename: &str) -> bool {
        if self.restricted_mode {
            let fname = filename.to_string();
            if fname.contains('/') || fname.contains('\\') {
                self.diag_warning(
                    "restricted mode (--restricted-mode) allows reads and writes only in the current directory, so a filename cannot contain a path separator",
                );
                self.xfst_lesser_fail();
                self.prompt();
                return false;
            }
        }
        self.prompt();
        true
    }

    // @brief Read strings (with or without spaces between the symbols,
    // as defined by \a spaces) from \a infile, disjunct them into
    // a single transducer and push it to the stack.
    fn read_text_or_spaced(
        &mut self,
        filename: &str,
        spaces: bool,
    ) -> crate::error::Result<&mut Self> {
        if !self.check_filename(filename) {
            return Ok(self);
        }
        let infile = match std::fs::File::open(filename) {
            Ok(f) => f,
            Err(e) => {
                self.diag_error(&format!("could not open file '{}': {}", filename, e));
                self.xfst_fail();
                self.prompt();
                return Ok(self);
            }
        };

        let tmp: NetId = self.alloc_net(HfstTransducer::new());
        let mcs: Vec<Symbol> = Vec::new(); // no multichar symbols
        // [spec:hfst:def:xfst-compiler.hfst.xfst.tok-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.tok-fn]
        let tok = crate::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer::new(
            &mcs,
            crate::hfst_symbol_defs::internal_epsilon,
        )?;
        let mut reader = std::io::BufReader::new(infile);

        loop {
            let mut buf = String::new();
            match reader.read_line(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            let line = self.remove_newline(buf);
            let spv = tok.tokenize_pair_string(&line, spaces)?;
            // [spec:hfst:def:xfst-compiler.hfst.xfst.line-tr-fn]
            // [spec:hfst:sem:xfst-compiler.hfst.xfst.line-tr-fn]
            let line_tr = HfstTransducer::new_string_pair_vector(&spv)?;
            self.net_mut(tmp).disjunct(&line_tr, true)?;
        }

        // The file is closed when 'reader' is dropped.

        let cfg = self.engine_config;
        self.net_mut(tmp).minimize_with_config(&cfg)?; // a trie is easily minimizable
        self.stack.push(tmp);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
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
