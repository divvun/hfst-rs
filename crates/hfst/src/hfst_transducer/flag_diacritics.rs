//! Flag-diacritic harmonization, illegal-path restriction, and elimination.

use super::*;

impl<B: Backend> HfstTransducer<B> {
    /*
       Check for missing flag diacritics (FG), i.e. FGs that are present in the
       alphabet of \a another but not in the alphabet of this transducer and insert
       them to \a missing_flags. \a return_on_first_miss defines whether function
       returns after first missing FG is found and inserted to \a missing_flags.
       @ retval Whether any missing FGs where found.
    */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    pub fn check_for_missing_flags_in_into(
        &self,
        another: &HfstTransducer<B>,
        missing_flags: &mut StringSet,
        return_on_first_miss: bool,
    ) -> bool {
        let mut retval = false;
        let this_alphabet: StringSet = self
            .get_alphabet()
            .expect("get_alphabet on a valid transducer cannot fail");
        let another_alphabet: StringSet = another
            .get_alphabet()
            .expect("get_alphabet on a valid transducer cannot fail");

        for it in another_alphabet.iter() {
            if FdOperation::is_diacritic(it) && (!this_alphabet.contains(it)) {
                missing_flags.insert(it.clone());
                retval = true;
                if return_on_first_miss {
                    return retval;
                }
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-freely-missing-flags-from-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-freely-missing-flags-from-fn]
    #[track_caller]
    pub fn insert_freely_missing_flags_from(&mut self, another: &HfstTransducer<B>) {
        let mut missing_flags: StringSet = StringSet::new();
        if self.check_for_missing_flags_in_into(
            another,
            &mut missing_flags,
            false, /* do not return on first miss */
        ) {
            let caller = std::panic::Location::caller();
            let state_count = self.number_of_states();
            let missing_flag_count = missing_flags.len();
            let added_arc_count = u64::from(state_count) * missing_flag_count as u64;
            tracing::warn!(
                target: "hfst::virtual_flags",
                state_count,
                missing_flag_count,
                added_arc_count,
                caller_file = caller.file(),
                caller_line = caller.line(),
                "materializing missing flag-diacritic self-loops eagerly; no virtual overlay was selected"
            );
            let mut basic: HfstBasicTransducer = HfstBasicTransducer::from_transducer(self);

            // Every state gains a free self-loop per missing flag, so the graph
            // grows by 'states x flags' transitions — on a Giella speller that is
            // hundreds of millions. Intern each flag's symbol number and alphabet
            // entry once instead of once per (state, flag), and give each state
            // room for exactly its new loops, so the transition vectors never
            // double past the size they end at.
            let loops: Vec<(u32, u32)> = missing_flags
                .iter()
                .map(|flag| {
                    let tr = HfstBasicTransition::new_symbols(
                        0,
                        flag.clone(),
                        flag.clone(),
                        0.0,
                        basic.coder_mut(),
                    );
                    basic.add_symbol_to_alphabet(flag);
                    (tr.get_input_number(), tr.get_output_number())
                })
                .collect();

            for s in 0..=basic.get_max_state() {
                let transitions = &mut basic.state_vector[s as usize];
                transitions.reserve_exact(loops.len());
                for (input, output) in &loops {
                    transitions.push(HfstBasicTransition::new_numbers(
                        s, *input, *output, 0.0, false,
                    ));
                }
            }

            *self = HfstTransducer::from_basic_owned(basic);
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
    pub fn has_flag_diacritics(&self) -> bool {
        has_flags(self)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
    pub fn twosided_flag_diacritics(&mut self) -> crate::error::Result<()> {
        let basic_fst: HfstBasicTransducer = HfstBasicTransducer::from_transducer(self);
        let mut basic_fst_copy: HfstBasicTransducer = HfstBasicTransducer::new();
        let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

        for (s, states) in basic_fst.state_vector.iter().enumerate() {
            let s = s as HfstState;
            for transition in states.iter() {
                let istr = transition.get_input_symbol(basic_fst.coder());
                let ostr = transition.get_output_symbol(basic_fst.coder());
                let istr_is_flag = FdOperation::is_diacritic(&istr);
                let ostr_is_flag = FdOperation::is_diacritic(&ostr);

                let extra_transition_needed = (istr_is_flag || ostr_is_flag) && (istr != ostr);

                if extra_transition_needed {
                    let new_state: HfstState = basic_fst_copy.add_state_new();

                    // flag:foo -> flag:flag 0:foo, foo:flag -> foo:0 flag:flag
                    // flag1:flag2 -> flag1:flag1 flag2:flag2

                    let mut input = istr.clone();
                    let mut out = if istr_is_flag {
                        istr.clone()
                    } else {
                        Symbol::new_static(crate::hfst_symbol_defs::internal_epsilon)
                    };

                    let tr = HfstBasicTransition::new_symbols(
                        new_state,
                        input,
                        out,
                        0.0, /*?*/
                        basic_fst_copy.coder_mut(),
                    );
                    basic_fst_copy.add_transition(s, &tr, true);

                    input = if ostr_is_flag {
                        ostr.clone()
                    } else {
                        Symbol::new_static(crate::hfst_symbol_defs::internal_epsilon)
                    };
                    out = ostr.clone();

                    let tr = HfstBasicTransition::new_symbols(
                        transition.get_target_state(),
                        input,
                        out,
                        transition.get_weight(), /*?*/
                        basic_fst_copy.coder_mut(),
                    );
                    basic_fst_copy.add_transition(new_state, &tr, true);
                } else {
                    let tr = HfstBasicTransition::new_symbols(
                        transition.get_target_state(),
                        istr.clone(),
                        ostr.clone(),
                        transition.get_weight(),
                        basic_fst_copy.coder_mut(),
                    );
                    basic_fst_copy.add_transition(s, &tr, true);
                }
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
        *self = HfstTransducer::new_from_basic(&basic_fst_copy)?;
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    pub fn check_for_missing_flags_in(&self, another: &HfstTransducer<B>) -> bool {
        let mut unused_missing_flags: StringSet = StringSet::new(); /* An obligatory argument that is not used. */
        self.check_for_missing_flags_in_into(
            another,
            &mut unused_missing_flags,
            true, /* return on first miss */
        )
    }
}

impl<B: AlgebraBackend> HfstTransducer<B> {
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-flag-diacritics-fn]
    #[track_caller]
    pub fn harmonize_flag_diacritics(
        &mut self,
        another: &mut HfstTransducer<B>,
        insert_renamed_flags: bool,
    ) -> crate::error::Result<()> {
        let this_has_flag_diacritics = has_flags(self);
        let another_has_flag_diacritics = has_flags(another);

        if this_has_flag_diacritics && another_has_flag_diacritics {
            rename_flag_diacritics(self, "_1");
            rename_flag_diacritics(another, "_2");

            if insert_renamed_flags {
                self.insert_freely_missing_flags_from(another);
                another.insert_freely_missing_flags_from(self);
                self.remove_illegal_flag_paths()?;
            }
        } else if this_has_flag_diacritics && insert_renamed_flags {
            another.insert_freely_missing_flags_from(self);
        } else if another_has_flag_diacritics && insert_renamed_flags {
            self.insert_freely_missing_flags_from(another);
        }
        Ok(())
    }

    // -------------------------------------------------------------------------
    // ----- Flag elimination -----
    // -------------------------------------------------------------------------

    pub fn eliminate_flags(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        let basic = crate::hfst_basic_transducer::HfstBasicTransducer::from_transducer(self);
        let flags = basic.get_flags();
        let filter = get_flag_filter(self, &flags, "")?;

        if let Some(filter) = filter {
            let mut filter_copy = HfstTransducer::new_from(&filter);
            {
                let self_copy = HfstTransducer::new_from(self);
                let filter_deref = HfstTransducer::new_from(&filter);
                // Compose the symbol-level flag-constraint filter with flags
                // encoded as ordinary symbols (see eliminate_flag for why).
                let cfg = EngineConfig {
                    xerox_composition: true,
                    ..EngineConfig::default()
                };
                filter_copy.compose_with_config(&self_copy, true, &cfg)?;
                filter_copy.compose_with_config(&filter_deref, true, &cfg)?;
            }
            flag_purge(&mut filter_copy, "")?;
            *self = filter_copy;
        } else {
            flag_purge(self, "")?;
        }

        self.optimize()
    }

    pub fn eliminate_flag(&mut self, flag: &str) -> crate::error::Result<&mut HfstTransducer<B>> {
        let basic = crate::hfst_basic_transducer::HfstBasicTransducer::from_transducer(self);
        let flags = basic.get_flags();
        let feature_found = flags
            .iter()
            .any(|it| crate::hfst_flag_diacritics::FdOperation::get_feature(it) == flag);
        if !feature_found {
            if !flag.contains('.') {
                crate::bail!(
                    Hfst,
                    format!(
                        "HfstTransducer::eliminate_flag: flag feature does not occur in the transducer: {}",
                        flag
                    )
                );
            } else {
                crate::bail!(
                    Hfst,
                    format!(
                        "HfstTransducer::eliminate_flag: only the flag feature must be given, no value or operator: {}",
                        flag
                    )
                );
            }
        }

        let filter = get_flag_filter(self, &flags, flag)?;
        if let Some(filter) = filter {
            let mut filter_copy = HfstTransducer::new_from(&filter);
            {
                let self_copy = HfstTransducer::new_from(self);
                let filter_deref = HfstTransducer::new_from(&filter);
                // The filter is a symbol-level constraint (built over escaped
                // flags so the flag features are ordinary symbols); apply it
                // with flag diacritics encoded as ordinary symbols in the
                // composition. Otherwise flag harmonization drops any path that
                // carries a flag of some OTHER feature, since the filter's
                // '?' (identity) will not match a foreign flag once that flag
                // is added to the filter's alphabet without an explicit arc.
                let cfg = EngineConfig {
                    xerox_composition: true,
                    ..EngineConfig::default()
                };
                filter_copy.compose_with_config(&self_copy, true, &cfg)?;
                filter_copy.compose_with_config(&filter_deref, true, &cfg)?;
            }
            flag_purge(&mut filter_copy, flag)?;
            *self = filter_copy;
        } else {
            flag_purge(self, flag)?;
        }

        self.optimize()
    }

    pub(crate) fn remove_illegal_flag_paths(
        &mut self,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let alphabet = self.get_alphabet()?;
        let mut _1_flags: StringSet = StringSet::new();
        let mut _2_flags: StringSet = StringSet::new();

        // Gather _1 and _2 flag diacritics.
        for it in &alphabet {
            if !FdOperation::is_diacritic(it) {
                continue;
            }

            if flag_ops::is_flag_suffix("_1", it) {
                _1_flags.insert(it.clone());
            }

            if flag_ops::is_flag_suffix("_2", it) {
                _2_flags.insert(it.clone());
            }
        }

        // if there aren't both _1 and _2 flag diaciritcs, there can be no
        // illegal paths.
        if _1_flags.is_empty() || _2_flags.is_empty() {
            return Ok(self);
        }

        // Rename @...@ flags to $...$ flags and compile restriction.
        let mut subst: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
        let mut back_subst: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();

        for _1_flag in &_1_flags {
            let at_flag = _1_flag.clone();
            // Replace the leading and trailing '@' (both ASCII) with '$'.
            let dollar_flag = Symbol::from(format!("${}$", &at_flag[1..at_flag.len() - 1]));

            subst.insert(at_flag.clone(), dollar_flag.clone());
            back_subst.insert(dollar_flag, at_flag);
        }

        for _2_flag in &_2_flags {
            let at_flag = _2_flag.clone();
            // Replace the leading and trailing '@' (both ASCII) with '$'.
            let dollar_flag = Symbol::from(format!("${}$", &at_flag[1..at_flag.len() - 1]));

            subst.insert(at_flag.clone(), dollar_flag.clone());
            back_subst.insert(dollar_flag, at_flag);
        }

        self.substitute_symbols(&subst)?;

        let mut restriction = get_flag_path_restriction(&_1_flags, &_2_flags);

        // Apply restrictions.
        self.compose(&restriction, true)?;
        let _ = &mut restriction;

        // Rename $...$ flags back to @...@ flags.
        self.substitute_symbols(&back_subst)?;

        Ok(self)
    }
}

// -----------------------------------------------------------------------------
// Flag-elimination helpers (file-scope free functions in the C++).
// -----------------------------------------------------------------------------

// if (required): return ~[(?* FAIL_FLAGS) ~$SUCCEED_FLAGS SELF ?*]
// if (! required): return ~[?* FAIL_FLAGS ~$SUCCEED_FLAGS SELF ?*]
// [spec:hfst:def:hfst-transducer.hfst.new-filter-fn]
// [spec:hfst:sem:hfst-transducer.hfst.new-filter-fn]
fn new_filter<B: AlgebraBackend>(
    fail_flags: &HfstTransducer<B>,
    succeed_flags: &HfstTransducer<B>,
    this: &HfstTransducer<B>,
    required: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut comp = crate::xre::XreCompiler::<B>::new();
    comp.set_expand_definitions(true);
    comp.define_transducer("Fail", fail_flags);
    comp.define_transducer("Succeed", succeed_flags);
    comp.define_transducer("Self", this);
    let mut result: HfstTransducer<B> = if required {
        comp.compile("~[(?* Fail) ~$Succeed Self ?*]")
    } else {
        comp.compile("~[?* Fail ~$Succeed Self ?*]")
    }
    .expect("the flag-filter xre is well-formed");

    // Should the xre compiler do this?
    result.remove_from_alphabet("Fail")?;
    result.remove_from_alphabet("Succeed")?;
    result.remove_from_alphabet("Self")?;

    Ok(result)
}

// Substitute each symbol '_@FLAG@' with '@FLAG@'
// [spec:hfst:def:hfst-transducer.hfst.substitute-escaped-flags-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-escaped-flags-fn]
fn substitute_escaped_flags<B: AlgebraBackend>(
    filter: &mut HfstTransducer<B>,
) -> crate::error::Result<()> {
    let alpha = filter.get_alphabet()?;
    for it in alpha.iter() {
        if it.len() > 1 {
            let bytes = it.as_bytes();
            if bytes[0] == b'_' && bytes[1] == b'@' {
                // 'std::string::erase(0)' drops the leading '_'; rebuild the
                // SmolStr from the remaining bytes instead of mutating in place.
                let s = Symbol::new(&it[1..]);
                filter.substitute_symbol(it, &s, true, true)?;
            }
        }
    }
    Ok(())
}

const FLAG_UNIFY: i32 = 1;
const FLAG_CLEAR: i32 = 2;
const FLAG_DISALLOW: i32 = 4;
const FLAG_NEGATIVE: i32 = 8;
const FLAG_POSITIVE: i32 = 16;
const FLAG_REQUIRE: i32 = 32;
#[allow(dead_code)]
const FLAG_EQUAL: i32 = 64;

const FLAG_FAIL: i32 = 1;
const FLAG_SUCCEED: i32 = 2;
const FLAG_NONE: i32 = 3;

// [spec:hfst:def:hfst-transducer.hfst.flag-build-fn]
// [spec:hfst:sem:hfst-transducer.hfst.flag-build-fn]
fn flag_build(
    ftype: i32,
    fname: &str,
    fvalue: &str,
    fftype: i32,
    ffname: &str,
    ffvalue: &str,
) -> i32 {
    if fname != ffname {
        return FLAG_NONE;
    }

    let mut selfnull = false; /* If current flag has no value, e.g. @R.A@ */
    if fvalue.is_empty() {
        selfnull = true;
    }

    let eq: i32 = if fvalue == ffvalue {
        0
    } else if fvalue < ffvalue {
        -1
    } else {
        1
    };

    if let Some(status) = unify_flag_status(ftype, fftype, eq) {
        return status;
    }
    if let Some(status) = require_flag_status(ftype, fftype, eq, selfnull) {
        return status;
    }
    if let Some(status) = disallow_flag_status(ftype, fftype, eq, selfnull) {
        return status;
    }

    FLAG_NONE
}

fn unify_flag_status(ftype: i32, fftype: i32, eq: i32) -> Option<i32> {
    /* U flags */
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_POSITIVE) && (eq == 0) {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_CLEAR) {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_UNIFY) && (eq != 0) {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_POSITIVE) && (eq != 0) {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_NEGATIVE) && (eq == 0) {
        return Some(FLAG_FAIL);
    }

    None
}

fn require_flag_status(ftype: i32, fftype: i32, eq: i32, selfnull: bool) -> Option<i32> {
    /* R flag with value = 0 */
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_UNIFY) && selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_POSITIVE) && selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_NEGATIVE) && selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_CLEAR) && selfnull {
        return Some(FLAG_FAIL);
    }

    /* R flag with value */
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_POSITIVE) && (eq == 0) && !selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_UNIFY) && (eq == 0) && !selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_POSITIVE) && (eq != 0) && !selfnull {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_UNIFY) && (eq != 0) && !selfnull {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_NEGATIVE) && !selfnull {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_CLEAR) && !selfnull {
        return Some(FLAG_FAIL);
    }

    None
}

