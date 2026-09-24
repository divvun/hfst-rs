//! OtherSymbolTransducer: its alphabet config, constructors and the language
//! operations the rules are built from.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::implementations::HfstState;

// ───────────────────────────────────────────────────────────────────────────
// OtherSymbolTransducer — 'rule_src/OtherSymbolTransducer.{h,cc}' bodies.
//
// The C++ class kept its alphabet config in five 'static' members; this port
// folds them into a single per-compile 'OstConfig', created by
// 'TwolcCompiler::compile' and threaded by '&OstConfig' (reads) / '&mut
// OstConfig' (the three config setters) through the rule/grammar walk. This
// removes the previous 'OST_CONFIG' thread-global mutable state.
// ───────────────────────────────────────────────────────────────────────────

impl Default for OstConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl OstConfig {
    pub(crate) fn new() -> Self {
        OstConfig {
            input_symbols: BTreeSet::new(),
            output_symbols: BTreeSet::new(),
            diacritics: BTreeSet::new(),
            symbol_pairs: BTreeSet::new(),
        }
    }
}

impl<B: AlgebraBackend> OtherSymbolTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Static config (writes the thread-local OstConfig) -----
    // -------------------------------------------------------------------------

    /// 'static void set_symbol_pairs(const HandySet<SymbolPair> &symbol_pairs)'.
    ///
    /// Clears the three derived sets, re-inserts the supplied pairs, splits them
    /// into the input/output symbol sets, and finally adds the
    /// '(TWOLC_DIAMOND, TWOLC_DIAMOND)' pair.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-symbol-pairs-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-symbol-pairs-fn]
    pub fn set_symbol_pairs(cfg: &mut OstConfig, symbol_pairs: &BTreeSet<SymbolPair>) {
        cfg.input_symbols.clear();
        cfg.output_symbols.clear();
        cfg.symbol_pairs.clear();
        for it in symbol_pairs.iter() {
            cfg.symbol_pairs.insert(it.clone());
        }
        for it in symbol_pairs.iter() {
            cfg.input_symbols.insert(it.0.clone());
            cfg.output_symbols.insert(it.1.clone());
        }
        cfg.symbol_pairs.insert((
            Symbol::new_static(TWOLC_DIAMOND),
            Symbol::new_static(TWOLC_DIAMOND),
        ));
    }

    /// 'static void define_diacritics(const std::vector<std::string> &diacritics)'.
    ///
    /// Records the diacritics and erases their identity / 'X:0' pairs and the
    /// matching input/output symbols from the alphabet config.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.define-diacritics-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.define-diacritics-fn]
    pub fn define_diacritics(cfg: &mut OstConfig, diacritics: &[Symbol]) {
        cfg.diacritics.clear();
        for d in diacritics.iter() {
            cfg.diacritics.insert(d.clone());
        }
        // Iterate over a snapshot of the diacritics so the mutable erases
        // below do not alias the loop (the C++ iterates 'diacritics' while
        // mutating 'symbol_pairs'/'input_symbols'/'output_symbols').
        let diac: Vec<Symbol> = cfg.diacritics.iter().cloned().collect();
        for it in diac.iter() {
            cfg.symbol_pairs.remove(&(it.clone(), it.clone()));
            cfg.symbol_pairs
                .remove(&(it.clone(), Symbol::new_static(TWOLC_EPSILON)));
            cfg.input_symbols.remove(it);
            cfg.output_symbols.remove(it);
        }
    }

    // ('static void set_transducer_type(ImplementationType transducer_type)'
    //  and the 'transducer_type' config read are gone: the transducer type is
    //  the backend type parameter 'B' — [dec:hfst:monomorphic-backends].)
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-transducer-type-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-transducer-type-fn]

    // -------------------------------------------------------------------------
    // ----- Constructors -----
    // -------------------------------------------------------------------------

    /// 'OtherSymbolTransducer(void)' — empty transducer of the configured type.
    pub fn new(_cfg: &OstConfig) -> crate::error::Result<Self> {
        Ok(OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new(),
        })
    }

    /// 'OtherSymbolTransducer(const std::string &i_symbol,
    ///  const std::string &o_symbol)' — build 'input_symbol:output_symbol'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.other-symbol-transducer-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.other-symbol-transducer-fn]
    pub fn new_pair(cfg: &OstConfig, i_symbol: &str, o_symbol: &str) -> crate::error::Result<Self> {
        let mut input_symbol = Symbol::new(i_symbol);
        let mut output_symbol = Symbol::new(o_symbol);

        if input_symbol == TWOLC_UNKNOWN {
            input_symbol = Symbol::new_static(HFST_UNKNOWN);
        }
        if output_symbol == TWOLC_UNKNOWN {
            output_symbol = Symbol::new_static(HFST_UNKNOWN);
        }

        let mut this = OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new(),
        };
        this.check_pair(cfg, &input_symbol, &output_symbol);
        if this.is_broken {
            return Ok(this);
        }
        if input_symbol == HFST_UNKNOWN && output_symbol == HFST_UNKNOWN {
            this.transducer = Self::get_universal(cfg)?.transducer;
        } else {
            let mut fst =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&this.transducer)?;
            let target = fst.add_state_new();
            fst.set_final_weight(target, &0.0);

            if input_symbol == HFST_UNKNOWN {
                let input_symbols = cfg.input_symbols.iter().cloned().collect::<Vec<_>>();
                let pairs = cfg.symbol_pairs.clone();
                for it in input_symbols.iter() {
                    if pairs.contains(&(it.clone(), output_symbol.clone())) {
                        let tr = HfstBasicTransition::new_symbols(
                            target,
                            it.clone(),
                            output_symbol.clone(),
                            0.0,
                            fst.coder_mut(),
                        );
                        fst.add_transition(0, &tr, true);
                    }
                }
            } else if output_symbol == HFST_UNKNOWN {
                let output_symbols = cfg.output_symbols.iter().cloned().collect::<Vec<_>>();
                let pairs = cfg.symbol_pairs.clone();
                for it in output_symbols.iter() {
                    if pairs.contains(&(input_symbol.clone(), it.clone())) {
                        let tr = HfstBasicTransition::new_symbols(
                            target,
                            input_symbol.clone(),
                            it.clone(),
                            0.0,
                            fst.coder_mut(),
                        );
                        fst.add_transition(0, &tr, true);
                    }
                }
            } else {
                let tr = HfstBasicTransition::new_symbols(
                    target,
                    input_symbol.clone(),
                    output_symbol.clone(),
                    0.0,
                    fst.coder_mut(),
                );
                fst.add_transition(0, &tr, true);
            }
            this.transducer = HfstTransducer::new_from_basic(&fst)?;
        }
        Ok(this)
    }

    /// 'OtherSymbolTransducer(const std::string &sym)' — build 'symbol:symbol'
    /// (or 'symbol:0' for a diacritic).
    pub fn new_symbol(cfg: &OstConfig, sym: &str) -> crate::error::Result<Self> {
        let mut symbol = Symbol::new(sym);
        if symbol == TWOLC_UNKNOWN {
            symbol = Symbol::new_static(HFST_UNKNOWN);
        }

        let is_diacritic = cfg.diacritics.contains(&symbol);

        let mut this = OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new(),
        };
        if is_diacritic {
            this.check_pair(cfg, &symbol, TWOLC_EPSILON);
        } else {
            this.check_pair(cfg, &symbol, &symbol);
        }

        if this.is_broken {
            return Ok(this);
        }

        if symbol == HFST_UNKNOWN {
            this.transducer = Self::get_universal(cfg)?.transducer;
        } else if is_diacritic {
            this.transducer = HfstTransducer::new_symbol_pair(&symbol, TWOLC_EPSILON)?;
        } else {
            this.transducer = HfstTransducer::new_symbol(&symbol)?;
        }
        Ok(this)
    }

    // -------------------------------------------------------------------------
    // ----- Protected helpers -----
    // -------------------------------------------------------------------------

    /// 'void check_pair(const std::string &input_symbol,
    ///  const std::string &output_symbol)' — set 'is_broken' if the pair is not
    /// in the configured alphabet (and report to stderr).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.check-pair-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.check-pair-fn]
    pub fn check_pair(&mut self, cfg: &OstConfig, input_symbol: &str, output_symbol: &str) {
        {
            // Always-valid pairs, each set 'is_broken = false':
            //   id:id, other:other, eps:eps, 0:0, diamond:diamond.
            if input_symbol == TWOLC_IDENTITY
                || (input_symbol == HFST_UNKNOWN && output_symbol == HFST_UNKNOWN)
                || (input_symbol == TWOLC_EPSILON && output_symbol == TWOLC_EPSILON)
                || (input_symbol == HFST_EPSILON && output_symbol == HFST_EPSILON)
                || input_symbol == TWOLC_DIAMOND
            {
                self.is_broken = false;
            }
            // other:X is valid, iff X is an output symbol or 0.
            else if input_symbol == HFST_UNKNOWN {
                self.is_broken =
                    !(output_symbol == TWOLC_EPSILON || cfg.output_symbols.contains(output_symbol));
            }
            // X:other is valid, iff X is an input symbol or 0.
            else if output_symbol == HFST_UNKNOWN {
                self.is_broken =
                    !(input_symbol == TWOLC_EPSILON || cfg.input_symbols.contains(input_symbol));
            }
            // 0:X is valid, iff X is an output symbol.
            else if input_symbol == TWOLC_EPSILON {
                self.is_broken = !cfg.output_symbols.contains(output_symbol);
            }
            // X:0 is valid, iff X is an input symbol or a diacritic.
            else if output_symbol == TWOLC_EPSILON {
                self.is_broken = !cfg.input_symbols.contains(input_symbol);
            }
            // X:X is valid if X is a diacritic.
            else if cfg.diacritics.contains(input_symbol) {
                self.is_broken = false;
            }
            // X:Y is valid iff it has been declared in the alphabet.
            else {
                self.is_broken = !cfg
                    .symbol_pairs
                    .contains(&(Symbol::new(input_symbol), Symbol::new(output_symbol)));
            }
        }
        if self.is_broken {
            error!("Unknown pair: {} {}", input_symbol, output_symbol);
        }
    }

    /// 'void add_diamond_transition(void)' — 'add_symbol_to_alphabet(DIAMOND)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-diamond-transition-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-diamond-transition-fn]
    pub fn add_diamond_transition(&mut self, cfg: &OstConfig) -> crate::error::Result<()> {
        self.add_symbol_to_alphabet(cfg, TWOLC_DIAMOND)?;
        Ok(())
    }

    /// 'static bool empty(const HfstBasicTransducer &fsm)' — true iff no
    /// reachable final state (the C++ scans every state for a final marker).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.empty-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.empty-fn]
    pub fn empty(fsm: &HfstBasicTransducer) -> bool {
        for (state, _) in fsm.iter().enumerate() {
            let state = state as HfstState;
            if fsm.is_final_state(state) {
                return false;
            }
        }
        true
    }

    // -------------------------------------------------------------------------
    // ----- Other instance / static ops -----
    // -------------------------------------------------------------------------

    /// 'OtherSymbolTransducer &harmonize_diacritics(OtherSymbolTransducer &t)'.
    ///
    /// For each diacritic present in 't''s alphabet but missing from '*this''s,
    /// add a 'd:d' self-loop-style transition alongside every 'TWOLC_IDENTITY'
    /// transition leaving a state.
    pub fn harmonize_diacritics(
        &mut self,
        cfg: &OstConfig,
        t: &mut OtherSymbolTransducer<B>,
    ) -> &mut Self {
        // [spec:hfst:def:other-symbol-transducer.basic-fn]
        // [spec:hfst:sem:other-symbol-transducer.basic-fn]
        let mut basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(
            &self.transducer,
        )
        .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        let alphabet: BTreeSet<Symbol> = basic.get_alphabet().clone();

        let basic_t = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&t.transducer)
            .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        let t_alphabet: BTreeSet<Symbol> = basic_t.get_alphabet().clone();

        let mut missing_diacritics: BTreeSet<Symbol> = BTreeSet::new();
        for it in cfg.diacritics.iter() {
            if t_alphabet.contains(it) && !alphabet.contains(it) {
                missing_diacritics.insert(it.clone());
            }
        }
        if missing_diacritics.is_empty() {
            return self;
        }

        // For every state, if it has a TWOLC_IDENTITY-input transition, add a
        // diacritic self-pair transition to that transition's target for each
        // missing diacritic (the C++ 'break's after the first identity arc).
        let num_states = basic.states_and_transitions().len();
        for s in 0..num_states {
            let mut identity_target: Option<HfstState> = None;
            for jt in basic
                .index(s as HfstState)
                .expect("s is a valid state of this transducer")
                .iter()
            {
                if jt.get_input_symbol(basic.coder()) == TWOLC_IDENTITY {
                    identity_target = Some(jt.get_target_state());
                    break;
                }
            }
            if let Some(target) = identity_target {
                for kt in missing_diacritics.iter() {
                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        kt.clone(),
                        kt.clone(),
                        0.0,
                        basic.coder_mut(),
                    );
                    basic.add_transition(s as HfstState, &tr, true);
                }
            }
        }
        self.transducer = HfstTransducer::new_from_basic(&basic)
            .expect("constructing a transducer from a valid basic transducer cannot fail");
        self
    }

    /// 'static OtherSymbolTransducer get_context(OtherSymbolTransducer &left,
    ///  OtherSymbolTransducer &right)' — build '?* X D ?* D Y ?*'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-context-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-context-fn]
    pub fn get_context(
        cfg: &OstConfig,
        left: &mut OtherSymbolTransducer<B>,
        right: &mut OtherSymbolTransducer<B>,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut universal = Self::get_universal(cfg)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;
        let mut result = universal.clone();
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;

        result.concatenate(cfg, left)?;
        result.concatenate(cfg, &diamond)?;
        result.concatenate(cfg, &universal)?;
        result.concatenate(cfg, &diamond)?;
        result.concatenate(cfg, right)?;
        result.concatenate(cfg, &universal)?;
        Ok(result)
    }

    /// 'static OtherSymbolTransducer get_universal(void)' — a one-symbol
    /// transducer recognizing the identity pair plus every configured pair
    /// except the diamond.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-universal-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-universal-fn]
    pub fn get_universal(cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let universal = OtherSymbolTransducer::<B> {
            is_broken: false,
            transducer: HfstTransducer::new(),
        };
        let mut fst =
            ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&universal.transducer)?;
        let target = fst.add_state_new();
        fst.set_final_weight(target, &0.0);
        let tr = HfstBasicTransition::new_symbols(
            target,
            Symbol::new_static(TWOLC_IDENTITY),
            Symbol::new_static(TWOLC_IDENTITY),
            0.0,
            fst.coder_mut(),
        );
        fst.add_transition(0, &tr, true);
        let pairs = cfg.symbol_pairs.clone();
        for it in pairs.iter() {
            if it.0 == TWOLC_DIAMOND {
                continue;
            }
            let tr = HfstBasicTransition::new_symbols(
                target,
                it.0.clone(),
                it.1.clone(),
                0.0,
                fst.coder_mut(),
            );
            fst.add_transition(0, &tr, true);
        }
        Ok(OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new_from_basic(&fst)?,
        })
    }

    /// 'void add_symbol_to_alphabet(const std::string &symbol)' — round-trip
    /// through the basic transducer to add 'symbol' (prevents harmonization).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-symbol-to-alphabet-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-symbol-to-alphabet-fn]
    pub fn add_symbol_to_alphabet(
        &mut self,
        _cfg: &OstConfig,
        symbol: &str,
    ) -> crate::error::Result<()> {
        let mut mutable_transducer =
            ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&self.transducer)?;
        mutable_transducer.add_symbol_to_alphabet(&Symbol::new(symbol));
        self.transducer = HfstTransducer::new_from_basic(&mutable_transducer)?;
        Ok(())
    }

    /// 'void remove_diacritics_from_output(void)' — for each diacritic, rewrite
    /// 'd:d' to 'd:0'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.remove-diacritics-from-output-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.remove-diacritics-from-output-fn]
    pub fn remove_diacritics_from_output(&mut self, cfg: &OstConfig) -> crate::error::Result<()> {
        let diac = cfg.diacritics.iter().cloned().collect::<Vec<_>>();
        for it in diac.iter() {
            self.apply_subst_pair(
                cfg,
                &(it.clone(), it.clone()),
                &(it.clone(), Symbol::new_static(TWOLC_EPSILON)),
            )?;
        }
        Ok(())
    }

    /// 'OtherSymbolTransducer &add_info_symbol(const std::string &info_symbol)'
    /// — append 'info_symbol' to the wrapped transducer's name.
    pub fn add_info_symbol(&mut self, info_symbol: &str) -> crate::error::Result<&mut Self> {
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut name = self.transducer.get_name();
        if !name.is_empty() {
            name += " & ";
        }
        name += info_symbol;
        self.transducer.set_name(&name);
        Ok(self)
    }

    /// 'static void add_transition(HfstBasicTransducer &center_t,
    ///  size_t source, size_t target, const std::string &input,
    ///  const std::string &output)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-transition-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-transition-fn]
    pub fn add_transition(
        center_t: &mut HfstBasicTransducer,
        source_state: usize,
        target_state: usize,
        input: &str,
        output: &str,
    ) {
        let tr = HfstBasicTransition::new_symbols(
            target_state as HfstState,
            Symbol::new(input),
            Symbol::new(output),
            0.0,
            center_t.coder_mut(),
        );
        center_t.add_transition(source_state as HfstState, &tr, true);
    }

    /// 'static void set_final(HfstBasicTransducer &center_t, size_t state)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-final-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-final-fn]
    pub fn set_final(center_t: &mut HfstBasicTransducer, state: usize) {
        center_t.set_final_weight(state as HfstState, &0.0);
    }

    /// 'OtherSymbolTransducer get_inverse_of_upper_projection(void)' — a copy in
    /// which every output symbol is replaced by an other-symbol (abstracts a
    /// center to its input side).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-inverse-of-upper-projection-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-inverse-of-upper-projection-fn]
    pub fn get_inverse_of_upper_projection(
        &self,
        cfg: &OstConfig,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let fst = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&self.transducer)?;
        let mut new_fst = HfstBasicTransducer::new();

        let output_symbols = cfg.output_symbols.iter().cloned().collect::<Vec<_>>();
        let symbol_pairs = cfg.symbol_pairs.clone();

        let num_states = fst.states_and_transitions().len();
        for state in 0..num_states {
            let st = state as HfstState;
            new_fst.add_state(st);
            if fst.is_final_state(st) {
                let w = fst.get_final_weight(st)?;
                new_fst.set_final_weight(st, &w);
            }
            for jt in fst.index(st)?.iter() {
                let input = jt.get_transition_data().get_input_symbol(fst.coder());
                let output = jt.get_transition_data().get_output_symbol(fst.coder());
                let target = jt.get_target_state();
                if input == HFST_UNKNOWN {
                    Self::add_transition(
                        &mut new_fst,
                        state,
                        target as usize,
                        HFST_UNKNOWN,
                        HFST_UNKNOWN,
                    );
                    for kt in output_symbols.iter() {
                        // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.has-symbol-fn]
                        // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.has-symbol-fn]
                        if fst.get_alphabet().contains(kt.as_str()) {
                            Self::add_transition(
                                &mut new_fst,
                                state,
                                target as usize,
                                HFST_UNKNOWN,
                                kt,
                            );
                        }
                    }
                } else {
                    Self::add_transition(&mut new_fst, state, target as usize, &input, &output);
                    for kt in symbol_pairs.iter() {
                        if kt.0 == input && fst.get_alphabet().contains(kt.1.as_str()) {
                            Self::add_transition(
                                &mut new_fst,
                                state,
                                target as usize,
                                &input,
                                &kt.1,
                            );
                        }
                    }
                    if input == TWOLC_EPSILON {
                        Self::add_transition(
                            &mut new_fst,
                            state,
                            target as usize,
                            HFST_EPSILON,
                            HFST_EPSILON,
                        );
                        Self::add_transition(
                            &mut new_fst,
                            state,
                            target as usize,
                            TWOLC_EPSILON,
                            HFST_UNKNOWN,
                        );
                    } else if input != TWOLC_DIAMOND {
                        Self::add_transition(
                            &mut new_fst,
                            state,
                            target as usize,
                            &input,
                            HFST_UNKNOWN,
                        );
                    }
                }
            }
        }
        let mut copy = self.clone();
        copy.transducer = HfstTransducer::new_from_basic(&new_fst)?;
        copy.apply_zero(cfg, |t| {
            t.minimize()?;
            Ok(())
        })?;
        Ok(copy)
    }

    /// 'OtherSymbolTransducer &contained(void)' — '?* X ?*'.
    pub fn contained(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        // [spec:hfst:def:other-symbol-transducer.universal-fn]
        // [spec:hfst:sem:other-symbol-transducer.universal-fn]
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;
        let mut result = universal.clone();
        result.concatenate(cfg, self)?;
        result.concatenate(cfg, &universal)?;
        *self = result;
        Ok(self)
    }

    /// 'OtherSymbolTransducer &contained_once(void)' —
    /// '?* X ?* - ?* X ?* X ?*'.
    pub fn contained_once(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;
        let mut result1 = universal.clone();
        result1.concatenate(cfg, self)?;
        result1.concatenate(cfg, &universal)?;
        let mut result2 = universal.clone();
        result2.concatenate(cfg, self)?;
        result2.concatenate(cfg, &universal)?;
        result2.concatenate(cfg, self)?;
        result2.concatenate(cfg, &universal)?;
        result1.subtract(cfg, &result2)?;
        *self = result1;
        Ok(self)
    }

    /// 'OtherSymbolTransducer &negated(void)' — '?* - X'.
    pub fn negated(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;
        universal.subtract(cfg, self)?;
        *self = universal;
        Ok(self)
    }

    /// 'OtherSymbolTransducer &term_complemented(void)' — '? - X'.
    pub fn term_complemented(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.subtract(cfg, self)?;
        *self = universal;
        Ok(self)
    }

    /// 'HfstTransducer get_transducer(void) const'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-transducer-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-transducer-fn]
    pub fn get_transducer(&self) -> crate::error::Result<HfstTransducer<B>> {
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        HfstTransducer::new_copy(&self.transducer)
    }

    /// 'void get_initial_transition_pairs(SymbolPairVector &pair_container)
    ///  const' — collect the symbol pairs on the transitions leaving the start
    /// state.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-initial-transition-pairs-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-initial-transition-pairs-fn]
    pub fn get_initial_transition_pairs(&self) -> crate::error::Result<SymbolPairVector> {
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut pair_container = SymbolPairVector::new();
        let fst = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&self.transducer)?;
        for jt in fst
            .index(0)
            .expect("s is a valid state of this transducer")
            .iter()
        {
            let input = jt.get_transition_data().get_input_symbol(fst.coder());
            let output = jt.get_transition_data().get_output_symbol(fst.coder());
            pair_container.push((input, output));
        }
        Ok(pair_container)
    }

    /// 'bool is_empty_intersection(const OtherSymbolTransducer &another,
    ///  StringVector &v)' — true iff '*this' and 'another' share no string;
    /// when non-empty, the first common string is stored in 'v'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-empty-intersection-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-empty-intersection-fn]
    pub fn is_empty_intersection(
        &self,
        another: &OtherSymbolTransducer<B>,
        v: &mut StringVector,
    ) -> bool {
        let this_fst = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(
            &self.transducer,
        )
        .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        let another_fst =
            ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&another.transducer)
                .expect(
                    "hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail",
                );
        let mut visited_pairs: BTreeSet<(HfstState, HfstState)> = BTreeSet::new();
        visited_pairs.insert((0, 0));
        !have_common_string(0, 0, &this_fst, &another_fst, &mut visited_pairs, v)
    }

    /// 'bool is_subset(const OtherSymbolTransducer &another)' — true iff
    /// 'another' is a subset of '*this' (computed as 'another - *this' empty).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-subset-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-subset-fn]
    pub fn is_subset(
        &self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<bool> {
        // Do this properly later.. (preserved C++ comment.)
        let mut another_fst = another.clone();
        another_fst.subtract(cfg, self)?;
        let internal = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(
            &another_fst.get_transducer()?,
        )?;
        Ok(Self::empty(&internal))
    }

    /// 'bool is_empty(void) const' — true iff the wrapped transducer has no
    /// reachable final state.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-empty-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-empty-fn]
    pub fn is_empty(&self) -> bool {
        Self::empty(
            &ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&self.transducer)
                .expect(
                    "hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail",
                ),
        )
    }
}

