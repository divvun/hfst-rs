//! Port of 'libhfst/src/HfstXeroxRules.{h,cc}' — the 'hfst::xeroxRules' namespace:
//! HFST-XFST replace functions and their 'Rule' data type.
//!
//! ABSOLUTE 1:1 literal C++->Rust translation (HFST port, Wave 2). NOT idiomatic.
//! Mirrors structure/control-flow/eval-order; preserves bugs. The free functions
//! build transducers via the facade type 'crate::hfst_transducer::HfstTransducer'.

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::fmt;

use crate::backend::AlgebraBackend;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_data_types::StringPair;
use crate::hfst_data_types::Symbol;
// HfstTransducer, plus the HfstTransducer-dependent aliases
// (HfstTransducerPair, HfstTransducerPairVector, HfstTransducerVector), live in the
// facade module that is ported concurrently. Bodies import them from
// 'crate::hfst_transducer'.
use crate::hfst_transducer::HfstTransducer;
use crate::hfst_transducer::HfstTransducerPair;
use crate::hfst_transducer::HfstTransducerPairVector;
use crate::hfst_transducer::HfstTransducerVector;

/// \brief The replace direction / type used by the 'xeroxRules' namespace.
///
/// Distinct from 'crate::hfst_rules::ReplaceType': this one has only four variants
/// (no 'REPL_DOWN_KARTTUNEN').
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-type]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ReplaceType {
    REPL_UP,
    REPL_DOWN,
    REPL_RIGHT,
    REPL_LEFT,
}

// this enum is used in xre_parse.yy for the regex2pfst tool
// it is not in the xre_parse.yy file because we couldn't make it work there
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-arrow]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ReplaceArrow {
    E_REPLACE_RIGHT,
    E_OPTIONAL_REPLACE_RIGHT,
    E_REPLACE_LEFT,
    E_OPTIONAL_REPLACE_LEFT,
    E_REPLACE_RIGHT_MARKUP,
    E_RTL_LONGEST_MATCH,
    E_RTL_SHORTEST_MATCH,
    E_LTR_LONGEST_MATCH,
    E_LTR_SHORTEST_MATCH,
}

/// \brief A rule that contains mapping and context and replace type (if any).
/// If rule is A -> B || L _ R , than mapping is cross product of transducers A and B,
///     context is pair of transducers L and R, and repl_type is enum REPL_UP.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule]
pub struct Rule<B: AlgebraBackend> {
    /* cross product of mapping transducers */
    pub(crate) mapping: HfstTransducerPairVector<B>,
    /* context */
    pub(crate) context: HfstTransducerPairVector<B>,
    /* if there is a context, it needs to have a direction (up, left, down or right) */
    pub(crate) repl_type: ReplaceType,
}

// Manual 'Clone' (a derive would demand 'B: Clone'; the fields only need the
// backend's own deep copy, which 'HfstTransducer<B>: Clone' already provides).
impl<B: AlgebraBackend> Clone for Rule<B> {
    fn clone(&self) -> Self {
        Rule {
            mapping: self.mapping.clone(),
            context: self.context.clone(),
            repl_type: self.repl_type,
        }
    }
}

// C++ 'friend std::ostream& operator<<(std::ostream&, const Rule&)' -> 'Display'.
// Delegates to the free-function port 'write_to' (defined below) which
// holds the actual 1:1 body; this bridges it to the std formatting machinery.
impl<B: AlgebraBackend> fmt::Display for Rule<B> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf: Vec<u8> = Vec::new();
        write_to(&mut buf, self);
        write!(out, "{}", String::from_utf8_lossy(&buf))
    }
}

// ===== flattened bodies (free fns share scope) =====
use crate::HFST_THROW_MESSAGE;
use crate::hfst_symbol_defs::HfstSymbolSubstitutions;
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_symbol_defs::internal_epsilon;
use crate::hfst_tokenizer::HfstTokenizer;
use std::io::Write;

impl<B: AlgebraBackend> Rule<B> {
    pub fn new_mapping(
        mapping_pair_vector: &HfstTransducerPairVector<B>,
    ) -> crate::error::Result<Self> {
        let mut tok = HfstTokenizer::new();
        tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

        // (The C++ same-type check over the mapping pairs is gone: every
        // member is 'HfstTransducer<B>' now, so a mismatch is unrepresentable.)

        let context_pair: HfstTransducerPair<B> = (
            HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
            HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        );
        let epsilon_context: HfstTransducerPairVector<B> = vec![context_pair];

        let mapping = mapping_pair_vector.clone();

        // HfstTransducerPairVector tmpV = mapping_pair_vector;
        // tmpV[0].0 = encode_flag_diacritics(tmpV[0].0);

        //mapping = tmpV;
        let context = epsilon_context;
        let repl_type = ReplaceType::REPL_UP;

        Ok(Rule {
            mapping,
            context,
            repl_type,
        })
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.rule-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.rule-fn]
    pub fn new_mapping_context_repl_type(
        mapping_pair_vector: &HfstTransducerPairVector<B>,
        a_context: &HfstTransducerPairVector<B>,
        a_repl_type: ReplaceType,
    ) -> crate::error::Result<Self> {
        // (The C++ same-type checks over the mapping and context pairs are
        // gone: every member is 'HfstTransducer<B>' now, so a mismatch is
        // unrepresentable.)

        //HfstTransducerPairVector tmpV = mapping_pair_vector;
        //tmpV[0].0 = encode_flag_diacritics(tmpV[0].0);

        let mapping = mapping_pair_vector.clone();
        // mapping = tmpV                       ;
        let context = a_context.clone();
        let repl_type = a_repl_type;

        Ok(Rule {
            mapping,
            context,
            repl_type,
        })
    }

    //copy
    pub fn new_rule(a_rule: &Rule<B>) -> Self {
        let mapping = a_rule.get_mapping();
        let context = a_rule.get_context();
        let repl_type = a_rule.get_repl_type();

        Rule {
            mapping,
            context,
            repl_type,
        }
    }

    // for SWIG
    // (C++ hardwired TROPICAL_OPENFST_TYPE here; the type is 'B' now.)
    pub fn new() -> crate::error::Result<Self> {
        let mut tok = HfstTokenizer::new();
        tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
        let context_pair: HfstTransducerPair<B> = (
            HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
            HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        );
        let epsilon_context: HfstTransducerPairVector<B> = vec![context_pair];
        let context = epsilon_context;
        let repl_type = ReplaceType::REPL_UP;
        // 'mapping' is left default-constructed (an empty vector), as in C++.
        let mapping: HfstTransducerPairVector<B> = Vec::new();

        Ok(Rule {
            mapping,
            context,
            repl_type,
        })
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-mapping-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-mapping-fn]
    pub fn get_mapping(&self) -> HfstTransducerPairVector<B> {
        self.mapping.clone()
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-context-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-context-fn]
    pub fn get_context(&self) -> HfstTransducerPairVector<B> {
        self.context.clone()
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-repl-type-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-repl-type-fn]
    pub fn get_repl_type(&self) -> ReplaceType {
        self.repl_type
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.encode-flags-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.encode-flags-fn]
    pub fn encode_flags(&mut self) -> crate::error::Result<()> {
        let mut tmp_m: HfstTransducerPairVector<B> = self.mapping.clone();

        for pair in tmp_m.iter_mut() {
            pair.0 = encode_flag_diacritics(&pair.0)?;
            pair.1 = encode_flag_diacritics(&pair.1)?;
        }

        let mut tmp_c: HfstTransducerPairVector<B> = self.context.clone();

        for pair in tmp_c.iter_mut() {
            pair.0 = encode_flag_diacritics(&pair.0)?;
            pair.1 = encode_flag_diacritics(&pair.1)?;
        }

        self.mapping = tmp_m;
        self.context = tmp_c;
        Ok(())
    }
}

// Ports 'std::ostream & operator<<(std::ostream &out, const Rule & r)'.
pub fn write_to<W: Write, B: AlgebraBackend>(out: &mut W, r: &Rule<B>) {
    writeln!(out, "hfst::xeroxRules::Rule:").expect("writing to the output sink does not fail");
    write!(out, "repl_type: ").expect("writing to the output sink does not fail");
    match r.repl_type {
        ReplaceType::REPL_UP => {
            write!(out, "REPL_UP").expect("writing to the output sink does not fail");
        }
        ReplaceType::REPL_DOWN => {
            write!(out, "REPL_DOWN").expect("writing to the output sink does not fail");
        }
        ReplaceType::REPL_RIGHT => {
            write!(out, "REPL_RIGHT").expect("writing to the output sink does not fail");
        }
        ReplaceType::REPL_LEFT => {
            write!(out, "REPL_LEFT").expect("writing to the output sink does not fail");
        }
    }
    writeln!(out).expect("writing to the output sink does not fail");

    writeln!(out, "mapping:").expect("writing to the output sink does not fail");
    for (i, it) in r.mapping.iter().enumerate() {
        writeln!(out, "#{} (right side):", i + 1)
            .expect("writing to the output sink does not fail");
        crate::hfst_transducer::write_to(out, &it.0);
        writeln!(out, "#{} (left side):", i + 1).expect("writing to the output sink does not fail");
        crate::hfst_transducer::write_to(out, &it.1);
    }

    writeln!(out, "context:").expect("writing to the output sink does not fail");
    for (i, it) in r.context.iter().enumerate() {
        writeln!(out, "#{} (right side):", i + 1)
            .expect("writing to the output sink does not fail");
        crate::hfst_transducer::write_to(out, &it.0);
        writeln!(out, "#{} (left side):", i + 1).expect("writing to the output sink does not fail");
        crate::hfst_transducer::write_to(out, &it.1);
    }
}

//////////////////////////////////////
// In the transducer tr, change all flag diacritics to "non-special" multichar symbols
// It means that @ sign will be changed to $ sign
// ie. @P.FOO.BAR@ will be changed into $P.FOO.BAR$
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.encode-flag-diacritics-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.encode-flag-diacritics-fn]
pub fn encode_flag_diacritics<B: AlgebraBackend>(
    tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut real_flags_to_fake_flags: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
    let mut remove_from_alphabet_set: StringSet = StringSet::new();
    let transducer_alphabet: StringSet = tr.get_alphabet()?;
    for s in transducer_alphabet.iter() {
        let alph: String = s.to_string();
        // mirrors std::string::substr(0,3): the first (up to) three bytes
        let alph_first3: String = {
            let n = std::cmp::min(3, alph.len());
            String::from_utf8_lossy(&alph.as_bytes()[..n]).into_owned()
        };

        //@operator.feature.value@ and @operator.feature@

        if alph_first3 == "@P."
            || alph_first3 == "@R."
            || alph_first3 == "@U."
            || alph_first3 == "@D."
            || alph_first3 == "@C."
            || alph_first3 == "@N."
            || alph_first3 == "@p."
            || alph_first3 == "@r."
            || alph_first3 == "@u."
            || alph_first3 == "@d."
            || alph_first3 == "@c."
            || alph_first3 == "@n."
        {
            let alph = alph.replace('@', "$");
            real_flags_to_fake_flags.insert(s.clone(), Symbol::from(alph));
            remove_from_alphabet_set.insert(s.clone());
        }
    }

    let mut retval: HfstTransducer<B> = tr.clone();
    retval.substitute_substitutions(&real_flags_to_fake_flags)?;

    retval.remove_from_alphabet_string_set(&remove_from_alphabet_set)?;
    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.decode-flag-diacritics-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.decode-flag-diacritics-fn]