fn disallow_flag_status(ftype: i32, fftype: i32, eq: i32, selfnull: bool) -> Option<i32> {
    /* D flag with value = 0 */
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_CLEAR) && selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_POSITIVE) && selfnull {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_UNIFY) && selfnull {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_NEGATIVE) && selfnull {
        return Some(FLAG_FAIL);
    }

    /* D flag with value */
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_POSITIVE) && (eq != 0) && !selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_CLEAR) && !selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_NEGATIVE) && (eq == 0) && !selfnull {
        return Some(FLAG_SUCCEED);
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_POSITIVE) && (eq == 0) && !selfnull {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_UNIFY) && (eq == 0) && !selfnull {
        return Some(FLAG_FAIL);
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_NEGATIVE) && (eq != 0) && !selfnull {
        return Some(FLAG_FAIL);
    }

    None
}

// [spec:hfst:def:hfst-transducer.hfst.hfst-operator-to-char-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-operator-to-char-fn]
fn hfst_operator_to_char(op: &str) -> i32 {
    let c = op.as_bytes()[0];
    if c == b'U' {
        return FLAG_UNIFY;
    }
    if c == b'C' {
        return FLAG_CLEAR;
    }
    if c == b'D' {
        return FLAG_DISALLOW;
    }
    if c == b'N' {
        return FLAG_NEGATIVE;
    }
    if c == b'P' {
        return FLAG_POSITIVE;
    }
    if c == b'R' {
        return FLAG_REQUIRE;
    }
    std::panic::panic_any("invalid operator");
}