/// 'OtherSymbolTransducer(const OtherSymbolTransducer &another)' /
/// 'operator=' — copy the 'is_broken' flag and the wrapped transducer.
impl<B: AlgebraBackend> Clone for OtherSymbolTransducer<B> {
    fn clone(&self) -> Self {
        OtherSymbolTransducer {
            is_broken: self.is_broken,
            transducer: HfstTransducer::new_copy(&self.transducer)
                .expect("copying a valid transducer cannot fail"),
        }
    }
}

/// 'bool have_common_string(HfstState state1, HfstState state2,
///  const HfstBasicTransducer &fst1, const HfstBasicTransducer &fst2,
///  HandySet<StatePair> &visited_pairs, StringVector &v)' — depth-first search
/// for a string accepted by both transducers, recording the path in 'v'.
// [spec:hfst:def:other-symbol-transducer.have-common-string-fn]
// [spec:hfst:sem:other-symbol-transducer.have-common-string-fn]
fn have_common_string(
    state1: HfstState,
    state2: HfstState,
    fst1: &HfstBasicTransducer,
    fst2: &HfstBasicTransducer,
    visited_pairs: &mut BTreeSet<(HfstState, HfstState)>,
    v: &mut StringVector,
) -> bool {
    if fst1.is_final_state(state1) && fst2.is_final_state(state2) {
        return true;
    }

    let fst1_transitions = fst1
        .index(state1)
        .expect("s is a valid state of this transducer");
    let fst2_transitions = fst2
        .index(state2)
        .expect("s is a valid state of this transducer");

    let mut fst1_transition_map: BTreeMap<SymbolPair, HfstState> = BTreeMap::new();
    for it in fst1_transitions.iter() {
        fst1_transition_map.insert(
            (
                it.get_input_symbol(fst1.coder()),
                it.get_output_symbol(fst1.coder()),
            ),
            it.get_target_state(),
        );
    }

    for it in fst2_transitions.iter() {
        let symbol_pair: SymbolPair = (
            it.get_input_symbol(fst2.coder()),
            it.get_output_symbol(fst2.coder()),
        );
        if let Some(&fst1_target) = fst1_transition_map.get(&symbol_pair) {
            let state_pair: (HfstState, HfstState) = (fst1_target, it.get_target_state());
            if !visited_pairs.contains(&state_pair) {
                v.push(Symbol::from(format!("{}:{}", symbol_pair.0, symbol_pair.1)));
                visited_pairs.insert(state_pair);
                if have_common_string(state_pair.0, state_pair.1, fst1, fst2, visited_pairs, v) {
                    return true;
                } else {
                    v.pop();
                }
            }
        }
    }
    false
}
