//! Flag-diacritic harmonization, illegal-path restriction, and elimination.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_tropical_transducer_transition_data::WeightType;
use std::collections::HashMap;

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
            let mut basic: HfstBasicTransducer =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self).expect(
                    "hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail",
                );

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

            *self = HfstTransducer::new_from_basic_owned(basic)
                .expect("converting a basic transducer to an available backend type cannot fail");
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
    // [spec:hfst:def:hfst-transducer.hfst.has-flags-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.has-flags-fn]
    pub fn has_flag_diacritics(&self) -> bool {
        let alphabet = self
            .get_alphabet()
            .expect("get_alphabet on a valid transducer cannot fail");
        for it in alphabet.iter() {
            if FdOperation::is_diacritic(it) {
                return true;
            }
        }
        false
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
    pub fn twosided_flag_diacritics(&mut self) -> crate::error::Result<()> {
        let basic_fst: HfstBasicTransducer =
            ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
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
        let this_has_flag_diacritics = self.has_flag_diacritics();
        let another_has_flag_diacritics = another.has_flag_diacritics();

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

    // [spec:hfst:req:flag-elimination.relation]
    pub fn eliminate_flags(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        let basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
        let flags = basic.get_flags();
        self.eliminate_flags_in(basic, &flags, "")
    }

    // [spec:hfst:req:flag-elimination.relation]
    pub fn eliminate_flag(&mut self, flag: &str) -> crate::error::Result<&mut HfstTransducer<B>> {
        let basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
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

        self.eliminate_flags_in(basic, &flags, flag)
    }

    /// Replace this transducer with the paths of `basic` whose flags of feature
    /// `flag` (of every feature when `flag` is empty) are consistent on both
    /// tapes, those flags turned into epsilons. The filter is applied while
    /// walking `basic`, so the work follows the (state, input flag state,
    /// output flag state) triples reachable from the start, not the size of a
    /// filter over every combination of flag values.
    // [spec:hfst:req:flag-elimination.relation]
    // [spec:hfst:req:flag-elimination.reachable-product]
    fn eliminate_flags_in(
        &mut self,
        basic: HfstBasicTransducer,
        flags: &StringSet,
        flag: &str,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut net = match get_flag_filter(&basic, flags, flag) {
            Some(filter) => filter.apply(basic),
            None => basic,
        };
        // [spec:hfst:def:hfst-transducer.hfst.flag-purge-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.flag-purge-fn]
        net.flag_purge(flag);
        *self = HfstTransducer::new_from_basic_owned(net)?;
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

        self.substitute_symbol_substitutions(&subst)?;

        let mut restriction = get_flag_path_restriction(&_1_flags, &_2_flags);

        // Apply restrictions.
        self.compose(&restriction, true)?;
        let _ = &mut restriction;

        // Rename $...$ flags back to @...@ flags.
        self.substitute_symbol_substitutions(&back_subst)?;

        Ok(self)
    }
}

// -----------------------------------------------------------------------------
// Flag-elimination helpers (file-scope free functions in the C++).
// -----------------------------------------------------------------------------

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

// One constrained flag: the U, R or D flag `this` with the flags of its
// feature that make it fail or succeed (see flag_build). Reading `this` is
// allowed unless the last of those flags read before it was a fail flag; an R
// flag (`required`) is also refused when none of them was read at all.
// Upstream compiles each constraint into a transducer,
// `~[?* FAIL_FLAGS ~$SUCCEED_FLAGS SELF ?*]`, or with `(?* FAIL_FLAGS)` for R.
// [spec:hfst:def:hfst-transducer.hfst.new-filter-fn+1]
// [spec:hfst:sem:hfst-transducer.hfst.new-filter-fn+1]
struct FlagConstraint {
    this: usize,
    fail_flags: Vec<usize>,
    succeed_flags: Vec<usize>,
    required: bool,
}

/* @brief Get flag filter for transducer \a transducer. */
// [spec:hfst:def:hfst-transducer.hfst.get-flag-filter-fn+1]
// [spec:hfst:sem:hfst-transducer.hfst.get-flag-filter-fn+1]
fn get_flag_filter(
    transducer: &HfstBasicTransducer,
    flags: &crate::hfst_symbol_defs::StringSet,
    flag: &str,
) -> Option<FlagFilter> {
    let flags: Vec<&Symbol> = flags.iter().collect();
    let mut constraints: Vec<FlagConstraint> = Vec::new();

    for (this, f) in flags.iter().enumerate() {
        let op = crate::hfst_flag_diacritics::FdOperation::get_operator(f).as_bytes()[0];
        if !(flag.is_empty() || crate::hfst_flag_diacritics::FdOperation::get_feature(f) == flag)
            || !(op == b'U' || op == b'R' || op == b'D')
        {
            continue;
        }
        let mut fail_flags = Vec::new();
        let mut succeed_flags = Vec::new();
        for (other, flag2) in flags.iter().enumerate() {
            match is_valid_flag_combination(f, flag2) {
                FLAG_FAIL => fail_flags.push(other),
                FLAG_SUCCEED => succeed_flags.push(other),
                _ => {}
            }
        }
        if !fail_flags.is_empty() || !succeed_flags.is_empty() {
            constraints.push(FlagConstraint {
                this,
                fail_flags,
                succeed_flags,
                required: op == b'R',
            });
        }
    }

    if constraints.is_empty() {
        return None;
    }
    Some(FlagFilter::new(transducer, &flags, &constraints))
}

// What reading one flag does to a tape's filter state.
struct FlagEffect {
    // The constraint bit this flag is checked against, if it is constrained.
    check: Option<usize>,
    // Constraint bits it sets (a succeed flag for them) and clears (a fail flag).
    set: Vec<u64>,
    clear: Vec<u64>,
}

// The intersection of every flag constraint, as a deterministic automaton
// whose state is one bit per constraint: set while the constrained flag may be
// read. Its states are built only as the walk in `apply` reaches them.
struct FlagFilter {
    // Per symbol number of the transducer's coder, the flag it names, if any.
    flag_of_symbol: Vec<Option<u32>>,
    effects: Vec<FlagEffect>,
    initial: Vec<u64>,
}

impl FlagFilter {
    fn new(
        transducer: &HfstBasicTransducer,
        flags: &[&Symbol],
        constraints: &[FlagConstraint],
    ) -> FlagFilter {
        let words = constraints.len().div_ceil(64);
        let mut effects: Vec<FlagEffect> = flags
            .iter()
            .map(|_| FlagEffect {
                check: None,
                set: vec![0; words],
                clear: vec![0; words],
            })
            .collect();
        let mut initial = vec![0; words];
        for (bit, constraint) in constraints.iter().enumerate() {
            let (word, mask) = (bit / 64, 1u64 << (bit % 64));
            effects[constraint.this].check = Some(bit);
            for &other in &constraint.succeed_flags {
                effects[other].set[word] |= mask;
            }
            for &other in &constraint.fail_flags {
                effects[other].clear[word] |= mask;
            }
            if !constraint.required {
                initial[word] |= mask;
            }
        }

        // The constraints name flags by position; the arcs name them by symbol
        // number. Upstream needs this step because it escapes the flags in the
        // filter it compiles and has to substitute them back.
        // [spec:hfst:def:hfst-transducer.hfst.substitute-escaped-flags-fn+1]
        // [spec:hfst:sem:hfst-transducer.hfst.substitute-escaped-flags-fn+1]
        let position: HashMap<&str, u32> = flags
            .iter()
            .enumerate()
            .map(|(i, f)| (f.as_str(), i as u32))
            .collect();
        let flag_of_symbol = transducer
            .coder()
            .number2symbol_slice()
            .iter()
            .map(|symbol| position.get(symbol.as_str()).copied())
            .collect();

        FlagFilter {
            flag_of_symbol,
            effects,
            initial,
        }
    }

    // The state after reading flag `flag` in state `bits`, or None if the
    // flag's constraint refuses it there.
    fn read(&self, bits: &[u64], flag: u32) -> Option<Vec<u64>> {
        let effect = &self.effects[flag as usize];
        if let Some(bit) = effect.check
            && bits[bit / 64] & (1u64 << (bit % 64)) == 0
        {
            return None;
        }
        Some(
            bits.iter()
                .zip(effect.set.iter().zip(&effect.clear))
                .map(|(b, (set, clear))| (b & !clear) | set)
                .collect(),
        )
    }

    // Keep the paths of `net` that the filter accepts on its input tape and on
    // its output tape. A state of the result is a reachable triple (state of
    // `net`, input filter state, output filter state); arcs and final weights
    // are copied unchanged.
    fn apply(&self, mut net: HfstBasicTransducer) -> HfstBasicTransducer {
        let finals: Vec<Option<WeightType>> = (0..net.state_vector.len())
            .map(|s| net.get_final_weight(s as HfstState).ok())
            .collect();
        let source = std::mem::take(&mut net.state_vector);
        for (s, weight) in finals.iter().enumerate() {
            if weight.is_some() {
                net.remove_final_weight(s as HfstState);
            }
        }

        let mut tapes = FilterStates::new(self);
        let start = tapes.intern(self.initial.clone());
        let mut triples: Vec<(HfstState, u32, u32)> = vec![(0, start, start)];
        let mut ids: HashMap<(HfstState, u32, u32), HfstState> = HashMap::new();
        ids.insert(triples[0], 0);
        let mut states: Vec<Vec<HfstBasicTransition>> = Vec::new();

        let mut next = 0;
        while next < triples.len() {
            let (q, input, output) = triples[next];
            let mut arcs = Vec::new();
            for tr in &source[q as usize] {
                let Some(target_input) = tapes.step(input, tr.get_input_number()) else {
                    continue;
                };
                let Some(target_output) = tapes.step(output, tr.get_output_number()) else {
                    continue;
                };
                let key = (tr.get_target_state(), target_input, target_output);
                let target = *ids.entry(key).or_insert_with(|| {
                    triples.push(key);
                    (triples.len() - 1) as HfstState
                });
                arcs.push(HfstBasicTransition::new_numbers(
                    target,
                    tr.get_input_number(),
                    tr.get_output_number(),
                    tr.get_weight(),
                    false,
                ));
            }
            states.push(arcs);
            next += 1;
        }

        net.state_vector = states;
        for (s, (q, _, _)) in triples.iter().enumerate() {
            if let Some(weight) = finals[*q as usize] {
                net.set_final_weight(s as HfstState, &weight);
            }
        }
        net
    }
}

// The filter states one `FlagFilter::apply` walk has reached, numbered, with
// the transitions between them computed once each.
struct FilterStates<'a> {
    filter: &'a FlagFilter,
    states: Vec<Vec<u64>>,
    ids: HashMap<Vec<u64>, u32>,
    steps: HashMap<(u32, u32), Option<u32>>,
}

