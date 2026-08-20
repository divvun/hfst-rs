//! Flag-diacritic encode, decode, detection, and rename helpers.

use super::*;

// C++ file-static substitution callbacks passed to `substitute_with_func`.
// [spec:hfst:def:hfst-transducer.hfst.substitute-one-sided-flags-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-one-sided-flags-fn]
pub(super) fn substitute_one_sided_flags(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    if FdOperation::is_diacritic(&sp.0) && sp.1 == internal_epsilon {
        sps.insert((sp.0.clone(), sp.0.clone()));
        return true;
    }
    if FdOperation::is_diacritic(&sp.1) && sp.0 == internal_epsilon {
        sps.insert((sp.1.clone(), sp.1.clone()));
        return true;
    }
    false
}

// [spec:hfst:def:hfst-transducer.hfst.substitute-input-flag-with-epsilon-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-input-flag-with-epsilon-fn]
pub(super) fn substitute_input_flag_with_epsilon(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    if FdOperation::is_diacritic(&sp.0) {
        sps.insert((Symbol::new_static(internal_epsilon), sp.1.clone()));
        return true;
    }
    false
}

// [spec:hfst:def:hfst-transducer.hfst.substitute-output-flag-with-epsilon-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-output-flag-with-epsilon-fn]
pub(super) fn substitute_output_flag_with_epsilon(
    sp: &StringPair,
    sps: &mut StringPairSet,
) -> bool {
    if FdOperation::is_diacritic(&sp.1) {
        sps.insert((sp.0.clone(), Symbol::new_static(internal_epsilon)));
        return true;
    }
    false
}

// C++ file-static flag-diacritic helpers. `has_flags` is read-only; the others
// mutate the transducer in place (C++ `fst = HfstTransducer(...)`), so they take
// `&mut HfstTransducer` (callers pass `&mut self` / `&mut another`).

// [spec:hfst:def:hfst-transducer.hfst.encode-flag-fn]
// [spec:hfst:sem:hfst-transducer.hfst.encode-flag-fn]
pub(crate) fn encode_flag(flag_diacritic: &str) -> Symbol {
    let mut retval: Vec<u8> = flag_diacritic.as_bytes().to_vec();
    let last = retval.len() - 1;
    retval[0] = b'%';
    retval[last] = b'%';
    Symbol::from(
        String::from_utf8(retval).expect("flag diacritic remains valid UTF-8 after %-escaping"),
    )
}

// [spec:hfst:def:hfst-transducer.hfst.decode-flag-fn]
// [spec:hfst:sem:hfst-transducer.hfst.decode-flag-fn]
pub(crate) fn decode_flag(flag_diacritic: &str) -> Symbol {
    let bytes = flag_diacritic.as_bytes();
    if bytes[0] != b'%' || bytes[bytes.len() - 1] != b'%' {
        return Symbol::new(flag_diacritic);
    }
    let mut retval: Vec<u8> = bytes.to_vec();
    let last = retval.len() - 1;
    retval[0] = b'@';
    retval[last] = b'@';
    Symbol::from(
        String::from_utf8(retval).expect("flag diacritic remains valid UTF-8 after @-unescaping"),
    )
}

// [spec:hfst:def:hfst-transducer.hfst.add-suffix-to-feature-name-fn]
// [spec:hfst:sem:hfst-transducer.hfst.add-suffix-to-feature-name-fn]
fn add_suffix_to_feature_name(flag_diacritic: &str, suffix: &str) -> Symbol {
    Symbol::from(
        "@".to_string()
            + &FdOperation::get_operator(flag_diacritic)
            + "."
            + &FdOperation::get_feature(flag_diacritic)
            + suffix
            + &(if FdOperation::has_value(flag_diacritic) {
                ".".to_string() + &FdOperation::get_value(flag_diacritic)
            } else {
                String::new()
            })
            + "@",
    )
}

// [spec:hfst:def:hfst-transducer.hfst.has-flags-fn]
// [spec:hfst:sem:hfst-transducer.hfst.has-flags-fn]
pub(super) fn has_flags<B: Backend>(fst: &HfstTransducer<B>) -> bool {
    let alphabet = fst
        .get_alphabet()
        .expect("get_alphabet on a valid transducer cannot fail");
    for it in alphabet.iter() {
        if FdOperation::is_diacritic(it) {
            return true;
        }
    }
    false
}

// Return true if the flag in flag_diacritic ends in suffix and false
// otherwise. E.g. if flag_diacritic = "@D.NeedNoun_1.ON@ and suffix =
// "_1", return true.
// [spec:hfst:def:hfst-transducer.hfst.is-flag-suffix-fn]
// [spec:hfst:sem:hfst-transducer.hfst.is-flag-suffix-fn]
pub(super) fn is_flag_suffix(suffix: &str, flag_diacritic: &str) -> bool {
    FdOperation::is_diacritic(flag_diacritic)
        && FdOperation::get_feature(flag_diacritic).ends_with(suffix)
}

