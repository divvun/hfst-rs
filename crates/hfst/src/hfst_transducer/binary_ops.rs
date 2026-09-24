//! Binary facade operations: composition, concatenation, disjunction, merge,
//! cross product, shuffle, and compose-intersect.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: AlgebraBackend> HfstTransducer<B> {
    /// The harmonization preamble of the former 'apply(..., HfstTransducer&,
    /// bool harmonize)' binary functor (HfstApply.cc) — the only part of the
    /// 'apply*' family that survives monomorphization. Every binary op calls
    /// this before its backend trait call.
    // [spec:hfst:def:hfst-apply.another-fn]
    // [spec:hfst:sem:hfst-apply.another-fn]
    pub(super) fn harmonize_for_binary_op(
        &mut self,
        another_tr: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut another = HfstTransducer::new_copy(another_tr)?;

        // prevent harmonization, if needed
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&another, false)?;
            another.insert_missing_symbols_to_alphabet_from(self, false)?;
        }

        // special symbols are never harmonized
        self.insert_missing_symbols_to_alphabet_from(&another, true)?;
        another.insert_missing_symbols_to_alphabet_from(self, true)?;
        // 'harmonize_copy' returns None for foma (use our own copy of 'another').
        let another: HfstTransducer<B> = match self.harmonize_copy(&another)? {
            Some(h) => h,
            None => HfstTransducer::new_copy(&another)?,
        };
        Ok(another)
    }

    // -------------------------------------------------------------------------
    // ----- Binary operators (HfstTransducer.cc ~4173-5423) -----
    // -------------------------------------------------------------------------

    pub fn merge(
        &mut self,
        another: &HfstTransducer<B>,
        args: &crate::xre::XreConstructorArguments<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut this_basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
        // [spec:hfst:def:hfst-transducer.hfst.another-basic-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.another-basic-fn]
        let mut another_basic =
            ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(another)?;
        let mut markers_added: BTreeSet<Symbol> = BTreeSet::new();
        let result = HfstBasicTransducer::merge(
            &mut this_basic,
            &mut another_basic,
            &args.list_definitions,
            &mut markers_added,
        )?;
        let mut initial_merge = HfstTransducer::new_from_basic(&result)?;
        initial_merge.optimize()?;

        // filter non-optimal paths
        // [ ? | #V ?:? ]* %#V:V ?:0 [ ? | #V ?:? | %#V:V ?:0 ]*
        // [spec:hfst:def:hfst-transducer.hfst.xre-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.xre-fn]
        let mut xre = crate::xre::XreCompiler::new_with_args(args);
        xre.set_verbosity(false);

        for it in &markers_added {
            let marker = it.clone();
            let symbol = (it.as_bytes()[1] as char).to_string(); // @X@ -> X
            let worsener_string = format!(
                "[ ? | \"{m}\" ?:? ]* \"{m}\":{s} ?:0 [ ? | \"{m}\" ?:? | \"{m}\":{s} ?:0 ]* ;",
                m = marker,
                s = symbol
            );

            let mut worsener: HfstTransducer<B> = xre
                .compile(&worsener_string)
                .expect("the merge worsener xre is well-formed");
            worsener.optimize()?;
            // [spec:hfst:def:hfst-transducer.hfst.cp-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.cp-fn]
            let mut cp = initial_merge.clone();
            cp.compose(&worsener, true)?.output_project()?.optimize()?;

            initial_merge.subtract(&cp, true)?.optimize()?;
            initial_merge.substitute_string(&marker, internal_epsilon, true, true)?;

            // [spec:hfst:def:hfst-transducer.hfst.fsm-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.fsm-fn]
            let fsm =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&initial_merge)?;
            let symbols = fsm.symbols_used();
            if !symbols.contains(symbol.as_str()) {
                initial_merge.remove_from_alphabet_string(&symbol)?;
            }
        }

        *self = initial_merge;
        Ok(self)
    }

    pub fn compose(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.compose_with_config(another, harmonize, &EngineConfig::default())
    }

    /// 'compose', reading the engine-policy flags it consults
    /// ('flag_is_epsilon_in_composition', 'unknown_symbols_in_use',
    /// 'xerox_composition') from the supplied config.
    pub fn compose_with_config(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.compose_with_config_and_flag_overlay(another, harmonize, config, None)
    }

    /// Compose with an optional lazy flag-diacritic self-loop overlay.
    ///
    /// The overlay must have been produced by
    /// [`Self::prepare_flag_diacritics_for_operation`] for these operands.  It is
    /// resolved to backend labels only after ordinary symbol harmonization has
    /// established the operands' shared canonical coding. In Xerox mode the
    /// overlay labels follow the operands through the `%...%` encoding pass; in
    /// flag-as-epsilon mode the backend exposes the missing loops as one-sided
    /// epsilon moves without modifying either operand.
    // [spec:hfst:req:virtual-flag-algebra.special-compose]
    pub fn compose_with_config_and_flag_overlay(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
        config: &EngineConfig,
        flag_overlay: Option<&FlagDiacriticComposeOverlay>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if flag_overlay.is_some() && !B::SUPPORTS_VIRTUAL_FLAG_COMPOSE {
            crate::bail!(
                Hfst,
                "this backend does not support virtual flag composition"
            );
        }

        self.is_trie = false;

        let mut another_copy: HfstTransducer<B> = another.clone();

        /* If we want flag diacritcs to be handled in the same way as epsilons
        in composition, we substitute output flags of first transducer with
        epsilons and input flags of second transducer with epsilons. */
        let virtual_flags_as_epsilon =
            config.flag_is_epsilon_in_composition && flag_overlay.is_some();
        if config.flag_is_epsilon_in_composition && !virtual_flags_as_epsilon {
            // The C++ caught a throw from these substitutions and rethrew it as
            // FlagDiacriticsAreNotIdentities; the ported substitute returns the
            // error instead, so remap it the same way.
            if self
                .substitute_with_func(substitute_output_flag_with_epsilon)
                .is_err()
                || another_copy
                    .substitute_with_func(substitute_input_flag_with_epsilon)
                    .is_err()
            {
                crate::bail!(FlagDiacriticsAreNotIdentities);
            }
        }

        // (The XFSM-only 'insert_missing_diacritics_to_alphabet_from' arm is
        // compiled out with the xfsm backend.)
        if config.xerox_composition {
            encode_flag_diacritics(self);
            encode_flag_diacritics(&mut another_copy);
        }
        let encoded_flag_overlay = if config.xerox_composition {
            flag_overlay.map(FlagDiacriticOverlay::xerox_encoded)
        } else {
            None
        };
        let backend_flag_overlay = encoded_flag_overlay.as_ref().or(flag_overlay);

        /* Prevent harmonization (i.e. matching unknown symbols), if requested. */
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&another_copy, false)?;
            another_copy.insert_missing_symbols_to_alphabet_from(self, false)?;
        }

        /* Special symbols are never harmonized. */
        self.insert_missing_symbols_to_alphabet_from(&another_copy, true)?;
        another_copy.insert_missing_symbols_to_alphabet_from(self, true)?;

        // Harmonize (FOMA and XFSM took care of this by default; both are
        // compiled out).
        another_copy = self
            .harmonize_copy_owned(another_copy)?
            .expect("harmonize_copy returns Some for tropical types");

        /* Take care of unknown and identity symbols being handled right in
        composition. */
        if config.unknown_symbols_in_use {
            self.substitute_string("@_IDENTITY_SYMBOL_@", "@_UNKNOWN_SYMBOL_@", false, true)?;
            another_copy.substitute_string(
                "@_IDENTITY_SYMBOL_@",
                "@_UNKNOWN_SYMBOL_@",
                true,
                false,
            )?;
        }

        // (The HFST_OL/HFST_OLW arm threw HfstTransducerTypeMismatch — compose
        // simply does not exist on the lookup instantiations now.)
        let left = std::mem::replace(&mut self.fst, B::empty());
        let flag_operation = if virtual_flags_as_epsilon {
            FlagDiacriticOperation::ComposeFlagsAsEpsilon
        } else {
            FlagDiacriticOperation::Compose
        };
        self.fst = left.try_flag_operation_owned(
            another_copy.fst,
            flag_operation,
            backend_flag_overlay,
            config.compose_memory_limit_bytes,
        )?;

        // Revert changes made before composition
        if config.xerox_composition {
            decode_flag_diacritics(self);
        }

        if config.flag_is_epsilon_in_composition {
            self.substitute_with_func(substitute_one_sided_flags)?;
        }

        if config.unknown_symbols_in_use {
            self.substitute_with_func(substitute_one_sided_identity)?;
        }

        Ok(self)
    }

    pub fn lenient_composition(
        &mut self,
        another: &HfstTransducer<B>,
        _harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut retval = self.clone();
        // true is a dummy variable, false means do not encode epsilons
        retval
            .compose(another, true)?
            .optimize()?
            .priority_union(self)?
            .optimize()?;

        *self = retval;
        Ok(self)
    }

    pub fn cross_product(
        &mut self,
        another: &HfstTransducer<B>,
        _harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut automata1 = self.clone();
        // [spec:hfst:def:hfst-transducer.hfst.automata2-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.automata2-fn]
        let mut automata2 = another.clone();

        // Check if both input transducers are automata
        // [spec:hfst:def:hfst-transducer.hfst.t1-proj-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t1-proj-fn]
        let mut t1_proj = automata1.clone();
        t1_proj.input_project()?;
        // [spec:hfst:def:hfst-transducer.hfst.t2-proj-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t2-proj-fn]
        let mut t2_proj = automata2.clone();
        t2_proj.input_project()?;

        if !t1_proj.compare(&automata1, true)? || !t2_proj.compare(&automata2, true)? {
            crate::bail!(TransducersAreNotAutomata, "HfstTransducer::cross_product");
        }

        // Put MARK all over lower part of automata1 and upper part of automata2,
        // and then compose them. Also, there should be created padding after
        // strings, on both sides
        automata1.insert_to_alphabet_string("@_MARK_@")?;
        automata2.insert_to_alphabet_string("@_MARK_@")?;

        let mut tok = HfstTokenizer::new();
        tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
        tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
        tok.add_multichar_symbol("@_MARK_@");

        // EpsilonToMark and MarkToEpsilon are paddings (if strings are not the
        // same size)
        let mut unknown_to_mark =
            HfstTransducer::new_tokenized_pair("@_UNKNOWN_SYMBOL_@", "@_MARK_@", &tok)?;
        let mut epsilon_to_mark =
            HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", "@_MARK_@", &tok)?;

        // [spec:hfst:def:hfst-transducer.hfst.mark-to-unknown-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.mark-to-unknown-fn]
        let mut mark_to_unknown = unknown_to_mark.clone();
        mark_to_unknown.invert()?;
        // [spec:hfst:def:hfst-transducer.hfst.mark-to-epsilon-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.mark-to-epsilon-fn]
        let mut mark_to_epsilon = epsilon_to_mark.clone();
        mark_to_epsilon.invert()?;

        unknown_to_mark.repeat_star()?.minimize()?; // minimization is safe
        epsilon_to_mark.repeat_star()?.minimize()?; // minimization is safe
        mark_to_unknown.repeat_star()?.minimize()?; // minimization is safe
        mark_to_epsilon.repeat_star()?.minimize()?; // minimization is safe

        // [spec:hfst:def:hfst-transducer.hfst.a1-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.a1-fn]
        let mut a1 = automata1.clone();
        a1.compose(&unknown_to_mark, true)?
            .optimize()?
            .concatenate(&epsilon_to_mark, true)?
            .optimize()?;

        // [spec:hfst:def:hfst-transducer.hfst.b1-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.b1-fn]
        let mut b1 = mark_to_unknown.clone();
        b1.compose(&automata2, true)?
            .optimize()?
            .concatenate(&mark_to_epsilon, true)?
            .optimize()?;

        // [spec:hfst:def:hfst-transducer.hfst.retval-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.retval-fn]
        let mut retval = a1.clone();
        retval.compose(&b1, true)?.optimize()?;

        // Expand ?:? transitions to ?:?|?
        let mut id_or_unk: StringPairSet = StringPairSet::new();
        id_or_unk.insert((
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
        ));
        id_or_unk.insert((
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
        ));
        retval.substitute_pair_with_pair_set(
            &(
                Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
                Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            ),
            &id_or_unk,
        )?;

        retval.remove_from_alphabet_string("@_MARK_@")?;

        *self = retval;
        Ok(self)
    }

    pub fn shuffle(
        &mut self,
        another: &HfstTransducer<B>,
        _b: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // We use HfstBasicTransducers for efficiency
        let mut this_basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
        let mut another_basic =
            ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(another)?;

        // Expand (unknowns and) identities
        this_basic.harmonize(&mut another_basic);

        // Find out the original alphabets of both transducers
        let mut this_alphabet: StringSet = this_basic.get_alphabet().clone();
        let mut another_alphabet: StringSet = another_basic.get_alphabet().clone();

        // Op-local state replacing the former process-global shuffle flags.
        let shuffle_failed = std::cell::Cell::new(false);
        let coding_case = std::cell::Cell::new(ShuffleCoding::ENCODE_FIRST_SHUFFLE_ARGUMENT);

        // Encode first transducer, i.e. prefix each symbol with "@1"
        coding_case.set(ShuffleCoding::ENCODE_FIRST_SHUFFLE_ARGUMENT);
        this_basic.substitute_with_func(|sp, sps| {
            code_symbols_for_shuffle_impl(sp, sps, &coding_case, &shuffle_failed)
        })?;
        // also remember to remove the unprefixed symbols from the alphabet
        this_basic.remove_symbols_from_alphabet(&this_alphabet);

        // Encode second transducer, i.e. prefix each symbol with "@2"
        coding_case.set(ShuffleCoding::ENCODE_SECOND_SHUFFLE_ARGUMENT);
        another_basic.substitute_with_func(|sp, sps| {
            code_symbols_for_shuffle_impl(sp, sps, &coding_case, &shuffle_failed)
        })?;
        // also remember to remove the unprefixed symbols from the alphabet
        another_basic.remove_symbols_from_alphabet(&another_alphabet);

        // See if shuffle failed, i.e. either transducer is not an automaton
        if shuffle_failed.get() {
            shuffle_failed.set(false);
            crate::bail!(
                TransducersAreNotAutomata,
                "HfstTransducer::shuffle(const HfstTransducer&)"
            );
        }

        // The new alphabets of transducers where each symbol is prefixed
        // with "@1" or "@2"
        this_alphabet = this_basic.get_alphabet().clone();
        another_alphabet = another_basic.get_alphabet().clone();

        // Transform alphabets of transducers into string pair sets for function
        // insert_freely
        let mut this_alphabet_pairset: StringPairSet = StringPairSet::new();
        for it in &this_alphabet {
            this_alphabet_pairset.insert((it.clone(), it.clone()));
        }
        let mut another_alphabet_pairset: StringPairSet = StringPairSet::new();
        for it in &another_alphabet {
            another_alphabet_pairset.insert((it.clone(), it.clone()));
        }

        // Freely insert any number of any symbol in the first transducer
        // to the second transducer and vice versa
        this_basic.insert_freely_set(&another_alphabet_pairset, 0.0)?;
        another_basic.insert_freely_set(&this_alphabet_pairset, 0.0)?;

        // We use HfstTransducers for intersection
        let mut this1: HfstTransducer<B> = HfstTransducer::new_from_basic(&this_basic)?;
        let another1: HfstTransducer<B> = HfstTransducer::new_from_basic(&another_basic)?;

        this1.intersect(&another1, true)?;
        this1.optimize()?;

        // We use HfstBasicTransducers again
        // [spec:hfst:def:hfst-transducer.hfst.this1-basic-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.this1-basic-fn]
        let mut this1_basic =
            ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&this1)?;

        // Decode the shuffled transducer, i.e. remove the prefixes
        // "@1" and "@2" from symbols
        coding_case.set(ShuffleCoding::DECODE_AFTER_SHUFFLE);
        this1_basic.substitute_with_func(|sp, sps| {
            code_symbols_for_shuffle_impl(sp, sps, &coding_case, &shuffle_failed)
        })?;
        // also remember to remove the prefixed symbols from the alphabet
        this1_basic.remove_symbols_from_alphabet(&this_alphabet);
        this1_basic.remove_symbols_from_alphabet(&another_alphabet);

        // Convert once again to HfstTransducer
        let this_finally = HfstTransducer::new_from_basic(&this1_basic)?;
        *self = this_finally;

        Ok(self)
    }

    // ---------------------- Shuffle functions end --------------------

    // Q .P. R = Q | [~[Q .u] .o. R ]
    // .u is input project
    pub fn priority_union(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let t1 = self.clone();
        // [spec:hfst:def:hfst-transducer.hfst.t2-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t2-fn]
        let t2 = another.clone();

        // [spec:hfst:def:hfst-transducer.hfst.t1upper-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t1upper-fn]
        let mut t1upper = t1.clone();
        t1upper.input_project()?.optimize()?;

        // DIVERGENCE from upstream C++ (hfst#341 investigation): when Q carries
        // flag diacritics, input_project keeps each flag as a LITERAL arc, so the
        // subsequent negate() treats the flag as an ordinary symbol. The flagless
        // string that Q actually accepts (flags obeyed) then falls OUTSIDE t1upper,
        // lands INSIDE the complement, and R's lower-priority mapping LEAKS through
        // — Q .P. R yields the string twice (Q's weight and R's weight) instead of
        // just Q's. Upstream shares this bug. We fix it by resolving flag diacritics
        // on the input projection FIRST: eliminate_flags rewrites t1upper into the
        // flagless automaton whose language is exactly the strings Q accepts with
        // flags obeyed — precisely the universe the complement must be taken over.
        if t1upper.has_flag_diacritics() {
            t1upper.eliminate_flags()?;
            t1upper.optimize()?;
        }

        // [spec:hfst:def:hfst-transducer.hfst.complement-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.complement-fn]
        let mut complement = t1upper.clone();
        complement.negate()?.prune_alphabet(false)?;

        complement.compose(&t2, true)?.optimize()?;

        let mut retval = t1.clone();
        retval.disjunct(&complement, true)?.optimize()?;

        *self = retval;
        Ok(self)
    }

    pub fn compose_intersect(
        &mut self,
        v: &HfstTransducerVector<B>,
        invert: bool,
        b: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.compose_intersect_with_config(v, invert, b, &EngineConfig::default())
    }

    /// Resource-aware compose-intersect. A single non-inverted rule can use
    /// backend label lookahead to reject dead product pairs before interning
    /// them; other modes retain the legacy lazy rule-intersection engine.
    pub fn compose_intersect_with_config(
        &mut self,
        v: &HfstTransducerVector<B>,
        invert: bool,
        _b: bool,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // (The C++ converted foma inputs to TROPICAL_OPENFST_TYPE first. This
        // runs over the generic backend, so no conversion is needed.)

        // The intersection of an empty set of rules is the empty language,
        // which makes the result empty.
        if v.is_empty() {
            *self = HfstTransducer::new();
            return Ok(self);
        }

        let first = &v[0];

        // If rule transducers contain word boundaries, add word boundaries to
        // the lexicon unless the lexicon already contains them.
        let rule_alphabet = first.get_alphabet()?;

        if rule_alphabet.contains("@#@") {
            let lexicon_alphabet = self.get_alphabet()?;
            let mut tokenizer = HfstTokenizer::new();
            tokenizer.add_multichar_symbol("@#@");
            tokenizer.add_multichar_symbol(internal_epsilon);
            let mut wb = HfstTransducer::new_tokenized_pair(internal_epsilon, "@#@", &tokenizer)?;
            // [spec:hfst:def:hfst-transducer.hfst.wb-copy-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.wb-copy-fn]
            let wb_copy = wb.clone();

            // Add the word boundary symbol to the alphabet so harmonization
            // won't touch it.
            let mut basic_this =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
            basic_this.add_symbol_to_alphabet(&Symbol::new_static("@#@"));
            *self = HfstTransducer::new_from_basic(&basic_this)?;

            wb.concatenate(self, true)?
                .concatenate(&wb_copy, true)?
                .optimize()?;
            *self = wb;
            let _ = lexicon_alphabet;
        }

        if let Some(result) =
            compose_intersect::try_lookahead(self, first, v.len(), invert, config)?
        {
            *self = result;
            return Ok(self);
        }

        let mut rule_1 = v[0].clone();

        // foma / no harmonization -> use our own copy.
        let mut harmonized_lexicon: HfstTransducer<B> =
            rule_1.harmonize_copy(self)?.unwrap_or_else(|| self.clone());
        drop(rule_1);

        if invert {
            harmonized_lexicon.invert()?;
            harmonized_lexicon.substitute_pair_with_pair(
                &(
                    Symbol::new_static("@#@"),
                    Symbol::new_static(internal_epsilon),
                ),
                &(
                    Symbol::new_static(internal_epsilon),
                    Symbol::new_static("@#@"),
                ),
            )?;
        }

        harmonized_lexicon.substitute_string(
            internal_identity,
            "||_IDENTITY_SYMBOL_||",
            true,
            true,
        )?;
        harmonized_lexicon.substitute_string(
            internal_unknown,
            "||_UNKNOWN_SYMBOL_||",
            true,
            true,
        )?;

        if v.len() == 1 {
            let mut rule_fst = v[0].clone();

            if invert {
                rule_fst.invert()?;
                rule_fst.substitute_pair_with_pair(
                    &(
                        Symbol::new_static(internal_epsilon),
                        Symbol::new_static("@#@"),
                    ),
                    &(
                        Symbol::new_static("@#@"),
                        Symbol::new_static(internal_epsilon),
                    ),
                )?;
            }

            // In case there is only onw rule, compose with that.
            // [spec:hfst:def:hfst-transducer.hfst.rule-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.rule-fn]
            // implementations::ComposeIntersectRule rule(rule_fst);
            //
            // The lexicon and rule basic transducers each carry their own symbol
            // coding; reindex both onto one shared `canonical` coder ONCE so their
            // symbol numbers can be combined directly in the lazy product (the
            // per-graph-coder replacement for the former process-global numbering).
            let mut canonical = crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
            let mut rule_basic =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&rule_fst)?;
            let mut lexicon_basic =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&harmonized_lexicon)?;
            lexicon_basic.intern_into(&mut canonical);
            rule_basic.intern_into(&mut canonical);
            lexicon_basic.reindex_into(&mut canonical);
            rule_basic.reindex_into(&mut canonical);

            let mut rule =
                crate::compose_intersect_rule_pair::ComposeIntersectRuleComponent::Rule(Box::new(
                    crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                        &rule_basic,
                    ),
                ));

            // Create a ComposeIntersectLexicon from *harmonized_lexicon.
            let mut lexicon =
                crate::compose_intersect_lexicon::ComposeIntersectLexicon::new_from_transducer(
                    &lexicon_basic,
                );

            let mut res: HfstBasicTransducer = lexicon.compose_with_rules(&mut rule)?;

            // The composition inputs (the lexicon copy inside 'lexicon', the
            // rule copy inside 'rule', and the interned basics) contribute
            // nothing to the output past this point — free them before the
            // basic→backend conversion below so they don't inflate its peak.
            drop(lexicon);
            drop(rule);
            drop(rule_basic);
            drop(lexicon_basic);

            res.prune_alphabet(true);
            *self = HfstTransducer::new_from_basic(&res)?;
        } else {
            // In case there are many rules, build a ComposeIntersectRulePair
            // recursively and compose with that.
            let mut first_rule_fst = v[0].clone();

            if invert {
                first_rule_fst.invert()?;
                first_rule_fst.substitute_pair_with_pair(
                    &(
                        Symbol::new_static(internal_epsilon),
                        Symbol::new_static("@#@"),
                    ),
                    &(
                        Symbol::new_static("@#@"),
                        Symbol::new_static(internal_epsilon),
                    ),
                )?;
            }

            let mut second_rule_fst = v[1].clone();

            if invert {
                second_rule_fst.invert()?;
                second_rule_fst.substitute_pair_with_pair(
                    &(
                        Symbol::new_static(internal_epsilon),
                        Symbol::new_static("@#@"),
                    ),
                    &(
                        Symbol::new_static("@#@"),
                        Symbol::new_static(internal_epsilon),
                    ),
                )?;
            }

            // std::vector<implementations::ComposeIntersectRule *> rule_vector;
            // (declared but unused in the C++; omitted)
            //
            // ComposeIntersectRule * first_rule = new ComposeIntersectRule(first_rule_fst);
            // ComposeIntersectRule * second_rule = new ComposeIntersectRule(second_rule_fst);
            // ComposeIntersectRulePair * rules =
            //     new ComposeIntersectRulePair(first_rule, second_rule);
            //
            // Reindex the lexicon and every rule basic transducer onto one shared
            // `canonical` coder ONCE so their symbol numbers can be combined
            // directly in the lazy product (the per-graph-coder replacement for the
            // former process-global numbering). Build every basic transducer first,
            // intern them ALL into the shared coder, then reindex each — so even
            // alphabet-only symbols agree across all of them.
            let mut lexicon_basic =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&harmonized_lexicon)?;
            let mut first_rule_basic =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&first_rule_fst)?;
            let mut second_rule_basic =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&second_rule_fst)?;
            let mut extra_rule_basics: Vec<HfstBasicTransducer> = Vec::new();
            for it in &v[2..] {
                let mut rule_fst = it.clone();

                if invert {
                    rule_fst.invert()?;
                    rule_fst.substitute_pair_with_pair(
                        &(
                            Symbol::new_static(internal_epsilon),
                            Symbol::new_static("@#@"),
                        ),
                        &(
                            Symbol::new_static("@#@"),
                            Symbol::new_static(internal_epsilon),
                        ),
                    )?;
                }
                extra_rule_basics.push(
                    ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&rule_fst)?,
                );
            }

            let mut canonical = crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
            lexicon_basic.intern_into(&mut canonical);
            first_rule_basic.intern_into(&mut canonical);
            second_rule_basic.intern_into(&mut canonical);
            for rb in extra_rule_basics.iter() {
                rb.intern_into(&mut canonical);
            }
            lexicon_basic.reindex_into(&mut canonical);
            first_rule_basic.reindex_into(&mut canonical);
            second_rule_basic.reindex_into(&mut canonical);
            for rb in extra_rule_basics.iter_mut() {
                rb.reindex_into(&mut canonical);
            }

            use crate::compose_intersect_rule_pair::{
                ComposeIntersectRuleComponent, ComposeIntersectRulePair,
            };
            let first_rule = ComposeIntersectRuleComponent::Rule(Box::new(
                crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                    &first_rule_basic,
                ),
            ));
            let second_rule = ComposeIntersectRuleComponent::Rule(Box::new(
                crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                    &second_rule_basic,
                ),
            ));
            let mut rules = ComposeIntersectRuleComponent::Pair(Box::new(
                ComposeIntersectRulePair::new(first_rule, second_rule),
            ));

            for rule_basic in extra_rule_basics.iter() {
                // rules = new ComposeIntersectRulePair(
                //     new ComposeIntersectRule(rule_fst), rules);
                let new_rule = ComposeIntersectRuleComponent::Rule(Box::new(
                    crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                        rule_basic,
                    ),
                ));
                rules = ComposeIntersectRuleComponent::Pair(Box::new(
                    ComposeIntersectRulePair::new(new_rule, rules),
                ));
            }

            // Create a ComposeIntersectLexicon from *harmonized_lexicon.
            let mut lexicon =
                crate::compose_intersect_lexicon::ComposeIntersectLexicon::new_from_transducer(
                    &lexicon_basic,
                );
            let mut res: HfstBasicTransducer = lexicon.compose_with_rules(&mut rules)?;

            // 'delete rules;' in the C++ — and more: the lexicon copy inside
            // 'lexicon', every rule copy inside 'rules', and the interned
            // basics contribute nothing to the output past this point. Free
            // them before the basic→backend conversion below so they don't
            // inflate its peak.
            drop(lexicon);
            drop(rules);
            drop(first_rule_basic);
            drop(second_rule_basic);
            drop(extra_rule_basics);
            drop(lexicon_basic);

            res.prune_alphabet(true);
            *self = HfstTransducer::new_from_basic(&res)?;

            if invert {
                self.invert()?;
            }
        }

        drop(harmonized_lexicon);

        self.substitute_string("||_IDENTITY_SYMBOL_||", internal_identity, true, true)?;
        self.substitute_string("||_UNKNOWN_SYMBOL_||", internal_unknown, true, true)?;

        Ok(self)
    }

    pub fn concatenate(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let another = self.harmonize_for_binary_op(another, harmonize)?;
        self.fst = self.fst.concatenate(&another.fst)?;
        Ok(self)
    }

    pub fn disjunct_spv(
        &mut self,
        spv: &StringPairVector,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // The tropical backend mutates in place via the trait impl.
        self.fst.disjunct_spv(spv);
        Ok(self)
    }

    pub fn disjunct(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        let another = self.harmonize_for_binary_op(another, harmonize)?;
        self.fst = self.fst.disjunct(&another.fst)?;
        Ok(self)
    }

    pub fn intersect(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.intersect_with_flag_overlay(another, harmonize, None)
    }

    pub fn subtract(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.subtract_with_flag_overlay(another, harmonize, None)
    }
}

