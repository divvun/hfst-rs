//! 'TwolcCompiler': the AST-walk driver that registers a grammar's sections
//! and drives its rules into a 'TwolCGrammar'.

use super::*;

// ───────────────────────────────────────────────────────────────────────────
// TwolcCompiler — the AST-walk driver (replaces TwolcCompiler.cc + the three
// Flex/Bison preprocessor passes).
// ───────────────────────────────────────────────────────────────────────────

/// The two shapes a rule center can evaluate to: a (possibly multi-)pair list
/// ('a:b | c:d'), or a regex transducer (':[ E ]:'). Drives the
/// ['TwolCGrammar::add_rule_pairs'] vs ['TwolCGrammar::add_rule_regex'] choice.
pub enum CenterEval<B: AlgebraBackend> {
    Pairs(SymbolPairVector),
    Regex(OtherSymbolTransducer<B>),
}

/// One concrete rule produced by expanding a ['TwolcRule']'s 'where'-variables:
/// a fully-substituted name, the evaluated center, the operator and the
/// evaluated (positive + negated-negative) contexts.
pub struct ConcreteRule<B: AlgebraBackend> {
    pub name: String,
    pub center: CenterEval<B>,
    pub oper: Operator,
    pub contexts: OtherSymbolTransducerVector<B>,
}

// [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler]
impl<B: AlgebraBackend> Default for TwolcCompiler<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: AlgebraBackend> TwolcCompiler<B> {
    /// Construct with the C++ default flags: 'silent = false',
    /// 'verbose = false', 'resolve_left_conflicts = false',
    /// 'resolve_right_conflicts = true' (the upstream 'htwolc' defaults).
    /// The C++ 'format' parameter is the backend type parameter 'B' now.
    pub fn new() -> Self {
        Self::new_with_options(false, false, false, true)
    }

    /// Construct with explicit flags (the C++ 'TwolcCompiler::compile'
    /// parameters 'silent', 'verbose', 'resolve_left_conflicts',
    /// 'resolve_right_conflicts').
    pub fn new_with_options(
        silent: bool,
        verbose: bool,
        resolve_left: bool,
        resolve_right: bool,
    ) -> Self {
        TwolcCompiler {
            silent,
            verbose,
            resolve_left_conflicts: resolve_left,
            resolve_right_conflicts: resolve_right,
            sets: BTreeMap::new(),
            definitions: BTreeMap::new(),
            source: String::new(),
            source_name: String::from("<twolc>"),
            set_spans: BTreeMap::new(),
        }
    }

    /// Name shown in source-anchored diagnostics (the twolc file name). Set
    /// before `compile` so warnings point at the right file; defaults to
    /// `"<twolc>"`.
    pub fn set_source_name(&mut self, name: &str) -> &mut Self {
        self.source_name = name.to_string();
        self
    }

    // [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    // [spec:hfst:sem:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    //
    // Replaces the three Flex/Bison passes with 'nfst_twolc::parse' + an
    // AST-walk: set the transducer type, register the alphabet / diacritics /
    // sets / definitions, build the grammar, drive every rule (expanding
    // 'where'-variables and evaluating centers + contexts), and return the
    // intersected result transducer (the same Option contract as
    // 'XreCompiler::compile'). A parse failure yields None.
    pub fn compile(&mut self, input: &str) -> Option<HfstTransducer<B>> {
        let (cfg, mut grammar) = self.build_grammar(input)?;
        let result = grammar.compile_and_store(&cfg).ok()?;
        Some(result)
    }

    // [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    // [spec:hfst:sem:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    //
    // The stream flavour of 'compile': same parse + grammar walk, but the
    // result is stored through 'TwolCGrammar::compile_and_store_stream' — one
    // named transducer per rule into the binary output stream (the archive the
    // C++ 'hfst-twolc' driver writes). A parse or compile failure yields None.
    pub fn compile_and_store(
        &mut self,
        input: &str,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
    ) -> Option<()> {
        let (cfg, mut grammar) = self.build_grammar(input)?;
        grammar.compile_and_store_stream(&cfg, out).ok()?;
        Some(())
    }

