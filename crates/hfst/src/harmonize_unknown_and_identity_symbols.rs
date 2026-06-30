//! Port of 'libhfst/src/HarmonizeUnknownAndIdentitySymbols.{h,cc}'.
//!
//! Expands the unknown ('@_UNKNOWN_SYMBOL_@') and identity ('@_IDENTITY_SYMBOL_@')
//! transitions of two 'HfstBasicTransducer's against each other's symbols. The
//! work is all in the constructor (a sentinel object). The 'debug_harmonize'
//! branches are compiled out ('TEST_…' undefined) and not ported beyond the
//! 'debug_harmonize_print' helpers.

use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::StringSet;
use crate::pmatch::PmatchAlphabet;

// [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.max-fn]
// [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.max-fn]
pub fn max_(t1: usize, t2: usize) -> usize {
    if t1 < t2 { t2 } else { t1 }
}

// [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.is-subset-fn]
// [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.is-subset-fn]
// Used only by the compiled-out 'debug_harmonize' asserts.
#[allow(dead_code)]
fn is_subset(subset: &StringSet, superset: &StringSet) -> bool {
    for it in subset.iter() {
        if !superset.contains(it) {
            return false;
        }
    }
    true
}

// [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.remove-flags-fn]
// [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.remove-flags-fn]
fn remove_flags(alpha: &StringSet) -> StringSet {
    let mut retval = StringSet::new();
    for it in alpha.iter() {
        if !FdOperation::is_diacritic(it) && !PmatchAlphabet::is_special(it) {
            retval.insert(it.clone());
        }
    }
    retval
}

// [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols]
#[allow(dead_code)]
pub struct HarmonizeUnknownAndIdentitySymbols {
    // C++ stores t1/t2 references and these symbol sets; only the constructor
    // uses them, so the references are not retained here.
    t1_symbol_set: StringSet,
    t2_symbol_set: StringSet,
}

impl HarmonizeUnknownAndIdentitySymbols {
    pub const identity: &str = "@_IDENTITY_SYMBOL_@";
    pub const unknown: &str = "@_UNKNOWN_SYMBOL_@";

    // [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-unknown-and-identity-symbols-fn]
    // [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-unknown-and-identity-symbols-fn]
    pub fn new(t1: &mut HfstBasicTransducer, t2: &mut HfstBasicTransducer) -> Self {
        let t1_symbol_set = remove_flags(t1.get_alphabet());
        let t2_symbol_set = remove_flags(t2.get_alphabet());

        // t1 symbols - t2 symbols
        let mut t1_symbols_minus_t2_symbols: StringSet =
            t1_symbol_set.difference(&t2_symbol_set).cloned().collect();
        t1_symbols_minus_t2_symbols.remove(Self::identity);
        t1_symbols_minus_t2_symbols.remove(Self::unknown);

        // t2 symbols - t1 symbols
        let t2_symbols_minus_t1_symbols: StringSet =
            t2_symbol_set.difference(&t1_symbol_set).cloned().collect();

        // BUG preserved: the C++ erases unknown/identity from
        // t1_symbols_minus_t2_symbols here (again) instead of from
        // t2_symbols_minus_t1_symbols, so the latter keeps them.
        t1_symbols_minus_t2_symbols.remove(Self::unknown);
        t1_symbols_minus_t2_symbols.remove(Self::identity);

        Self::harmonize_identity_symbols(t1, &t2_symbols_minus_t1_symbols);
        Self::harmonize_identity_symbols(t2, &t1_symbols_minus_t2_symbols);

        Self::harmonize_unknown_symbols(t1, &t2_symbols_minus_t1_symbols);
        Self::harmonize_unknown_symbols(t2, &t1_symbols_minus_t2_symbols);

        // Add new symbols to the alphabets of the transducers.
        let t2_alpha = t2.get_alphabet().clone();
        Self::add_symbols_to_alphabet(t1, &t2_alpha);
        let t1_alpha = t1.get_alphabet().clone();
        Self::add_symbols_to_alphabet(t2, &t1_alpha);

        HarmonizeUnknownAndIdentitySymbols {
            t1_symbol_set,
            t2_symbol_set,
        }
    }

