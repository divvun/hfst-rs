//! The rule containers and the 'TwolCGrammar' that owns them.

use super::*;

// ───────────────────────────────────────────────────────────────────────────
// Rule containers (rule_src/RuleContainer.cc, RightArrowRuleContainer.cc,
// LeftArrowRuleContainer.cc).
//
// The C++ containers held 'std::vector<Rule*>' and 'delete'd the pointers in
// the destructor; here every rule is OWNED as a ['TwolcRule'] enum value in
// 'rule_vector', so vector drop replaces the deleting destructor. The C++ maps
// stored 'Rule*' keyed by center-pair / input-symbol; here they store INDICES
// into the owning 'rule_vector' (a 'Rule*' replacement that survives the
// borrow checker because the rules are owned in one place).
//
// Conflict resolution touches only the 'Rule' base data ('context'/'name'),
// reachable through the ['TwolcRule'] 'rule()'/'rule_mut()' accessors, plus
// the free 'wbize' helper; so the container code never needs to match the
// enum back to its concrete conflict-resolving variant.
// ───────────────────────────────────────────────────────────────────────────

// C++ '~RuleContainer' iterates 'rule_vector' and 'delete's every 'Rule*'.
// Here each rule is owned as a ['TwolcRule'] in 'rule_vector', so dropping
// 'rule_vector' is the deleting destructor.
// [spec:hfst:def:rule-container.rule-container.rule-container-fn]
// [spec:hfst:sem:rule-container.rule-container.rule-container-fn]
// [spec:hfst:def:rule-container.rule-container]
impl<B: AlgebraBackend> RuleContainer<B> {
    /// C++ 'RuleContainer::RuleContainer(void): report(true) {}'.
    pub fn new() -> Self {
        RuleContainer {
            report: true,
            rule_vector: Vec::new(),
        }
    }

    // [spec:hfst:def:rule-container.rule-container.add-rule-fn]
    // [spec:hfst:sem:rule-container.rule-container.add-rule-fn]
    //
    // C++ 'rule_vector.push_back(rule)'. Returns the index of the stored rule
    // (the 'Rule*' replacement used by the grammar's subcase handles).
    pub fn add_rule(&mut self, rule: TwolcRule<B>) -> usize {
        self.rule_vector.push(rule);
        self.rule_vector.len() - 1
    }

    // [spec:hfst:def:rule-container.rule-container.compile-fn]
    // [spec:hfst:sem:rule-container.rule-container.compile-fn]
    //
    // C++ iterates 'rule_vector', optionally prints the print-name and calls
    // '(*it)->compile()'. The verbose message is sent to stderr (the C++
    // 'msg_out' is always 'std::cerr' at the call sites).
    pub fn compile(&mut self, cfg: &OstConfig, be_verbose: bool) -> crate::error::Result<()> {
        for rule in self.rule_vector.iter_mut() {
            if be_verbose {
                debug!("Compiling {}", Rule::<B>::get_print_name(&rule.rule().name));
            }
            rule.compile(cfg)?;
        }
        Ok(())
    }

    // [spec:hfst:def:rule-container.rule-container.store-fn]
    // [spec:hfst:sem:rule-container.rule-container.store-fn]
    //
    // C++ takes (HfstOutputStream &out, std::ostream &msg_out, bool be_verbose);
    // the progress messages go to stderr here (as elsewhere in this port).
    pub fn store(
        &mut self,
        cfg: &OstConfig,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
        be_verbose: bool,
    ) -> crate::error::Result<()> {
        for rule in self.rule_vector.iter_mut() {
            if be_verbose {
                let name = Rule::<B>::get_print_name(&rule.rule().get_name());
                debug!("Storing {name}");
            }
            rule.rule_mut().store(cfg, out)?;
        }
        Ok(())
    }

    // [spec:hfst:def:rule-container.rule-container.add-missing-symbols-freely-fn]
    // [spec:hfst:sem:rule-container.rule-container.add-missing-symbols-freely-fn]
    pub fn add_missing_symbols_freely(
        &mut self,
        cfg: &OstConfig,
        diacritics: &SymbolRange,
    ) -> crate::error::Result<()> {
        for rule in self.rule_vector.iter_mut() {
            rule.rule_mut()
                .add_missing_symbols_freely(cfg, diacritics)?;
        }
        Ok(())
    }