pub fn decode_flag_diacritics<B: AlgebraBackend>(
    tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut fake_flags_to_real_flags: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();

    let transducer_alphabet: StringSet = tr.get_alphabet()?;
    let mut remove_from_alphabet_set: StringSet = StringSet::new();
    for s in transducer_alphabet.iter() {
        let alph: String = s.to_string();
        // mirrors std::string::substr(0,3): the first (up to) three bytes
        let alph_first3: String = {
            let n = std::cmp::min(3, alph.len());
            String::from_utf8_lossy(&alph.as_bytes()[..n]).into_owned()
        };

        //@operator.feature.value@ and @operator.feature@

        if alph_first3 == "$P."
            || alph_first3 == "$R."
            || alph_first3 == "$U."
            || alph_first3 == "$D."
            || alph_first3 == "$C."
            || alph_first3 == "$N."
            || alph_first3 == "$p."
            || alph_first3 == "$r."
            || alph_first3 == "$u."
            || alph_first3 == "$d."
            || alph_first3 == "$c."
            || alph_first3 == "$n."
        {
            let alph = alph.replace('$', "@");
            fake_flags_to_real_flags.insert(s.clone(), Symbol::from(alph));
            remove_from_alphabet_set.insert(s.clone());
        }
    }

    let mut retval: HfstTransducer<B> = tr.clone();
    retval.substitute_substitutions(&fake_flags_to_real_flags)?;
    retval.remove_from_alphabet_string_set(&remove_from_alphabet_set)?;
    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.disjunct-vector-members-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.disjunct-vector-members-fn]
pub fn disjunct_vector_members<B: AlgebraBackend>(
    tr_vector: &HfstTransducerVector<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval: HfstTransducer<B> = tr_vector[0].clone();
    for tr in &tr_vector[1..] {
        retval.disjunct(tr, true)?.optimize()?;
    }
    Ok(retval)
}

//////////////////////////////////////
// Port of 'libhfst/src/HfstXeroxRules.cc' lines 320..1500 (functions defined in
// that span). Sibling areas of 'crate::hfst_xerox_rules' own everything outside
// this span (e.g. 'decode_flag_diacritics', 'Rule', 'ReplaceType', the 'replace*'
// interface functions); they reach this module via 'use super::*'.

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.remove-markers-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.remove-markers-fn]
pub fn remove_markers<B: AlgebraBackend>(
    tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval = tr.clone();

    let left_marker: Symbol = Symbol::new_static("@LM@");
    let right_marker: Symbol = Symbol::new_static("@RM@");

    retval
        .substitute_symbol_pair(
            &(left_marker.clone(), left_marker.clone()),
            &(
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            ),
        )?
        .optimize()?;
    retval
        .substitute_symbol_pair(
            &(right_marker.clone(), right_marker.clone()),
            &(
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            ),
        )?
        .optimize()?;

    retval.remove_from_alphabet_symbol(&left_marker)?;
    retval.remove_from_alphabet_symbol(&right_marker)?;

    retval.optimize()?;

    retval = decode_flag_diacritics(&retval)?;

    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.zero-weight-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.zero-weight-fn]
pub fn zero_weight(f: f32) -> f32 {
    let _ = f;
    0.0
}

/*
 * Generalized Lenient Composition, described in Anssi Yli-Jyrä 2008b
 */
// tmp = t.1 .o. Constr .o. t.1
// (t.1 - tmp.2) .o. t
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.constraint-composition-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.constraint-composition-fn]
pub fn constraint_composition<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
    constraint: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval = t.clone();
    retval.transform_weights(zero_weight)?;

    retval.input_project()?.optimize()?;

    let mut tmp = retval.clone();
    tmp.compose(constraint, true)?.optimize()?;

    tmp.compose(&retval, true)?.optimize()?;
    tmp.output_project()?.optimize()?;
    retval.subtract(&tmp, true)?.optimize()?;

    //transform weights to zero
    retval.transform_weights(zero_weight)?;
    retval.compose(t, true)?.optimize()?;

    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.insert-freely-all-the-brackets-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.insert-freely-all-the-brackets-fn]
pub fn insert_freely_all_the_brackets<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    optional: bool,
) -> crate::error::Result<()> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();
    let left_marker2: String = "@LM2@".to_string();
    let right_marker2: String = "@RM2@".to_string();

    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);
    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    t.insert_freely_transducer(&left_bracket, false)?
        .optimize()?;
    t.insert_freely_transducer(&right_bracket, false)?
        .optimize()?;

    if !optional {
        let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
        let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;

        t.insert_freely_transducer(&left_bracket2, false)?
            .optimize()?;
        t.insert_freely_transducer(&right_bracket2, false)?
            .optimize()?;
    }
    Ok(())
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.expand-contexts-with-mapping-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.expand-contexts-with-mapping-fn]
pub fn expand_contexts_with_mapping<B: AlgebraBackend>(
    context_vector: &HfstTransducerPairVector<B>,
    mapping_with_brackets_and_tmp_boundary: &HfstTransducer<B>,
    identity_expanded: &HfstTransducer<B>,
    repl_type: ReplaceType,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut union_context_replace = HfstTransducer::new();

    let mut tok = HfstTokenizer::new();
    // tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    // HfstTransducer epsilon("@_EPSILON_SYMBOL_@", tok, type);

    for context_pair in context_vector.iter() {
        // Expand context with mapping
        // Cr' = (Rc .*) << Markers (<,>,|) .o. [I:I | <a:b>]*
        // Cr = Cr|Cr'
        // (same for left context)

        // Lc = (*. Lc) << {<,>}

        let identity_pair = HfstTransducer::identity_pair();
        let mut identity_star = identity_pair.clone();
        identity_star.repeat_star()?;

        let mut first_context = identity_star.clone();
        first_context.concatenate(&context_pair.0, true)?;
        first_context.transform_weights(zero_weight)?;
        first_context.optimize()?;

        insert_freely_all_the_brackets(&mut first_context, optional)?;

        // Rc =  (Rc .*) << {<,>}
        let mut second_context = context_pair.1.clone();
        second_context.concatenate(&identity_star, true)?;
        second_context.transform_weights(zero_weight)?;
        second_context.optimize()?;
        insert_freely_all_the_brackets(&mut second_context, optional)?;

        /* RULE:    LC:        RC:
         * up        up        up
         * left        up        down
         * right    down    up
         * down        down    down
         */

        let mut left_context_expanded = HfstTransducer::new();
        let mut right_context_expanded = HfstTransducer::new();

        // both contexts are in upper language
        if repl_type == ReplaceType::REPL_UP {
            // compose them with [I:I | <a:b>]*
            left_context_expanded = first_context.clone();
            right_context_expanded = second_context.clone();

            left_context_expanded.compose(identity_expanded, true)?;
            right_context_expanded.compose(identity_expanded, true)?;
        }
        // left context is in lower language, right in upper ( // )
        if repl_type == ReplaceType::REPL_RIGHT {
            // compose them with [I:I | <a:b>]*

            // left compose opposite way
            left_context_expanded = identity_expanded.clone();
            right_context_expanded = second_context.clone();

            left_context_expanded.compose(&first_context, true)?;
            right_context_expanded.compose(identity_expanded, true)?;
        }
        // right context is in lower language, left in upper ( \\ )
        if repl_type == ReplaceType::REPL_LEFT {
            // compose them with [I:I | <a:b>]*
            left_context_expanded = first_context.clone();
            right_context_expanded = identity_expanded.clone();

            left_context_expanded.compose(identity_expanded, true)?;
            right_context_expanded.compose(&second_context, true)?;
        }
        if repl_type == ReplaceType::REPL_DOWN {
            // compose them with [I:I | <a:b>]*
            left_context_expanded = identity_expanded.clone();
            right_context_expanded = identity_expanded.clone();

            left_context_expanded.compose(&first_context, true)?;
            right_context_expanded.compose(&second_context, true)?;
        }

        left_context_expanded.transform_weights(zero_weight)?;
        right_context_expanded.transform_weights(zero_weight)?;
        left_context_expanded.optimize()?;
        right_context_expanded.optimize()?;

        first_context.disjunct(&left_context_expanded, true)?;
        first_context.optimize()?;

        second_context.disjunct(&right_context_expanded, true)?;
        second_context.optimize()?;

        // add boundary symbol before/after contexts
        let boundary_marker: String = ".#.".to_string();
        tok.add_multichar_symbol(&boundary_marker);
        let boundary = HfstTransducer::new_tokenized(&boundary_marker, &tok)?;

        identity_star.insert_to_alphabet_symbol(&boundary_marker)?;

        // to first_context
        let first_context_alphabet = first_context.get_alphabet()?;
        let mut has_boundary = false;
        for s in first_context_alphabet.iter() {
            if boundary_marker == *s {
                has_boundary = true;
            }
        }

        if !has_boundary {
            first_context.insert_to_alphabet_symbol(&boundary_marker)?;
            let mut tmp = boundary.clone();
            tmp.concatenate(&identity_star, true)?.optimize()?;
            tmp.concatenate(&first_context, true)?;
            first_context = tmp;
        }

        // to second_context
        let second_context_alphabet = second_context.get_alphabet()?;
        has_boundary = false;
        for s in second_context_alphabet.iter() {
            if boundary_marker == *s {
                has_boundary = true;
            }
        }

        if !has_boundary {
            second_context.insert_to_alphabet_symbol(&boundary_marker)?;
            second_context
                .concatenate(&identity_star, true)?
                .concatenate(&boundary, true)?
                .optimize()?;
        }

        // put mapping between (expanded) contexts
        let mut one_context_replace = first_context.clone();
        one_context_replace
            .concatenate(mapping_with_brackets_and_tmp_boundary, true)?
            .concatenate(&second_context, true)?;

        one_context_replace.transform_weights(zero_weight)?;
        union_context_replace.disjunct(&one_context_replace, true)?;
        union_context_replace.optimize()?;
    }
    Ok(union_context_replace)
}

