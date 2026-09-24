//! Algebraic operations and OpenFST-algorithm wrappers.

use super::*;

/// Port of the 'CHECK_EPSILON_CYCLES(x, y)' macro: convert 'x' to an
/// 'HfstBasicTransducer', and if it has negative-weight epsilon cycles, emit a
/// 'tracing' warning.
///
/// A negative-weight cycle must contain at least one negative-weight arc, so
/// when the smallest weight in 'x' is non-negative the diagnostic can never
/// fire. The C++ macro built the whole 'HfstBasicTransducer' unconditionally;
/// here we first run the cheap O(states+arcs) weight scan (no allocation) and
/// only pay for the throwaway deep-copy conversion when a negative weight is
/// actually present. Faithful: the warning fires in exactly the same cases.
pub(super) fn check_epsilon_cycles(x: &StdVectorFst, y: &str) {
    if TropicalWeightTransducer::get_smallest_weight(x) >= 0.0 {
        return;
    }
    let fsm = crate::convert_transducer_format::ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(x, true)
        .expect("converting a valid transducer to HfstBasicTransducer cannot fail");
    if fsm.has_negative_epsilon_cycles() {
        tracing::warn!(
            "{}: transducer has epsilon cycles with a negative weight",
            y
        );
    }
}

/// 'dst->SetInputSymbols(src->InputSymbols())' — copy 'src''s input symbol table
/// (as a shared 'Arc') onto 'dst'. No-op when 'src' has no input symbols.
#[allow(dead_code)]
fn copy_input_symbol_table(src: &StdVectorFst, dst: &mut StdVectorFst) {
    if let Some(symt) = src.input_symbols().map(std::sync::Arc::clone) {
        dst.set_input_symbols(symt);
    }
}

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
impl TropicalWeightTransducer {
    // This function can be moved to its own file if TropicalWeightTransducer.o
    // yields a 'File too big' error.
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-labels-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-labels-fn]
    pub fn push_labels(t: &StdVectorFst, to_initial_state: bool) -> StdVectorFst {
        assert!(t.input_symbols().is_some());

        check_epsilon_cycles(t, "push_labels");

        let mut retval = StdVectorFst::new();
        if to_initial_state {
            algorithms::Push(
                t,
                &mut retval,
                algorithms::FstReweightType::ReweightToInitial,
                algorithms::PushType::PUSH_LABELS,
            );
        } else {
            algorithms::Push(
                t,
                &mut retval,
                algorithms::FstReweightType::ReweightToFinal,
                algorithms::PushType::PUSH_LABELS,
            );
        }
        copy_input_symbol_table(t, &mut retval);
        retval
    }

    // This function can be moved to its own file if TropicalWeightTransducer.o
    // yields a 'File too big' error.
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-weights-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-weights-fn]
    pub fn push_weights(t: &StdVectorFst, to_initial_state: bool) -> StdVectorFst {
        assert!(t.input_symbols().is_some());

        check_epsilon_cycles(t, "push_weights");

        let mut retval = StdVectorFst::new();
        if to_initial_state {
            algorithms::Push(
                t,
                &mut retval,
                algorithms::FstReweightType::ReweightToInitial,
                algorithms::PushType::PUSH_WEIGHTS,
            );
        } else {
            algorithms::Push(
                t,
                &mut retval,
                algorithms::FstReweightType::ReweightToFinal,
                algorithms::PushType::PUSH_WEIGHTS,
            );
        }
        copy_input_symbol_table(t, &mut retval);
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-epsilons-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-epsilons-fn]
    pub fn remove_epsilons(t: &StdVectorFst) -> StdVectorFst {
        check_epsilon_cycles(t, "remove_epsilons");
        // C++: return new StdVectorFst(RmEpsilonFst<StdArc>(*t));
        let mut retval = t.clone();
        algorithms::RmEpsilon(&mut retval);
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.prune-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.prune-fn]
    pub fn prune(t: &StdVectorFst) -> StdVectorFst {
        // C++: fst::Prune(*t, retval, TropicalWeight::One());
        // The hfst-openfst adapter's Prune is in-place (rustfst gap), so we prune
        // a clone with threshold One().
        let mut retval = t.clone();
        algorithms::Prune(&mut retval, TropicalWeight::one());
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.n-best-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.n-best-fn]
    pub fn n_best(t: &StdVectorFst, n: u32) -> StdVectorFst {
        check_epsilon_cycles(t, "n_best");

        let mut scaled = t.clone();
        algorithms::RmEpsilon(&mut scaled);
        let w = TropicalWeightTransducer::get_smallest_weight(&scaled);
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut scaled, -w);
        }
        // fst::ShortestPath(*scaled, n_best_fst, (size_t)n); the C++ bad_alloc
        // catch -> HfstFatalException is dropped (Rust aborts on OOM).
        let config = hfst_openfst::rustfst::algorithms::ShortestPathConfig::default()
            .with_nshortest(n as usize);
        let mut n_best_fst: StdVectorFst =
            hfst_openfst::rustfst::algorithms::shortest_path_with_config(&scaled, config)
                .expect("rustfst shortest_path");
        algorithms::RmEpsilon(&mut n_best_fst);
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut n_best_fst, w);
        }
        n_best_fst
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-star-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-star-fn]
    pub fn repeat_star(t: &StdVectorFst) -> StdVectorFst {
        // C++: return new StdVectorFst(ClosureFst<StdArc>(*t, CLOSURE_STAR));
        let mut t = t.clone();
        hfst_openfst::rustfst::algorithms::closure::closure(
            &mut t,
            hfst_openfst::rustfst::algorithms::closure::ClosureType::ClosureStar,
        );
        t
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-plus-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-plus-fn]
    pub fn repeat_plus(t: &StdVectorFst) -> StdVectorFst {
        // C++: return new StdVectorFst(ClosureFst<StdArc>(*t, CLOSURE_PLUS));
        let mut t = t.clone();
        hfst_openfst::rustfst::algorithms::closure::closure(
            &mut t,
            hfst_openfst::rustfst::algorithms::closure::ClosureType::ClosurePlus,
        );
        t
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-n-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-n-fn]
    pub fn repeat_n(t: &StdVectorFst, n: u32) -> StdVectorFst {
        if n == 0 {
            return TropicalWeightTransducer::create_epsilon_transducer();
        }

        let mut repetition = TropicalWeightTransducer::create_epsilon_transducer();
        copy_input_symbol_table(t, &mut repetition);
        for _ in 0..n {
            algorithms::Concat(&mut repetition, t);
        }
        repetition
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-le-n-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-le-n-fn]
    pub fn repeat_le_n(t: &StdVectorFst, n: u32) -> StdVectorFst {
        if n == 0 {
            return TropicalWeightTransducer::create_epsilon_transducer();
        }

        let mut repetition = TropicalWeightTransducer::create_epsilon_transducer();
        copy_input_symbol_table(t, &mut repetition);

        for _ in 0..n {
            let mut optional_t = TropicalWeightTransducer::optionalize(t);
            copy_input_symbol_table(t, &mut optional_t);
            algorithms::Concat(&mut repetition, &optional_t);
        }
        repetition
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.optionalize-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.optionalize-fn]
    pub fn optionalize(t: &StdVectorFst) -> StdVectorFst {
        let mut eps = TropicalWeightTransducer::create_epsilon_transducer();
        if let Some(symt) = t.input_symbols().map(std::sync::Arc::clone) {
            eps.set_input_symbols(symt);
        }
        if let Some(symt) = t.output_symbols().map(std::sync::Arc::clone) {
            eps.set_output_symbols(symt);
        }
        algorithms::Union(&mut eps, t);
        eps
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.invert-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.invert-fn]
    pub fn invert(t: &StdVectorFst) -> StdVectorFst {
        let mut inverse = t.clone();
        hfst_openfst::rustfst::algorithms::invert(&mut inverse);
        copy_input_symbol_table(t, &mut inverse);
        inverse
    }

    /* Makes valgrind angry... */
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.reverse-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.reverse-fn]
    pub fn reverse(transducer: &StdVectorFst) -> StdVectorFst {
        let mut reversed = StdVectorFst::new();
        algorithms::Reverse(transducer, &mut reversed);
        copy_input_symbol_table(transducer, &mut reversed);
        reversed
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-input-language-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-input-language-fn]
    pub fn extract_input_language(t: &StdVectorFst) -> StdVectorFst {
        // C++: new StdVectorFst(ProjectFst<StdArc>(*t, ProjectType::INPUT));
        let mut proj = t.clone();
        algorithms::ProjectInput(&mut proj);
        // substitute unknown with identity
        let mut retval = TropicalWeightTransducer::substitute_number(&proj, 1, 2);
        copy_input_symbol_table(t, &mut retval);
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-output-language-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-output-language-fn]
    pub fn extract_output_language(t: &StdVectorFst) -> StdVectorFst {
        // C++: new StdVectorFst(ProjectFst<StdArc>(*t, ProjectType::OUTPUT));
        let mut proj = t.clone();
        algorithms::ProjectOutput(&mut proj);
        // substitute unknown with identity
        let mut retval = TropicalWeightTransducer::substitute_number(&proj, 1, 2);
        copy_input_symbol_table(t, &mut retval);
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.concatenate-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.concatenate-fn]
    pub fn concatenate(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
        let mut result = t1.clone();
        copy_input_symbol_table(t1, &mut result);
        algorithms::Concat(&mut result, t2);
        result
    }

    pub fn disjunct(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
        let mut result = t1.clone();
        copy_input_symbol_table(t1, &mut result);
        algorithms::Union(&mut result, t2);
        result
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-fn]
    pub fn disjunct_spv<'a>(
        t: &'a mut StdVectorFst,
        spv: &StringPairVector,
    ) -> &'a mut StdVectorFst {
        let mut st: SymbolTable = (**t
            .input_symbols()
            .expect("transducer has an input symbol table"))
        .clone();

        let mut s = t.start().expect("input transducer has a start state");

        for it in spv {
            let inumber = st.add_symbol(it.0.as_str());
            let onumber = st.add_symbol(it.1.as_str());

            let mut transition_found = false;
            let mut next: StateId = 0;
            for a in t.get_trs(s).expect("s is a valid state of this fst").trs() {
                if a.ilabel == inumber && a.olabel == onumber {
                    transition_found = true;
                    next = a.nextstate;
                    break;
                }
            }

            if transition_found {
                s = next;
            } else {
                let new_state = t.add_state();
                t.add_tr(
                    s,
                    StdTransition::new(inumber, onumber, TropicalWeight::new(0.0), new_state),
                )
                .expect("transition added to a valid state of this fst");
                s = new_state;
            }
        }

        t.set_final(s, TropicalWeight::new(0.0))
            .expect("s is a valid state of this fst");

        t.set_input_symbols(std::sync::Arc::new(st));
        t
    }

    pub fn disjunct_npv<'a>(
        t: &'a mut StdVectorFst,
        npv: &NumberPairVector,
    ) -> &'a mut StdVectorFst {
        let mut s = t.start().expect("input transducer has a start state");

        for it in npv {
            let inumber = it.0;
            let onumber = it.1;

            let mut transition_found = false;
            let mut next: StateId = 0;
            for a in t.get_trs(s).expect("s is a valid state of this fst").trs() {
                if a.ilabel == inumber && a.olabel == onumber {
                    transition_found = true;
                    next = a.nextstate;
                    break;
                }
            }

            if transition_found {
                s = next;
            } else {
                let new_state = t.add_state();
                t.add_tr(
                    s,
                    StdTransition::new(inumber, onumber, TropicalWeight::new(0.0), new_state),
                )
                .expect("transition added to a valid state of this fst");
                s = new_state;
            }
        }

        t.set_final(s, TropicalWeight::new(0.0))
            .expect("s is a valid state of this fst");
        t
    }

    /// 'static fst::StdVectorFst * disjunct_as_tries(fst::StdVectorFst * t1,
    ///   const fst::StdVectorFst * t2)' — public trie-disjunction entry point.
    pub fn disjunct_as_tries_pub<'a>(
        t1: &'a mut StdVectorFst,
        t2: &StdVectorFst,
    ) -> &'a mut StdVectorFst {
        let t1_state = t1.start().expect("t1 has a start state");
        let t2_state = t2.start().expect("t2 has a start state");
        TropicalWeightTransducer::disjunct_as_tries(t1, t1_state, t2, t2_state);
        t1
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.subtract-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.subtract-fn]
    pub fn subtract(t1: &StdVectorFst, t2: &StdVectorFst) -> StdVectorFst {
        // bool DEBUG = false; (debug printfs dropped)

        // C++ mutates t1/t2 in place; operate on local clones.
        let mut t1 = t1.clone();
        let mut t2 = t2.clone();

        if t1.output_symbols().is_none() {
            let a = t1.input_symbols().map(std::sync::Arc::clone);
            if let Some(a) = a {
                t1.set_output_symbols(a);
            }
        }
        if t2.output_symbols().is_none() {
            let a = t2.input_symbols().map(std::sync::Arc::clone);
            if let Some(a) = a {
                t2.set_output_symbols(a);
            }
        }

        check_epsilon_cycles(&t1, "subtract");
        check_epsilon_cycles(&t2, "subtract");

        algorithms::RmEpsilon(&mut t1);
        algorithms::RmEpsilon(&mut t2);

        algorithms::ArcSortOutput(&mut t1);
        algorithms::ArcSortInput(&mut t2);

        // Remove weights from t2, is this really needed?
        let mut t2_copy = t2.clone();

        for s in 0..t2_copy.num_states() as StateId {
            let ntrs = t2_copy
                .get_trs(s)
                .expect("s is a valid state of this fst")
                .trs()
                .len();
            {
                let mut aiter = t2_copy
                    .tr_iter_mut(s)
                    .expect("s is a valid state of this fst");
                for i in 0..ntrs {
                    aiter
                        .set_weight(i, TropicalWeight::new(0.0))
                        .expect("i is within the transition count of this state");
                }
            }
            if t2_copy.is_final(s).expect("s is a valid state of this fst") {
                t2_copy
                    .set_final(s, TropicalWeight::new(0.0))
                    .expect("s is a valid state of this fst");
            }
        }

        // EncodeMapper<StdArc> encoder(kEncodeLabels, ENCODE); shared by t1 AND t2.
        let encoder = algorithms::Encode(&mut t1, algorithms::EncodeType::EncodeLabels);
        let encoder = algorithms::EncodeInto(&mut t2_copy, encoder);

        algorithms::ArcSortOutput(&mut t1);
        algorithms::ArcSortInput(&mut t2_copy);

        let mut det2 = StdVectorFst::new();
        algorithms::Determinize(&t2_copy, &mut det2);

        let mut difference = StdVectorFst::new();
        algorithms::Difference(&t1, &det2, &mut difference);

        // DecodeFst<StdArc> subtract(*difference, encoder);
        algorithms::Decode(&mut difference, encoder);

        // t1->SetOutputSymbols(NULL); t2->SetOutputSymbols(NULL); (caller-side only)
        difference
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.are-equivalent-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.are-equivalent-fn]
    pub fn are_equivalent(
        one: &StdVectorFst,
        another: &StdVectorFst,
        encode_weights: bool,
    ) -> bool {
        let mut a = one.clone();
        let mut b = another.clone();

        check_epsilon_cycles(&a, "are_equivalent");
        check_epsilon_cycles(&b, "are_equivalent");

        algorithms::RmEpsilon(&mut a);
        algorithms::RmEpsilon(&mut b);

        let encode_type = if encode_weights {
            algorithms::EncodeType::EncodeWeightsAndLabels
        } else {
            algorithms::EncodeType::EncodeLabels
        };

        // Encode both fsts through ONE shared table (OpenFST's
        // Encode(fst, &encoder)): the same (ilabel, olabel) pair then maps to
        // the same encoded label in both, so the subsequent Equivalent does
        // not depend on the order the global symbol table numbered the labels.
        let table = algorithms::Encode(&mut a, encode_type);
        let _table = algorithms::EncodeInto(&mut b, table);

        let mut deta = StdVectorFst::new();
        let mut detb = StdVectorFst::new();

        algorithms::Determinize(&a, &mut deta);
        algorithms::Determinize(&b, &mut detb);

        algorithms::Equivalent(&deta, &detb)
    }

    // ----- TRIE FUNCTIONS BEGINS -----

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-arc-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-arc-fn]
    fn has_arc(t: &StdVectorFst, sourcestate: StateId, ilabel: u32, olabel: u32) -> Option<usize> {
        t.get_trs(sourcestate)
            .expect("sourcestate is a valid state of this fst")
            .trs()
            .iter()
            .position(|a| a.ilabel == ilabel && a.olabel == olabel)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-as-tries-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-as-tries-fn]
    fn disjunct_as_tries(
        t1: &mut StdVectorFst,
        t1_state: StateId,
        t2: &StdVectorFst,
        t2_state: StateId,
    ) {
        if t2
            .is_final(t2_state)
            .expect("t2_state is a valid state of t2")
        {
            let t1_final = t1
                .final_weight(t1_state)
                .expect("t1_state is a valid state of t1")
                .unwrap_or_else(TropicalWeight::zero);
            let t2_final = t2
                .final_weight(t2_state)
                .expect("t2_state is a valid state of t2")
                .expect("t2_state confirmed final via is_final");
            t1.set_final(
                t1_state,
                t1_final.plus(t2_final).expect("tropical plus is total"),
            )
            .expect("t1_state is a valid state of t1");
        }
        let trs = t2
            .get_trs(t2_state)
            .expect("t2_state is a valid state of t2")
            .trs()
            .to_vec();
        for arc in &trs {
            match TropicalWeightTransducer::has_arc(t1, t1_state, arc.ilabel, arc.olabel) {
                None => {
                    let new_state = t1.add_state();
                    t1.add_tr(
                        t1_state,
                        StdTransition::new(arc.ilabel, arc.olabel, arc.weight, new_state),
                    )
                    .expect("target state was just added");
                    TropicalWeightTransducer::add_sub_trie(t1, new_state, t2, arc.nextstate);
                }
                Some(arc_index) => {
                    // MutableArcIterator ajter(&t1, t1_state); ajter.Seek(arc_index);
                    let next = t1
                        .get_trs(t1_state)
                        .expect("t1_state is a valid state")
                        .trs()[arc_index]
                        .nextstate;
                    TropicalWeightTransducer::disjunct_as_tries(t1, next, t2, arc.nextstate);
                }
            }
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-sub-trie-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-sub-trie-fn]
    fn add_sub_trie(
        t1: &mut StdVectorFst,
        t1_state: StateId,
        t2: &StdVectorFst,
        t2_state: StateId,
    ) {
        if t2
            .is_final(t2_state)
            .expect("t2_state is a valid state of t2")
        {
            let t1_final = t1
                .final_weight(t1_state)
                .expect("t1_state is a valid state of t1")
                .unwrap_or_else(TropicalWeight::zero);
            let t2_final = t2
                .final_weight(t2_state)
                .expect("t2_state is a valid state of t2")
                .expect("t2_state confirmed final via is_final");
            t1.set_final(
                t1_state,
                t1_final.plus(t2_final).expect("tropical plus is total"),
            )
            .expect("t1_state is a valid state of t1");
        }
        let trs = t2
            .get_trs(t2_state)
            .expect("t2_state is a valid state of t2")
            .trs()
            .to_vec();
        for arc in &trs {
            let new_state = t1.add_state();
            t1.add_tr(
                t1_state,
                StdTransition::new(arc.ilabel, arc.olabel, arc.weight, new_state),
            )
            .expect("transition added to t1_state targeting the state just added");
            TropicalWeightTransducer::add_sub_trie(t1, new_state, t2, arc.nextstate);
        }
    }

    // ----- TRIE FUNCTIONS END -----
}
