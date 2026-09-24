//! Algebraic operations over rustfst's algorithms.

use hfst_openfst::rustfst::algorithms::compose::compose;
use hfst_openfst::rustfst::algorithms::concat::concat;
use hfst_openfst::rustfst::algorithms::determinize::determinize;
use hfst_openfst::rustfst::algorithms::union::union;

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

/// Reverse 'fst'. The two sides trade places, so its symbol tables do too.
pub(super) fn reverse_swapping_tables(fst: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
    let mut reversed: StdVectorFst = reverse(fst).map_err(openfst_error("reverse"))?;
    if let Some(symbols) = fst.output_symbols() {
        reversed.set_input_symbols(std::sync::Arc::clone(symbols));
    }
    if let Some(symbols) = fst.input_symbols() {
        reversed.set_output_symbols(std::sync::Arc::clone(symbols));
    }
    Ok(reversed)
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
    pub fn push_labels(
        t: &StdVectorFst,
        to_initial_state: bool,
    ) -> crate::error::Result<StdVectorFst> {
        assert!(t.input_symbols().is_some());

        check_epsilon_cycles(t, "push_labels");

        let reweight_type = if to_initial_state {
            ReweightType::ReweightToInitial
        } else {
            ReweightType::ReweightToFinal
        };
        let mut retval: StdVectorFst =
            push(t, reweight_type, PushType::PUSH_LABELS).map_err(openfst_error("push"))?;
        carry_symbol_tables(t, &mut retval);
        Ok(retval)
    }

    // This function can be moved to its own file if TropicalWeightTransducer.o
    // yields a 'File too big' error.
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-weights-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-weights-fn]
    pub fn push_weights(
        t: &StdVectorFst,
        to_initial_state: bool,
    ) -> crate::error::Result<StdVectorFst> {
        assert!(t.input_symbols().is_some());

        check_epsilon_cycles(t, "push_weights");

        let reweight_type = if to_initial_state {
            ReweightType::ReweightToInitial
        } else {
            ReweightType::ReweightToFinal
        };
        let mut retval: StdVectorFst =
            push(t, reweight_type, PushType::PUSH_WEIGHTS).map_err(openfst_error("push"))?;
        carry_symbol_tables(t, &mut retval);
        Ok(retval)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-epsilons-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-epsilons-fn]
    pub fn remove_epsilons(t: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
        check_epsilon_cycles(t, "remove_epsilons");
        // C++: return new StdVectorFst(RmEpsilonFst<StdArc>(*t));
        let mut retval = t.clone();
        rm_epsilon(&mut retval).map_err(openfst_error("rm_epsilon"))?;
        Ok(retval)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.prune-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.prune-fn]
    pub fn prune(t: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
        // C++: fst::Prune(*t, retval, TropicalWeight::One());
        // rustfst's prune is in-place, so we prune a clone with threshold One().
        let mut retval = t.clone();
        prune(&mut retval, TropicalWeight::one()).map_err(openfst_error("prune"))?;
        Ok(retval)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.n-best-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.n-best-fn]
    pub fn n_best(t: &StdVectorFst, n: u32) -> crate::error::Result<StdVectorFst> {
        check_epsilon_cycles(t, "n_best");

        let mut scaled = t.clone();
        rm_epsilon(&mut scaled).map_err(openfst_error("rm_epsilon"))?;
        let w = TropicalWeightTransducer::get_smallest_weight(&scaled);
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut scaled, -w);
        }
        // fst::ShortestPath(*scaled, n_best_fst, (size_t)n); the C++ bad_alloc
        // catch -> HfstFatalException is dropped (Rust aborts on OOM).
        let config = ShortestPathConfig::default().with_nshortest(n as usize);
        let mut n_best_fst: StdVectorFst =
            shortest_path_with_config(&scaled, config).map_err(openfst_error("shortest_path"))?;
        rm_epsilon(&mut n_best_fst).map_err(openfst_error("rm_epsilon"))?;
        if w < 0.0 {
            TropicalWeightTransducer::add_to_weights(&mut n_best_fst, w);
        }
        Ok(n_best_fst)
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
    pub fn repeat_n(t: &StdVectorFst, n: u32) -> crate::error::Result<StdVectorFst> {
        if n == 0 {
            return Ok(TropicalWeightTransducer::create_epsilon_transducer());
        }

        let mut repetition = TropicalWeightTransducer::create_epsilon_transducer();
        copy_input_symbol_table(t, &mut repetition);
        for _ in 0..n {
            concat(&mut repetition, t).map_err(openfst_error("concat"))?;
        }
        Ok(repetition)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-le-n-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-le-n-fn]
    pub fn repeat_le_n(t: &StdVectorFst, n: u32) -> crate::error::Result<StdVectorFst> {
        if n == 0 {
            return Ok(TropicalWeightTransducer::create_epsilon_transducer());
        }

        let mut repetition = TropicalWeightTransducer::create_epsilon_transducer();
        copy_input_symbol_table(t, &mut repetition);

        for _ in 0..n {
            let mut optional_t = TropicalWeightTransducer::optionalize(t)?;
            copy_input_symbol_table(t, &mut optional_t);
            concat(&mut repetition, &optional_t).map_err(openfst_error("concat"))?;
        }
        Ok(repetition)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.optionalize-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.optionalize-fn]
    pub fn optionalize(t: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
        let mut eps = TropicalWeightTransducer::create_epsilon_transducer();
        carry_symbol_tables(t, &mut eps);
        union(&mut eps, t).map_err(openfst_error("union"))?;
        Ok(eps)
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
    pub fn reverse(transducer: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
        let mut reversed = reverse_swapping_tables(transducer)?;
        copy_input_symbol_table(transducer, &mut reversed);
        Ok(reversed)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-input-language-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-input-language-fn]
    pub fn extract_input_language(t: &StdVectorFst) -> StdVectorFst {
        // C++: new StdVectorFst(ProjectFst<StdArc>(*t, ProjectType::INPUT));
        let mut proj = t.clone();
        project(&mut proj, ProjectType::ProjectInput);
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
        project(&mut proj, ProjectType::ProjectOutput);
        // substitute unknown with identity
        let mut retval = TropicalWeightTransducer::substitute_number(&proj, 1, 2);
        copy_input_symbol_table(t, &mut retval);
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.concatenate-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.concatenate-fn]
    pub fn concatenate(t1: &StdVectorFst, t2: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
        let mut result = t1.clone();
        copy_input_symbol_table(t1, &mut result);
        concat(&mut result, t2).map_err(openfst_error("concat"))?;
        Ok(result)
    }

    pub fn disjunct(t1: &StdVectorFst, t2: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
        let mut result = t1.clone();
        copy_input_symbol_table(t1, &mut result);
        union(&mut result, t2).map_err(openfst_error("union"))?;
        Ok(result)
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
    pub fn subtract(t1: &StdVectorFst, t2: &StdVectorFst) -> crate::error::Result<StdVectorFst> {
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

        rm_epsilon(&mut t1).map_err(openfst_error("rm_epsilon"))?;
        rm_epsilon(&mut t2).map_err(openfst_error("rm_epsilon"))?;

        tr_sort(&mut t1, OLabelCompare {});
        tr_sort(&mut t2, ILabelCompare {});

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
        let encoder = encode(&mut t1, EncodeType::EncodeLabels).map_err(openfst_error("encode"))?;
        let encoder = encode_into(&mut t2_copy, encoder).map_err(openfst_error("encode"))?;

        tr_sort(&mut t1, OLabelCompare {});
        tr_sort(&mut t2_copy, ILabelCompare {});

        let det2: StdVectorFst = determinize(&t2_copy).map_err(openfst_error("determinize"))?;

        // Difference(t1, det2): t1 composed with the complement of det2 over
        // the alphabet of both.
        let mut sigma = algorithms::input_labels(&t1);
        sigma.append(&mut algorithms::input_labels(&det2));
        let complement =
            algorithms::complement_acceptor(&det2, &sigma).map_err(openfst_error("complement"))?;
        let mut difference: StdVectorFst =
            compose(&t1, &complement).map_err(openfst_error("difference"))?;
        carry_symbol_tables(&t1, &mut difference);

        // DecodeFst<StdArc> subtract(*difference, encoder);
        decode(&mut difference, encoder).map_err(openfst_error("decode"))?;

        // t1->SetOutputSymbols(NULL); t2->SetOutputSymbols(NULL); (caller-side only)
        Ok(difference)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.are-equivalent-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.are-equivalent-fn]
    pub fn are_equivalent(
        one: &StdVectorFst,
        another: &StdVectorFst,
        encode_weights: bool,
    ) -> crate::error::Result<bool> {
        let mut a = one.clone();
        let mut b = another.clone();

        check_epsilon_cycles(&a, "are_equivalent");
        check_epsilon_cycles(&b, "are_equivalent");

        rm_epsilon(&mut a).map_err(openfst_error("rm_epsilon"))?;
        rm_epsilon(&mut b).map_err(openfst_error("rm_epsilon"))?;

        let encode_type = if encode_weights {
            EncodeType::EncodeWeightsAndLabels
        } else {
            EncodeType::EncodeLabels
        };

        // Encode both fsts through ONE shared table (OpenFST's
        // Encode(fst, &encoder)): the same (ilabel, olabel) pair then maps to
        // the same encoded label in both, so the subsequent Equivalent does
        // not depend on the order the global symbol table numbered the labels.
        let table = encode(&mut a, encode_type).map_err(openfst_error("encode"))?;
        encode_into(&mut b, table).map_err(openfst_error("encode"))?;

        let deta: StdVectorFst = determinize(&a).map_err(openfst_error("determinize"))?;
        let detb: StdVectorFst = determinize(&b).map_err(openfst_error("determinize"))?;

        algorithms::equivalent(&deta, &detb).map_err(openfst_error("equivalent"))
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