// [spec:hfst:def:hfst-transducer.hfst.is-valid-flag-combination-fn]
// [spec:hfst:sem:hfst-transducer.hfst.is-valid-flag-combination-fn]
fn is_valid_flag_combination(flag1: &str, flag2: &str) -> i32 {
    let operator1 = hfst_operator_to_char(&crate::hfst_flag_diacritics::FdOperation::get_operator(
        flag1,
    ));
    let feature1 = crate::hfst_flag_diacritics::FdOperation::get_feature(flag1);
    let value1 = crate::hfst_flag_diacritics::FdOperation::get_value(flag1);

    let operator2 = hfst_operator_to_char(&crate::hfst_flag_diacritics::FdOperation::get_operator(
        flag2,
    ));
    let feature2 = crate::hfst_flag_diacritics::FdOperation::get_feature(flag2);
    let value2 = crate::hfst_flag_diacritics::FdOperation::get_value(flag2);

    flag_build(operator1, &feature1, &value1, operator2, &feature2, &value2)
}

/* @brief Get flag filter for transducer \a transducer. */
// [spec:hfst:def:hfst-transducer.hfst.get-flag-filter-fn]
// [spec:hfst:sem:hfst-transducer.hfst.get-flag-filter-fn]
fn get_flag_filter<B: AlgebraBackend>(
    transducer: &HfstTransducer<B>,
    flags: &crate::hfst_symbol_defs::StringSet,
    flag: &str,
) -> crate::error::Result<Option<HfstTransducer<B>>> {
    let _ = transducer;
    let mut flag_found = false;
    let mut filter: Option<HfstTransducer<B>> = None;

    for f in flags.iter() {
        let this = HfstTransducer::new_symbol(&format!("_{}", f))?; // escape flags
        let mut succeed_flags = HfstTransducer::new();
        let mut fail_flags = HfstTransducer::new();

        let op = crate::hfst_flag_diacritics::FdOperation::get_operator(f).as_bytes()[0];
        if (flag.is_empty() || crate::hfst_flag_diacritics::FdOperation::get_feature(f) == flag)
            && (op == b'U' || op == b'R' || op == b'D')
        // Equal flag?
        {
            for flag2 in flags.iter() {
                let fstatus = is_valid_flag_combination(f, flag2);

                if fstatus == 1 {
                    fail_flags
                        .disjunct(&HfstTransducer::new_symbol(&format!("_{}", flag2))?, true)?;
                    flag_found = true;
                } else if fstatus == 2 {
                    succeed_flags
                        .disjunct(&HfstTransducer::new_symbol(&format!("_{}", flag2))?, true)?;
                    flag_found = true;
                }
            }
        }

        if flag_found {
            let newfilter = new_filter(
                &fail_flags,
                &succeed_flags,
                &this,
                crate::hfst_flag_diacritics::FdOperation::get_operator(f).as_bytes()[0] == b'R',
            )?;

            // intersect filter with newfilter
            match filter.as_mut() {
                None => filter = Some(newfilter),
                Some(filt) => {
                    filt.intersect(&newfilter, true)?;
                }
            }
        }
        flag_found = false;
    }

    if let Some(filt) = filter.as_mut() {
        substitute_escaped_flags(filt)?; // unescape the flags
        filt.optimize()?;
    }

    Ok(filter)
}