    // The shared front half of both 'compile' flavours: parse the grammar
    // source and walk the AST into a ready-to-compile 'TwolCGrammar' (plus the
    // per-compile alphabet config). A parse failure reports its diagnostics
    // (unless silent) and yields None, like the C++ preprocessor passes that
    // printed to their error stream and made the driver exit.
    fn build_grammar(&mut self, input: &str) -> Option<(OstConfig, TwolCGrammar<B>)> {
        // Retain the source so diagnostics can render the offending snippet.
        self.source = input.to_string();
        let file = match nfst_twolc::parse(input) {
            Ok(f) => f,
            Err(e) => {
                self.report_parse_errors(&e.diagnostics);
                return None;
            }
        };
        let twolc_file = &file.value;

        // Per-compile alphabet config (formerly the 'OST_CONFIG' thread-local),
        // threaded by reference through the rule/grammar walk. (The C++
        // 'set_transducer_type' call is gone: the type is the parameter 'B'.)
        let mut cfg = OstConfig::new();

        let mut grammar = TwolCGrammar::new(
            self.silent,
            self.verbose,
            self.resolve_left_conflicts,
            self.resolve_right_conflicts,
        );

        // Sets are registered before the alphabet so the completion pass can
        // recognise (and skip) set-name pair sides, as the htwolcpre1 lexer's
        // '__HFST_TWOLC_SET_NAME=' marking let the C++ passes do.
        self.register_sets(&twolc_file.sets);
        self.register_alphabet(&mut cfg, twolc_file).ok()?;
        self.register_diacritics(&mut cfg, &twolc_file.diacritics, &mut grammar);
        self.register_definitions(&cfg, &twolc_file.definitions)
            .ok()?;

        // Keep going after a bad rule, so one run reports every rule at fault.
        let mut failed = false;
        for rule in twolc_file.rules.iter() {
            if let Err(e) = self.drive_rule(&cfg, &rule.value, &mut grammar) {
                self.report_failure(&e, &rule.span.range, "This rule");
                failed = true;
            }
        }
        if failed {
            return None;
        }

        Some((cfg, grammar))
    }

    /// Register the 'Alphabet' section: collect the declared symbol pairs and
    /// publish them to ['OtherSymbolTransducer::set_symbol_pairs'] (which also
    /// inserts the diamond:diamond pair).
    // [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.complete-alphabet-fn+1]
    pub fn register_alphabet(
        &mut self,
        cfg: &mut OstConfig,
        twolc_file: &TwolcFile,
    ) -> crate::error::Result<()> {
        let mut symbol_pairs: BTreeSet<SymbolPair> = BTreeSet::new();
        for p in &twolc_file.alphabet {
            symbol_pairs.insert((
                declared_symbol(&p.value.upper),
                declared_symbol(&p.value.lower),
            ));
        }
        // htwolcpre2's 'complete_alphabet()': add all explicit X:Y pairs used
        // anywhere in the grammar (rule centers, contexts, definitions —
        // 'where'-variables expanded first, as htwolcpre1 expanded them before
        // pre2 ran) which are missing from the Alphabet section, plus the
        // absolute word boundary pair. Undeclared ordinary symbols are legal
        // here: warn about possible typos (hfst#334), but retain C++ completion
        // semantics so existing grammars such as lang-fao still compile.
        let declared = self.collect_declared_symbols(twolc_file);
        let undeclared = self.collect_grammar_pairs(twolc_file, &declared, &mut symbol_pairs)?;
        let definitions: BTreeSet<Symbol> = twolc_file
            .definitions
            .iter()
            .map(|d| Symbol::new(d.value.name.as_str()))
            .collect();
        let mut in_source_order: Vec<_> = undeclared.into_iter().collect();
        in_source_order.sort_by_key(|(_, span)| span.start);
        for (symbol, span) in in_source_order {
            self.report_undeclared_symbol(symbol.as_str(), span, &declared, &definitions);
        }
        for implied in self.declare_set_centre_pairs(twolc_file, &mut symbol_pairs)? {
            self.report_implied_pairs(&implied);
        }
        symbol_pairs.insert((
            Symbol::new_static("__HFST_TWOLC_.#."),
            Symbol::new_static("__HFST_TWOLC_.#."),
        ));
        OtherSymbolTransducer::<B>::set_symbol_pairs(cfg, &symbol_pairs);
        Ok(())
    }

