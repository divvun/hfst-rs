//! Stack commands: the top of the stack, push, pop, turn, rotate, and
//! pushing a compiled regex.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.top-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.top-fn]
    // @brief The topmost transducer in the stack.
    // If empty, print a warning message and return NULL.
    pub(super) fn top(&mut self) -> Option<NetId> {
        if self.stack.is_empty() {
            // EMPTY_STACK
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            self.prompt();
            return None;
        }
        let retval = *self.stack.last().expect("stack non-empty, checked above");
        {
            let t = self.net(retval);
            if t.get_type() == ImplementationType::HFST_OL_TYPE
                || t.get_type() == ImplementationType::HFST_OLW_TYPE
                || t.get_type() == ImplementationType::THFST_TYPE
            {
                self.diag_warning(
                    "operation not supported for optimized lookup format; 'remove-optimization' converts the network back to ordinary format",
                );
                self.prompt();
                return None;
            }
        }
        Some(retval)
    }

    pub(super) fn print_transducer_info(&mut self) -> &mut Self {
        if self.verbose && !self.stack.is_empty() {
            let top = *self.stack.last().expect("stack non-empty, checked above");
            {
                let t = self.net(top);
                if t.get_type() != B::TYPE {
                    return self;
                }
                println!(
                    "? bytes. {} states, {} arcs, ? paths",
                    t.number_of_states(),
                    t.number_of_arcs()
                );
            }
            let print_sigma_on =
                self.variables.get("print-sigma").map(|s| s.as_str()) == Some("ON");
            if print_sigma_on {
                let mut out = std::io::stdout();
                let _ = self.print_sigma(&mut out, false);
            }
        }
        self
    }

    // @brief Name top of stack
    // @todo HFST automata do not remember their names
    pub fn name_net(&mut self, name: &str) -> &mut Self {
        if self.stack.is_empty() {
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            return self;
        }
        let t = *self.stack.last().expect("stack non-empty, checked above");
        self.net_mut(t).set_name(name);
        self.names.insert(Symbol::new(name), t);
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Get current stack of compiler
    pub fn get_stack(&self) -> &Vec<NetId> {
        &self.stack
    }

    // @brief Clear stack
    pub fn clear(&mut self) -> &mut Self {
        self.stack.clear();
        self.latest_regex_compiled = None;
        self.prompt();
        self
    }

    // @brief Pop stack
    pub fn pop(&mut self) -> &mut Self {
        if self.stack.is_empty() {
            println!("Stack is empty.");
        } else {
            self.stack.pop();
        }
        self.prompt();
        self
    }

    // @brief Push definition on stack
    pub fn push(&mut self, name: &str) -> crate::error::Result<&mut Self> {
        if !self.definitions.contains_key(name) {
            println!("no such defined network: '{}'", name);
            self.prompt();
            return Ok(self);
        }

        let def = self.definitions[name];
        let t = HfstTransducer::new_copy(self.net(def))?;
        let t = self.alloc_net(t);
        self.stack.push(t);
        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Push last definition on stack
    pub fn push_latest(&mut self) -> crate::error::Result<&mut Self> {
        let defs: Vec<NetId> = self.definitions.values().copied().collect();
        for def in defs {
            let t = HfstTransducer::new_copy(self.net(def))?;
            let t = self.alloc_net(t);
            self.stack.push(t);
        }

        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Reverse stack
    pub fn turn(&mut self) -> &mut Self {
        self.stack.reverse();
        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Move top of stack to bottom
    pub fn rotate(&mut self) -> &mut Self {
        if self.stack.is_empty() {
            self.prompt();
            return self;
        }

        self.stack.reverse();

        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        self
    }

    /* Compile a regex string starting from \a indata, store the resulting
    transducer to variable XfstCompiler::latest_regex_compiled and
    store the number of characters read from \a indata to \a chars_read.

    This function is used by the xfst lexer to determine where a regex
    starting from \a indata ends. */
    pub fn compile_regex(&mut self, indata: &str, chars_read: &mut u32) -> &mut Self {
        if self.latest_regex_compiled.is_some() {
            self.latest_regex_compiled = None;
        }
        let compiled = self.xre.compile_first(indata, chars_read); // XRE
        self.latest_regex_compiled = compiled.map(|t| self.alloc_net(t));
        self
    }

    // @brief Compile regex of @a indata and save on stack.
    // Actually, the function assumes that the function compile_regex has
    // been called earlier when extracting the portion of input that
    // constitutes the regex \a indata.
    pub fn read_regex(&mut self, indata: &str) -> crate::error::Result<&mut Self> {
        // When calling this function, the regex \a indata should already have
        // been compiled into a transducer which should have been stored to
        // the variable latest_regex_compiled.
        let compiled = self.latest_regex_compiled;
        if let Some(compiled) = compiled {
            let t = HfstTransducer::new_copy(self.net(compiled))?;
            let t = self.alloc_net(t);
            let cfg = self.engine_config;
            self.net_mut(t).optimize_with_config(&cfg)?;
            self.stack.push(t);
            self.print_transducer_info();
        } else {
            self.diag_error(&format!("could not read regex '{}'", indata));
            self.xfst_fail();
        }
        self.prompt();
        Ok(self)
    }
}