impl<'a> FilterStates<'a> {
    fn new(filter: &'a FlagFilter) -> Self {
        FilterStates {
            filter,
            states: Vec::new(),
            ids: HashMap::new(),
            steps: HashMap::new(),
        }
    }

    fn intern(&mut self, bits: Vec<u64>) -> u32 {
        if let Some(&id) = self.ids.get(&bits) {
            return id;
        }
        let id = self.states.len() as u32;
        self.states.push(bits.clone());
        self.ids.insert(bits, id);
        id
    }

    // The filter state after reading symbol number `symbol` in state `state`:
    // unchanged for epsilon and every symbol that is not a filtered flag, None
    // when the filter refuses the flag.
    fn step(&mut self, state: u32, symbol: u32) -> Option<u32> {
        let Some(&Some(flag)) = self.filter.flag_of_symbol.get(symbol as usize) else {
            return Some(state);
        };
        if let Some(&next) = self.steps.get(&(state, flag)) {
            return next;
        }
        let next = self
            .filter
            .read(&self.states[state as usize], flag)
            .map(|bits| self.intern(bits));
        self.steps.insert((state, flag), next);
        next
    }
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

    HfstTransducer::new_from_basic(&basic_restriction)
        .expect("converting a basic transducer to an available backend type cannot fail")
}