    /// Borrow the rule at 'index' (the 'Rule*' deref the grammar performs when
    /// intersecting subcases by handle).
    pub(crate) fn rule_ref(&self, index: usize) -> &TwolcRule<B> {
        &self.rule_vector[index]
    }
}

impl<B: AlgebraBackend> Default for RuleContainer<B> {
    fn default() -> Self {
        RuleContainer::new()
    }
}

// [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container]
impl<B: AlgebraBackend> RightArrowRuleContainer<B> {
    /// C++ default state: 'report_right_arrow_conflicts = true',
    /// 'resolve_right_arrow_conflicts = true' (file-scope static initialisers).
    pub fn new() -> Self {
        RightArrowRuleContainer {
            base: RuleContainer::new(),
            report_right_arrow_conflicts: true,
            resolve_right_arrow_conflicts: true,
            center_to_rule_map: BTreeMap::new(),
        }
    }

    // [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.set-report-right-arrow-conflicts-fn]
    // [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.set-report-right-arrow-conflicts-fn]
    pub fn set_report_right_arrow_conflicts(&mut self, option: bool) {
        self.report_right_arrow_conflicts = option;
    }

    // [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.set-resolve-right-arrow-conflicts-fn]
    // [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.set-resolve-right-arrow-conflicts-fn]
    pub fn set_resolve_right_arrow_conflicts(&mut self, option: bool) {
        self.resolve_right_arrow_conflicts = option;
    }

    // [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
    // [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
    //
    // C++ keyed conflict on 'center_pair': if the map already holds a rule with
    // the same center-pair, the rules conflict (the map lookup IS the
    // 'conflicts_this' test). When resolving, the EXISTING rule's contexts are
    // joined with the incoming rule's ('disjunct' + 'minimize'), the incoming
    // rule's name is appended to the existing rule's name, and the incoming
    // rule is marked 'is_empty' (so the subcase intersection skips it). The
    // incoming rule is still pushed into 'rule_vector' here — unlike C++, which
    // leaks it — so the grammar's subcase handle for it stays valid; an empty
    // rule contributes nothing to the intersection.
    //
    // The conflict-resolution itself ('disjunct'/'minimize' of 'context', name
    // append) is the 'ConflictResolvingRightArrowRule::resolve_conflict' body,
    // applied here through the base 'Rule' data so the owned ['TwolcRule']
    // need not be matched back to its concrete variant.
    pub fn add_rule_resolving_conflicts(
        &mut self,
        cfg: &OstConfig,
        mut rule: ConflictResolvingRightArrowRule<B>,
    ) -> crate::error::Result<usize> {
        let center_pair = rule.center_pair.clone();
        if let Some(&existing_index) = self.center_to_rule_map.get(&center_pair) {
            if self.report_right_arrow_conflicts {
                let existing_name = self.base.rule_vector[existing_index].rule().name.clone();
                let incoming_name = rule.base.base.name.clone();
                warn!(
                    "There is a =>-rule conflict between {} and {}.\nResolving the conflict by joining contexts.",
                    Rule::<B>::get_print_name(&existing_name),
                    Rule::<B>::get_print_name(&incoming_name)
                );
            }

            if self.resolve_right_arrow_conflicts {
                // ConflictResolvingRightArrowRule::resolve_conflict:
                //   existing.context.disjunct(cfg, incoming.context).minimize(cfg);
                //   existing.name += " and " + incoming.name;
                let incoming_context = clone_ost(&rule.base.base.context);
                let incoming_name = rule.base.base.name.clone();
                {
                    let existing = self.base.rule_vector[existing_index].rule_mut();
                    existing.context.disjunct(cfg, &incoming_context)?;
                    existing.context.minimize(cfg)?;
                    existing.name = format!("{} and {}", existing.name, incoming_name);
                }
                rule.base.base.is_empty = true;
                Ok(self
                    .base
                    .add_rule(TwolcRule::ConflictResolvingRightArrow(rule)))
            } else {
                Ok(self
                    .base
                    .add_rule(TwolcRule::ConflictResolvingRightArrow(rule)))
            }
        } else {
            let index = self
                .base
                .add_rule(TwolcRule::ConflictResolvingRightArrow(rule));
            self.center_to_rule_map.insert(center_pair, index);
            Ok(index)
        }
    }