/*
 * unconditional replace, in multiple contexts
 * first: (.* T<a:b>T .*) - [( .* L1 T<a:b>T R1 .*) u (.* L2 T<a:b>T R2 .*)...],
 *                         where .* = [I:I (+ {tmp_marker (T), <,>} in alphabet) | <a:b>]*
 * then: remove tmp_marker from transducer and alphabet, and do negation:
 *         .* - result from upper operations
 */

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:def:hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:sem:hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
pub fn bracketed_replace<B: AlgebraBackend>(
    rule: &Rule<B>,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();
    let tmp_marker: Symbol = Symbol::new_static("@TMPM@");
    let left_marker2: String = "@LM2@".to_string();
    let right_marker2: String = "@RM2@".to_string();
    let new_epsilon: String = "$Epsilon$".to_string();

    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);
    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);
    tok.add_multichar_symbol(&tmp_marker);
    tok.add_multichar_symbol(&new_epsilon);
    tok.add_multichar_symbol(".#.");

    //first, encode all flag diacritics
    let mut ruletmp = rule.clone();
    ruletmp.encode_flags()?;

    let mapping_pair_vector: HfstTransducerPairVector<B> = ruletmp.get_mapping();
    let context_vector: HfstTransducerPairVector<B> = ruletmp.get_context();
    let repl_type: ReplaceType = ruletmp.get_repl_type();

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let epsilon: HfstTransducer<B> = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    let mut mapping = HfstTransducer::new();
    for (i, pair) in mapping_pair_vector.iter().enumerate() {
        let mut one_mapping_pair = pair.0.clone();

        //markup rules are already cross product in the pair's first member
        //(second is empty), so the cross product should not be done for markup rules
        if pair.0.get_property("isMarkup") != "yes" {
            one_mapping_pair.cross_product(&pair.1, true)?;
        }

        // for removing .#. from the center
        let mut identity_without_boundary = identity.clone();
        identity_without_boundary.insert_to_alphabet_symbol(".#.")?;
        let mut remove_hash = identity_without_boundary.clone();
        let boundary = HfstTransducer::new_tokenized(".#.", &tok)?;
        remove_hash
            .concatenate(&boundary, true)?
            .concatenate(&identity_without_boundary, true)?
            .optimize()?;

        if i == 0 {
            // remove .#. from the center
            // center - (?* .#. ?*)
            one_mapping_pair.subtract(&remove_hash, false)?.optimize()?;
            one_mapping_pair.remove_from_alphabet_symbol(".#.")?;
            mapping = one_mapping_pair;
        } else {
            one_mapping_pair.subtract(&remove_hash, false)?.optimize()?;
            one_mapping_pair.remove_from_alphabet_symbol(".#.")?;
            mapping.disjunct(&one_mapping_pair, true)?.optimize()?;
        }
    }

    // In case of ? -> x replacement
    // If left side is empty, return identity transducer
    // If right side is empty, return identity transducer
    //    with alphabet from the left side
    let empty = HfstTransducer::new();
    if mapping.compare(&empty, true)? {
        mapping = identity.clone();
        if mapping_pair_vector[0].1.compare(&empty, true)? {
            let transducer_alphabet = mapping_pair_vector[0].0.get_alphabet()?;
            for s in transducer_alphabet.iter() {
                mapping.insert_to_alphabet_symbol(s)?;
            }
        }
    }

    mapping.insert_to_alphabet_symbol(&left_marker)?;
    mapping.insert_to_alphabet_symbol(&right_marker)?;
    mapping.insert_to_alphabet_symbol(&tmp_marker)?;

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;
    let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
    let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;
    let tmp_bracket = HfstTransducer::new_tokenized(&tmp_marker, &tok)?;

    // Surround mapping with brackets
    let mut tmp_mapping = left_bracket.clone();
    tmp_mapping
        .concatenate(&mapping, true)?
        .concatenate(&right_bracket, true)?
        .optimize()?;

    let mut mapping_with_brackets = tmp_mapping.clone();

    // Identity pair
    // for non-optional replacements
    if !optional {
        // non - optional
        // mapping = <a:b> u <2a:a>2

        let mut mapping_with_brackets2 = left_bracket2.clone();
        let mut left_mapping_union = mapping_pair_vector[0].0.clone();
        for pair in &mapping_pair_vector[1..] {
            left_mapping_union.disjunct(&pair.0, true)?.optimize()?;
        }
        // needed in case of ? -> x replacement
        left_mapping_union.insert_to_alphabet_symbol(&left_marker2)?;
        left_mapping_union.insert_to_alphabet_symbol(&right_marker2)?;
        left_mapping_union.insert_to_alphabet_symbol(&left_marker)?;
        left_mapping_union.insert_to_alphabet_symbol(&right_marker)?;
        left_mapping_union.insert_to_alphabet_symbol(&tmp_marker)?;

        mapping_with_brackets2
            .concatenate(&left_mapping_union, true)?
            .concatenate(&right_bracket2, true)?
            .optimize()?;

        // mapping_with_brackets...... expanded
        mapping_with_brackets.insert_to_alphabet_symbol(&left_marker2)?;
        mapping_with_brackets.insert_to_alphabet_symbol(&right_marker2)?;
        mapping_with_brackets
            .disjunct(&mapping_with_brackets2, true)?
            .optimize()?;
    }

    // Identity with bracketed mapping and marker symbols and TmpMarker in alphabet
    // [I:I | <a:b>]* (+ tmp_marker in alphabet)
    let mut identity_expanded = identity_pair.clone();

    identity_expanded.insert_to_alphabet_symbol(&left_marker)?;
    identity_expanded.insert_to_alphabet_symbol(&right_marker)?;
    identity_expanded.insert_to_alphabet_symbol(&tmp_marker)?;

    if !optional {
        identity_expanded.insert_to_alphabet_symbol(&left_marker2)?;
        identity_expanded.insert_to_alphabet_symbol(&right_marker2)?;
    }

    identity_expanded
        .disjunct(&mapping_with_brackets, true)?
        .optimize()?;
    identity_expanded.repeat_star()?.optimize()?;

    // when there aren't any contexts, result is identity_expanded
    if context_vector.len() == 1 {
        let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
        if context_vector[0].0.compare(&epsilon, true)?
            && context_vector[0].1.compare(&epsilon, true)?
        {
            identity_expanded.remove_from_alphabet_symbol(&tmp_marker)?;
            return Ok(identity_expanded);
        }
    }

    // Surround mapping with tmp boudaries
    let mut mapping_with_brackets_and_tmp_boundary = tmp_bracket.clone();
    mapping_with_brackets_and_tmp_boundary
        .concatenate(&mapping_with_brackets, true)?
        .concatenate(&tmp_bracket, true)?
        .optimize()?;

    // .* |<a:b>| :*
    let mut bracketed_replace = identity_expanded.clone();
    bracketed_replace
        .concatenate(&mapping_with_brackets_and_tmp_boundary, true)?
        .concatenate(&identity_expanded, true)?
        .optimize()?;

    // Expand all contexts with mapping taking in consideration replace type
    // result is their union
    let union_context_replace = expand_contexts_with_mapping(
        &context_vector,
        &mapping_with_brackets_and_tmp_boundary,
        &identity_expanded,
        repl_type,
        optional,
    )?;

    // subtract all mappings in contexts from replace without contexts
    let mut replace_without_contexts = bracketed_replace.clone();
    replace_without_contexts
        .subtract(&union_context_replace, true)?
        .optimize()?;

    // remove tmpMaprker
    replace_without_contexts
        .substitute_symbol_pair(
            &(tmp_marker.clone(), tmp_marker.clone()),
            &(
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            ),
        )?
        .optimize()?;
    replace_without_contexts.remove_from_alphabet_symbol(&tmp_marker)?;
    replace_without_contexts.optimize()?;

    identity_expanded.remove_from_alphabet_symbol(&tmp_marker)?;

    // final negation
    let mut unconditional_tr = identity_expanded.clone();
    unconditional_tr
        .subtract(&replace_without_contexts, true)?
        .optimize()?;

    Ok(unconditional_tr)
}

// Return the string "@N@" where N is the string representation of i.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.get-marker-string-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.get-marker-string-fn]
fn get_marker_string(i: u32) -> String {
    let oss: String = i.to_string();
    String::from("@") + &oss + &String::from("@")
}

// Return the number representation of N in string "@N@".
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.get-marker-number-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.get-marker-number-fn]
fn get_marker_number(str: &str) -> u32 {
    let number_str = str[1..str.len() - 1].to_string();
    let _ = number_str;
    // iss should be iss(number_str); i guess, but cannot be fixed, because some
    // HfstXeroxRules tests will fail...
    // unsigned int retval; iss >> retval;
    //return retval;
    100000
}

