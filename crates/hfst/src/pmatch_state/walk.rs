//! One invocation of one net, and the lookup it performs.

use super::*;

/// One invocation of one net: the net borrowed out of the shared core, plus the
/// local-variable stack that invocation owns.
///
/// The C++ put the stack on the `PmatchTransducer` itself, so a net could only
/// be run once at a time and a re-entrant RTN return had to clone the net to get
/// a second stack (hfst/hfst#354). A walk borrows instead, so re-entering a net
/// costs a `&`.
// [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer]
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
pub(crate) struct PmatchWalk<'a> {
    core: &'a PmatchCore,
    net: &'a PmatchTransducer,
    /// The owning symbol of `net` — the C++ 'this' pointer in our ownership
    /// scheme. The toplevel (name "TOP") has none, so NO_SYMBOL_NUMBER.
    symbol: SymbolNumber,
    local_stack: LocalVariablesStack,
}

#[allow(clippy::too_many_arguments)]
impl<'a> PmatchWalk<'a> {
    /// Enter `net` with a pristine frame — what the C++ got by cloning the net
    /// out of its slot, whose stack was still the one its constructor built.
    pub(super) fn entering(core: &'a PmatchCore, net: &'a PmatchTransducer) -> PmatchWalk<'a> {
        PmatchWalk::resuming(
            core,
            net,
            LocalVariables {
                flag_state: core.flag_state_proto.clone(),
                tape_step: 1,
                max_context_length_remaining: 254,
                context: ContextChecking::none,
                context_placeholder: 0,
                default_symbol_trap: false,
                negative_context_success: false,
                pending_passthrough: false,
            },
        )
    }

    /// Resume `net` from the frame it held when it made an RTN call.
    fn resuming(
        core: &'a PmatchCore,
        net: &'a PmatchTransducer,
        frame: LocalVariables,
    ) -> PmatchWalk<'a> {
        let symbol = if net.name == "TOP" {
            NO_SYMBOL_NUMBER
        } else {
            match core.alphabet.rtn_names.get(&net.name) {
                Some(s) => *s,
                None => NO_SYMBOL_NUMBER,
            }
        };
        PmatchWalk {
            core,
            net,
            symbol,
            local_stack: vec![frame],
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-rtn-fn]
    fn net_for(core: &'a PmatchCore, sym: SymbolNumber) -> &'a PmatchTransducer {
        if sym == NO_SYMBOL_NUMBER {
            core.toplevel
                .as_deref()
                .expect("toplevel present for RTN call")
        } else {
            core.alphabet.rtns[sym as usize]
                .as_deref()
                .expect("RTN slot occupied for RTN call")
        }
    }

    #[inline]
    fn frame(&self) -> &LocalVariables {
        self.local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
    }

    #[inline]
    fn frame_mut(&mut self) -> &mut LocalVariables {
        self.local_stack
            .last_mut()
            .expect("local_stack is non-empty during a match walk")
    }

    // [PORT NOTE / DIVERGENCE hfst/hfst#399, #354]
    // Try to enter a plain-epsilon configuration on the CURRENT DFS PATH. Returns
    // Some(key) if this exact (transducer, target, input_pos, context, tape_step)
    // config is not already an ANCESTOR on the path being explored — the caller
    // must remove the key (via 'epsilon_leave') once it returns from recursing
    // into 'target'. Returns None when re-entering an ancestor: that is a genuine
    // epsilon cycle (e.g. the '0:?*' loop of hfst#399) and must be pruned to
    // terminate.
    //
    // The stack is PATH-scoped (push on descent, pop on backtrack), not a global
    // per-attempt memo. A global memo also prunes CONVERGENT paths —
    // two distinct plain-epsilon branches that meet at the same state (e.g. the
    // 'cat+N' and 'cat+V' analyses of an ambiguous tokeniser converge on a shared
    // final state) — silently dropping every analysis but the first (hfst#354's
    // "missing wordforms"). Path-scoping keeps only true cycles pruned.
    //
    // Only PLAIN epsilon-input arcs (never flag diacritics, never Ins/RTN arcs,
    // which carry their own FdState/tape state) reach this helper, so it cannot
    // swallow legitimate flag or RTN traversal.
    fn epsilon_enter(
        &self,
        input_pos: u32,
        target: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) -> Option<EpsilonVisitKey> {
        let frame = self.frame();
        let key: EpsilonVisitKey = (
            self.symbol,
            target,
            input_pos,
            frame.context as u8,
            frame.tape_step,
        );
        if run
            .epsilon_path
            .iter()
            .rev()
            .any(|ancestor| *ancestor == key)
        {
            return None;
        }
        run.epsilon_path.push(key);
        Some(key)
    }

    // Leave a plain-epsilon configuration entered via 'epsilon_enter', so a
    // sibling branch that later reaches the same state is not mistaken for a
    // cycle. [hfst#399, #354]
    fn epsilon_leave(run: &mut PmatchContainer, key: EpsilonVisitKey) {
        let left = run.epsilon_path.pop();
        debug_assert_eq!(left, Some(key), "epsilon configurations leave in DFS order");
    }

    // ---- the mutually recursive lookup-handling functions ----

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-epsilons-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-epsilons-fn]
    fn take_epsilons(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        let mut i = self.net.make_transition_table_index(i, 0);
        while PmatchTransducer::is_good(i) {
            let input = self.net.transition_input(i);
            if input != 0
                && !self.core.alphabet.is_flag_diacritic(input)
                && !self.core.alphabet.has_rtn_sym(input)
            {
                return;
            }

            let output = self.net.transition_output(i);
            let target = self.net.transition_target(i);
            let old_weight = run.get_weight();
            run.increment_weight(self.net.transition_weight(i));

            if self.checking_context() {
                if self.try_exiting_context(output) {
                    // We've successfully completed a context check
                    let cp = self.frame().context_placeholder;
                    self.get_analyses(cp, tape_pos, target, run);
                    self.local_stack.pop();
                } else if self.frame().negative_context_success {
                    // We've succeeded in a negative context, just back out
                    return;
                } else if self.core.alphabet.is_flag_diacritic(input) {
                    self.take_flag(input, input_pos, tape_pos, i, run);
                } else if self.core.alphabet.has_rtn_sym(input) {
                    let caller = self.symbol;
                    let locals = self.frame().clone();
                    PmatchWalk::entering(self.core, PmatchWalk::net_for(self.core, input))
                        .rtn_call_in_context(input_pos, tape_pos, caller, target, locals, run);
                } else {
                    // Don't alter tapes when checking context
                    // [DIVERGENCE hfst/hfst#399, #354] prune re-entered epsilon
                    // cycles (path-scoped), but keep convergent analyses.
                    if let Some(k) = self.epsilon_enter(input_pos, target, run) {
                        self.get_analyses(input_pos, tape_pos, target, run);
                        Self::epsilon_leave(run, k);
                    }
                }
            } else if input == 0 {
                if run.profile_mode {
                    run.count(output);
                }
                if !self.try_entering_context(output, run) {
                    // no context to enter, regular input epsilon
                    run.tape.write_pair(tape_pos, 0, output);

                    // [DIVERGENCE hfst/hfst#399] An entry/exit/capture arc
                    // mutates entry_stack/captures — state the memo key does
                    // not capture — so it is NOT a "plain" epsilon and is never
                    // pruned; only truly plain epsilon-input arcs are memoized.
                    let plain_epsilon = output
                        != self.core.alphabet.get_special(SpecialSymbol::entry)
                        && output != self.core.alphabet.get_special(SpecialSymbol::exit)
                        && !run.is_capture_tag_sym(output)
                        && !run.is_captured_tag_sym(output);

                    let mut orig_entry_stack_back: u32 = 0;
                    // if it's an entry or exit arc, adjust entry stack
                    if output == self.core.alphabet.get_special(SpecialSymbol::entry) {
                        run.entry_stack.push(input_pos);
                    } else if output == self.core.alphabet.get_special(SpecialSymbol::exit) {
                        orig_entry_stack_back = *run
                            .entry_stack
                            .last()
                            .expect("exit arc has a matching entry on the stack");
                        run.entry_stack.pop();
                    } else if run.is_capture_tag_sym(output) {
                        // if it's a capture tag, remember where we were
                        let capture = Capture {
                            begin: *run
                                .entry_stack
                                .last()
                                .expect("capture tag has a matching entry on the stack"),
                            end: input_pos,
                            name: output,
                        };
                        run.captures.push(capture);
                    } else if run.is_captured_tag_sym(output) {
                        // if it's a captured tag, try each previously
                        // captured sequence
                        let key = run.captured2capture(output);
                        let cap = run.get_longest_matching_capture(key, input_pos);

                        if cap.1 - cap.0 != 0 {
                            let slice: Vec<SymbolNumber> = run.input[cap.0..cap.1].to_vec();
                            run.tape.write_slice(tape_pos, &slice);
                            let span = (cap.1 - cap.0) as u32;
                            self.get_analyses(input_pos + span, tape_pos + span, target, run);
                        }
                        i += 1;
                        run.set_weight(old_weight);
                        continue;
                    }

                    if !plain_epsilon {
                        self.get_analyses(input_pos, tape_pos + 1, target, run);
                    } else if let Some(k) = self.epsilon_enter(input_pos, target, run) {
                        self.get_analyses(input_pos, tape_pos + 1, target, run);
                        Self::epsilon_leave(run, k);
                    }

                    if output == self.core.alphabet.get_special(SpecialSymbol::entry) {
                        run.entry_stack.pop();
                    } else if output == self.core.alphabet.get_special(SpecialSymbol::exit) {
                        run.entry_stack.push(orig_entry_stack_back);
                    } else if run.is_capture_tag_sym(output) {
                        run.captures.pop();
                    }
                } else {
                    self.check_context(input_pos, tape_pos, i, run);
                }
            } else if self.core.alphabet.is_flag_diacritic(input) {
                self.take_flag(input, input_pos, tape_pos, i, run);
            } else if self.core.alphabet.has_rtn_sym(input) {
                let caller = self.symbol;
                let caller_frame = self.frame().clone();
                PmatchWalk::entering(self.core, PmatchWalk::net_for(self.core, input)).rtn_call(
                    input_pos,
                    tape_pos,
                    caller,
                    target,
                    caller_frame,
                    run,
                );
            }
            i += 1;
            run.set_weight(old_weight);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.check-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.check-context-fn]
    fn check_context(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        // The context placeholder remembers the position in the input before
        // a context check. If the context check is successful, the placeholder
        // will be used as the input position going forwards.
        self.frame_mut().context_placeholder = input_pos;
        let mut input_pos = input_pos;
        let ctx = self.frame().context;
        if ctx == ContextChecking::LC || ctx == ContextChecking::NLC {
            // Jump to the left-hand side of the input
            input_pos = run
                .entry_stack
                .last()
                .expect("entry_stack populated during a left-context check")
                .wrapping_sub(1);
        }
        let target = self.net.transition_target(i);
        self.get_analyses(input_pos, tape_pos, target, run);

        // In case we have a negative context, we check to see if the context
        // matched. If it didn't, we schedule a passthrough arc after we've
        // processed epsilons.
        let mut schedule_passthrough = false;
        let ctx = self.frame().context;
        if (ctx == ContextChecking::NLC || ctx == ContextChecking::NRC)
            && !self.frame().negative_context_success
        {
            schedule_passthrough = true;
        }
        // Pop the local stack that got pushed by entering the context
        self.local_stack.pop();
        if schedule_passthrough {
            self.frame_mut().pending_passthrough = true;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-flag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-flag-fn]
    fn take_flag(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        let mut old_global_values: Vec<i16> = Vec::new();
        if self.core.alphabet.is_global_flag_sym(input) {
            old_global_values = run.global_flag_state.get_values().clone();
            let op = self
                .core
                .alphabet
                .get_operation(input)
                .expect("flag diacritic has an operation")
                .clone();
            if !run.global_flag_state.apply_operation(&op) {
                return;
            }
        }
        let old_values = self.frame().flag_state.get_values().clone();
        let op = self
            .core
            .alphabet
            .get_operation(input)
            .expect("flag diacritic has an operation")
            .clone();
        if self.frame_mut().flag_state.apply_operation(&op) {
            // flag diacritic allowed
            // generally we shouldn't care to write flags
            //                container->tape.write(tape_pos, input, output);
            let target = self.net.transition_target(i);
            self.get_analyses(input_pos, tape_pos, target, run);
        }
        if self.core.alphabet.is_global_flag_sym(input) {
            run.global_flag_state.assign_values(&old_global_values);
        }
        self.frame_mut().flag_state.assign_values(&old_values);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-transitions-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-transitions-fn]
    fn take_transitions(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        let mut i = self.net.make_transition_table_index(i, input);

        while PmatchTransducer::is_good(i) {
            let mut this_input = self.net.transition_input(i);
            let mut this_output = self.net.transition_output(i);
            let target = self.net.transition_target(i);
            if this_input == NO_SYMBOL_NUMBER {
                return;
            } else if this_input == input {
                let old_weight = run.get_weight();
                run.increment_weight(self.net.transition_weight(i));
                if !self.checking_context() {
                    if self.core.alphabet.is_meta_arc(this_output)
                        || (run.list2symbols(this_output) != NO_SYMBOL_NUMBER)
                    {
                        // we got here via a meta-arc, so look back in the
                        // input tape to find the symbol we want to write
                        this_output = run.input[input_pos as usize];
                        this_input = run.input[input_pos as usize];
                    }
                    if this_input
                        == self
                            .core
                            .alphabet
                            .get_special(SpecialSymbol::Pmatch_passthrough)
                    {
                        self.get_analyses(input_pos, tape_pos, target, run); // awkward
                    } else {
                        run.tape.write_pair(tape_pos, this_input, this_output);
                        self.get_analyses(input_pos + 1, tape_pos + 1, target, run);
                    }
                } else {
                    // Checking context so don't touch output
                    if self.frame().max_context_length_remaining > 0 {
                        if (self.frame().tape_step < 0) && (input_pos == 0) {
                            // (C++ marks FIXME here) prevents segfault but
                            self.get_analyses(input_pos, tape_pos, target, run); // awkward
                        } else {
                            self.frame_mut().max_context_length_remaining -= 1;
                            let step = self.frame().tape_step;
                            let new_input_pos = (input_pos as i64 + step as i64) as u32;
                            self.get_analyses(new_input_pos, tape_pos, target, run);
                            self.frame_mut().max_context_length_remaining += 1;
                        }
                    }
                }
                self.frame_mut().default_symbol_trap = false;
                run.set_weight(old_weight);
            } else {
                return;
            }
            i += 1;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.get-analyses-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.get-analyses-fn]
    fn get_analyses(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        index: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        let i = index;
        if run.get_weight() > run.max_weight {
            return;
        }
        if run.max_time > 0.0 {
            run.call_counter += 1;
            // Have we spent too much time?
            if run.limit_reached
                || (run.call_counter.is_multiple_of(1000000)
                    && (run.candidate_found()
                        // if we have at least something, stop doing more work
                        && run
                            .start_clock
                            .expect("start_clock set when max_time is enabled")
                            .elapsed()
                            .as_secs_f64()
                            > run.max_time))
            {
                run.limit_reached = true;
                return;
            }
        }
        if !run.try_recurse() {
            if run.verbose {
                warn!("out of stack space, truncating result");
            }
            return;
        }
        self.frame_mut().default_symbol_trap = true;
        self.take_epsilons(input_pos, tape_pos, i + 1, run);
        if self.frame().pending_passthrough {
            self.frame_mut().pending_passthrough = false;
            // A negative context failed (successfully)
            let passthrough = self
                .core
                .alphabet
                .get_special(SpecialSymbol::Pmatch_passthrough);
            self.take_transitions(passthrough, input_pos, tape_pos, i + 1, run);
        }
        // Check for finality even if the input string hasn't ended
        if self.net.is_final(i) {
            let old_weight = run.get_weight();
            run.increment_weight(self.net.get_weight(i));
            self.handle_final_state(input_pos, tape_pos, run);
            run.set_weight(old_weight);
        }

        if !run.has_queued_input(input_pos) {
            run.unrecurse();
            return;
        }
        let input = run.input[input_pos as usize];

        let list_idx = run.symbol2lists(input);
        if list_idx != NO_SYMBOL_NUMBER {
            // At least one symbol list could allow this symbol
            let list = run.symbol_list(list_idx).clone();
            for it in list.iter() {
                self.take_transitions(*it, input_pos, tape_pos, i + 1, run);
            }
        }
        if self.core.alphabet.get_special(SpecialSymbol::UnicodeAlpha) != NO_SYMBOL_NUMBER
            && run.is_unicode_alpha(input)
        {
            let s = self.core.alphabet.get_special(SpecialSymbol::UnicodeAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }
        if self
            .core
            .alphabet
            .get_special(SpecialSymbol::UnicodeUpperAlpha)
            != NO_SYMBOL_NUMBER
            && run.is_unicode_upperalpha(input)
        {
            let s = self
                .core
                .alphabet
                .get_special(SpecialSymbol::UnicodeUpperAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }
        if self
            .core
            .alphabet
            .get_special(SpecialSymbol::UnicodeLowerAlpha)
            != NO_SYMBOL_NUMBER
            && run.is_unicode_loweralpha(input)
        {
            let s = self
                .core
                .alphabet
                .get_special(SpecialSymbol::UnicodeLowerAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }
        if self
            .core
            .alphabet
            .get_special(SpecialSymbol::UnicodeWhitespace)
            != NO_SYMBOL_NUMBER
            && run.is_unicode_whitespace(input)
        {
            let s = self
                .core
                .alphabet
                .get_special(SpecialSymbol::UnicodeWhitespace);
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }

        // The "normal" case where we have a regular input symbol
        if input < self.net.orig_symbol_count {
            self.take_transitions(input, input_pos, tape_pos, i + 1, run);
        } else {
            if self.core.alphabet.get_identity_symbol() != NO_SYMBOL_NUMBER {
                let s = self.core.alphabet.get_identity_symbol();
                self.take_transitions(s, input_pos, tape_pos, i + 1, run);
            }
            if self.core.alphabet.get_unknown_symbol() != NO_SYMBOL_NUMBER {
                let s = self.core.alphabet.get_unknown_symbol();
                self.take_transitions(s, input_pos, tape_pos, i + 1, run);
            }
        }
        if self.core.alphabet.get_default_symbol() != NO_SYMBOL_NUMBER
            && self.frame().default_symbol_trap
        {
            let s = self.core.alphabet.get_default_symbol();
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }
        run.unrecurse();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.checking-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.checking-context-fn]
    fn checking_context(&self) -> bool {
        self.frame().context != ContextChecking::none
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.try-entering-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.try-entering-context-fn]
    fn try_entering_context(&mut self, symbol: SymbolNumber, run: &PmatchContainer) -> bool {
        let mut new_top: LocalVariables;
        if symbol == self.core.alphabet.get_special(SpecialSymbol::LC_entry) {
            new_top = self.frame().clone();
            new_top.context = ContextChecking::LC;
            new_top.tape_step = -1;
        } else if symbol == self.core.alphabet.get_special(SpecialSymbol::RC_entry) {
            new_top = self.frame().clone();
            new_top.context = ContextChecking::RC;
            new_top.tape_step = 1;
        } else if symbol == self.core.alphabet.get_special(SpecialSymbol::NLC_entry) {
            new_top = self.frame().clone();
            new_top.context = ContextChecking::NLC;
            new_top.tape_step = -1;
        } else if symbol == self.core.alphabet.get_special(SpecialSymbol::NRC_entry) {
            new_top = self.frame().clone();
            new_top.context = ContextChecking::NRC;
            new_top.tape_step = 1;
        } else {
            return false;
        }
        new_top.max_context_length_remaining = run.props.max_context_length;
        self.local_stack.push(new_top);
        true
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.try-exiting-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.try-exiting-context-fn]
    fn try_exiting_context(&mut self, symbol: SymbolNumber) -> bool {
        match self.frame().context {
            ContextChecking::LC
                if symbol == self.core.alphabet.get_special(SpecialSymbol::LC_exit) =>
            {
                self.exit_context();
                true
            }
            ContextChecking::RC
                if symbol == self.core.alphabet.get_special(SpecialSymbol::RC_exit) =>
            {
                self.exit_context();
                true
            }
            // NOTE: faithful to C++: the NRC case has no 'else'/'break', so on a
            // non-matching symbol it falls through to the NLC case (and then to
            // default). We reproduce that fallthrough explicitly.
            ContextChecking::NRC => {
                if symbol == self.core.alphabet.get_special(SpecialSymbol::NRC_exit) {
                    self.frame_mut().negative_context_success = true;
                    return false;
                }
                if symbol == self.core.alphabet.get_special(SpecialSymbol::NLC_exit) {
                    self.frame_mut().negative_context_success = true;
                    return false;
                }
                false
            }
            ContextChecking::NLC => {
                if symbol == self.core.alphabet.get_special(SpecialSymbol::NLC_exit) {
                    self.frame_mut().negative_context_success = true;
                    return false;
                }
                false
            }
            // `none`, plus `LC`/`RC` whose exit-symbol guards above did not fire.
            ContextChecking::none | ContextChecking::LC | ContextChecking::RC => false,
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.exit-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.exit-context-fn]
    fn exit_context(&mut self) {
        let mut new_top = self.frame().clone();
        new_top.context = ContextChecking::none;
        new_top.negative_context_success = false;
        new_top.tape_step = 1;
        self.local_stack.push(new_top);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.match-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.match-fn]
    pub(super) fn do_match(&mut self, input_pos: u32, tape_pos: u32, run: &mut PmatchContainer) {
        {
            let top = self.frame_mut();
            top.context = ContextChecking::none;
            top.tape_step = 1;
            top.context_placeholder = 0;
            top.default_symbol_trap = false;
        }
        // [DIVERGENCE hfst/hfst#399] Fresh epsilon-cycle memo per top-level
        // match attempt (see PmatchContainer::epsilon_path).
        run.epsilon_path.clear();
        self.get_analyses(input_pos, tape_pos, 0, run);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-call-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-call-fn]
    fn rtn_call(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        caller: SymbolNumber,
        caller_index: TransitionTableIndex,
        caller_frame: LocalVariables,
        run: &mut PmatchContainer,
    ) {
        run.push_rtn_call(caller_index, caller, caller_frame);
        run.increase_stack_depth();
        let mut new_top = self.frame().clone();
        new_top.flag_state = self.core.flag_state_proto.clone();
        new_top.tape_step = 1;
        new_top.context = ContextChecking::none;
        new_top.context_placeholder = 0;
        new_top.default_symbol_trap = false;
        self.local_stack.push(new_top);
        self.get_analyses(input_pos, tape_pos, 0, run);
        self.local_stack.pop();
        run.decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        run.rtn_stack_pop();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-call-in-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-call-in-context-fn]
    fn rtn_call_in_context(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        caller: SymbolNumber,
        caller_index: TransitionTableIndex,
        locals: LocalVariables,
        run: &mut PmatchContainer,
    ) {
        // 'locals' is the caller's frame at the call site; stash a copy for the
        // eventual return before it is repurposed as the callee's own frame.
        run.push_rtn_call(caller_index, caller, locals.clone());
        run.increase_stack_depth();
        let mut new_top = locals;
        new_top.flag_state = self.core.flag_state_proto.clone();
        self.local_stack.push(new_top);
        self.get_analyses(input_pos, tape_pos, 0, run);
        self.local_stack.pop();
        run.decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        run.rtn_stack_pop();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-return-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-return-fn]
    fn rtn_return(&mut self, input_pos: u32, tape_pos: u32, run: &mut PmatchContainer) {
        run.decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        let entry_index = run.rtn_stack_top().caller_index;
        self.get_analyses(input_pos, tape_pos, entry_index, run);
        run.increase_stack_depth();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.handle-final-state-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.handle-final-state-fn]
    fn handle_final_state(&mut self, input_pos: u32, tape_pos: u32, run: &mut PmatchContainer) {
        if run.get_stack_depth() > 0 {
            // We're not the toplevel, return to caller. The caller is suspended
            // higher on the Rust call stack; with its local-variable stack held
            // by the walk rather than the net, resuming it is a second shared
            // borrow of the same net plus the frame it held at the RTN call.
            // [hfst/hfst#354]
            let core = self.core;
            let rtn_target = run.get_latest_rtn_caller();
            let caller_frame = run.get_latest_caller_frame();
            PmatchWalk::resuming(core, PmatchWalk::net_for(core, rtn_target), caller_frame)
                .rtn_return(input_pos, tape_pos, run);
        } else if run.is_in_locate_mode() {
            run.grab_location(input_pos, tape_pos);
        } else {
            run.note_analysis(input_pos, tape_pos);
        }
    }
}