// [spec:hfst:def:hfst-transducer.hfst.rename-flag-diacritics-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rename-flag-diacritics-fn]
pub(super) fn rename_flag_diacritics<B: Backend>(fst: &mut HfstTransducer<B>, suffix: &str) {
    let basic_fst = HfstBasicTransducer::from_transducer(fst);
    let mut basic_fst_copy = HfstBasicTransducer::new();
    let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

    // Rebuilding from transitions alone drops symbols that occur only in the
    // alphabet. Those symbols are semantically significant to flag
    // harmonization, so carry the complete alphabet across and apply the same
    // feature rename to alphabet-only flags.
    for symbol in basic_fst.get_alphabet() {
        let renamed = if FdOperation::is_diacritic(symbol) {
            add_suffix_to_feature_name(symbol, suffix)
        } else {
            symbol.clone()
        };
        basic_fst_copy.add_symbol_to_alphabet(&renamed);
    }

    for (s, states) in basic_fst.state_vector.iter().enumerate() {
        let s = s as HfstState;
        for transition in states.iter() {
            let input_symbol = transition.get_input_symbol(basic_fst.coder());
            let output_symbol = transition.get_output_symbol(basic_fst.coder());
            let isym = if FdOperation::is_diacritic(&input_symbol) {
                add_suffix_to_feature_name(&input_symbol, suffix)
            } else {
                input_symbol
            };
            let osym = if FdOperation::is_diacritic(&output_symbol) {
                add_suffix_to_feature_name(&output_symbol, suffix)
            } else {
                output_symbol
            };
            let tr = HfstBasicTransition::new_symbols(
                transition.get_target_state(),
                isym,
                osym,
                transition.get_weight(),
                basic_fst_copy.coder_mut(),
            );
            basic_fst_copy.add_transition(s, &tr, true);
        }

        if basic_fst.is_final_state(s) {
            basic_fst_copy.set_final_weight(
                s,
                &basic_fst
                    .get_final_weight(s)
                    .expect("state was confirmed final via is_final_state"),
            );
        }
    }
    *fst = HfstTransducer::from_basic(&basic_fst_copy);
}

// The flag encode/decode passes (`encode_flag_diacritics` /
// `decode_flag_diacritics`) dispatch to the backend. The tropical backend does
// a pure in-place SymbolTable rename (equivalent automaton, bytes diverge from
// the whole-graph round-trip by design — [node:flag-encode-diverge]); every
// other backend keeps the C++ round-trip via the Backend default. Either way
// the facade metadata is reset exactly as the former `*fst = from_basic(...)`
// did: name -> "", props -> {}, anonymous/is_trie -> false.
fn reset_facade_metadata_after_flag_pass<B: Backend>(fst: &mut HfstTransducer<B>) {
    fst.name = String::new();
    fst.props = BTreeMap::new();
    fst.anonymous = false;
    fst.is_trie = false;
}

// [spec:hfst:def:hfst-transducer.hfst.encode-flag-diacritics-fn]
// [spec:hfst:sem:hfst-transducer.hfst.encode-flag-diacritics-fn]
pub(super) fn encode_flag_diacritics<B: Backend>(fst: &mut HfstTransducer<B>) {
    fst.fst.encode_flag_diacritics();
    reset_facade_metadata_after_flag_pass(fst);
}

// [spec:hfst:def:hfst-transducer.hfst.decode-flag-diacritics-fn]
// [spec:hfst:sem:hfst-transducer.hfst.decode-flag-diacritics-fn]
pub(super) fn decode_flag_diacritics<B: Backend>(fst: &mut HfstTransducer<B>) {
    fst.fst.decode_flag_diacritics();
    reset_facade_metadata_after_flag_pass(fst);
}

impl FlagDiacriticOverlay {
    /// Whether preparation found no missing flag transitions on either side.
    pub fn is_empty(&self) -> bool {
        self.left_self_loops.is_empty() && self.right_self_loops.is_empty()
    }

    pub(crate) fn xerox_encoded(&self) -> Self {
        let encode =
            |symbols: &StringSet| symbols.iter().map(|symbol| encode_flag(symbol)).collect();
        Self {
            left_self_loops: encode(&self.left_self_loops),
            right_self_loops: encode(&self.right_self_loops),
            enforce_left_before_right: self.enforce_left_before_right,
        }
    }
}

impl<B: AlgebraBackend> HfstTransducer<B> {
    /// Prepare operands for a flag-diacritic-aware algebra operation without
    /// materializing the `states * missing_flags` self-loops.
    ///
    /// This performs the same flag renaming as [`Self::harmonize_flag_diacritics`]
    /// when both operands originally contain flags. It then harmonizes only the
    /// missing flag symbols into the opposite alphabets and returns those exact
    /// differences for a backend operation overlay.
    // [spec:hfst:req:virtual-flag-algebra.materialized-reference]
    // [spec:hfst:req:virtual-flag-algebra.backend-core]
    pub fn prepare_flag_diacritics_for_operation(
        &mut self,
        another: &mut HfstTransducer<B>,
    ) -> crate::error::Result<FlagDiacriticOverlay> {
        let left_had_flags = has_flags(self);
        let right_had_flags = has_flags(another);

        if left_had_flags && right_had_flags {
            rename_flag_diacritics(self, "_1");
            rename_flag_diacritics(another, "_2");
        }

        // Do this in order: after right-side flags are inserted into the left
        // alphabet, the reverse difference still consists exactly of the
        // original left-side flags because the post-rename sets are disjoint.
        let left_self_loops = self.insert_missing_diacritics_to_alphabet_from(another)?;
        let right_self_loops = another.insert_missing_diacritics_to_alphabet_from(self)?;
        let enforce_left_before_right = left_had_flags
            && right_had_flags
            && !left_self_loops.is_empty()
            && !right_self_loops.is_empty();

        Ok(FlagDiacriticOverlay {
            left_self_loops,
            right_self_loops,
            enforce_left_before_right,
        })
    }

    /// Compatibility wrapper for callers preparing ordinary composition.
    pub fn prepare_flag_diacritics_for_compose(
        &mut self,
        another: &mut HfstTransducer<B>,
    ) -> crate::error::Result<FlagDiacriticComposeOverlay> {
        self.prepare_flag_diacritics_for_operation(another)
    }
}