    /// A rule centre naming a set, as in 'Cns:0 <=> ...', controls one pair
    /// per member, the same as 'Cx:0 <=> ... where Cx in Cns'. Xerox twolc
    /// declares every pair a rule mentions, the where-variable form's included,
    /// so the centre declares its member pairs too. Upstream HFST declared none
    /// and then dropped the rule. Only centres declare: a set pair in a context
    /// filters the pairs already declared ('Cns:' is any declared pair with a
    /// consonant on top), and declaring from contexts is how a stray '%+Pl'
    /// silently adds '%+Pl:%+Pl' and the grammar over-generates. Returns what
    /// each centre added, for a warning; runs after ordinary completion so a
    /// pair the grammar spells out elsewhere is not reported.
    fn declare_set_centre_pairs(
        &self,
        file: &TwolcFile,
        pairs: &mut BTreeSet<SymbolPair>,
    ) -> crate::error::Result<Vec<ImpliedPairs>> {
        let diacritics: BTreeSet<Symbol> = file
            .diacritics
            .iter()
            .map(|d| declared_symbol(&d.value))
            .collect();
        let mut implied = Vec::new();
        for rule in &file.rules {
            let RuleCenter::Pair(centre) = &rule.value.center else {
                continue;
            };
            for vvm in self.variable_assignments(&rule.value)? {
                for p in centre {
                    let (CenterSide::Symbol(u), CenterSide::Symbol(l)) =
                        (&p.value.upper.value, &p.value.lower.value)
                    else {
                        continue;
                    };
                    let (upper, lower) = (substitute_symbol(u, &vvm), substitute_symbol(l, &vvm));
                    if !self.sets.contains_key(upper.as_str())
                        && !self.sets.contains_key(lower.as_str())
                    {
                        continue;
                    }
                    let mut added = Vec::new();
                    for x in self.set_of(&upper) {
                        for y in self.set_of(&lower) {
                            // A diacritic pairs only with itself.
                            if diacritics.contains(&x) && x != y {
                                continue;
                            }
                            let pair = (declared_symbol(&x), declared_symbol(&y));
                            if pairs.insert(pair.clone()) {
                                added.push(pair);
                            }
                        }
                    }
                    if !added.is_empty() {
                        implied.push(ImpliedPairs {
                            span: p.span.range.clone(),
                            upper,
                            lower,
                            added,
                        });
                    }
                }
            }
        }
        Ok(implied)
    }

    /// The set of symbols the grammar DECLARES: every symbol named on either
    /// side of an 'Alphabet'-section pair, every diacritic, every 'Sets' member
    /// and set name, every 'Definitions' name, plus the always-available
    /// internal / special symbols (the two-level epsilon, the 'Any' wildcard,
    /// the word boundaries and the diamond). A grammar pair whose side is not in
    /// this set names an undeclared symbol (hfst#334) and produces a warning
    /// while still completing the alphabet. Symbols are stored in their declared
    /// ('declared_symbol') form so they compare equal to the collected pairs.
    fn collect_declared_symbols(&self, file: &TwolcFile) -> BTreeSet<Symbol> {
        let mut declared: BTreeSet<Symbol> = BTreeSet::new();
        for p in &file.alphabet {
            declared.insert(declared_symbol(&p.value.upper));
            declared.insert(declared_symbol(&p.value.lower));
        }
        for d in &file.diacritics {
            declared.insert(declared_symbol(&d.value));
        }
        for s in &file.sets {
            declared.insert(Symbol::new(s.value.name.value.as_str()));
            for m in &s.value.members {
                declared.insert(declared_symbol(&m.value));
            }
        }
        for d in &file.definitions {
            declared.insert(Symbol::new(d.value.name.as_str()));
        }
        // Always-available internal / special symbols.
        for special in [
            TWOLC_EPSILON,
            HFST_EPSILON,
            TWOLC_UNKNOWN,
            HFST_UNKNOWN,
            TWOLC_IDENTITY,
            HFST_IDENTITY,
            TWOLC_DIAMOND,
            "__HFST_TWOLC_.#.",
        ] {
            declared.insert(Symbol::new_static(special));
        }
        declared
    }