// -----------------------------------------------------------------------------
// Binary operators — free helpers.
// -----------------------------------------------------------------------------

// [spec:hfst:def:hfst-transducer.hfst.substitute-single-identity-with-the-other-symbol-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-single-identity-with-the-other-symbol-fn]
pub fn substitute_one_sided_identity(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    let mut isymbol: Symbol = sp.0.clone();
    let mut osymbol: Symbol = sp.1.clone();

    if isymbol == "@_IDENTITY_SYMBOL_@" && (osymbol != "@_IDENTITY_SYMBOL_@") {
        isymbol = Symbol::new_static("@_UNKNOWN_SYMBOL_@");
        sps.insert((isymbol, osymbol));
        true
    } else if osymbol == "@_IDENTITY_SYMBOL_@" && (isymbol != "@_IDENTITY_SYMBOL_@") {
        osymbol = Symbol::new_static("@_UNKNOWN_SYMBOL_@");
        sps.insert((isymbol, osymbol));
        true
    } else {
        false
    }
}

// [spec:hfst:def:hfst-transducer.hfst.substitute-unknown-identity-pairs-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-unknown-identity-pairs-fn]
pub fn substitute_unknown_identity_pairs(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    let mut isymbol: Symbol = sp.0.clone();
    let mut osymbol: Symbol = sp.1.clone();

    if isymbol == "@_UNKNOWN_SYMBOL_@" && osymbol == "@_IDENTITY_SYMBOL_@" {
        isymbol = Symbol::new_static("@_IDENTITY_SYMBOL_@");
        osymbol = Symbol::new_static("@_IDENTITY_SYMBOL_@");
        sps.insert((isymbol, osymbol));
        return true;
    }
    false
}