// Bracketed replace for parallel rules.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.parallel-bracketed-replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.parallel-bracketed-replace-fn]
pub fn parallel_bracketed_replace<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    // For each parallel rule, we need to concatenate a special marker symbol
    // to its output side. This is needed so that overlapping mappings with
    // different weights and contexts are kept separate. If we have N rules,
    // we need marker symbols "@1@", "@2@", ... , "@N@" ("@0@" is reserved
    // for epsilon symbol). At the end, we must substitute any marker symbols
    // with epsilons.

    let mut marker_symbols: StringSet = StringSet::new(); // "@1@", "@2@", ... , "@N@"
    let mut marker_substitutions: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
    for i in 0..rule_vector.len() {
        let marker_string = get_marker_string((i + 1) as u32);
        marker_symbols.insert(Symbol::from(marker_string.clone()));
        marker_substitutions.insert(
            Symbol::from(marker_string.clone()),
            Symbol::new_static(internal_epsilon),
        );
    }

    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();

    let left_marker2: String = "@LM2@".to_string();
    let right_marker2: String = "@RM2@".to_string();

    let tmp_marker: Symbol = Symbol::new_static("@TMPM@");
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);
    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);
    tok.add_multichar_symbol(&tmp_marker);
    tok.add_multichar_symbol(".#.");

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;
    let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
    let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;
    let tmp_bracket = HfstTransducer::new_tokenized(&tmp_marker, &tok)?;

    // Identity pair (unknowns/identities must not be expanded to marker
    // symbols)
    let mut identity_pair = HfstTransducer::identity_pair();
    identity_pair.insert_to_alphabet_set(&marker_symbols)?;

    let mut identity = identity_pair.clone();
    // unknowns/identities must not be expanded to marker symbols
    identity.insert_to_alphabet_set(&marker_symbols)?;
    identity.repeat_star()?.optimize()?;

    let mut identity_expanded = identity_pair.clone();
    identity_expanded.insert_to_alphabet_symbol(&left_marker)?;
    identity_expanded.insert_to_alphabet_symbol(&right_marker)?;
    identity_expanded.insert_to_alphabet_symbol(&left_marker2)?;
    identity_expanded.insert_to_alphabet_symbol(&right_marker2)?;
    identity_expanded.insert_to_alphabet_symbol(&tmp_marker)?;
    identity_expanded.insert_to_alphabet_set(&marker_symbols)?;
    // will be expanded with mappings

    // for removing .#. from the center
    let mut identity_without_boundary = identity.clone();
    identity_without_boundary.insert_to_alphabet_symbol(".#.")?;
    // (must not be expanded to marker symbols)
    identity_without_boundary.insert_to_alphabet_set(&marker_symbols)?;
    let mut remove_hash = identity_without_boundary.clone();
    let boundary = HfstTransducer::new_tokenized(".#.", &tok)?;
    remove_hash
        .concatenate(&boundary, true)?
        .concatenate(&identity_without_boundary, true)?
        .optimize()?;

    let mut mapping_with_brackets_vector: HfstTransducerVector<B> = Vec::new();
    let mut no_contexts = true;

    // go through vector and do everything for each rule
    for (i, rule) in rule_vector.iter().enumerate() {
        let mut ruletmp = rule.clone();
        ruletmp.encode_flags()?;

        let mapping_pair_vector = ruletmp.get_mapping();
        let mut mapping = HfstTransducer::new();
        for (j, mapping_pair) in mapping_pair_vector.iter().enumerate() {
            // i+1 because @0@ is epsilon..
            let marker_string = get_marker_string((i + 1) as u32);
            let marker = HfstTransducer::new_symbol(&marker_string)?;
            let mut one_mapping_pair = mapping_pair.0.clone();
            // unknowns/identities must not be expanded to marker symbols
            one_mapping_pair.insert_to_alphabet_set(&marker_symbols)?;
            let mut mapping_output = mapping_pair.1.clone();
            mapping_output.insert_to_alphabet_set(&marker_symbols)?;
            one_mapping_pair.cross_product(mapping_output.concatenate(&marker, true)?, true)?;

            if j == 0 {
                // remove .#. from the center
                // center - (?* .#. ?*)
                one_mapping_pair.subtract(&remove_hash, false)?.optimize()?;
                one_mapping_pair.remove_from_alphabet_symbol(".#.")?;
                mapping = one_mapping_pair;
            } else {
                one_mapping_pair.subtract(&remove_hash, false)?.optimize()?;
                one_mapping_pair.remove_from_alphabet_symbol(".#.")?;
                mapping.disjunct(&one_mapping_pair, true)?.optimize()?;
            }
        }

        let context_vector = ruletmp.get_context();

        // when there aren't any contexts, result is identity_expanded
        if context_vector.len() == 1 {
            let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
            if !(context_vector[0].0.compare(&epsilon, true)?
                && context_vector[0].1.compare(&epsilon, true)?)
            {
                no_contexts = false;
            }
        }

        //////////////////////////////////////////////////////////////////
        // In case of ? -> x replacement
        // If left side is empty, return identity transducer
        // If right side is empty, return identity transducer
        //    with alphabet from the left side
        let empty = HfstTransducer::new();

        if mapping.compare(&empty, true)? {
            mapping = identity.clone();
            if mapping_pair_vector[0].1.compare(&empty, true)? {
                let transducer_alphabet = mapping_pair_vector[0].0.get_alphabet()?;
                for s in transducer_alphabet.iter() {
                    mapping.insert_to_alphabet_symbol(s)?;
                }
            }
        }
        //////////////////////////////////////////////////////////////////

        mapping.insert_to_alphabet_symbol(&left_marker)?;
        mapping.insert_to_alphabet_symbol(&right_marker)?;
        mapping.insert_to_alphabet_symbol(&tmp_marker)?;

        // Surround mapping with brackets
        let mut mapping_with_brackets = left_bracket.clone();
        mapping_with_brackets
            .concatenate(&mapping, true)?
            .concatenate(&right_bracket, true)?
            .optimize()?;

        // non - optional
        // mapping = <a:b> u <2a:a>2
        if !optional {
            // needed in case of ? -> x replacement
            mapping.insert_to_alphabet_symbol(&left_marker2)?;
            mapping.insert_to_alphabet_symbol(&right_marker2)?;
            mapping_with_brackets.insert_to_alphabet_symbol(&left_marker2)?;
            mapping_with_brackets.insert_to_alphabet_symbol(&right_marker2)?;

            let mut mapping_project = mapping.clone();
            mapping_project.input_project()?.optimize()?;

            let mut mapping_with_brackets_non_optional = left_bracket2.clone();

            mapping_with_brackets_non_optional
                .concatenate(&mapping_project, true)?
                .concatenate(&right_bracket2, true)?
                .optimize()?;
            // mapping_with_brackets...... expanded
            mapping_with_brackets
                .disjunct(&mapping_with_brackets_non_optional, true)?
                .optimize()?;
        }

        identity_expanded
            .disjunct(&mapping_with_brackets, true)?
            .optimize()?;
        mapping_with_brackets_vector.push(mapping_with_brackets);
    }

    identity_expanded.repeat_star()?.optimize()?;

    // if none of the rules have contexts, return identity_expanded
    if no_contexts {
        identity_expanded.remove_from_alphabet_symbol(&tmp_marker)?;
        // substitute markers with epsilons
        identity_expanded.substitute_symbols(&marker_substitutions)?;
        identity_expanded.remove_from_alphabet_set(&marker_symbols)?;
        return Ok(identity_expanded);
    }

    // if they have contexts, process them
    if rule_vector.len() != mapping_with_brackets_vector.len() {
        crate::bail!(TransducerTypeMismatch, "Vector sizes don't match");
    }

    let context_replace_map: std::collections::BTreeMap<
        String,
        crate::hfst_basic_transducer::HfstBasicTransducer,
    > = std::collections::BTreeMap::new();
    let _ = &context_replace_map;

    let mut union_context_replace = HfstTransducer::new();
    let mut bracketed_replace = HfstTransducer::new();
    for i in 0..rule_vector.len() {
        let mut ruletmp = rule_vector[i].clone();
        ruletmp.encode_flags()?;

        // Surround mapping with brackets with tmp boudaries
        let mut mapping_with_brackets_and_tmp_boundary = tmp_bracket.clone();
        mapping_with_brackets_and_tmp_boundary
            .concatenate(&mapping_with_brackets_vector[i], true)?
            .concatenate(&tmp_bracket, true)?
            .optimize()?;
        // .* |<a:b>| :*
        let mut bracketed_replace_tmp = identity_expanded.clone();
        bracketed_replace_tmp
            .concatenate(&mapping_with_brackets_and_tmp_boundary, true)?
            .concatenate(&identity_expanded, true)?
            .optimize()?;

        bracketed_replace_tmp.transform_weights(zero_weight)?;
        bracketed_replace
            .disjunct(&bracketed_replace_tmp, true)?
            .optimize()?;

        //Create context part
        // For each context that uses the output side (REPL_DOWN,
        // REPL_LEFT, REPL_RIGHT) we must freely allow all markers that can
        // be generated by other rules.
        let mut cont = ruletmp.get_context();

        if ruletmp.get_repl_type() != ReplaceType::REPL_UP {
            for cont_it in cont.iter_mut() {
                for sit in marker_symbols.iter() {
                    if get_marker_number(sit) != i as u32 {
                        let marker_pair = (sit.clone(), sit.clone());
                        // 'false' makes sure harmonization is not done
                        cont_it.0.insert_freely_pair(&marker_pair, false)?;
                        cont_it.1.insert_freely_pair(&marker_pair, false)?;
                    }
                }
            }
        }

        let mut union_context_replace_tmp = expand_contexts_with_mapping(
            &cont,
            &mapping_with_brackets_and_tmp_boundary,
            &identity_expanded,
            ruletmp.get_repl_type(),
            optional,
        )?;

        union_context_replace_tmp.transform_weights(zero_weight)?;

        union_context_replace
            .disjunct(&union_context_replace_tmp, true)?
            .optimize()?;
    }

    // subtract all mappings in contexts from replace without contexts
    let mut replace_without_contexts = bracketed_replace.clone();
    replace_without_contexts
        .subtract(&union_context_replace, true)?
        .optimize()?;

    // remove tmpMaprker
    replace_without_contexts
        .substitute_symbol_pair(
            &(tmp_marker.clone(), tmp_marker.clone()),
            &(
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            ),
        )?
        .optimize()?;
    replace_without_contexts.remove_from_alphabet_symbol(&tmp_marker)?;
    replace_without_contexts.optimize()?;

    identity_expanded.remove_from_alphabet_symbol(&tmp_marker)?;

    // final negation
    let mut unconditional_tr = identity_expanded.clone();
    unconditional_tr
        .subtract(&replace_without_contexts, true)?
        .optimize()?;

    // substitute markers with epsilons
    unconditional_tr.substitute_symbols(&marker_substitutions)?;
    unconditional_tr.remove_from_alphabet_set(&marker_symbols)?;

    Ok(unconditional_tr)
}

//---------------------------------
//    CONSTRAINTS
//---------------------------------

// (help function)
// returns: [ B:0 | 0:B | ?-B ]*
// which is used in some constraints
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.constraints-right-part-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.constraints-right-part-fn]
pub fn constraints_right_part<B: AlgebraBackend>() -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    // Identity pair (normal)
    let identity_pair = HfstTransducer::identity_pair();

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    // Create Right Part
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;

    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    let epsilon_to_left_mark: HfstTransducer<B> =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &left_marker, &tok)?;
    let left_mark_to_epsilon: HfstTransducer<B> =
        HfstTransducer::new_tokenized_pair(&left_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let _ = (&epsilon_to_left_mark, &left_mark_to_epsilon);

    let mut epsilon_to_brackets = epsilon.clone();
    epsilon_to_brackets.cross_product(&b, true)?;

    let mut brackets_to_epsilon = b.clone();
    brackets_to_epsilon.cross_product(&epsilon, true)?;

    let mut identity_pair_minus_brackets = identity_pair.clone();
    identity_pair_minus_brackets
        .subtract(&b, true)?
        .optimize()?; //.repeat_plus().optimize();

    let mut right_part = epsilon_to_brackets.clone();
    right_part
        .disjunct(&brackets_to_epsilon, true)?
        .disjunct(&identity_pair_minus_brackets, true)?
        .optimize()?
        .repeat_star()?
        .optimize()?;

    Ok(right_part)
}

// .#. ?* <:0 0:> ?* .#.
// filters out empty string
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.one-betterthan-none-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.one-betterthan-none-constraint-fn]
pub fn one_betterthan_none_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol(".#.");

    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let left_bracket_to_zero =
        HfstTransducer::new_tokenized_pair(&left_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let right_bracket_to_zero =
        HfstTransducer::new_tokenized_pair(&right_marker, "@_EPSILON_SYMBOL_@", &tok)?;

    let boundary = HfstTransducer::new_tokenized(".#.", &tok)?;
    let mut constraint = boundary.clone();
    constraint.concatenate(&identity, true)?;
    constraint
        .concatenate(&left_bracket_to_zero, true)?
        .concatenate(&right_bracket_to_zero, true)?
        .concatenate(&boundary, true)?
        .concatenate(&identity, true)?
        .optimize()?;

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let retval = constraint_composition(unconditional_tr, &constraint)?;

    Ok(retval)
}

// .#. ?* <:0 [B:0]* [I-B] [ B:0 | 0:B | ?-B ]* .#.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.left-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.left-most-constraint-fn]
pub fn left_most_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    tok.add_multichar_symbol(".#.");

    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let right_part = constraints_right_part()?;

    // epsilon
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    // B
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;
    // (B:0)*

    let mut brackets_to_epsilon_star = b.clone();
    brackets_to_epsilon_star
        .cross_product(&epsilon, true)?
        .optimize()?
        .repeat_star()?
        .optimize()?;

    // (I-B) and (I-B)+
    let mut identity_pair_minus_brackets = identity_pair.clone();
    identity_pair_minus_brackets
        .subtract(&b, true)?
        .optimize()?;

    let mut identity_pair_minus_brackets_plus = identity_pair_minus_brackets.clone();
    identity_pair_minus_brackets_plus
        .repeat_plus()?
        .optimize()?;

    let left_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&left_marker, "@_EPSILON_SYMBOL_@", &tok)?;

    let boundary = HfstTransducer::new_tokenized(".#.", &tok)?;

    let mut constraint = boundary.clone();
    constraint.concatenate(&identity, true)?;

    // ?* <:0 [B:0]* [I-B] [ B:0 | 0:B | ?-B ]*
    constraint
        .concatenate(&left_bracket_to_epsilon, true)?
        .concatenate(&brackets_to_epsilon_star, true)?
        .concatenate(&identity_pair_minus_brackets, true)?
        .concatenate(&right_part, true)?
        .optimize()?;

    constraint.concatenate(&boundary, true)?.optimize()?;

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let retval = constraint_composition(unconditional_tr, &constraint)?;

    Ok(retval)
}

// [ B:0 | 0:B | ?-B ]* [I-B]+  >:0 [ ?-B ]*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.right-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.right-most-constraint-fn]
pub fn right_most_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");

    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let right_part = constraints_right_part()?;

    // epsilon
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    // B
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;
    // (B:0)*
    let mut brackets_to_epsilon_star = b.clone();
    brackets_to_epsilon_star
        .cross_product(&epsilon, true)?
        .optimize()?
        .repeat_star()?
        .optimize()?;

    // (I-B) and (I-B)+
    let mut identity_pair_minus_brackets = identity_pair.clone();
    identity_pair_minus_brackets
        .subtract(&b, true)?
        .optimize()?;

    let mut identity_pair_minus_brackets_plus = identity_pair_minus_brackets.clone();
    identity_pair_minus_brackets_plus
        .repeat_plus()?
        .optimize()?;

    let mut identity_pair_minus_brackets_star = identity_pair_minus_brackets.clone();
    identity_pair_minus_brackets_star
        .repeat_star()?
        .optimize()?;

    let right_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&right_marker, "@_EPSILON_SYMBOL_@", &tok)?;

    let mut constraint = right_part.clone();
    // [ B:0 | 0:B | ?-B ]* [I-B]+  >:0 [ ?-B ]*

    constraint
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .concatenate(&right_bracket_to_epsilon, true)?
        .concatenate(&identity, true)?
        .optimize()?;

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let retval = constraint_composition(unconditional_tr, &constraint)?;

    Ok(retval)
}
// (unwrapped mod xerox_helpers_b {)