    /// htwolcpre2's 'complete_alphabet()' / 'insert_alphabet_pairs()': collect
    /// every explicit concrete 'X:Y' pair appearing anywhere in the grammar
    /// (rule centers, rule contexts, 'except' contexts and definition bodies)
    /// into 'pairs', so pairs used only in rules (e.g. 'e:0') complete the
    /// Alphabet section. Rules with 'where'-variables are expanded first (as
    /// htwolcpre1 did before the completion pass ran), so variable centers
    /// like 'Vx:Vy' contribute their substituted pairs. One-sided pairs
    /// ('e:', ':i' — an 'Any' side) are skipped, as the C++ token scan skipped
    /// pairs with an internal '__HFST_TWOLC_?' side; pairs with a set-name
    /// side are skipped like the C++ 'is_set_pair' filter skipped the marked
    /// '__HFST_TWOLC_SET_NAME=' pairs everywhere they could be observed.
    ///
    /// A pair side that is neither in the declared vocabulary nor a set name is
    /// an undeclared symbol (hfst#334): its first source occurrence is recorded
    /// so 'register_alphabet' can warn without rejecting the grammar.
    fn collect_grammar_pairs(
        &mut self,
        file: &TwolcFile,
        declared: &BTreeSet<Symbol>,
        pairs: &mut BTreeSet<SymbolPair>,
    ) -> crate::error::Result<BTreeMap<Symbol, Range<usize>>> {
        let mut undeclared = BTreeMap::new();
        let empty_vvm = VariableValueMap::new();
        for def in &file.definitions {
            self.collect_regex_pairs(
                &def.value.body,
                &empty_vvm,
                declared,
                pairs,
                &mut undeclared,
            );
        }
        for rule in &file.rules {
            for vvm in self.variable_assignments(&rule.value)? {
                match &rule.value.center {
                    RuleCenter::Pair(ps) => {
                        for p in ps {
                            // A wildcard side ('a:', ':b', 'a:?') names no
                            // symbol to complete the Alphabet with: it stands
                            // for whatever the Alphabet already declares. Skip
                            // it, as the C++ token scan skipped pairs with an
                            // internal '__HFST_TWOLC_?' side, and as the regex
                            // walk below skips a 'TwolcRegex::Any' operand.
                            let (CenterSide::Symbol(u), CenterSide::Symbol(l)) =
                                (&p.value.upper.value, &p.value.lower.value)
                            else {
                                continue;
                            };
                            let upper = substitute_symbol(u, &vvm);
                            let lower = substitute_symbol(l, &vvm);
                            self.insert_grammar_pair(
                                (upper, lower),
                                &p.span.range,
                                declared,
                                pairs,
                                &mut undeclared,
                            );
                        }
                    }
                    RuleCenter::Regex(e) => {
                        self.collect_regex_pairs(e, &vvm, declared, pairs, &mut undeclared)
                    }
                }
                for ctx in rule
                    .value
                    .positive_contexts
                    .iter()
                    .chain(rule.value.negative_contexts.iter())
                {
                    self.collect_regex_pairs(&ctx.left, &vvm, declared, pairs, &mut undeclared);
                    self.collect_regex_pairs(&ctx.right, &vvm, declared, pairs, &mut undeclared);
                }
            }
        }
        Ok(undeclared)
    }

    /// The per-rule variable assignments the ['RuleVariables'] odometer yields
    /// (a single empty assignment when the rule has no 'where'-clause) — the
    /// same expansion ['expand_rule_variables'] drives rules with.
    fn variable_assignments(
        &self,
        rule: &AstTwolcRule,
    ) -> crate::error::Result<Vec<VariableValueMap>> {
        let rule_variables = match rule.variables.as_ref() {
            Some(blocks) if !blocks.is_empty() => self.build_rule_variables(blocks),
            _ => RuleVariables::new(),
        };
        if rule_variables.empty() {
            return Ok(vec![VariableValueMap::new()]);
        }
        let mut result: Vec<VariableValueMap> = Vec::new();
        let mut it = rule_variables.begin()?;
        let end = rule_variables.end()?;
        while it.ne(&end) {
            let mut vvm = VariableValueMap::new();
            it.set_values(&mut vvm);
            result.push(vvm);
            it.increment();
        }
        Ok(result)
    }