//
// -------------------- Shuffle functions --------------------
//

// Possible cases for function code_symbols_for_shuffle.
// [spec:hfst:def:hfst-transducer.hfst.shuffle-coding]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(non_camel_case_types)]
pub enum ShuffleCoding {
    ENCODE_FIRST_SHUFFLE_ARGUMENT,
    ENCODE_SECOND_SHUFFLE_ARGUMENT,
    DECODE_AFTER_SHUFFLE,
}

// A function that is given as a parameter to substitute function
// during the shuffle operation. The purpose of this function is (1)
// to encode symbols in the two argument transducers so that no symbol
// is present at both transducers or (2) to decode the symbols
// in the shuffled transducer back to the original ones.
//
// The 'coding_case'/'shuffle_failed' state was process-global in the C++
// (file-static); it is now passed in as op-local Cells owned by 'shuffle'.
// [spec:hfst:def:hfst-transducer.hfst.code-symbols-for-shuffle-fn]
// [spec:hfst:sem:hfst-transducer.hfst.code-symbols-for-shuffle-fn]
fn code_symbols_for_shuffle_impl(
    sp: &StringPair,
    sps: &mut StringPairSet,
    coding_case: &std::cell::Cell<ShuffleCoding>,
    shuffle_failed: &std::cell::Cell<bool>,
) -> bool {
    // not automaton, shuffle fails
    if sp.0 != sp.1 {
        shuffle_failed.set(true);
        return false;
    }
    // special symbols are not coded, except identities
    if is_epsilon(&sp.0) || is_unknown(&sp.0) {
        return false;
    }
    let case = coding_case.get();
    match case {
        // substitute each symbol foo in the first argument transducer
        // with a symbol @1foo
        ShuffleCoding::ENCODE_FIRST_SHUFFLE_ARGUMENT => {
            let symbol_escaped = Symbol::from(format!("@1{}", sp.0));
            let new_sp: StringPair = (symbol_escaped.clone(), symbol_escaped);
            sps.insert(new_sp);
        }
        // substitute each symbol bar in the second argument transducer
        // with a symbol @2bar
        ShuffleCoding::ENCODE_SECOND_SHUFFLE_ARGUMENT => {
            let symbol_escaped = Symbol::from(format!("@2{}", sp.0));
            let new_sp: StringPair = (symbol_escaped.clone(), symbol_escaped);
            sps.insert(new_sp);
        }
        // substitute each symbol @1foo or @2bar in the shuffled transducer
        // with the original foo or bar.
        ShuffleCoding::DECODE_AFTER_SHUFFLE => {
            let symbol_unescaped = Symbol::new(&sp.0[2..]);
            let new_sp: StringPair = (symbol_unescaped.clone(), symbol_unescaped);
            sps.insert(new_sp);
        }
    }

    true
}
