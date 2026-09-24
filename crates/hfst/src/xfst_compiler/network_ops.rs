//! Network operations on the stack: the unary and binary algebra, flag
//! elimination, and the other commands that rewrite the top network.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // @brief Sort top network of the stack
    // @todo HFST automata sort or not by default
    pub fn sort_net(&mut self) -> &mut Self {
        self.diag_warning("sort is not implemented; the network was left as it was");
        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Substring top network of stack
    // @todo unimplementedd
    pub fn substring_net(&mut self) -> &mut Self {
        self.diag_warning("substring is not implemented; the network was left as it was");
        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Compose stack
    pub fn compose_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_binary_operation_iteratively(BinaryOperation::COMPOSE_NET)
    }

    // @brief concatenate stack
    pub fn concatenate_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_binary_operation_iteratively(BinaryOperation::CONCATENATE_NET)
    }

    // @brief Crossproduct top of stack
    pub fn crossproduct_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_binary_operation(BinaryOperation::CROSSPRODUCT_NET)
    }

    // @brief Ignore top of stack with second automaton
    pub fn ignore_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_binary_operation(BinaryOperation::IGNORE_NET)
    }

    // @brief Intersect stack
    pub fn intersect_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_binary_operation_iteratively(BinaryOperation::INTERSECT_NET)
    }

    // @brief Subtract second from top of stack
    pub fn minus_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_binary_operation(BinaryOperation::MINUS_NET)
    }

    // @brief Shuffle top network with second
    pub fn shuffle_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_binary_operation_iteratively(BinaryOperation::SHUFFLE_NET)
    }

    // @brief Disjunct the stack
    pub fn union_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_binary_operation_iteratively(BinaryOperation::UNION_NET)
    }

    // @brief Apply operation on two top transducers in the stack.
    // The top transducers are popped, the operation is applied
    // (the topmost transducer is the first transducer in the operation),
    // and the result is pushed to the top of the stack.
    // If the stack has less than two transducers, print a warning.
    fn apply_binary_operation(
        &mut self,
        operation: BinaryOperation,
    ) -> crate::error::Result<&mut Self> {
        if self.stack.len() < 2 {
            self.error_message("not enough networks on the stack: this operation needs two");
            self.flush();
            self.xfst_lesser_fail();
            return Ok(self);
        }
        let result = *self.stack.last().expect("stack has >= 2, checked above");
        self.stack.pop();
        let another = *self
            .stack
            .last()
            .expect("stack still non-empty after one pop");
        self.stack.pop();
        let another_inner = self.net(another).clone();

        match operation {
            BinaryOperation::IGNORE_NET => {
                self.net_mut(result).insert_freely(&another_inner, true)?;
            }
            BinaryOperation::MINUS_NET => {
                self.net_mut(result).subtract(&another_inner, true)?;
            }
            BinaryOperation::CROSSPRODUCT_NET => {
                let cross = self
                    .net_mut(result)
                    .cross_product(&another_inner, true)
                    .map(|_| ());
                if let Err(e) = cross {
                    if matches!(e.kind, crate::error::ErrorKind::TransducersAreNotAutomata) {
                        self.error_message("transducers are not automata");
                        self.flush();
                        self.xfst_fail();
                        self.stack.push(another);
                        self.stack.push(result);
                        self.prompt();
                        return Ok(self);
                    } else {
                        return Err(e);
                    }
                }
            }
            BinaryOperation::INTERSECT_NET
            | BinaryOperation::COMPOSE_NET
            | BinaryOperation::CONCATENATE_NET
            | BinaryOperation::UNION_NET
            | BinaryOperation::SHUFFLE_NET => {
                self.error_message("ERROR: unknown binary operation");
                self.flush();
                self.xfst_fail();
            }
        }

        let cfg = self.engine_config;
        self.net_mut(result).optimize_with_config(&cfg)?;
        self.stack.push(result);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Apply operation on all transducers in the stack.
    // The top transducer (n1) is popped, the operation is applied iteratively
    // for all next transducers (n2, n3, n4 ...) in the stack:
    // [[[n1 OPERATION n2] OPERATION n3] OPERATION n4] ...
    // popping each of them and the result is pushed to the stack.
    // If the stack is empty, print a warning.
    fn apply_binary_operation_iteratively(
        &mut self,
        operation: BinaryOperation,
    ) -> crate::error::Result<&mut Self> {
        if self.stack.len() < 2 {
            self.error_message("not enough networks on the stack: this operation needs two");
            self.flush();
            self.xfst_lesser_fail();
            return Ok(self);
        }
        let result = *self.stack.last().expect("stack has >= 2, checked above");

        self.stack.pop();
        while !self.stack.is_empty() {
            let t = *self.stack.last().expect("stack non-empty in this loop");

            let t_type = self.net(t).get_type();
            let result_type = self.net(result).get_type();
            if t_type != result_type {
                self.error_message("Stack contains transducers whose type differs.");
                self.flush();
                self.xfst_lesser_fail();
                break;
            }

            match operation {
                BinaryOperation::INTERSECT_NET => {
                    let (rm, tm) = self.net_pair_mut(result, t);
                    rm.intersect(tm, true)?;
                }
                BinaryOperation::IGNORE_NET => {
                    let (rm, tm) = self.net_pair_mut(result, t);
                    rm.insert_freely(tm, true)?;
                }
                BinaryOperation::COMPOSE_NET => {
                    let left_has_flags = self.net(result).has_flag_diacritics();
                    let right_has_flags = self.net(t).has_flag_diacritics();
                    let harmonize_flags = self.harmonize_flags;
                    if left_has_flags && right_has_flags && !harmonize_flags && self.verbose {
                        self.error_message(
                            "Both composition arguments contain flag diacritics. \
                             Set harmonize-flags ON to harmonize them.",
                        );
                        self.flush();
                    }
                    let cfg = self.engine_config;
                    let overlay = {
                        let (left, right) = self.net_pair_mut(result, t);
                        prepare_compose_flag_overlay(left, right, harmonize_flags, &cfg)?
                    };

                    let (rm, tm) = self.net_pair_mut(result, t);
                    let composed = rm
                        .compose_with_config_and_flag_overlay(tm, true, &cfg, overlay.as_ref())
                        .map(|_| ());
                    if let Err(e) = composed {
                        if matches!(
                            e.kind,
                            crate::error::ErrorKind::FlagDiacriticsAreNotIdentities
                        ) {
                            self.error_message(
                                "Error: flag diacritics must be identities in \
                                 composition if flag-is-epsilon is ON.\n\
                                 I.e. only FLAG:FLAG is allowed, not FLAG1:FLAG2, \
                                 FLAG:bar or foo:FLAG\n\
                                 Apply twosided flag-diacritics (tfd) before \
                                 composition.",
                            );
                            self.flush();
                            self.xfst_lesser_fail();
                            self.prompt();
                            return Ok(self);
                        } else {
                            return Err(e);
                        }
                    }
                }
                BinaryOperation::CONCATENATE_NET => {
                    let (rm, tm) = self.net_pair_mut(result, t);
                    rm.concatenate(tm, true)?;
                }
                BinaryOperation::UNION_NET => {
                    let (rm, tm) = self.net_pair_mut(result, t);
                    rm.disjunct(tm, true)?;
                }
                BinaryOperation::SHUFFLE_NET => {
                    let (rm, tm) = self.net_pair_mut(result, t);
                    rm.shuffle(tm, true)?;
                }
                BinaryOperation::MINUS_NET | BinaryOperation::CROSSPRODUCT_NET => {
                    self.error_message("ERROR: unknown binary operation");
                    self.flush();
                }
            }
            self.stack.pop();
        }
        let cfg = self.engine_config;
        self.net_mut(result).optimize_with_config(&cfg)?;
        self.stack.push(result);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Remove unnecessary symbols using ?
    // @todo HFST does not support ?
    pub fn compact_sigma(&mut self) -> crate::error::Result<&mut Self> {
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        self.net_mut(top).prune_alphabet(true)?;
        self.prompt();
        Ok(self)
    }

    // @brief Eliminate flag diacritic
    // @todo unimplemented yet
    pub fn eliminate_flag(&mut self, name: &str) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        // [spec:hfst:def:xfst-compiler.hfst.xfst.name-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.name-fn]
        let name_str = name.to_string();
        let elim = self.net_mut(tmp).eliminate_flag(name).map(|_| ());
        if let Err(__e) = elim {
            let __name = __e.message.clone().unwrap_or_default();
            self.diag_error(&format!("could not eliminate flag '{}': {}", name, __name));
            if self.variables["quit-on-fail"] == "ON" {
                self.fail_flag = true;
            }
        }
        let _ = name_str;
        self.prompt();
        self
    }

    // @brief Eliminate all flag diacritics
    // @todo unimplemented yet
    pub fn eliminate_flags(&mut self) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        self.net_mut(tmp).eliminate_flags()?;
        self.prompt();
        Ok(self)
    }

    pub fn twosided_flags(&mut self) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        self.net_mut(tmp).twosided_flag_diacritics()?;
        self.prompt();
        Ok(self)
    }

    // @brief do some label pushing
    // @todo HFST automata cannot push labels
    pub fn cleanup_net(&mut self) -> &mut Self {
        self.diag_warning("cleanup is not implemented; the network was left as it was");
        if self.stack.is_empty() {
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            return self;
        }
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Make transducer functional
    // @todo unimplemented
    pub fn complete_net(&mut self) -> crate::error::Result<&mut Self> {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let mut fsm =
            ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(topmost))?;
        fsm.complete()?;
        let result: NetId = self.alloc_net(HfstTransducer::new_from_basic(&fsm)?);
        self.stack.pop();
        let cfg = self.engine_config;
        self.net_mut(result).optimize_with_config(&cfg)?;
        self.stack.push(result);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Determinize top of stack
    pub fn determinize_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::DETERMINIZE_NET)
    }

    // @brief Remove epsilons from top of stack
    pub fn epsilon_remove_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::EPSILON_REMOVE_NET)
    }

    // @brief invert top of stack
    pub fn invert_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::INVERT_NET)
    }

    // @brief Make top of stack label network
    // @todo Find out wtf this is
    pub fn label_net(&mut self) -> crate::error::Result<&mut Self> {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let result: NetId = self.alloc_net(HfstTransducer::new());
        let mut label_set: BTreeSet<(Symbol, Symbol)> = BTreeSet::new();
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(topmost))?;
        for it in fsm.iter() {
            for tr_it in it.iter() {
                label_set.insert((
                    tr_it.get_input_symbol(fsm.coder()),
                    tr_it.get_output_symbol(fsm.coder()),
                ));
            }
        }
        for it in label_set.iter() {
            let label_tr = HfstTransducer::new_symbol_pair(&it.0, &it.1)?;
            self.net_mut(result).disjunct(&label_tr, true)?;
        }
        // Unconditional even under 'minimal OFF': a set of single-label arcs is
        // trivially minimizable, and upstream minimized here for the same reason.
        let cfg = self.engine_config;
        self.net_mut(result).minimize_with_config(&cfg)?;
        self.stack.pop();
        self.stack.push(result);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Project input for top of stack
    pub fn lower_side_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::LOWER_SIDE_NET)
    }

    // @brief Project output for top of stack
    pub fn upper_side_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::UPPER_SIDE_NET)
    }

    // @brief Minimize top of stack
    pub fn minimize_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::MINIMIZE_NET)
    }

    // @brief Negate top of stack
    pub fn negate_net(&mut self) -> crate::error::Result<&mut Self> {
        if self.stack.is_empty() {
            self.diag_warning("empty stack: this command needs a network on the stack");
            self.xfst_lesser_fail();
            return Ok(self);
        }

        let t = *self.stack.last().expect("stack non-empty, checked above");
        let t_op = t;

        let negated = self.net_mut(t_op).negate().map(|_| ());
        if let Err(__e) = negated {
            if matches!(__e.kind, crate::error::ErrorKind::TransducerIsNotAutomaton) {
                self.diag_error_with_notes(
                    "negation is defined only for automata, and the top of the stack is a transducer",
                    &[String::from(
                        "subtract from the universal relation instead: [[?:?]* - A]",
                    )],
                );
                self.xfst_lesser_fail();
                return Ok(self);
            } else {
                return Err(__e);
            }
        }

        let cfg = self.engine_config;
        self.net_mut(t).optimize_with_config(&cfg)?;
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Kleene plus top network of stack
    pub fn one_plus_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::ONE_PLUS_NET)
    }

    // @brief Kleene star top network of stack
    pub fn zero_plus_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::ZERO_PLUS_NET)
    }

    // @brief Prune top network of stack
    // @todo Most of HFST automata are pruned by default?
    pub fn prune_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::PRUNE_NET_)
    }

    // @brief Reverse top network of the stack
    pub fn reverse_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::REVERSE_NET)
    }

    // @brief Sigma top network of stack
    // @todo Find out wtf this is
    pub fn sigma_net(&mut self) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let mut alpha: StringSet = self.net(tmp).get_alphabet()?;
        alpha.remove("@_UNKNOWN_SYMBOL_@");
        alpha.remove("@_IDENTITY_SYMBOL_@");
        alpha.remove("@_EPSILON_SYMBOL_@");
        let alpha = crate::hfst_symbol_defs::symbols::to_string_pair_set(&alpha);
        let sigma: NetId = self.alloc_net(HfstTransducer::new_string_pair_set(&alpha, false)?);
        let cfg = self.engine_config;
        self.net_mut(sigma).optimize_with_config(&cfg)?;
        self.stack.push(sigma);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Repeat 0..1 times
    pub fn optional_net(&mut self) -> crate::error::Result<&mut Self> {
        self.apply_unary_operation(UnaryOperation::OPTIONAL_NET)
    }

    // @brief Apply \a operation on top transducer in the stack.
    // If the stack is empty, print a warning.
    fn apply_unary_operation(
        &mut self,
        operation: UnaryOperation,
    ) -> crate::error::Result<&mut Self> {
        let Some(result) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        self.stack.pop();
        let result_op = result;
        let cfg = self.engine_config;

        match operation {
            UnaryOperation::DETERMINIZE_NET => {
                self.net_mut(result_op).determinize_with_config(&cfg)?;
            }
            UnaryOperation::EPSILON_REMOVE_NET => {
                self.net_mut(result_op).remove_epsilons()?;
            }
            UnaryOperation::INVERT_NET => {
                self.net_mut(result_op).invert()?;
            }
            UnaryOperation::LOWER_SIDE_NET => {
                self.net_mut(result_op).output_project()?;
            }
            UnaryOperation::UPPER_SIDE_NET => {
                self.net_mut(result_op).input_project()?;
            }
            UnaryOperation::ZERO_PLUS_NET => {
                self.net_mut(result_op).repeat_star()?;
            }
            UnaryOperation::ONE_PLUS_NET => {
                self.net_mut(result_op).repeat_plus()?;
            }
            UnaryOperation::OPTIONAL_NET => {
                self.net_mut(result_op).optionalize()?;
            }
            UnaryOperation::REVERSE_NET => {
                self.net_mut(result_op).reverse()?;
            }
            UnaryOperation::MINIMIZE_NET => {
                // explicit minimization requested, do not use optimize()
                self.net_mut(result_op).minimize_with_config(&cfg)?;
            }
            UnaryOperation::PRUNE_NET_ => {
                // 'prune' is a tropical-only facade operation; the C++
                // converted other types to TROPICAL_OPENFST_TYPE first. The
                // conversion is typed now ([dec:hfst:monomorphic-backends]):
                // round-trip through the interchange transducer.
                let tr = self.net_mut(result_op);
                let mut tropical: HfstTransducer<hfst_openfst::StdVectorFst> =
                    HfstTransducer::new_from_basic(&tr.to_basic()?)?;
                tropical.prune()?;
                *tr = HfstTransducer::new_from_basic(&tropical.to_basic()?)?;
            }
        }

        if operation != UnaryOperation::MINIMIZE_NET
            && operation != UnaryOperation::DETERMINIZE_NET
            && operation != UnaryOperation::EPSILON_REMOVE_NET
        {
            self.net_mut(result_op).optimize_with_config(&cfg)?;
        }
        self.stack.push(result);
        self.print_transducer_info();

        self.prompt();
        Ok(self)
    }
}