    /// The regex walk of ['collect_grammar_pairs']: record every 'Pair' node
    /// whose both sides are concrete symbols under the variable assignment.
    /// Undeclared pair sides (hfst#334) are also recorded for diagnostics.
    fn collect_regex_pairs(
        &self,
        e: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
        declared: &BTreeSet<Symbol>,
        pairs: &mut BTreeSet<SymbolPair>,
        undeclared: &mut BTreeMap<Symbol, Range<usize>>,
    ) {
        match &e.value {
            TwolcRegex::Pair { upper, lower } => {
                if let (Some(u), Some(l)) = (
                    Self::concrete_symbol(upper, vvm),
                    Self::concrete_symbol(lower, vvm),
                ) {
                    self.insert_grammar_pair((u, l), &e.span.range, declared, pairs, undeclared);
                }
            }
            TwolcRegex::Group(inner) | TwolcRegex::Optional(inner) => {
                self.collect_regex_pairs(inner, vvm, declared, pairs, undeclared);
            }
            TwolcRegex::Binary(_, l, r) => {
                self.collect_regex_pairs(l, vvm, declared, pairs, undeclared);
                self.collect_regex_pairs(r, vvm, declared, pairs, undeclared);
            }
            TwolcRegex::Unary(_, inner)
            | TwolcRegex::RepeatN(inner, _)
            | TwolcRegex::RepeatNToK(inner, _, _) => {
                self.collect_regex_pairs(inner, vvm, declared, pairs, undeclared);
            }
            // A bare symbol atom is an implicit identity pair. Upstream's
            // htwolcpre1 rewrote a lone 'X' into 'X __HFST_TWOLC_: X' before
            // completion ran ('PAIR: GRAMMAR_SYMBOL_SPACE',
            // htwolcpre1-parser.yy:481-493), so 'complete_alphabet' saw an
            // ordinary pair. Walking only explicit 'X:Y' nodes lost every
            // identity pair, so a grammar declaring its vocabulary in 'Sets'
            // and then using it bare got no 'a:a' and died at rule-compile
            // time with 'Unknown pair: a a'. Undeclared bare symbols also
            // contribute their pairs, with a warning from 'register_alphabet'.
            TwolcRegex::Symbol(s) => {
                let sym = substitute_symbol(s, vvm);
                let pair = (sym.clone(), sym);
                self.insert_grammar_pair(pair, &e.span.range, declared, pairs, undeclared);
            }
            TwolcRegex::Epsilon | TwolcRegex::Any => {}
        }
    }

    /// Insert one collected pair, skipping pairs with a set-name side (the
    /// 'is_set_pair' filter). A side that is neither declared nor a set name is
    /// an undeclared symbol (hfst#334): record its location for a warning,
    /// and complete the alphabet with the pair as C++ does.
    fn insert_grammar_pair(
        &self,
        (upper, lower): SymbolPair,
        span: &Range<usize>,
        declared: &BTreeSet<Symbol>,
        pairs: &mut BTreeSet<SymbolPair>,
        undeclared: &mut BTreeMap<Symbol, Range<usize>>,
    ) {
        // A bare '#' is built-in; escaped '%#' remains an ordinary literal.
        let boundaries = (upper == TWOLC_HASH, lower == TWOLC_HASH);
        // Completion records bare '#' as the plain relative-boundary symbol
        // needed by the rule evaluator's '[.#.:.#. | #:output]' disjunction.
        let upper = declared_symbol(&upper);
        let lower = declared_symbol(&lower);
        if self.sets.contains_key(upper.as_str()) || self.sets.contains_key(lower.as_str()) {
            return;
        }
        for (symbol, boundary) in [(&upper, boundaries.0), (&lower, boundaries.1)] {
            if !boundary && !declared.contains(symbol) {
                undeclared.entry(symbol.clone()).or_insert(span.clone());
            }
        }
        pairs.insert((upper, lower));
    }

