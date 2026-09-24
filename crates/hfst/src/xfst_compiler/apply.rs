//! Lookup: apply up, apply down and apply med, and lookup optimization.

use super::*;

static APPLY_END_STRING: &str = "<ctrl-d>";

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // @brief Perform lookdowns on top of the stack, one per line
    // @todo lookdown is missing from HFST
    pub fn apply_up(&mut self, indata: &str) -> crate::error::Result<&mut Self> {
        // strtok splits on '\n' and skips empty tokens.
        for line in indata.split('\n').filter(|s| !s.is_empty()) {
            if line == APPLY_END_STRING {
                break;
            }
            self.apply_up_line(line)?;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Perform lookups on top of the stack, one per line
    // @todo lookup is missing from HFST
    pub fn apply_down(&mut self, indata: &str) -> crate::error::Result<&mut Self> {
        // strtok splits on '\n' and skips empty tokens.
        for line in indata.split('\n').filter(|s| !s.is_empty()) {
            if line == APPLY_END_STRING {
                break;
            }
            self.apply_down_line(line)?;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Perform lookmeds on top of the stack, one per line
    // @todo lookmed is missing from HFST
    pub fn apply_med(&mut self, indata: &str) -> &mut Self {
        // strtok splits on '\n' and skips empty tokens.
        for line in indata.split('\n').filter(|s| !s.is_empty()) {
            self.apply_med_line(line);
        }
        self.prompt();
        self
    }

    pub fn lookup_optimize(&mut self) -> crate::error::Result<&mut Self> {
        if self.stack.is_empty() {
            // EMPTY_STACK
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }

        let t = *self.stack.last().expect("stack non-empty, checked above");
        let t_type = self.net(t).get_type();

        let to_format: ImplementationType;
        if t_type == ImplementationType::HFST_OL_TYPE
            || t_type == ImplementationType::HFST_OLW_TYPE
            || t_type == ImplementationType::THFST_TYPE
        {
            info!("Network is already optimized for lookup.");
            self.prompt();
            return Ok(self);
        } else if t_type == ImplementationType::TROPICAL_OPENFST_TYPE {
            to_format = ImplementationType::HFST_OLW_TYPE;
        } else {
            to_format = ImplementationType::HFST_OL_TYPE;
        }

        // The stack is monomorphically typed as 'B'
        // ([dec:hfst:monomorphic-backends]); the former in-place
        // 'convert(to_format)' of every stack entry cannot change the backend
        // type parameter, so the optimized-lookup conversion is unsupported.
        self.diag_warning(&format!(
            "cannot convert transducer type from {} to {}: the stack is monomorphically typed, ignoring 'lookup-optimize'",
            crate::hfst_data_types::implementation_type_to_format(t_type),
            crate::hfst_data_types::implementation_type_to_format(to_format)
        ));

        self.prompt();
        Ok(self)
    }

    pub fn remove_optimization(&mut self) -> crate::error::Result<&mut Self> {
        if self.stack.is_empty() {
            // EMPTY_STACK
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }
        let t = *self.stack.last().expect("stack non-empty, checked above");
        let t_type = self.net(t).get_type();

        if t_type != ImplementationType::HFST_OL_TYPE
            && t_type != ImplementationType::HFST_OLW_TYPE
            && t_type != ImplementationType::THFST_TYPE
        {
            info!("Network is already in ordinary format.");
            self.prompt();
            return Ok(self);
        }

        // Unreachable under a monomorphically typed stack
        // ([dec:hfst:monomorphic-backends]): 'B: AlgebraBackend' excludes the
        // OL backends, so the early return above is always taken. The warning
        // logic is kept 1:1; the former in-place 'convert' loop cannot exist
        // (the backend is the type parameter).
        if self.verbose {
            debug!(
                "converting transducer type from {} to {}, this might take a while...",
                crate::hfst_data_types::implementation_type_to_format(t_type),
                crate::hfst_data_types::implementation_type_to_format(B::TYPE)
            );
            if !crate::hfst_transducer::is_safe_conversion(t_type, B::TYPE) {
                self.diag_warning(
                    "converting from weighted to unweighted format, loss of information is possible",
                );
            }
        }

        self.prompt();
        Ok(self)
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-apply-prompt-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-apply-prompt-fn]
    // @brief Get the prompt that is used when applying up or down
    // (as specified by \a direction).
    fn get_apply_prompt(&mut self, direction: ApplyDirection) -> String {
        if !self.verbose {
            return String::new();
        }
        if direction == ApplyDirection::APPLY_UP_DIRECTION {
            return "apply up> ".to_string();
        } else if direction == ApplyDirection::APPLY_DOWN_DIRECTION {
            return "apply down> ".to_string();
        }
        String::new()
    }

    // @brief Perform lookup on the top transducer using strings in \a infile.
    // \a direction specifies whether apply is done on input (up) or output (down)
    // side. The results are printed to standard output.
    fn apply(
        &mut self,
        indata: &str,
        direction: ApplyDirection,
    ) -> crate::error::Result<&mut Self> {
        // The C++ overload read lines from a FILE*; here lines come from stdin
        // via 'read_prompted_line' so the 'indata' source is unused.
        let _ = indata;

        if self.stack.is_empty() {
            // EMPTY_STACK
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }
        let top = *self.stack.last().expect("stack non-empty, checked above");
        // number of cycles needs to be limited for an infinitely ambiguous ol
        // transducer because it doesn't support
        // is_lookup_infinitely_ambiguous(const string &)
        let mut ol_cutoff: usize = parse_size(&self.variables["lookup-cycle-cutoff"]);

        // Owned inverted copy for apply-up; None means operate on the shared top.
        let mut owned_t: Option<HfstTransducer<B>> = None;
        // Basic transducer used for ordinary (non-OL) lookups.
        let mut fsm: Option<HfstBasicTransducer> = None;

        if direction == ApplyDirection::APPLY_UP_DIRECTION {
            let ty = self.net(top).get_type();
            if ty == ImplementationType::HFST_OL_TYPE
                || ty == ImplementationType::HFST_OLW_TYPE
                || ty == ImplementationType::THFST_TYPE
            {
                self.diag_warning(
                    "operation not supported for optimized lookup format; 'remove-optimization' converts the network back to ordinary format",
                );
                self.prompt();
                return Ok(self);
            }

            // lookdown not yet implemented in HFST
            if self.verbose {
                self.diag_warning(
                    "apply up on this format is done by inverting and applying down; invert and minimize the top network yourself to avoid the cost",
                );
            }
            let mut c = HfstTransducer::new_copy(self.net(top))?;
            // the user has been warned for possible slow performance
            c.invert()?.minimize_with_config(&self.engine_config)?;
            owned_t = Some(c);
        }

        let work_type = match &owned_t {
            Some(c) => c.get_type(),
            None => self.net(top).get_type(),
        };

        // The OL fast-lookup arm is unreachable under a monomorphically typed
        // stack ([dec:hfst:monomorphic-backends]): 'B: AlgebraBackend'
        // excludes the OL backends, so 'work_type' is never OL/OLW and lookup
        // always goes through the basic transducer.
        if work_type != ImplementationType::HFST_OL_TYPE
            && work_type != ImplementationType::HFST_OLW_TYPE
            && work_type != ImplementationType::THFST_TYPE
        {
            fsm = Some(match &owned_t {
                Some(c) => HfstBasicTransducer::from_transducer(c),
                None => HfstBasicTransducer::from_transducer(self.net(top)),
            });
        }

        // prompt is printed only when reading from the user (always stdin here)
        let promptstr: String = if self.verbose {
            self.get_apply_prompt(direction)
        } else {
            String::new()
        };

        let ind = self.current_history_index(); // readline history to return to

        // get lines from stdin..
        loop {
            let line_opt = self.read_prompted_line(&promptstr);
            // .. until end of file...
            match line_opt {
                None => {
                    // the next command must start on a fresh line
                    println!();
                    break;
                }
                Some(line) => {
                    let line = self.remove_newline(line);
                    // .. or until special end string
                    if line == APPLY_END_STRING {
                        break;
                    }
                    // perform lookup/lookdown (the OL 'self.lookup' arm is
                    // unreachable: 'fsm' is always built above)
                    if let Some(fsm_ref) = fsm.as_ref() {
                        self.lookup_basic(&line, fsm_ref);
                    }
                }
            }
        }

        // ignore all readline history given to the apply command
        self.ignore_history_after_index(ind);

        self.prompt();
        Ok(self)
    }

    // (The former OL-only 'fn lookup' is gone: its 'lookup_fd_string' /
    // 'lookup_string' surface exists only on the OL instantiations, which
    // 'B: AlgebraBackend' excludes; every apply path uses 'lookup_basic'.)

    fn lookup_basic(&mut self, line: &str, t: &HfstBasicTransducer) -> &mut Self {
        let token = trim_whitespace(line);

        let alpha = t.get_input_symbols();
        let mut tok = crate::hfst_tokenizer::HfstTokenizer::new();
        for it in alpha.iter() {
            tok.add_multichar_symbol(it);
        }
        // XXX: seting for splitc-chars ?
        let lookup_path: Vec<Symbol> = tok.tokenize_one_level(&token, false);

        let mut cutoff: usize = usize::MAX; // (size_t)-1
        if t.is_lookup_infinitely_ambiguous_string_vector(
            &lookup_path,
            self.variables["obey-flags"] == "ON",
        ) {
            cutoff = parse_size(&self.variables["lookup-cycle-cutoff"]);
            if self.verbose {
                self.diag_warning(&format!(
                    "lookup is infinitely ambiguous, limiting the number of cycles to {}",
                    cutoff
                ));
            }
        }

        let mut results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();

        if self.variables["maximum-weight"] == "OFF" {
            t.lookup(
                &lookup_path,
                &mut results,
                Some(cutoff),
                None,
                -1, /*max_number*/
                self.variables["obey-flags"] == "ON",
            );
        } else {
            let max_weight: f32 = string_to_float(&self.variables["maximum-weight"]);
            t.lookup(
                &lookup_path,
                &mut results,
                Some(cutoff),
                Some(&max_weight),
                -1, /*max_number*/
                self.variables["obey-flags"] == "ON",
            );
        }

        let mut printed = false; // if anything was printed

        if self.variables["print-pairs"] == "OFF" {
            let paths = extract_output_paths(&results);
            let mut out = std::io::stdout();
            printed = self.print_paths_one(&paths, &mut out, -1);
        } else {
            let mut out = std::io::stdout();
            printed = self.print_paths_two(&results, &mut out, -1);
        }

        if !printed {
            println!("???");
        }
        self
    }

    // apply_down_line -> apply_up_line
    fn apply_up_line(&mut self, line: &str) -> crate::error::Result<&mut Self> {
        // GET_TOP(t)
        let Some(t) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        // lookdown not yet implemented in HFST
        if self.verbose {
            self.diag_warning(
                "apply up on this format is done by inverting and applying down; invert and minimize the top network yourself to avoid the cost",
            );
        }
        let mut copy = HfstTransducer::new_copy(self.net(t))?;
        // the user has been warned for possible slow performance
        copy.invert()?.minimize_with_config(&self.engine_config)?;
        let fsm = HfstBasicTransducer::from_transducer(&copy);
        self.lookup_basic(line, &fsm);
        Ok(self)
    }

    // apply_up_line -> apply_down_line
    fn apply_down_line(&mut self, line: &str) -> crate::error::Result<&mut Self> {
        if self.stack.is_empty() {
            // EMPTY_STACK
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }
        let t = *self.stack.last().expect("stack non-empty, checked above");
        // The OL fast-lookup tail is unreachable under a monomorphically
        // typed stack ([dec:hfst:monomorphic-backends]): 'B: AlgebraBackend'
        // excludes the OL backends, so this non-OL branch is always taken.
        // hfst_fprintf(warnstream_, "lookup might be slow, consider
        // 'convert net'\n");
        let fsm = HfstBasicTransducer::from_transducer(self.net(t));
        Ok(self.lookup_basic(line, &fsm))
    }

    fn apply_med_line(&mut self, line: &str) -> &mut Self {
        let _ = line;
        self.diag_warning("apply med is not implemented; no output was produced");
        self
    }
}

fn trim_whitespace(s: &str) -> String {
    let bytes = s.as_bytes();
    let is_space = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    let mut start = 0;
    while start < bytes.len() && is_space(bytes[start]) {
        start += 1;
    }
    let mut end = bytes.len();
    while end > start && is_space(bytes[end - 1]) {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.string-to-float-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.string-to-float-fn]
fn string_to_float(str: &str) -> f32 {
    // Mirror 'std::istringstream >> float': skip leading whitespace, read the
    // leading numeric prefix, and yield 0 if nothing parses.
    let t = str.trim_start();
    let bytes = t.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        let mut e = end + 1;
        if e < bytes.len() && (bytes[e] == b'+' || bytes[e] == b'-') {
            e += 1;
        }
        let mut saw = false;
        while e < bytes.len() && bytes[e].is_ascii_digit() {
            e += 1;
            saw = true;
        }
        if saw {
            end = e;
        }
    }
    t[..end].parse::<f32>().unwrap_or(0.0)
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.extract-output-paths-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-output-paths-fn]
fn extract_output_paths(paths: &HfstTwoLevelPaths) -> HfstOneLevelPaths {
    let mut retval = HfstOneLevelPaths::new();
    for it in paths.iter() {
        let mut new_path: Vec<Symbol> = Vec::new();
        let path = &it.second;
        for p in path.iter() {
            if p.1 != "@0@" && p.1 != "@_EPSILON_SYMBOL_@" {
                if p.1 == "@_UNKNOWN_SYMBOL_@" {
                    new_path.push(Symbol::new_static("?"));
                } else {
                    new_path.push(p.1.clone());
                }
            }
        }
        retval.insert(crate::hfst_data_types::HfstOneLevelPath {
            first: it.first,
            second: new_path,
        });
    }
    retval
}