    /// C++ 'RuleContainer::compile' forwarded through the base member.
    pub fn compile(&mut self, cfg: &OstConfig, be_verbose: bool) -> crate::error::Result<()> {
        self.base.compile(cfg, be_verbose)?;
        Ok(())
    }

    pub(crate) fn rule_ref(&self, index: usize) -> &TwolcRule<B> {
        self.base.rule_ref(index)
    }
}

impl<B: AlgebraBackend> Default for RightArrowRuleContainer<B> {
    fn default() -> Self {
        RightArrowRuleContainer::new()
    }
}

// [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container]
impl<B: AlgebraBackend> LeftArrowRuleContainer<B> {
    /// C++ default state: 'resolve_left_arrow_conflicts = false',
    /// 'report_left_arrow_conflicts = false' (file-scope static initialisers).
    pub fn new() -> Self {
        LeftArrowRuleContainer {
            base: RuleContainer::new(),
            report_left_arrow_conflicts: false,
            resolve_left_arrow_conflicts: false,
            input_to_rule_map: BTreeMap::new(),
        }
    }

    // [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.set-resolve-left-arrow-conflicts-fn]
    // [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.set-resolve-left-arrow-conflicts-fn]
    pub fn set_resolve_left_arrow_conflicts(&mut self, option: bool) {
        self.resolve_left_arrow_conflicts = option;
    }

    // [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.set-report-left-arrow-conflicts-fn]
    // [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.set-report-left-arrow-conflicts-fn]
    pub fn set_report_left_arrow_conflicts(&mut self, option: bool) {
        self.report_left_arrow_conflicts = option;
    }

    // [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
    // [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
    //
    // C++ groups '<='-rules by their center input symbol. For every previously
    // added rule sharing the input symbol, if the incoming rule conflicts
    // ('(*it)->conflicts_this(*rule, ctx)' — a non-empty intersection of the
    // existing context with the word-boundary-ized incoming context), the
    // conflict is reported and, if resolution is enabled, resolved by
    // restricting (subtracting) whichever rule's context is resolvable.
    //
    // The conflict predicates / resolution touch only the base 'Rule' context
    // (via 'is_empty_intersection'/'is_subset'/'subtract') plus the free
    // 'wbize' helper, so they are applied here through the base 'Rule' data
    // without downcasting. The incoming rule is then filed under its input
    // symbol and pushed into 'rule_vector'.
    pub fn add_rule_resolving_conflicts(
        &mut self,
        cfg: &OstConfig,
        mut rule: ConflictResolvingLeftArrowRule<B>,
    ) -> crate::error::Result<usize> {
        let input = rule.input_symbol.clone();
        if let Some(indices) = self.input_to_rule_map.get(&input) {
            let existing_indices: Vec<usize> = indices.clone();
            for existing_index in existing_indices {
                // (*it)->conflicts_this(*rule, conflicting_context):
                //   ! existing.context.is_empty_intersection(wbize(cfg, rule.context))
                let wbized_incoming = wbize(cfg, &rule.base.base.context)?;
                let mut conflicting_context: StringVector = Vec::new();
                let conflicts = {
                    let existing = self.base.rule_vector[existing_index].rule();
                    !existing
                        .context
                        .is_empty_intersection(&wbized_incoming, &mut conflicting_context)
                };
                if conflicts {
                    if self.report_left_arrow_conflicts {
                        let existing_name =
                            self.base.rule_vector[existing_index].rule().name.clone();
                        let mut line = format!(
                            "There is a <=-rule conflict between {} and {}.\nE.g. in context ",
                            Rule::<B>::get_print_name(&existing_name),
                            Rule::<B>::get_print_name(&rule.base.base.name)
                        );
                        let mut diamond_seen = false;
                        for sp in conflicting_context.iter() {
                            let mut symbol_pair = sp.replace(TWOLC_EPSILON, "");
                            if symbol_pair == "__HFST_TWOLC_DIAMOND:__HFST_TWOLC_DIAMOND" {
                                if diamond_seen {
                                    continue;
                                }
                                symbol_pair = "_".to_string();
                                diamond_seen = true;
                            } else if symbol_pair
                                == "@_TWOLC_IDENTITY_SYMBOL_@:@_TWOLC_IDENTITY_SYMBOL_@"
                            {
                                symbol_pair = "?".to_string();
                            }
                            line.push_str(&format!("{} ", symbol_pair));
                        }
                        warn!("{}", line);
                    }
                    if self.resolve_left_arrow_conflicts {
                        // existing.resolvable_conflict(rule):
                        //   existing.context.is_subset(wbize(cfg, rule.context))
                        let existing_resolvable = {
                            let existing = self.base.rule_vector[existing_index].rule();
                            existing.context.is_subset(cfg, &wbized_incoming)?
                        };
                        if existing_resolvable {
                            if self.report_left_arrow_conflicts {
                                let existing_name =
                                    self.base.rule_vector[existing_index].rule().name.clone();
                                warn!(
                                    "Resolving the conflict by restricting the context of {}.",
                                    Rule::<B>::get_print_name(&existing_name)
                                );
                            }
                            // existing.resolve_conflict(rule):
                            //   existing.context.subtract(cfg, rule.context);
                            let incoming_context = clone_ost(&rule.base.base.context);
                            let existing = self.base.rule_vector[existing_index].rule_mut();
                            existing.context.subtract(cfg, &incoming_context)?;
                        } else {
                            // rule.resolvable_conflict(*it):
                            //   rule.context.is_subset(wbize(cfg, existing.context))
                            let wbized_existing = {
                                let existing = self.base.rule_vector[existing_index].rule();
                                wbize(cfg, &existing.context)?
                            };
                            let incoming_resolvable =
                                rule.base.base.context.is_subset(cfg, &wbized_existing)?;
                            if incoming_resolvable {
                                if self.report_left_arrow_conflicts {
                                    warn!(
                                        "Resolving the conflict by restricting the context of {}.",
                                        rule.base.base.name
                                    );
                                }
                                // rule.resolve_conflict(*it):
                                //   rule.context.subtract(cfg, existing.context);
                                let existing_context = {
                                    let existing = self.base.rule_vector[existing_index].rule();
                                    clone_ost(&existing.context)
                                };
                                rule.base.base.context.subtract(cfg, &existing_context)?;
                            } else if self.report_left_arrow_conflicts {
                                warn!("The conflict is unresolvable.");
                            }
                        }
                    }
                }
            }
        }
        let index = self
            .base
            .add_rule(TwolcRule::ConflictResolvingLeftArrow(rule));
        self.input_to_rule_map.entry(input).or_default().push(index);
        Ok(index)
    }