use std::collections::BTreeSet;

// Longest match
// it should be composed to left most transducer........
// ?* < [?-B]+ 0:> [ ? | 0:< | <:0 | 0:> | B ] [ B:0 | 0:B | ?-B ]*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.longest-match-left-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.longest-match-left-most-constraint-fn]
pub fn longest_match_left_most_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker = String::from("@LM@");
    let right_marker = String::from("@RM@");
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    // Identity
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    // epsilon
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    // B
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;
    // (B:0)*
    let mut brackets_to_epsilon_star = b.clone();
    brackets_to_epsilon_star
        .cross_product(&epsilon, true)?
        .optimize()?
        .repeat_star()?
        .optimize()?;

    // (I-B) and (I-B)+
    let mut identity_pair_minus_brackets = identity_pair.clone();
    identity_pair_minus_brackets
        .subtract(&b, true)?
        .optimize()?;

    let mut identity_pair_minus_brackets_plus = identity_pair_minus_brackets.clone();
    identity_pair_minus_brackets_plus
        .repeat_plus()?
        .optimize()?;

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let right_part = constraints_right_part()?;

    let right_bracket_to_epsilon: HfstTransducer<B> =
        HfstTransducer::new_tokenized_pair(&right_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let epsilon_to_right_bracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &right_marker, &tok)?;
    let left_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&left_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let epsilon_to_left_bracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &left_marker, &tok)?;

    //[ ? | 0:< | <:0 | 0:> | B ]
    //     HfstTransducer non_closing_bracket_insertion(identity_pair);
    let mut non_closing_bracket_insertion = epsilon_to_left_bracket.clone();
    non_closing_bracket_insertion
        //disjunct(epsilon_to_left_bracket).
        .disjunct(&left_bracket_to_epsilon, true)?
        .disjunct(&epsilon_to_right_bracket, true)?
        .disjunct(&b, true)?
        .optimize()?;
    //    printf("non_closing_bracket_insertion: \n");
    //    non_closing_bracket_insertion.write_in_att_format(stdout, 1);

    non_closing_bracket_insertion
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .optimize()?;

    let mut middle_part = identity_pair_minus_brackets.clone();
    middle_part
        .disjunct(&non_closing_bracket_insertion, true)?
        .optimize()?;

    // ?* < [?-B]+ 0:> [ ? | 0:< | <:0 | 0:> | B ] [?-B]+ [ B:0 | 0:B | ?-B ]*
    let mut constraint = identity.clone();
    constraint
        .concatenate(&left_bracket, true)?
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .concatenate(&epsilon_to_right_bracket, true)?
        //    concatenate(non_closing_bracket_insertion).
        //    concatenate(identity_pair_minus_brackets_plus).
        .concatenate(&middle_part, true)?
        .concatenate(&right_part, true)?
        .optimize()?;
    //printf("constraint Longest Match: \n");
    //constraint.write_in_att_format(stdout, 1);

    //unconditional_tr should be left most for the left most longest match
    let retval = constraint_composition(unconditional_tr, &constraint)?;

    Ok(retval)
}

// Longest match RIGHT most
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.longest-match-right-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.longest-match-right-most-constraint-fn]
pub fn longest_match_right_most_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker = String::from("@LM@");
    let right_marker = String::from("@RM@");
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    // Identity
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    // epsilon
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    // B
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;
    // (B:0)*
    let mut brackets_to_epsilon_star = b.clone();
    brackets_to_epsilon_star
        .cross_product(&epsilon, true)?
        .optimize()?
        .repeat_star()?
        .optimize()?;

    // (I-B) and (I-B)+
    let mut identity_pair_minus_brackets = identity_pair.clone();
    identity_pair_minus_brackets
        .subtract(&b, true)?
        .optimize()?;

    let mut identity_pair_minus_brackets_plus = identity_pair_minus_brackets.clone();
    identity_pair_minus_brackets_plus
        .repeat_plus()?
        .optimize()?;

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let right_part = constraints_right_part()?;

    let right_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&right_marker, "@_EPSILON_SYMBOL_@", &tok)?;

    let epsilon_to_right_bracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &right_marker, &tok)?;
    let left_bracket_to_epsilon: HfstTransducer<B> =
        HfstTransducer::new_tokenized_pair(&left_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let epsilon_to_left_bracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &left_marker, &tok)?;

    //[ ? | 0:< | >:0 | 0:> | B ]
    let mut non_closing_bracket_insertion = identity_pair.clone();
    non_closing_bracket_insertion
        .disjunct(&epsilon_to_left_bracket, true)?
        .disjunct(&right_bracket_to_epsilon, true)?
        .disjunct(&epsilon_to_right_bracket, true)?
        .disjunct(&b, true)?
        .optimize()?;

    // [ B:0 | 0:B | ?-B ]* [?-B]+ [ ? | 0:< | <:0 | 0:> | B ] 0:< [?-B]+ > ?*

    let mut constraint = right_part.clone();
    constraint
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .concatenate(&non_closing_bracket_insertion, true)?
        .optimize()?
        .concatenate(&epsilon_to_left_bracket, true)?
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .concatenate(&right_bracket, true)?
        .concatenate(&identity, true)?
        .optimize()?;
    //printf("constraint Longest Match: \n");
    //constraint.write_in_att_format(stdout, 1);

    //unconditional_tr should be left most for the left most longest match
    let retval = constraint_composition(unconditional_tr, &constraint)?;

    Ok(retval)
}

// Shortest match
// it should be composed to left most transducer........
// ?* < [?-B]+ >:0
// [?-B] or [ ? | 0:< | <:0 | >:0 | B ][?-B]+
// [ B:0 | 0:B | ?-B ]*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.shortest-match-left-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.shortest-match-left-most-constraint-fn]
pub fn shortest_match_left_most_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker = String::from("@LM@");
    let right_marker = String::from("@RM@");
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    // Identity
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let right_part = constraints_right_part()?;

    // [?-B] and [?-B]+
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;
    let mut identity_pair_minus_brackets = identity_pair.clone();
    identity_pair_minus_brackets
        .subtract(&b, true)?
        .optimize()?;
    let mut identity_pair_minus_brackets_plus = identity_pair_minus_brackets.clone();
    identity_pair_minus_brackets_plus
        .repeat_plus()?
        .optimize()?;

    let right_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&right_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let epsilon_to_right_bracket: HfstTransducer<B> =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &right_marker, &tok)?;
    let left_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&left_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let epsilon_to_left_bracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &left_marker, &tok)?;

    // [ 0:< | <:0 | >:0 | B ][?-B]+
    let mut non_closing_bracket_insertion = epsilon_to_left_bracket.clone();
    non_closing_bracket_insertion
        //disjunct(epsilon_to_left_bracket).
        .disjunct(&left_bracket_to_epsilon, true)?
        .disjunct(&right_bracket_to_epsilon, true)?
        .disjunct(&b, true)?
        .optimize()?;

    non_closing_bracket_insertion
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .optimize()?;

    let mut middle_part = identity_pair_minus_brackets.clone();
    middle_part
        .disjunct(&non_closing_bracket_insertion, true)?
        .optimize()?;

    //    printf("non_closing_bracket_insertion: \n");
    //    non_closing_bracket_insertion.write_in_att_format(stdout, 1);

    // ?* < [?-B]+ >:0
    // [?-B] or [ ? | 0:< | <:0 | >:0 | B ][?-B]+
    //[ B:0 | 0:B | ?-B ]*
    let mut constraint = identity.clone();
    constraint
        .concatenate(&left_bracket, true)?
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .concatenate(&right_bracket_to_epsilon, true)?
        .concatenate(&middle_part, true)?
        .optimize()?
        .concatenate(&right_part, true)?
        .optimize()?;

    //printf("constraint Shortest Match: \n");
    //constraint.write_in_att_format(stdout, 1);

    //unconditional_tr should be left most for the left most shortest match
    let retval = constraint_composition(unconditional_tr, &constraint)?;

    Ok(retval)
}

// Shortest match
// it should be composed to left most transducer........
//[ B:0 | 0:B | ?-B ]*
// [?-B] or [?-B]+  [ ? | 0:> | >:0 | <:0 | B ]
// <:0 [?-B]+   > ?*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.shortest-match-right-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.shortest-match-right-most-constraint-fn]
pub fn shortest_match_right_most_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker = String::from("@LM@");
    let right_marker = String::from("@RM@");
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    // Identity
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let right_part = constraints_right_part()?;

    // [?-B] and [?-B]+
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;
    let mut identity_pair_minus_brackets = identity_pair.clone();
    identity_pair_minus_brackets
        .subtract(&b, true)?
        .optimize()?;
    let mut identity_pair_minus_brackets_plus = identity_pair_minus_brackets.clone();
    identity_pair_minus_brackets_plus
        .repeat_plus()?
        .optimize()?;

    let right_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&right_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let epsilon_to_right_bracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &right_marker, &tok)?;
    let left_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&left_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let epsilon_to_left_bracket: HfstTransducer<B> =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &left_marker, &tok)?;

    // [?-B]+ [ 0:> | >:0 | <:0 | B ]
    let mut non_closing_bracket_insertion_tmp = epsilon_to_right_bracket.clone();
    non_closing_bracket_insertion_tmp
        .disjunct(&right_bracket_to_epsilon, true)?
        .disjunct(&left_bracket_to_epsilon, true)?
        .disjunct(&b, true)?
        .optimize()?;
    let mut non_closing_bracket_insertion = identity_pair_minus_brackets_plus.clone();
    non_closing_bracket_insertion
        .concatenate(&non_closing_bracket_insertion_tmp, true)?
        .optimize()?;

    let mut middle_part = identity_pair_minus_brackets.clone();
    middle_part
        .disjunct(&non_closing_bracket_insertion, true)?
        .optimize()?;

    //[ B:0 | 0:B | ?-B ]*
    // [?-B] or [?-B]+  [ ? | 0:> | >:0 | <:0 | B ]
    // <:0 [?-B]+   > ?*

    let mut constraint = right_part.clone();
    constraint
        .concatenate(&middle_part, true)?
        .concatenate(&left_bracket_to_epsilon, true)?
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .concatenate(&right_bracket, true)?
        .concatenate(&identity, true)?
        .optimize()?;

    //printf("constraint Shortest Match: \n");
    //constraint.write_in_att_format(stdout, 1);

    //unconditional_tr should be left most for the left most longest match
    let retval = constraint_composition(unconditional_tr, &constraint)?;

    Ok(retval)
}