// Replace arcs in \a transducer that use flag \a flag with epsilon arcs
// and remove \a flag from alphabet of \a transducer. If \a flag is the empty
// string, replace/remove all flags.
// [spec:hfst:def:hfst-transducer.hfst.flag-purge-fn]
// [spec:hfst:sem:hfst-transducer.hfst.flag-purge-fn]
fn flag_purge<B: Backend>(
    transducer: &mut HfstTransducer<B>,
    flag: &str,
) -> crate::error::Result<()> {
    let mut net = crate::hfst_basic_transducer::HfstBasicTransducer::from_transducer(transducer);
    net.flag_purge(flag);
    *transducer = HfstTransducer::new_from_basic(&net)?;
    Ok(())
}

// Composition with this transducer restricts _1_flags ($X.Y_1.Z$) so
// they can't succeed _2_flags ($X.Y_2.Z$) immediately. Used for
// filtering illegal combinations of flag diacritics after binary
// operations.
// [spec:hfst:def:hfst-transducer.hfst.get-flag-path-restriction-fn]
// [spec:hfst:sem:hfst-transducer.hfst.get-flag-path-restriction-fn]
pub fn get_flag_path_restriction<B: Backend>(
    _1_flags: &StringSet,
    _2_flags: &StringSet,
) -> HfstTransducer<B> {
    // Two state fst with borh states final.
    let mut basic_restriction = HfstBasicTransducer::new();
    basic_restriction.add_state_new();
    let start_state: HfstState = 0;
    let seen_2_state: HfstState = 1;

    basic_restriction.set_final_weight(start_state, &0.0);
    basic_restriction.set_final_weight(seen_2_state, &0.0);

    let tr = HfstBasicTransition::new_symbols(
        start_state,
        Symbol::new_static(internal_identity),
        Symbol::new_static(internal_identity),
        0.0,
        basic_restriction.coder_mut(),
    );
    basic_restriction.add_transition(start_state, &tr, true);

    let tr = HfstBasicTransition::new_symbols(
        start_state,
        Symbol::new_static(internal_identity),
        Symbol::new_static(internal_identity),
        0.0,
        basic_restriction.coder_mut(),
    );
    basic_restriction.add_transition(seen_2_state, &tr, true);

    // All _1_flags are allowed as long as no _2_flags with no
    // intervening symbols were observed.
    for dollar_flag in _1_flags {
        let inner = dollar_flag
            .strip_prefix('@')
            .and_then(|s| s.strip_suffix('@'))
            .expect("flag diacritic is @-delimited");
        let dollar_flag = Symbol::from(format!("${inner}$"));

        let tr = HfstBasicTransition::new_symbols(
            start_state,
            dollar_flag.clone(),
            dollar_flag,
            0.0,
            basic_restriction.coder_mut(),
        );
        basic_restriction.add_transition(start_state, &tr, true);
    }

    // If _2_flags are observed, _1_flags are illegal before an
    // intervening regular symbol is seen.
    for dollar_flag in _2_flags {
        let inner = dollar_flag
            .strip_prefix('@')
            .and_then(|s| s.strip_suffix('@'))
            .expect("flag diacritic is @-delimited");
        let dollar_flag = Symbol::from(format!("${inner}$"));

        let tr = HfstBasicTransition::new_symbols(
            seen_2_state,
            dollar_flag.clone(),
            dollar_flag.clone(),
            0.0,
            basic_restriction.coder_mut(),
        );
        basic_restriction.add_transition(start_state, &tr, true);

        let tr = HfstBasicTransition::new_symbols(
            seen_2_state,
            dollar_flag.clone(),
            dollar_flag,
            0.0,
            basic_restriction.coder_mut(),
        );
        basic_restriction.add_transition(seen_2_state, &tr, true);
    }

    HfstTransducer::from_basic(&basic_restriction)
}