    /// Resolve a pair side to a concrete internal symbol under the variable
    /// assignment, or None when the side is non-concrete ('Any' / a nested
    /// expression).
    fn concrete_symbol(e: &Spanned<TwolcRegex>, vvm: &VariableValueMap) -> Option<Symbol> {
        match &e.value {
            TwolcRegex::Symbol(s) => Some(substitute_symbol(s, vvm)),
            TwolcRegex::Epsilon => Some(Symbol::new_static(TWOLC_EPSILON)),
            TwolcRegex::Group(inner) => Self::concrete_symbol(inner, vvm),
            TwolcRegex::Pair { .. }
            | TwolcRegex::Any
            | TwolcRegex::Optional(_)
            | TwolcRegex::Binary(..)
            | TwolcRegex::Unary(..)
            | TwolcRegex::RepeatN(..)
            | TwolcRegex::RepeatNToK(..) => None,
        }
    }

    /// Register the 'Diacritics' section: publish the diacritic list to both the
    /// 'OtherSymbolTransducer' config and the grammar.
    pub fn register_diacritics(
        &mut self,
        cfg: &mut OstConfig,
        diacritics: &[Spanned<Symbol>],
        grammar: &mut TwolCGrammar<B>,
    ) {
        let list: SymbolRange = diacritics
            .iter()
            .map(|d| Symbol::from(d.value.clone()))
            .collect();
        grammar.define_diacritics(cfg, &list);
    }

    /// Register the 'Sets' section: record each set's ordered member list, so a
    /// 'Symbol' naming a set expands to the disjunction of its members.
    ///
    /// A member may itself name an earlier set, as in `Sgm = Vow Cns ;`, and is
    /// replaced by that set's members -- otherwise `Sgm` holds the literal
    /// symbols `Vow` and `Cns`, which are in no alphabet, and every pair set
    /// built from it is empty. Expansion is against the sets registered so far,
    /// so a member naming a later set, or naming nothing, stays literal.
    pub fn register_sets(&mut self, sets: &[Spanned<SetDefinition>]) {
        for s in sets {
            let mut members: Vec<Symbol> = Vec::new();
            for m in &s.value.members {
                // Self-reference would splice in the partially built list.
                match self.sets.get(m.value.as_str()) {
                    Some(n) if m.value != s.value.name.value => members.extend(n.iter().cloned()),
                    _ => members.push(Symbol::new(&m.value)),
                }
            }
            // A symbol reachable through two sets is listed once.
            let mut seen = std::collections::HashSet::new();
            members.retain(|sym| seen.insert(sym.clone()));
            let name = &s.value.name;
            self.set_spans
                .insert(name.value.clone(), name.span.range.clone());
            self.sets.insert(name.value.clone(), members);
        }
    }