// ?* [ BL:0 (?-B)+ BR:0 ?* ]+
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.most-brackets-plus-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.most-brackets-plus-constraint-fn]
pub fn most_brackets_plus_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker = String::from("@LM@");
    let right_marker = String::from("@RM@");
    let left_marker2 = String::from("@LM2@");
    let right_marker2 = String::from("@RM2@");

    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);
    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;
    let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
    let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut identity_plus = identity_pair.clone();
    identity_plus.repeat_plus()?.optimize()?;

    let mut identity_star = identity_pair.clone();
    identity_star.repeat_star()?.optimize()?;

    // epsilon
    let epsilon = String::from("@_EPSILON_SYMBOL_@");

    // BL:0 ( <1 : 0, <2 : 0)
    let left_bracket_to_epsilon = HfstTransducer::new_tokenized_pair(&left_marker, &epsilon, &tok)?;
    let left_bracket2_to_epsilon =
        HfstTransducer::new_tokenized_pair(&left_marker2, &epsilon, &tok)?;
    let mut all_left_brackets_to_epsilon = left_bracket_to_epsilon.clone();
    all_left_brackets_to_epsilon
        .disjunct(&left_bracket2_to_epsilon, true)?
        .optimize()?;

    //    printf("all_left_brackets_to_epsilon: \n");
    //    all_left_brackets_to_epsilon.write_in_att_format(stdout, 1);

    // BR:0 ( >1 : 0, >2 : 0)
    let right_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&right_marker, &epsilon, &tok)?;
    let right_bracket2_to_epsilon =
        HfstTransducer::new_tokenized_pair(&right_marker2, &epsilon, &tok)?;
    let mut all_right_brackets_to_epsilon = right_bracket_to_epsilon.clone();
    all_right_brackets_to_epsilon
        .disjunct(&right_bracket2_to_epsilon, true)?
        .optimize()?;

    // B (B1 | B2)
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;
    b.disjunct(&left_bracket2, true)?.optimize()?;
    b.disjunct(&right_bracket2, true)?.optimize()?;

    // (? - B)+
    let mut identity_pair_minus_brackets_plus = identity_pair.clone();
    identity_pair_minus_brackets_plus
        .subtract(&b, true)?
        .optimize()?
        .repeat_plus()?
        .optimize()?;

    // repeating_part ( BL:0 (?-B)+ BR:0 ?* )+
    let mut repeating_part = all_left_brackets_to_epsilon.clone();
    repeating_part
        .concatenate(&identity_pair_minus_brackets_plus, true)?
        .optimize()?;
    repeating_part
        .concatenate(&all_right_brackets_to_epsilon, true)?
        .optimize()?;
    repeating_part
        .concatenate(&identity_star, true)?
        .optimize()?;
    repeating_part.repeat_plus()?.optimize()?;
    //printf("middle_part: \n");
    //middle_part.write_in_att_format(stdout, 1);

    let mut constraint = identity_star.clone();
    constraint.concatenate(&repeating_part, true)?.optimize()?;
    //printf("constraint: \n");
    //constraint.write_in_att_format(stdout, 1);

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let retval = constraint_composition(unconditional_tr, &constraint)?;

    //printf("After composition: \n");
    //retval.write_in_att_format(stdout, 1);

    Ok(retval)
}

// ?* [ BL:0 (?-B)* BR:0 ?* ]+
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.most-brackets-star-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.most-brackets-star-constraint-fn]
pub fn most_brackets_star_constraint<B: AlgebraBackend>(
    unconditional_tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker = String::from("@LM@");
    let right_marker = String::from("@RM@");
    let left_marker2 = String::from("@LM2@");
    let right_marker2 = String::from("@RM2@");
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);
    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
    let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut identity_plus = identity_pair.clone();
    identity_plus.repeat_plus()?.optimize()?;

    let mut identity_star = identity_pair.clone();
    identity_star.repeat_star()?.optimize()?;

    // epsilon
    let epsilon = String::from("@_EPSILON_SYMBOL_@");

    // BL:0 ( <1 : 0, <2 : 0)
    let left_bracket_to_epsilon = HfstTransducer::new_tokenized_pair(&left_marker, &epsilon, &tok)?;
    let left_bracket2_to_epsilon =
        HfstTransducer::new_tokenized_pair(&left_marker2, &epsilon, &tok)?;
    let mut all_left_brackets_to_epsilon = left_bracket_to_epsilon.clone();
    all_left_brackets_to_epsilon
        .disjunct(&left_bracket2_to_epsilon, true)?
        .optimize()?;

    //    printf("all_left_brackets_to_epsilon: \n");
    //    all_left_brackets_to_epsilon.write_in_att_format(stdout, 1);

    // BR:0 ( >1 : 0, >2 : 0)
    let right_bracket_to_epsilon =
        HfstTransducer::new_tokenized_pair(&right_marker, &epsilon, &tok)?;
    let right_bracket2_to_epsilon =
        HfstTransducer::new_tokenized_pair(&right_marker2, &epsilon, &tok)?;
    let mut all_right_brackets_to_epsilon = right_bracket_to_epsilon.clone();
    all_right_brackets_to_epsilon
        .disjunct(&right_bracket2_to_epsilon, true)?
        .optimize()?;

    // B (B1 | B2)
    let mut b = left_bracket.clone();
    b.disjunct(&right_bracket, true)?.optimize()?;
    b.disjunct(&left_bracket2, true)?.optimize()?;
    b.disjunct(&right_bracket2, true)?.optimize()?;

    // (? - B)*
    let mut identity_pair_minus_brackets_star = identity_pair.clone();
    identity_pair_minus_brackets_star
        .subtract(&b, true)?
        .optimize()?
        .repeat_star()?
        .optimize()?;

    // repeating_part [ BL:0 (?-B)* BR:0 ?* ]+
    let mut repeating_part = all_left_brackets_to_epsilon.clone();
    repeating_part
        .concatenate(&identity_pair_minus_brackets_star, true)?
        .optimize()?;
    repeating_part
        .concatenate(&all_right_brackets_to_epsilon, true)?
        .optimize()?;
    repeating_part
        .concatenate(&identity_star, true)?
        .optimize()?;
    repeating_part.repeat_plus()?.optimize()?;
    //printf("middle_part: \n");
    //repeating_part.write_in_att_format(stdout, 1);

    let mut constraint = identity_star.clone();
    constraint.concatenate(&repeating_part, true)?.optimize()?;
    //printf("constraint: \n");
    //constraint.write_in_att_format(stdout, 1);

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t
    let retval = constraint_composition(unconditional_tr, &constraint)?;

    //printf("After composition: \n");
    //retval.write_in_att_format(stdout, 1);
    Ok(retval)
}

// ?* B2 ?*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.remove-b2-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.remove-b2-constraint-fn]
pub fn remove_b2_constraint<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker2 = String::from("@LM2@");
    let right_marker2 = String::from("@RM2@");

    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);

    let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
    let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut identity_star = identity_pair.clone();
    identity_star.repeat_star()?.optimize()?;

    // B (B2)
    let mut b = left_bracket2.clone();
    b.disjunct(&right_bracket2, true)?.optimize()?;

    let mut constraint = identity_star.clone();
    constraint.concatenate(&b, true)?.optimize()?;
    constraint.concatenate(&identity_star, true)?.optimize()?;

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let mut retval = constraint_composition(t, &constraint)?;

    retval.remove_from_alphabet(&left_marker2)?;
    retval.remove_from_alphabet(&right_marker2)?;

    //printf("Remove B2 After composition: \n");
    //retval.write_in_att_format(stdout, 1);

    Ok(retval)
}

// to avoid repetition in empty replace rule
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.no-repetition-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.no-repetition-constraint-fn]
pub fn no_repetition_constraint<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_marker = String::from("@LM@");
    let right_marker = String::from("@RM@");
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);

    let left_marker2 = String::from("@LM2@");
    let right_marker2 = String::from("@RM2@");

    //if the transdcuer is optional, LM2 and RM2 are not there
    let mut optional = true;
    let transducer_alphabet: StringSet = t.get_alphabet()?;
    for s in transducer_alphabet.iter() {
        let alph = s.clone();
        if alph == left_marker2 {
            optional = false;
            break;
        }
    }

    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
    let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;

    let mut left_brackets = left_bracket.clone();
    if !optional {
        left_brackets.disjunct(&left_bracket2, true)?.optimize()?;
    }

    let mut right_brackets = right_bracket.clone();
    if !optional {
        right_brackets.disjunct(&right_bracket2, true)?.optimize()?;
    }
    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    /*
    identity_pair.insert_to_alphabet(left_marker);
    identity_pair.insert_to_alphabet(right_marker);
    identity_pair.insert_to_alphabet(left_marker);
    identity_pair.insert_to_alphabet(right_marker2);
     */

    let mut identity_star = identity_pair.clone();
    identity_star.repeat_star()?.optimize()?;

    let mut constraint = identity_star.clone();
    constraint
        .concatenate(&left_brackets, true)?
        .concatenate(&right_brackets, true)?
        .concatenate(&left_brackets, true)?
        .concatenate(&right_brackets, true)?
        .concatenate(&identity_star, true)?
        .optimize()?;

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    //printf("...constraint: \n");
    //constraint.write_in_att_format(stdout, 1);

    let retval = constraint_composition(t, &constraint)?;

    //    retval = remove_b2_constraint(retval);

    Ok(retval)
}

// to apply boundary marker (.#.)
/*
 * [0:.#. | ? - .#.]*
 *         .o.
 *     tr., ie. a -> b || .#. _ ;
 *         .o.
 *     .#. (? - .#.)* .#.
 *         .o.
 * [.#.:0 | ? - .#.]*
 */
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.apply-boundary-mark-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.apply-boundary-mark-fn]
pub fn apply_boundary_mark<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    tok.add_multichar_symbol("@TMP_UNKNOWN@");

    let boundary_marker = String::from(".#.");
    tok.add_multichar_symbol(&boundary_marker);
    let boundary = HfstTransducer::new_tokenized(&boundary_marker, &tok)?;

    let mut identity_pair = HfstTransducer::identity_pair();
    identity_pair.insert_to_alphabet(&boundary_marker)?;
    // ? - .#.
    let mut identity_minus_boundary = identity_pair.clone();
    identity_minus_boundary
        .subtract(&boundary, true)?
        .optimize()?;

    // (? - .#.)*
    let mut identity_minus_boundary_star = identity_minus_boundary.clone();
    identity_minus_boundary_star.repeat_star()?.optimize()?;

    // .#. (? - .#.)* .#.
    let mut boundary_anything_boundary = boundary.clone();
    boundary_anything_boundary
        .concatenate(&identity_minus_boundary_star, true)?
        .concatenate(&boundary, true)?
        .optimize()?;

    // [0:.#. | ? - .#.]*
    let zero_to_boundary =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &boundary_marker, &tok)?;
    let mut retval = zero_to_boundary.clone();
    retval
        .disjunct(&identity_minus_boundary, true)?
        .optimize()?
        .repeat_star()?
        .optimize()?;

    //printf("retval .o. t: \n");
    //retval.write_in_att_format(stdout, 1);
    // [.#.:0 | ? - .#.]*
    let boundary_to_zero =
        HfstTransducer::new_tokenized_pair(&boundary_marker, "@_EPSILON_SYMBOL_@", &tok)?;
    let mut remove_boundary = boundary_to_zero.clone();
    remove_boundary
        .disjunct(&identity_minus_boundary, true)?
        .optimize()?
        .repeat_star()?
        .optimize()?;

    // apply boundary to the transducer
    // compose [0:.#. | ? - .#.]* .o. t
    let mut tr = t.clone();

    //tr.insert_to_alphabet(boundary_marker);
    // substitutute unknowns with tmp symbol
    // this is necessary because of first composition
    tr.substitute("@_UNKNOWN_SYMBOL_@", "@TMP_UNKNOWN@", true, true)?;

    //printf("----first: ----\n");
    //tr.write_in_att_format(stdout, 1);

    retval.compose(&tr, true)?.optimize()?;

    //            printf("first composition: \n");
    //            retval.write_in_att_format(stdout, 1);

    // compose with .#. (? - .#.)* .#.
    retval
        .compose(&boundary_anything_boundary, true)?
        .optimize()?;

    //            printf("2. composition: \n");
    //            retval.write_in_att_format(stdout, 1);

    // compose with [.#.:0 | ? - .#.]*
    retval.compose(&remove_boundary, true)?.optimize()?;

    //            printf("3. composition: \n");
    //            retval.write_in_att_format(stdout, 1);

    // bring back unknown symbols
    retval.substitute("@TMP_UNKNOWN@", "@_UNKNOWN_SYMBOL_@", true, true)?;
    retval.remove_from_alphabet("@TMP_UNKNOWN@")?;

    // remove boundary from alphabet
    retval.remove_from_alphabet(&boundary_marker)?;
    Ok(retval)
}

