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

mod bracketing;
mod constraints;
mod replace;
mod restriction;
pub use bracketing::{
    bracketed_replace, constraint_composition, expand_contexts_with_mapping,
    insert_freely_all_the_brackets, parallel_bracketed_replace, remove_markers, zero_weight,
};
pub use constraints::{
    apply_boundary_mark, constraints_right_part, left_most_constraint,
    longest_match_left_most_constraint, longest_match_right_most_constraint,
    most_brackets_plus_constraint, most_brackets_star_constraint, no_repetition_constraint,
    one_betterthan_none_constraint, remove_b2_constraint, right_most_constraint,
    shortest_match_left_most_constraint, shortest_match_right_most_constraint,
};
pub use replace::{
    create_mapping_for_mark_up_replace, replace_epenthesis_rule, replace_epenthesis_rule_vector,
    replace_left_rule, replace_left_rule_vector, replace_leftmost_longest_match_rule,
    replace_leftmost_longest_match_rule_vector, replace_leftmost_shortest_match_rule,
    replace_leftmost_shortest_match_rule_vector, replace_rightmost_longest_match_rule,
    replace_rightmost_longest_match_rule_vector, replace_rightmost_shortest_match_rule,
    replace_rightmost_shortest_match_rule_vector, replace_rule, replace_rule_vector,
};
pub use restriction::{after, before, restriction};

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
