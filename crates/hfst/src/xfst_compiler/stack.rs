//! Stack commands: the top of the stack, push, pop, turn, rotate, and
//! pushing a compiled regex.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.top-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.top-fn]
    /// The network on top of the stack.
    pub(super) fn top(&self) -> CmdResult<NetId> {
        self.stack
            .last()
            .copied()
            .ok_or_else(CommandError::empty_stack)
    }

    /// Fail unless the stack holds at least two networks.
    pub(super) fn require_two(&self) -> CmdResult {
        if self.stack.len() < 2 {
            return Err(CommandError::need_two());
        }
        Ok(())
    }

    pub(super) fn print_transducer_info(&mut self) {
        if self.verbose && !self.stack.is_empty() {
            let top = *self.stack.last().expect("stack non-empty, checked above");
            {
                let t = self.net(top);
                if t.get_type() != B::TYPE {
                    return;
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
    }

    // @brief Name top of stack
    // @todo HFST automata do not remember their names
    pub fn name_net(&mut self, name: &str) -> CmdResult {
        let t = self.top()?;
        self.net_mut(t).set_name(name);
        self.names.insert(Symbol::new(name), t);
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    // @brief Get current stack of compiler
    pub fn get_stack(&self) -> &Vec<NetId> {
        &self.stack
    }

    // @brief Clear stack
    pub fn clear(&mut self) {
        self.stack.clear();
        self.prompt();
    }

    // @brief Pop stack
    pub fn pop(&mut self) -> CmdResult {
        self.stack.pop().ok_or_else(CommandError::empty_stack)?;
        self.prompt();
        Ok(())
    }

    // @brief Push definition on stack
    pub fn push(&mut self, name: &str) -> CmdResult {
        let Some(&def) = self.definitions.get(name) else {
            return Err(self.unknown_definition(name));
        };
        let t = HfstTransducer::new_copy(self.net(def))?;
        let t = self.alloc_net(t);
        self.stack.push(t);
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    // @brief Push every definition on the stack
    pub fn push_latest(&mut self) -> CmdResult {
        let defs: Vec<NetId> = self.definitions.values().copied().collect();
        for def in defs {
            let t = HfstTransducer::new_copy(self.net(def))?;
            let t = self.alloc_net(t);
            self.stack.push(t);
        }

        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    // @brief Reverse stack
    pub fn turn(&mut self) {
        self.stack.reverse();
        self.print_transducer_info();
        self.prompt();
    }

    // @brief Move top of stack to bottom
    pub fn rotate(&mut self) {
        if let Some(top) = self.stack.pop() {
            self.stack.insert(0, top);
        }
        self.print_transducer_info();
        self.prompt();
    }
}