//---------------------------------
//    INTERFACE HELPING FUNCTIONS
//---------------------------------

// used by hfst-regexp parser
// creates markup crossproduct and sets property of the first transducer in the mapping to "isMarkup" = "yes"
// the other transducer in the mapping is set to epsilon transducer
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.create-mapping-for-mark-up-replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.create-mapping-for-mark-up-replace-fn]
pub fn create_mapping_for_mark_up_replace<B: AlgebraBackend>(
    mapping_pair: &HfstTransducerPair<B>,
    marks: &HfstTransducerPair<B>,
) -> crate::error::Result<HfstTransducerPair<B>> {
    let mut tok = HfstTokenizer::new();
    let epsilon = String::from("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol(&epsilon);
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_mark = marks.0.clone();
    let right_mark = marks.1.clone();

    let mut epsilon_to_left_mark = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    epsilon_to_left_mark
        .cross_product(&left_mark, true)?
        .optimize()?;

    let mut epsilon_to_right_mark = HfstTransducer::new_tokenized(&epsilon, &tok)?;
    epsilon_to_right_mark
        .cross_product(&right_mark, true)?
        .optimize()?;

    //Go through left part of every mapping pair
    // and concatenate: epsilon_to_left_mark.leftMapping.epsilon_to_right_mark
    //then put it into right part of the new transducerPairVector
    let mut mapping_cross_product = epsilon_to_left_mark.clone();
    mapping_cross_product
        .concatenate(&mapping_pair.0, true)?
        .concatenate(&epsilon_to_right_mark, true)?
        .optimize()?;

    mapping_cross_product.set_property("isMarkup", "yes");

    let epsilon_tr = HfstTransducer::new_tokenized(&epsilon, &tok)?;
    let retval: HfstTransducerPair<B> = (mapping_cross_product, epsilon_tr);

    Ok(retval)
}

// DIVERGENCE from upstream C++ (fixes hfst/hfst#571):
//
// An obligatory epenthesis rule whose left-hand side is epsilon and whose
// context is empty — e.g. `[] -> a`, `0 -> a`, `[..] -> a`, all of which parse
// to an @0@:a center — must NOT force one insertion at every position while
// dropping the identity string. Upstream's most_brackets_star_constraint
// (HfstXeroxRules.cc:2402-2437, applied at :706-717 when !optional) does exactly
// that, yielding a 2-state machine where `xy -> axaya` ONLY. The intended
// (and already-correct) semantics are the ones the optional arrow produces:
// free insertion at every position WITH identity preserved.
//
// So for an epsilon-LHS + empty-context rule we route the non-optional path to
// the optional one: we skip most_brackets_star_constraint. A context-full
// epenthesis rule (e.g. `0 -> p || m _ k`) is unaffected — its context is not
// empty, so this returns false and the obligatory constraint still applies.
//
// The check MUST run on a flag-encoded rule so that flag diacritics encoded
// into a context are not misread as an empty context (mirrors bracketed_replace,
// which encode_flags() before inspecting the context; see the flag-complement
// audit deferral in test_flag_complement.rs:402-415).
fn is_epsilon_lhs_empty_context<B: AlgebraBackend>(rule: &Rule<B>) -> crate::error::Result<bool> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let epsilon: HfstTransducer<B> = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    // Evaluate AFTER encode_flags() so encoded flags in a context are not
    // mistaken for an empty context.
    let mut ruletmp = rule.clone();
    ruletmp.encode_flags()?;

    let mapping = ruletmp.get_mapping();
    let context = ruletmp.get_context();

    // Empty (universal) context: exactly one epsilon:epsilon pair, matching the
    // empty-context short circuit in bracketed_replace (:821-825/:1021-1024).
    if context.len() != 1 {
        return Ok(false);
    }
    if !(context[0].0.compare(&epsilon, true)? && context[0].1.compare(&epsilon, true)?) {
        return Ok(false);
    }

    // Epsilon left-hand side: every mapping pair maps epsilon on its left.
    if mapping.is_empty() {
        return Ok(false);
    }
    for pair in mapping.iter() {
        if !pair.0.compare(&epsilon, true)? {
            return Ok(false);
        }
    }

    Ok(true)
}

// replace up, left, right, down
pub fn replace_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval: HfstTransducer<B> = bracketed_replace(rule, optional)?;

    //printf("---bracketed replace done---: \n");
    //retval.optimize().write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row

    retval = no_repetition_constraint(&retval)?;

    //printf("-----no_repetition_constraint-----: \n");
    //retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol, must be before most_brackets_star_constraint
    retval = apply_boundary_mark(&retval)?;

    //printf("----after apply_boundary_mark: ----\n");
    //retval.write_in_att_format(stdout, 1);
    // hfst/hfst#571: an epsilon-LHS + empty-context rule must not be forced to
    // insert at every position (see is_epsilon_lhs_empty_context above); treat
    // it as optional and skip most_brackets_star_constraint.
    if !optional && !is_epsilon_lhs_empty_context(rule)? {
        //printf(" ----------  most_brackets_star_constraint --------------\n");
        // Epenthesis rules behave differently if used most_brackets_plus_constraint
        //retval = most_brackets_plus_constraint(retval);
        retval = most_brackets_star_constraint(&retval)?;
        //printf("after non optional: \n");
        //retval.write_in_att_format(stdout, 1);
    }
    retval = remove_b2_constraint(&retval)?;
    retval = remove_markers(&retval)?;
    //printf("after remove_markers: \n");
    //retval.write_in_att_format(stdout, 1);
    Ok(retval)
}

// for parallel rules
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-fn]
pub fn replace_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    // std::cerr << "replace"<< std::endl;

    // If there is only one rule in the vector, it is not parallel
    let mut retval: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], optional)?
    } else {
        parallel_bracketed_replace(rule_vector, optional)?
    };

    //std::cerr << "after bracketed replace"<< std::endl;
    //         printf("- bracketed replace -\n");
    //         retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;

    //   printf("----after no_repetition_constraint: ----\n");
    //   retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    //printf("----after apply_boundary_mark: ----\n");
    //retval.write_in_att_format(stdout, 1);

    // hfst/hfst#571: skip the obligatory constraint when every rule in the
    // vector is an epsilon-LHS + empty-context epenthesis (see
    // is_epsilon_lhs_empty_context). If any rule has a real context or a
    // non-epsilon LHS the constraint still applies.
    let mut all_epsilon_empty = !rule_vector.is_empty();
    for rule in rule_vector.iter() {
        if !is_epsilon_lhs_empty_context(rule)? {
            all_epsilon_empty = false;
            break;
        }
    }
    if !optional && !all_epsilon_empty {
        // Epenthesis rules behave differently if used most_brackets_plus_constraint
        // retval = most_brackets_plus_constraint(retval);
        retval = most_brackets_star_constraint(&retval)?;
    }

    // printf("----after most_brackets_star_constraint: ----\n");
    //  retval.write_in_att_format(stdout, 1);

    retval = remove_b2_constraint(&retval)?;

    //printf("----after remove_b2_constraint: ----\n");
    // retval.write_in_att_format(stdout, 1);

    retval = remove_markers(&retval)?;

    //printf("----after remove_markers: ----\n");
    //retval.write_in_att_format(stdout, 1);
    Ok(retval)
}

// replace left
pub fn replace_left_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mapping_pair_vector: HfstTransducerPairVector<B> = rule.get_mapping();
    //HfstTransducer newMapping = rule.get_mapping();
    //newMapping.invert().optimize();

    let mut new_mapping_pair_vector: HfstTransducerPairVector<B> = HfstTransducerPairVector::new();
    for pair in &mapping_pair_vector {
        // in every mapping pair invert first and second
        //HfstTransducer newMapping = rule.get_mapping();
        let first: HfstTransducer<B> = pair.0.clone();
        let second: HfstTransducer<B> = pair.1.clone();
        new_mapping_pair_vector.push((second, first));
    }

    let new_rule: Rule<B> = Rule::new_mapping_context_repl_type(
        &new_mapping_pair_vector,
        &rule.get_context(),
        rule.get_repl_type(),
    )?;
    let mut retval: HfstTransducer<B> = replace_rule(&new_rule, optional)?;

    retval.invert()?.optimize()?;
    Ok(retval)
}

// replace left parallel
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-left-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-left-fn]
pub fn replace_left_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut left_rule_vector: Vec<Rule<B>> = Vec::new();

    for rule in rule_vector {
        let mapping_pair_vector: HfstTransducerPairVector<B> = rule.get_mapping();
        //HfstTransducer newMapping = rule.get_mapping();
        //newMapping.invert().optimize();

        let mut new_mapping_pair_vector: HfstTransducerPairVector<B> =
            HfstTransducerPairVector::new();
        for pair in &mapping_pair_vector {
            // in every mapping pair invert first and second
            //HfstTransducer newMapping = rule.get_mapping();
            let first: HfstTransducer<B> = pair.0.clone();
            let second: HfstTransducer<B> = pair.1.clone();
            new_mapping_pair_vector.push((second, first));
        }

        let new_rule: Rule<B> = Rule::new_mapping_context_repl_type(
            &new_mapping_pair_vector,
            &rule.get_context(),
            rule.get_repl_type(),
        )?;

        left_rule_vector.push(new_rule);
    }

    let mut retval: HfstTransducer<B> = replace_rule_vector(&left_rule_vector, optional)?;
    retval.invert()?.optimize()?;

    Ok(retval)
}

// left to right
pub fn replace_leftmost_longest_match_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut unconditional_tr: HfstTransducer<B> = bracketed_replace(rule, true)?;
    //unconditional_tr = bracketed_replace(rule, true);

    //printf("LM unconditional_tr: \n");
    //unconditional_tr.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    // it should be before left_most_constraint
    unconditional_tr = no_repetition_constraint(&unconditional_tr)?;

    let mut retval: HfstTransducer<B> = left_most_constraint(&unconditional_tr)?;

    //to remove empty strings
    retval = one_betterthan_none_constraint(&retval)?;

    // printf("left_most_constraint: \n");
    // retval.write_in_att_format(stdout, 1);
    retval = longest_match_left_most_constraint(&retval)?;

    //printf("longest_match_left_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = remove_b2_constraint(&retval)?;
    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// left to right
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-longest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-longest-match-fn]
pub fn replace_leftmost_longest_match_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    //printf("\n replace_leftmost_longest_match \n");

    let mut unconditional_tr: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], true)?
    } else {
        parallel_bracketed_replace(rule_vector, true)?
    };

    //printf("retval unconditional 1 \n");
    // unconditional_tr.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    // it should be before left_most_constraint
    unconditional_tr = no_repetition_constraint(&unconditional_tr)?;
    //printf("unconditional_tr epenthesis \n");
    //unconditional_tr.write_in_att_format(stdout, 1);

    let mut retval: HfstTransducer<B> = left_most_constraint(&unconditional_tr)?;

    //to remove empty strings
    retval = one_betterthan_none_constraint(&retval)?;

    retval = longest_match_left_most_constraint(&retval)?;
    //printf("retval longest_match_left_most_constraint \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    //printf("retval remove_b2_constraint \n");
    //retval.write_in_att_format(stdout, 1);

    retval = remove_markers(&retval)?;

    //       printf("LM remove_markers: \n");
    //        retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    // printf("LM apply_boundary_mark: \n");
    // retval.write_in_att_format(stdout, 1);

    Ok(retval)
}