    /// Register the 'Definitions' section: evaluate each named regex body to an
    /// ['OtherSymbolTransducer'] so a 'Symbol' naming a definition expands to
    /// it (mirrors the C++ 'NameToRegexMap').
    pub fn register_definitions(
        &mut self,
        cfg: &OstConfig,
        defs: &[Spanned<TwolcDefinition>],
    ) -> crate::error::Result<()> {
        let mut first_error = None;
        for d in defs {
            match self.eval_regex(cfg, &d.value.body) {
                Ok(t) => {
                    self.definitions.insert(d.value.name.clone(), t);
                }
                Err(e) => {
                    self.report_failure(&e, &d.span.range, "This definition");
                    first_error.get_or_insert(e);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Drive one ['TwolcRule']: expand its 'where'-variables into concrete
    /// rules and feed each into the grammar via the matching 'add_rule'
    /// overload.
    pub fn drive_rule(
        &mut self,
        cfg: &OstConfig,
        rule: &AstTwolcRule,
        grammar: &mut TwolCGrammar<B>,
    ) -> crate::error::Result<()> {
        for concrete in self.expand_rule_variables(cfg, rule)? {
            match concrete.center {
                // htwolcpre3-parser dispatched on the expanded center size:
                // a single pair takes the SymbolPair overload (rule named
                // as-is), only genuine multi-pair centers take the vector
                // overload with its per-pair ' CENTER=in:out' name suffixes.
                CenterEval::Pairs(pairs) if pairs.len() == 1 => {
                    grammar.add_rule_pair(
                        cfg,
                        &concrete.name,
                        &pairs[0],
                        concrete.oper,
                        &concrete.contexts,
                    )?;
                }
                CenterEval::Pairs(pairs) => {
                    grammar.add_rule_pairs(
                        cfg,
                        &concrete.name,
                        &pairs,
                        concrete.oper,
                        &concrete.contexts,
                    )?;
                }
                CenterEval::Regex(center) => {
                    grammar.add_rule_regex(
                        cfg,
                        &concrete.name,
                        &center,
                        concrete.oper,
                        &concrete.contexts,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Expand a ['TwolcRule']'s 'where'-blocks into concrete rules using the
    /// ['RuleVariables'] odometer. With no 'where'-clause the rule yields one
    /// concrete rule with an empty variable assignment.
    pub fn expand_rule_variables(
        &mut self,
        cfg: &OstConfig,
        rule: &AstTwolcRule,
    ) -> crate::error::Result<Vec<ConcreteRule<B>>> {
        let regex_center = matches!(rule.center, RuleCenter::Regex(_));
        let oper = Self::operator_of(rule.operator, regex_center);

        // No variables: emit the rule once with no substitutions. The variable
        // markers in names only matter when a 'where'-clause is present, so the
        // rule name is used as-is.
        let blocks_opt = rule.variables.as_ref();
        let rule_variables = match blocks_opt {
            Some(blocks) if !blocks.is_empty() => self.build_rule_variables(blocks),
            _ => RuleVariables::new(),
        };

        let mut result: Vec<ConcreteRule<B>> = Vec::new();

        if rule_variables.empty() {
            let empty_vvm = VariableValueMap::new();
            if let Some(cr) = self.build_concrete_rule(cfg, rule, oper, &empty_vvm)? {
                result.push(cr);
            }
            return Ok(result);
        }

        // Odometer over the cross-product of the where-blocks.
        let mut it = rule_variables.begin()?;
        let end = rule_variables.end()?;
        while it.ne(&end) {
            let mut vvm = VariableValueMap::new();
            it.set_values(&mut vvm);
            if let Some(cr) = self.build_concrete_rule(cfg, rule, oper, &vvm)? {
                result.push(cr);
            }
            it.increment();
        }
        Ok(result)
    }

    /// Build a single ['ConcreteRule'] from a rule template and a variable
    /// assignment: substitute the assignment into the name/center symbols, then
    /// evaluate the center and contexts.
    fn build_concrete_rule(
        &mut self,
        cfg: &OstConfig,
        rule: &AstTwolcRule,
        oper: Operator,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<Option<ConcreteRule<B>>> {
        // Compose the subcase-qualified name. The C++ rule template carried a
        // '__HFST_TWOLC_RULE_NAME' marker that 'RuleSymbolVector' rewrote with
        // the 'SUBCASE:'/'var=value' markers; here the rule's own name plays
        // that role and the marker rewrite is applied directly.
        // The parser keeps the name's escapes; upstream's first pass unescapes
        // it like any other token, so '"%{p1%}:0"' is stored as '"{p1}:0"'.
        let name = build_rule_name(&crate::string_manipulation::unescape(&rule.name)?, vvm);

        let center = self.eval_center(cfg, &rule.center, vvm)?;
        let contexts =
            self.eval_contexts(cfg, &rule.positive_contexts, &rule.negative_contexts, vvm)?;
        Ok(Some(ConcreteRule {
            name,
            center,
            oper,
            contexts,
        }))
    }

    /// AST 'where'-blocks -> ['RuleVariables'] (the C++
    /// 'set_variable'/'add_values'/'set_matcher' sequence per block).
    pub fn build_rule_variables(&self, blocks: &[VariableBlock]) -> RuleVariables {
        let mut rv = RuleVariables::new();
        for block in blocks {
            for assignment in block.assignments.iter() {
                rv.set_variable(&assignment.name.value);
                // A value that names a Set expands to the set's members: the
                // 'where'-variable iterates over them ('where Cx in (DelCns)'
                // ranges over g8, m8, n8, h8). nfst_twolc keeps the where-clause
                // verbatim (the values are raw source strings), so the set
                // reference is resolved here — a plain symbol is its own
                // singleton via 'set_of'.
                let expanded: Vec<String> = assignment
                    .values
                    .iter()
                    .flat_map(|v| self.set_of(&v.value).into_iter().map(|s| s.to_string()))
                    .collect();
                rv.add_values(&expanded);
            }
            rv.set_matcher(matcher_from_var_matcher(block.matcher));
        }
        rv
    }

    /// Map a ['RuleOp'] + center kind to a ['Operator']. Pair centers use the
    /// plain operators; regex centers (':[ E ]:') use the 'RE_*' variants. The
    /// '<=>' operator maps to 'LEFT_RIGHT'/'RE_LEFT_RIGHT', '/<=' to
    /// 'NOT_LEFT'/'RE_NOT_LEFT'.
    pub fn operator_of(op: RuleOp, regex_center: bool) -> Operator {
        match (op, regex_center) {
            (RuleOp::Right, false) => Operator::RIGHT,
            (RuleOp::Left, false) => Operator::LEFT,
            (RuleOp::LeftRight, false) => Operator::LEFT_RIGHT,
            (RuleOp::NotLeft, false) => Operator::NOT_LEFT,
            (RuleOp::Right, true) => Operator::RE_RIGHT,
            (RuleOp::Left, true) => Operator::RE_LEFT,
            (RuleOp::LeftRight, true) => Operator::RE_LEFT_RIGHT,
            (RuleOp::NotLeft, true) => Operator::RE_NOT_LEFT,
        }
    }
}

/// Map a declared (Alphabet-section) symbol into the form the alphabet
/// stores. The nfst-twolc lexer performs htwolcpre1's marker renamings itself
/// — bare '0' / '#' / '.#.' arrive as '__HFST_TWOLC_'-namespaced markers,
/// their '%'-escaped spellings as plain literal symbols — so most symbols
/// pass through untouched. The one declaration-time mapping is htwolcpre2's
/// alphabet completion: a declared bare '#' becomes the plain '#' pair
/// symbol (the RELATIVE word boundary the rules' '#'-split disjoins with);
/// the epsilon and absolute-boundary markers stay markers.
fn declared_symbol(sym: &str) -> Symbol {
    if sym == TWOLC_HASH {
        Symbol::new_static("#")
    } else {
        Symbol::new(sym)
    }
}

/// Substitute a single symbol via a variable assignment: if 'sym' is a variable
/// in 'vvm', return its value, else 'sym' unchanged (the 'RuleSymbolVector'
/// per-symbol 'vvm' lookup). No renaming happens here: the nfst-twolc lexer
/// already delivers bare specials as '__HFST_TWOLC_' markers and escaped
/// specials as plain literals, and the distinction must survive to the
/// evaluation sites (the '#'-split dispatch).
pub(super) fn substitute_symbol(sym: &str, vvm: &VariableValueMap) -> Symbol {
    match vvm.get(sym) {
        Some(val) => Symbol::new(val),
        None => Symbol::new(sym),
    }
}

/// Build a subcase-qualified rule name from a template name + a variable
/// assignment, reproducing the C++ token shape end to end: the htwolcpre1
/// lexer kept the rule name's surrounding double quotes in the token, and
/// 'RuleSymbolVector::replace_variables' inserted the 'SUBCASE:'/'var=value'
/// markers *before the closing quote*. 'TwolCGrammar::get_original_name'
/// (split at 'SUBCASE:') therefore recovers '"name ' — opening quote kept,
/// trailing space, closing quote lost to the cut — for where-rules, and the
/// intact '"name"' for rules without variables. The stored transducer names
/// carry these quotes, so byte-parity with C++ twolc output depends on this
/// shape.
fn build_rule_name(name: &str, vvm: &VariableValueMap) -> String {
    if vvm.is_empty() {
        return format!("\"{name}\"");
    }
    let mut result = format!("\"{name} SUBCASE:");
    for (k, v) in vvm.iter() {
        result.push_str(&format!(" {k}={v}"));
    }
    result.push('"');
    result
}