    // [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.populate-symbol-set-fn]
    // [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.populate-symbol-set-fn]
    pub fn populate_symbol_set(t: &HfstBasicTransducer, s: &mut StringSet) {
        let coder = t.coder();
        for it in t.state_vector.iter() {
            for jt in it.iter() {
                s.insert(jt.get_input_symbol(coder));
                s.insert(jt.get_output_symbol(coder));
            }
        }
    }

    // [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.add-symbols-to-alphabet-fn]
    // [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.add-symbols-to-alphabet-fn]
    pub fn add_symbols_to_alphabet(t: &mut HfstBasicTransducer, s: &StringSet) {
        for it in s.iter() {
            t.add_symbol_to_alphabet(it);
        }
    }

    // [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-identity-symbols-fn]
    // [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-identity-symbols-fn]
    pub fn harmonize_identity_symbols(t: &mut HfstBasicTransducer, missing_symbols: &StringSet) {
        if missing_symbols.is_empty() {
            return;
        }

        for s in 0..t.state_vector.len() {
            // Capture the identity transitions' target/weight via the immutable
            // coder borrow first, so the build pass below can take 't.coder_mut()'.
            let mut identity_targets: Vec<(HfstState, f32)> = Vec::new();
            {
                let coder = t.coder();
                for j in 0..t.state_vector[s].len() {
                    if t.state_vector[s][j].get_input_symbol(coder) == Self::identity {
                        assert!(t.state_vector[s][j].get_output_symbol(coder) == Self::identity);
                        let target = t.state_vector[s][j].get_target_state();
                        let weight = t.state_vector[s][j].get_weight();
                        identity_targets.push((target, weight));
                    }
                }
            }

            let mut added_transitions: Vec<HfstBasicTransition> = Vec::new();
            let coder = t.coder_mut();
            for (target, weight) in identity_targets {
                for kt in missing_symbols.iter() {
                    added_transitions.push(HfstBasicTransition::new_symbols(
                        target,
                        kt.clone(),
                        kt.clone(),
                        weight,
                        coder,
                    ));
                }
            }
            t.state_vector[s].extend(added_transitions);
        }
    }

    // [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-unknown-symbols-fn]
    // [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-unknown-symbols-fn]
    pub fn harmonize_unknown_symbols(t: &mut HfstBasicTransducer, missing_symbols: &StringSet) {
        if missing_symbols.is_empty() {
            return;
        }

        for s in 0..t.state_vector.len() {
            // Read every transition's symbols/target/weight via the immutable
            // coder borrow first; the build pass below takes 't.coder_mut()'.
            let arc_data: Vec<(String, String, HfstState, f32)> = {
                let coder = t.coder();
                t.state_vector[s]
                    .iter()
                    .map(|tr| {
                        (
                            tr.get_input_symbol(coder),
                            tr.get_output_symbol(coder),
                            tr.get_target_state(),
                            tr.get_weight(),
                        )
                    })
                    .collect()
            };

            let mut added_transitions: Vec<HfstBasicTransition> = Vec::new();
            let coder = t.coder_mut();
            for (isym, osym, target, weight) in arc_data {
                if isym == Self::unknown {
                    assert!(osym != Self::identity);
                    for kt in missing_symbols.iter() {
                        added_transitions.push(HfstBasicTransition::new_symbols(
                            target,
                            kt.clone(),
                            osym.clone(),
                            weight,
                            coder,
                        ));
                    }
                }
                if osym == Self::unknown {
                    assert!(isym != Self::identity);
                    for kt in missing_symbols.iter() {
                        added_transitions.push(HfstBasicTransition::new_symbols(
                            target,
                            isym.clone(),
                            kt.clone(),
                            weight,
                            coder,
                        ));
                    }
                }
                if isym == Self::unknown && osym == Self::unknown {
                    for kt in missing_symbols.iter() {
                        for lt in missing_symbols.iter() {
                            if kt == lt {
                                continue;
                            }
                            added_transitions.push(HfstBasicTransition::new_symbols(
                                target,
                                lt.clone(),
                                kt.clone(),
                                weight,
                                coder,
                            ));
                        }
                    }
                }
            }
            t.state_vector[s].extend(added_transitions);
        }
    }
}

// [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.debug-harmonize-print-fn]
// [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.debug-harmonize-print-fn]
pub fn debug_harmonize_print_set(s: &StringSet) {
    for it in s.iter() {
        tracing::debug!("{}", it);
    }
}

pub fn debug_harmonize_print_str(s: &str) {
    tracing::debug!("{}", s);
}