    pub fn compile(&mut self, cfg: &OstConfig, be_verbose: bool) -> crate::error::Result<()> {
        self.base.compile(cfg, be_verbose)?;
        Ok(())
    }

    pub(crate) fn rule_ref(&self, index: usize) -> &TwolcRule<B> {
        self.base.rule_ref(index)
    }
}

impl<B: AlgebraBackend> Default for LeftArrowRuleContainer<B> {
    fn default() -> Self {
        LeftArrowRuleContainer::new()
    }
}

/// Copy an ['OtherSymbolTransducer'] (the C++ copy constructor /
/// 'operator=' that copies 'is_broken' + the wrapped transducer). Used by the
/// container conflict-resolution code, which must read one rule's context while
/// mutating another's.
pub(super) fn clone_ost<B: AlgebraBackend>(
    t: &OtherSymbolTransducer<B>,
) -> OtherSymbolTransducer<B> {
    OtherSymbolTransducer {
        is_broken: t.is_broken,
        transducer: t.transducer.clone(),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// TwolCGrammar (rule_src/TwolCGrammar.cc).
// ───────────────────────────────────────────────────────────────────────────

// [spec:hfst:def:twol-c-grammar.twol-c-grammar]
impl<B: AlgebraBackend> TwolCGrammar<B> {
    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.twol-c-grammar-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.twol-c-grammar-fn]
    //
    // C++ ctor wires the container conflict flags:
    //   left.set_report_left_arrow_conflicts(! be_quiet);
    //   left.set_resolve_left_arrow_conflicts(resolve_left_conflicts);
    //   right.set_report_right_arrow_conflicts(be_verbose);
    //   right.set_resolve_right_arrow_conflicts(resolve_right_conflicts);
    pub fn new(
        be_quiet: bool,
        be_verbose: bool,
        resolve_left_conflicts: bool,
        resolve_right_conflicts: bool,
    ) -> Self {
        let mut left_arrow_rule_container = LeftArrowRuleContainer::new();
        let mut right_arrow_rule_container = RightArrowRuleContainer::new();
        left_arrow_rule_container.set_report_left_arrow_conflicts(!be_quiet);
        left_arrow_rule_container.set_resolve_left_arrow_conflicts(resolve_left_conflicts);
        right_arrow_rule_container.set_report_right_arrow_conflicts(be_verbose);
        right_arrow_rule_container.set_resolve_right_arrow_conflicts(resolve_right_conflicts);
        TwolCGrammar {
            be_quiet,
            be_verbose,
            name_to_rule_subcases: BTreeMap::new(),
            left_arrow_rule_container,
            right_arrow_rule_container,
            other_rule_container: RuleContainer::new(),
            compiled_rule_container: RuleContainer::new(),
            diacritics: Vec::new(),
        }
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.get-original-name-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.get-original-name-fn]
    //
    // C++ 'name.substr(0, name.find("SUBCASE:"))'.
    pub fn get_original_name(name: &str) -> String {
        match name.find("SUBCASE:") {
            Some(pos) => name[..pos].to_string(),
            None => name.to_string(),
        }
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.define-diacritics-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.define-diacritics-fn]
    pub fn define_diacritics(&mut self, cfg: &mut OstConfig, diacritics: &SymbolRange) {
        self.diacritics = diacritics.to_vec();
        OtherSymbolTransducer::<B>::define_diacritics(cfg, diacritics);
    }

    /// Record a subcase handle for 'name''s original (pre-'SUBCASE:') name.
    fn insert_subcase(&mut self, name: &str, handle: RuleHandle) {
        self.name_to_rule_subcases
            .entry(Self::get_original_name(name))
            .or_default()
            .insert(handle);
    }

    /// 'TwolCGrammar::add_rule(name, const SymbolPair &center, oper, contexts)'
    /// — the single-pair center overload ('RIGHT'/'LEFT'/'LEFT_RIGHT'/
    /// 'NOT_LEFT').
    pub fn add_rule_pair(
        &mut self,
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<()> {
        match oper {
            Operator::RIGHT => {
                let rule = ConflictResolvingRightArrowRule::new(cfg, name, center, contexts)?;
                let index = self
                    .right_arrow_rule_container
                    .add_rule_resolving_conflicts(cfg, rule)?;
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Right,
                        index,
                    },
                );
            }
            Operator::LEFT => {
                let rule = ConflictResolvingLeftArrowRule::new(cfg, name, center, contexts)?;
                let index = self
                    .left_arrow_rule_container
                    .add_rule_resolving_conflicts(cfg, rule)?;
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Left,
                        index,
                    },
                );
            }
            Operator::LEFT_RIGHT => {
                let right_rule = ConflictResolvingRightArrowRule::new(cfg, name, center, contexts)?;
                let right_index = self
                    .right_arrow_rule_container
                    .add_rule_resolving_conflicts(cfg, right_rule)?;
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Right,
                        index: right_index,
                    },
                );
                let left_rule = ConflictResolvingLeftArrowRule::new(cfg, name, center, contexts)?;
                let left_index = self
                    .left_arrow_rule_container
                    .add_rule_resolving_conflicts(cfg, left_rule)?;
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Left,
                        index: left_index,
                    },
                );
            }
            Operator::NOT_LEFT => {
                let rule = LeftRestrictionArrowRule::new_pair(cfg, name, center, contexts)?;
                let index = self
                    .other_rule_container
                    .add_rule(TwolcRule::LeftRestrictionArrow(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RE_RIGHT
            | Operator::RE_LEFT
            | Operator::RE_NOT_LEFT
            | Operator::RE_LEFT_RIGHT => {
                panic!("TwolCGrammar::add_rule_pair: unexpected operator {oper:?}")
            }
        }
        Ok(())
    }

    /// 'TwolCGrammar::add_rule(name, const OtherSymbolTransducer &center, oper,
    /// contexts)' — the regex-center overload ('RE_*'). The center is wrapped
    /// by 'Rule::get_center(restricted_center)' ('?* D center D ?*').
    pub fn add_rule_regex(
        &mut self,
        cfg: &OstConfig,
        name: &str,
        center: &OtherSymbolTransducer<B>,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<()> {
        let center_fst = Rule::get_center_restricted(cfg, center)?;
        match oper {
            Operator::RE_RIGHT => {
                let rule = RightArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let index = self
                    .other_rule_container
                    .add_rule(TwolcRule::RightArrow(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RE_LEFT => {
                let rule = LeftArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let index = self
                    .other_rule_container
                    .add_rule(TwolcRule::LeftArrow(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RE_LEFT_RIGHT => {
                let right_rule = RightArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let right_index = self
                    .other_rule_container
                    .add_rule(TwolcRule::RightArrow(right_rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index: right_index,
                    },
                );
                let left_rule = LeftArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let left_index = self
                    .other_rule_container
                    .add_rule(TwolcRule::LeftArrow(left_rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index: left_index,
                    },
                );
            }
            Operator::RE_NOT_LEFT => {
                let rule =
                    LeftRestrictionArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let index = self
                    .other_rule_container
                    .add_rule(TwolcRule::LeftRestrictionArrow(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RIGHT | Operator::LEFT | Operator::NOT_LEFT | Operator::LEFT_RIGHT => {
                panic!("TwolCGrammar::add_rule_regex: unexpected operator {oper:?}")
            }
        }
        Ok(())
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.add-rule-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.add-rule-fn]
    //
    // 'TwolCGrammar::add_rule(name, const SymbolPairVector &center, oper,
    // contexts)' — the multi-pair center overload, building one rule per center
    // pair named 'name CENTER=<in>:<out>'.
    pub fn add_rule_pairs(
        &mut self,
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPairVector,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<()> {
        for pair in center.iter() {
            let center_name = format!("{} CENTER={}:{}", name, pair.0, pair.1);
            match oper {
                Operator::RIGHT => {
                    let rule =
                        ConflictResolvingRightArrowRule::new(cfg, &center_name, pair, contexts)?;
                    let index = self
                        .right_arrow_rule_container
                        .add_rule_resolving_conflicts(cfg, rule)?;
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Right,
                            index,
                        },
                    );
                }
                Operator::LEFT => {
                    let rule =
                        ConflictResolvingLeftArrowRule::new(cfg, &center_name, pair, contexts)?;
                    let index = self
                        .left_arrow_rule_container
                        .add_rule_resolving_conflicts(cfg, rule)?;
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Left,
                            index,
                        },
                    );
                }
                Operator::LEFT_RIGHT => {
                    let right_rule =
                        ConflictResolvingRightArrowRule::new(cfg, &center_name, pair, contexts)?;
                    let right_index = self
                        .right_arrow_rule_container
                        .add_rule_resolving_conflicts(cfg, right_rule)?;
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Right,
                            index: right_index,
                        },
                    );
                    let left_rule =
                        ConflictResolvingLeftArrowRule::new(cfg, &center_name, pair, contexts)?;
                    let left_index = self
                        .left_arrow_rule_container
                        .add_rule_resolving_conflicts(cfg, left_rule)?;
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Left,
                            index: left_index,
                        },
                    );
                }
                Operator::NOT_LEFT => {
                    let rule =
                        LeftRestrictionArrowRule::new_pair(cfg, &center_name, pair, contexts)?;
                    let index = self
                        .other_rule_container
                        .add_rule(TwolcRule::LeftRestrictionArrow(rule));
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Other,
                            index,
                        },
                    );
                }
                Operator::RE_RIGHT
                | Operator::RE_LEFT
                | Operator::RE_NOT_LEFT
                | Operator::RE_LEFT_RIGHT => {
                    panic!("TwolCGrammar::add_rule_pairs: unexpected operator {oper:?}")
                }
            }
        }
        Ok(())
    }

    /// Borrow the rule a ['RuleHandle'] points at.
    fn rule_at(&self, handle: RuleHandle) -> &TwolcRule<B> {
        match handle.container {
            RuleContainerKind::Left => self.left_arrow_rule_container.rule_ref(handle.index),
            RuleContainerKind::Right => self.right_arrow_rule_container.rule_ref(handle.index),
            RuleContainerKind::Other => self.other_rule_container.rule_ref(handle.index),
        }
    }

    // The compile phase shared by both 'compile_and_store' flavours: compile
    // each container, then for every original rule name build a
    // 'Rule(name, RuleVector)' intersecting its subcases, and add the missing
    // diacritics freely. Fills 'compiled_rule_container'.
    fn compile_rules(&mut self, cfg: &OstConfig) -> crate::error::Result<()> {
        if !self.be_quiet {
            info!("Compiling rules.");
        }

        let verbose = (!self.be_quiet) && self.be_verbose;
        self.left_arrow_rule_container.compile(cfg, verbose)?;
        self.right_arrow_rule_container.compile(cfg, verbose)?;
        self.other_rule_container.compile(cfg, verbose)?;

        // Build one intersecting 'ResultRule' per original rule name. The C++
        // 'StringRuleSetMap' was keyed by the still-escaped htwolcpre1 token —
        // every space encoded as '__HFST_TWOLC_SPACE' — so '_' (0x5F), not
        // ' ' (0x20), is the byte that competes against punctuation in
        // neighbouring names. Sort by that encoding to reproduce the C++
        // stored-rule order byte for byte.
        let mut names: Vec<String> = self.name_to_rule_subcases.keys().cloned().collect();
        names.sort_by_key(|n| n.replace(' ', "__HFST_TWOLC_SPACE"));
        for name in names {
            let handles: Vec<RuleHandle> =
                self.name_to_rule_subcases[&name].iter().copied().collect();
            let subcases: Vec<&TwolcRule<B>> = handles.iter().map(|&h| self.rule_at(h)).collect();
            let result_rule = Rule::new_from_vector(cfg, &name, &subcases)?;
            self.compiled_rule_container
                .add_rule(TwolcRule::Result(result_rule));
        }
        let diacritics = self.diacritics.clone();
        self.compiled_rule_container
            .add_missing_symbols_freely(cfg, &diacritics)?;
        Ok(())
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    //
    // C++ compiles each container, then for every original rule name builds a
    // 'Rule(name, RuleVector)' intersecting its subcases, adds the missing
    // diacritics freely, and stores the result. This flavour RETURNS the
    // assembled result transducer (the intersection of every compiled rule)
    // instead of writing to a stream, so a smoke can drive the compiler;
    // 'compile_and_store_stream' below is the 1:1 stream-store path.
    pub fn compile_and_store(
        &mut self,
        cfg: &OstConfig,
    ) -> crate::error::Result<HfstTransducer<B>> {
        self.compile_rules(cfg)?;

        if !self.be_quiet {
            info!("Storing rules.");
        }

        // Intersect the compiled result rules into one transducer and return
        // it as the grammar's result (the union of all rule constraints over
        // the shared '?*' universe — intersection of the per-rule
        // 'rule_transducer's).
        assemble_result_transducer(cfg, &self.compiled_rule_container)
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    //
    // The 1:1 port of the C++ 'compile_and_store(HfstOutputStream &out)' store
    // path: compile every rule, then write ONE transducer PER RULE (named via
    // 'Rule::store''s info-symbol/name mapping) into the binary output stream
    // — the rule archive 'hfst-twolc' emits and 'hfst-compose-intersect'
    // consumes.
    pub fn compile_and_store_stream(
        &mut self,
        cfg: &OstConfig,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
    ) -> crate::error::Result<()> {
        self.compile_rules(cfg)?;

        if !self.be_quiet {
            info!("Storing rules.");
        }

        let verbose = (!self.be_quiet) && self.be_verbose;
        self.compiled_rule_container.store(cfg, out, verbose)
    }
}

/// Intersect every non-empty compiled 'ResultRule''s rule-transducer into one
/// 'HfstTransducer' — the value the deferred 'compile_and_store' store path
/// would otherwise serialise. Starts from '?*' (the universal language over the
/// twolc unknown symbol) so an empty grammar yields the universal automaton,
/// matching 'Rule(name, RuleVector)''s own '?*'-seeded intersection.
fn assemble_result_transducer<B: AlgebraBackend>(
    cfg: &OstConfig,
    container: &RuleContainer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut result = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
    result.repeat_star(cfg)?;
    for rule in container.rule_vector.iter() {
        if !rule.rule().is_empty {
            let rt = clone_ost(rule.rule_transducer());
            result.intersect(cfg, &rt)?;
        }
    }
    Ok(result.transducer)
}