// right to left
pub fn replace_rightmost_longest_match_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let unconditional_tr: HfstTransducer<B> = bracketed_replace(rule, true)?;
    //unconditional_tr = bracketed_replace(rule, true);

    let mut retval: HfstTransducer<B> = right_most_constraint(&unconditional_tr)?;
    //retval = right_most_constraint(unconditional_tr);

    //printf("right_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = longest_match_right_most_constraint(&retval)?;

    //printf("longest_match_left_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;
    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// right to left
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-longest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-longest-match-fn]
pub fn replace_rightmost_longest_match_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    let unconditional_tr: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], true)?
    } else {
        parallel_bracketed_replace(rule_vector, true)?
    };

    let mut retval: HfstTransducer<B> = right_most_constraint(&unconditional_tr)?;
    //retval = right_most_constraint(unconditional_tr);

    //printf("right_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = longest_match_right_most_constraint(&retval)?;

    //printf("longest_match_left_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;
    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

pub fn replace_leftmost_shortest_match_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut unconditional_tr: HfstTransducer<B> = bracketed_replace(rule, true)?;
    //    unconditional_tr = bracketed_replace(rule, true);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    //has to be before left_most_constraint
    unconditional_tr = no_repetition_constraint(&unconditional_tr)?;

    let mut retval: HfstTransducer<B> = left_most_constraint(&unconditional_tr)?;
    //to remove empty strings
    retval = one_betterthan_none_constraint(&retval)?;

    retval = shortest_match_left_most_constraint(&retval)?;

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-shortest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-shortest-match-fn]
pub fn replace_leftmost_shortest_match_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    let mut unconditional_tr: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], true)?
    } else {
        parallel_bracketed_replace(rule_vector, true)?
    };

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    unconditional_tr = no_repetition_constraint(&unconditional_tr)?;

    let mut retval: HfstTransducer<B> = left_most_constraint(&unconditional_tr)?;

    //to remove empty strings
    retval = one_betterthan_none_constraint(&retval)?;

    retval = shortest_match_left_most_constraint(&retval)?;

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

pub fn replace_rightmost_shortest_match_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let unconditional_tr: HfstTransducer<B> = bracketed_replace(rule, true)?;
    //unconditional_tr = bracketed_replace( rule, true);

    let mut retval: HfstTransducer<B> = right_most_constraint(&unconditional_tr)?;
    //retval = right_most_constraint(unconditional_tr);
    retval = shortest_match_right_most_constraint(&retval)?;

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;
    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-shortest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-shortest-match-fn]
pub fn replace_rightmost_shortest_match_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    let unconditional_tr: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], true)?
    } else {
        parallel_bracketed_replace(rule_vector, true)?
    };
    let mut retval: HfstTransducer<B> = right_most_constraint(&unconditional_tr)?;
    //retval = right_most_constraint(unconditional_tr);
    retval = shortest_match_right_most_constraint(&retval)?;

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;
    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// replace up, left, right, down
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-epenthesis-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-epenthesis-fn]
pub fn replace_epenthesis_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    replace_rule(rule, optional)
}

// replace up, left, right, down
pub fn replace_epenthesis_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    replace_rule_vector(rule_vector, optional)
}

//---------------------------------
//    RESTRICTION FUNCTIONS
//---------------------------------

/*
  define U [ ? - %<D%> ] ;

  define CENTER [ x y | x x y y ];

  define L1 [ a ] ;
  define R1 [ b ] ;

  define L2 [ x ] ;
  define R2 [ y ] ;

  define RES1 [ U* L1 %<D%> U* %<D%> R1 U* ] ;
  define RES2 [ U* L2 %<D%> U* %<D%> R2 U* ] ;

  define CEN1 [ U* %<D%> CENTER %<D%> U* ] ;

  define NODU [ U | 0:%<D%> ]* ;
  define NODL [ U | %<D%>:0 ]* ;

  regex U* - [ NODU .o. [ CEN1 - [ RES1 | RES2 ] ] .o. NODL ] ;
*/
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.restriction-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.restriction-fn]
pub fn restriction<B: AlgebraBackend>(
    _center: &HfstTransducer<B>,
    context: &HfstTransducerPairVector<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    //check if the center is automata
    let mut proj1: HfstTransducer<B> = _center.clone();
    proj1.input_project()?;
    let mut proj2: HfstTransducer<B> = _center.clone();
    proj2.output_project()?;

    if !proj1.compare(_center, true)? || !proj2.compare(_center, true)? {
        crate::bail!(TransducersAreNotAutomata, "HfstXeroxRules::restriction");
    }

    let restriction_mark: String = "@_D_@".to_string();

    let mut tok: HfstTokenizer = HfstTokenizer::new();
    tok.add_multichar_symbol(&restriction_mark);
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mark: HfstTransducer<B> =
        HfstTransducer::new_string_tokenizer_type(&restriction_mark, &tok)?;
    let epsilon: HfstTransducer<B> =
        HfstTransducer::new_string_tokenizer_type("@_EPSILON_SYMBOL_@", &tok)?;

    // Identity
    let identity_pair: HfstTransducer<B> = HfstTransducer::identity_pair();
    let mut identity: HfstTransducer<B> = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut universal_without_d: HfstTransducer<B> = identity.clone();
    universal_without_d.insert_to_alphabet_string(&restriction_mark)?;
    let mut universal_without_d_star: HfstTransducer<B> = universal_without_d.clone();
    universal_without_d_star.repeat_star()?.optimize()?;

    // NODU
    let mut no_d_upper: HfstTransducer<B> = HfstTransducer::new_string_string_tokenizer_type(
        "@_EPSILON_SYMBOL_@",
        &restriction_mark,
        &tok,
    )?;
    no_d_upper
        .disjunct(&universal_without_d, true)?
        .repeat_star()?
        .optimize()?;

    // NODL
    let mut no_d_lower: HfstTransducer<B> = HfstTransducer::new_string_string_tokenizer_type(
        &restriction_mark,
        "@_EPSILON_SYMBOL_@",
        &tok,
    )?;
    no_d_lower
        .disjunct(&universal_without_d, true)?
        .repeat_star()?
        .optimize()?;

    // 1. Surround center with marks
    // [ U* %<D%> CENTER %<D%> U* ]
    let mut center: HfstTransducer<B> = _center.clone();
    center.insert_to_alphabet_string(&restriction_mark)?;

    let mut center_marked: HfstTransducer<B> = universal_without_d_star.clone();
    center_marked
        .concatenate(&mark, true)?
        .concatenate(&center, true)?
        .concatenate(&mark, true)?
        .concatenate(&universal_without_d_star, true)?
        .optimize()?;

    // 2. Put mark in context
    // [ U* L1 %<D%> U* %<D%> R1 U* ]
    let mut context_marked: HfstTransducer<B> = HfstTransducer::new();
    for (i, context_pair) in context.iter().enumerate() {
        let mut lef_context: HfstTransducer<B> = context_pair.0.clone();
        lef_context.insert_to_alphabet_string(&restriction_mark)?;

        let mut right_context: HfstTransducer<B> = context_pair.1.clone();
        right_context.insert_to_alphabet_string(&restriction_mark)?;

        let mut res: HfstTransducer<B> = universal_without_d_star.clone();
        res.concatenate(&lef_context, true)?
            .concatenate(&mark, true)?
            .concatenate(&universal_without_d_star, true)?
            .concatenate(&mark, true)?
            .concatenate(&right_context, true)?
            .concatenate(&universal_without_d_star, true)?
            .optimize()?;

        if i == 0 {
            context_marked = res;
        } else {
            context_marked.disjunct(&res, true)?.optimize()?;
        }
    }
    let mut center_minus_ctx: HfstTransducer<B> = center_marked.clone();
    center_minus_ctx
        .subtract(&context_marked, true)?
        .optimize()?;

    let mut tmp: HfstTransducer<B> = no_d_upper.clone();
    tmp.compose(&center_minus_ctx, true)?
        .compose(&no_d_lower, true)?
        .optimize()?;

    let mut retval: HfstTransducer<B> = universal_without_d_star.clone();
    retval.subtract(&tmp, true)?.optimize()?;

    retval.remove_from_alphabet_string(&restriction_mark)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// a < b
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.before-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.before-fn]
pub fn before<B: AlgebraBackend>(
    left: &HfstTransducer<B>,
    right: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    //check if the center is automata
    let mut l_proj1: HfstTransducer<B> = left.clone();
    l_proj1.input_project()?;
    let mut l_proj2: HfstTransducer<B> = left.clone();
    l_proj2.output_project()?;
    let mut r_proj1: HfstTransducer<B> = right.clone();
    r_proj1.input_project()?;
    let mut r_proj2: HfstTransducer<B> = right.clone();
    r_proj2.output_project()?;

    if !l_proj1.compare(left, true)?
        || !l_proj2.compare(left, true)?
        || !r_proj1.compare(right, true)?
        || !r_proj2.compare(right, true)?
    {
        crate::bail!(TransducersAreNotAutomata, "HfstXeroxRules::restriction");
    }

    // Identity
    let identity_pair: HfstTransducer<B> = HfstTransducer::identity_pair();
    let mut identity: HfstTransducer<B> = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut tmp: HfstTransducer<B> = identity.clone();
    tmp.concatenate(right, true)?
        .concatenate(&identity, true)?
        .concatenate(left, true)?
        .concatenate(&identity, true)?
        .optimize()?;

    let mut retval: HfstTransducer<B> = identity.clone();
    retval.subtract(&tmp, true)?.optimize()?;

    Ok(retval)
}

// a > b
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.after-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.after-fn]
pub fn after<B: AlgebraBackend>(
    left: &HfstTransducer<B>,
    right: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    //check if the center is automata
    let mut l_proj1: HfstTransducer<B> = left.clone();
    l_proj1.input_project()?;
    let mut l_proj2: HfstTransducer<B> = left.clone();
    l_proj2.output_project()?;
    let mut r_proj1: HfstTransducer<B> = right.clone();
    r_proj1.input_project()?;
    let mut r_proj2: HfstTransducer<B> = right.clone();
    r_proj2.output_project()?;

    if !l_proj1.compare(left, true)?
        || !l_proj2.compare(left, true)?
        || !r_proj1.compare(right, true)?
        || !r_proj2.compare(right, true)?
    {
        crate::bail!(TransducersAreNotAutomata, "HfstXeroxRules::restriction");
    }

    // Identity
    let identity_pair: HfstTransducer<B> = HfstTransducer::identity_pair();
    let mut identity: HfstTransducer<B> = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut tmp: HfstTransducer<B> = identity.clone();
    tmp.concatenate(left, true)?
        .concatenate(&identity, true)?
        .concatenate(right, true)?
        .concatenate(&identity, true)?
        .optimize()?;

    let mut retval: HfstTransducer<B> = identity.clone();
    retval.subtract(&tmp, true)?.optimize()?;

    Ok(retval)
}
