//! Port of `libhfst/src/HfstXeroxRules.{h,cc}` — the `hfst::xeroxRules` namespace:
//! HFST-XFST replace functions and their `Rule` data type.
//!
//! ABSOLUTE 1:1 literal C++->Rust translation (HFST port, Wave 2). NOT idiomatic.
//! Mirrors structure/control-flow/eval-order; preserves bugs. The free functions
//! build transducers via the facade type `crate::hfst_transducer::HfstTransducer`.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::fmt;

use crate::hfst_data_types::ImplementationType;
use crate::hfst_data_types::StringPair;
// HfstTransducer, plus the HfstTransducer-dependent aliases
// (HfstTransducerPair, HfstTransducerPairVector, HfstTransducerVector), live in the
// facade module that is ported concurrently. Bodies import them from
// `crate::hfst_transducer`.
use crate::hfst_transducer::HfstTransducer;
use crate::hfst_transducer::HfstTransducerPair;
use crate::hfst_transducer::HfstTransducerPairVector;
use crate::hfst_transducer::HfstTransducerVector;

/// \brief The replace direction / type used by the `xeroxRules` namespace.
///
/// Distinct from `crate::hfst_rules::ReplaceType`: this one has only four variants
/// (no `REPL_DOWN_KARTTUNEN`).
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
///     context is pair of transducers L and R, and replType is enum REPL_UP.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule]
#[derive(Clone)]
pub struct Rule {
    /* cross product of mapping transducers */
    pub(crate) mapping: HfstTransducerPairVector,
    /* context */
    pub(crate) context: HfstTransducerPairVector,
    /* if there is a context, it needs to have a direction (up, left, down or right) */
    pub(crate) replType: ReplaceType,
}

// C++ `friend std::ostream& operator<<(std::ostream&, const Rule&)` -> `Display`.
impl fmt::Display for Rule {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!("deferred: xeroxRules::operator<<(ostream, Rule) — body agent")
    }
}

// ===== flattened bodies (free fns share scope) =====
use crate::HFST_THROW_MESSAGE;
use crate::hfst_exception_defs::TransducerTypeMismatchException;
use crate::hfst_exception_defs::TransducersAreNotAutomataException;
use crate::hfst_symbol_defs::HfstSymbolSubstitutions;
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_symbol_defs::internal_epsilon;
use crate::hfst_tokenizer::HfstTokenizer;
use std::io::Write;

impl Rule {
    pub fn new_mapping(mappingPairVector: &HfstTransducerPairVector) -> Self {
        let mut TOK = HfstTokenizer::new();
        TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

        let type_: ImplementationType = mappingPairVector[0].0.get_type();
        // Check if all transducer types are the same
        for i in 0..mappingPairVector.len() {
            if mappingPairVector[i].0.get_type() != type_
                || mappingPairVector[i].1.get_type() != type_
            {
                crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "Rule mapping");
            }
        }

        let contextPair: HfstTransducerPair = (
            HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_),
            HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_),
        );
        let mut epsilonContext: HfstTransducerPairVector = Vec::new();
        epsilonContext.push(contextPair);

        let mapping = mappingPairVector.clone();

        // HfstTransducerPairVector tmpV = mappingPairVector;
        // tmpV[0].0 = encodeFlagDiacritics(tmpV[0].0);

        //mapping = tmpV;
        let context = epsilonContext;
        let replType = ReplaceType::REPL_UP;

        Rule {
            mapping,
            context,
            replType,
        }
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.rule-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.rule-fn]
    pub fn new_mapping_context_repl_type(
        mappingPairVector: &HfstTransducerPairVector,
        a_context: &HfstTransducerPairVector,
        a_replType: ReplaceType,
    ) -> Self {
        // Check if all transducer types are the same
        let type_: ImplementationType = mappingPairVector[0].0.get_type();
        for i in 0..mappingPairVector.len() {
            if mappingPairVector[i].0.get_type() != type_
                || mappingPairVector[i].1.get_type() != type_
            {
                crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "Rule mapping");
            }
        }
        for j in 0..a_context.len() {
            if a_context[j].0.get_type() != type_ || a_context[j].1.get_type() != type_ {
                crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "Rule context");
            }
        }

        //HfstTransducerPairVector tmpV = mappingPairVector;
        //tmpV[0].0 = encodeFlagDiacritics(tmpV[0].0);

        let mapping = mappingPairVector.clone();
        // mapping = tmpV                       ;
        let context = a_context.clone();
        let replType = a_replType;

        Rule {
            mapping,
            context,
            replType,
        }
    }

    //copy
    pub fn new_rule(a_rule: &Rule) -> Self {
        let mapping = a_rule.get_mapping();
        let context = a_rule.get_context();
        let replType = a_rule.get_replType();

        Rule {
            mapping,
            context,
            replType,
        }
    }

    // for SWIG
    pub fn new() -> Self {
        let mut TOK = HfstTokenizer::new();
        TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
        let type_: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;
        let contextPair: HfstTransducerPair = (
            HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_),
            HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_),
        );
        let mut epsilonContext: HfstTransducerPairVector = Vec::new();
        epsilonContext.push(contextPair);
        let context = epsilonContext;
        let replType = ReplaceType::REPL_UP;
        // `mapping` is left default-constructed (an empty vector), as in C++.
        let mapping: HfstTransducerPairVector = Vec::new();

        Rule {
            mapping,
            context,
            replType,
        }
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-mapping-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-mapping-fn]
    pub fn get_mapping(&self) -> HfstTransducerPairVector {
        self.mapping.clone()
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-context-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-context-fn]
    pub fn get_context(&self) -> HfstTransducerPairVector {
        self.context.clone()
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-repl-type-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-repl-type-fn]
    pub fn get_replType(&self) -> ReplaceType {
        self.replType
    }

    // [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.encode-flags-fn]
    // [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.encode-flags-fn]
    pub fn encodeFlags(&mut self) {
        let mut tmpM: HfstTransducerPairVector = self.mapping.clone();

        for i in 0..tmpM.len() {
            tmpM[i].0 = encodeFlagDiacritics(&tmpM[i].0);
            tmpM[i].1 = encodeFlagDiacritics(&tmpM[i].1);
        }

        let mut tmpC: HfstTransducerPairVector = self.context.clone();

        for i in 0..tmpC.len() {
            tmpC[i].0 = encodeFlagDiacritics(&tmpC[i].0);
            tmpC[i].1 = encodeFlagDiacritics(&tmpC[i].1);
        }

        self.mapping = tmpM;
        self.context = tmpC;
    }
}

// Ports `std::ostream & operator<<(std::ostream &out, const Rule & r)`.
pub fn operator_shl_os(out: &mut dyn Write, r: &Rule) {
    writeln!(out, "hfst::xeroxRules::Rule:").unwrap();
    write!(out, "replType: ").unwrap();
    match r.replType {
        ReplaceType::REPL_UP => {
            write!(out, "REPL_UP").unwrap();
        }
        ReplaceType::REPL_DOWN => {
            write!(out, "REPL_DOWN").unwrap();
        }
        ReplaceType::REPL_RIGHT => {
            write!(out, "REPL_RIGHT").unwrap();
        }
        ReplaceType::REPL_LEFT => {
            write!(out, "REPL_LEFT").unwrap();
        }
    }
    writeln!(out).unwrap();

    writeln!(out, "mapping:").unwrap();
    let mut index: u32 = 1;
    for it in r.mapping.iter() {
        writeln!(out, "#{} (right side):", index).unwrap();
        crate::hfst_transducer::operator_shl_os(out, &it.0);
        writeln!(out, "#{} (left side):", index).unwrap();
        crate::hfst_transducer::operator_shl_os(out, &it.1);
        index += 1;
    }

    writeln!(out, "context:").unwrap();
    index = 1;
    for it in r.context.iter() {
        writeln!(out, "#{} (right side):", index).unwrap();
        crate::hfst_transducer::operator_shl_os(out, &it.0);
        writeln!(out, "#{} (left side):", index).unwrap();
        crate::hfst_transducer::operator_shl_os(out, &it.1);
        index += 1;
    }
}

//////////////////////////////////////
// In the transducer tr, change all flag diacritics to "non-special" multichar symbols
// It means that @ sign will be changed to $ sign
// ie. @P.FOO.BAR@ will be changed into $P.FOO.BAR$
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.encode-flag-diacritics-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.encode-flag-diacritics-fn]
pub fn encodeFlagDiacritics(tr: &HfstTransducer) -> HfstTransducer {
    let mut realFlagstoFakeFlags: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
    let mut removeFromAlphabet: StringSet = StringSet::new();
    let transducerAlphabet: StringSet = tr.get_alphabet();
    for s in transducerAlphabet.iter() {
        let alph: String = s.clone();
        // mirrors std::string::substr(0,3): the first (up to) three bytes
        let alphFirst3: String = {
            let n = std::cmp::min(3, alph.len());
            String::from_utf8_lossy(&alph.as_bytes()[..n]).into_owned()
        };

        //@operator.feature.value@ and @operator.feature@

        if alphFirst3 == "@P."
            || alphFirst3 == "@R."
            || alphFirst3 == "@U."
            || alphFirst3 == "@D."
            || alphFirst3 == "@C."
            || alphFirst3 == "@N."
            || alphFirst3 == "@p."
            || alphFirst3 == "@r."
            || alphFirst3 == "@u."
            || alphFirst3 == "@d."
            || alphFirst3 == "@c."
            || alphFirst3 == "@n."
        {
            let alph = alph.replace('@', "$");
            realFlagstoFakeFlags.insert(s.clone(), alph);
            removeFromAlphabet.insert(s.clone());
        }
    }

    let mut retval: HfstTransducer = tr.clone();
    retval.substitute_substitutions(&realFlagstoFakeFlags);

    retval.remove_from_alphabet_string_set(&removeFromAlphabet);
    retval
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.decode-flag-diacritics-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.decode-flag-diacritics-fn]
pub fn decodeFlagDiacritics(tr: &HfstTransducer) -> HfstTransducer {
    let mut fakeFlagsToRealFlags: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();

    let transducerAlphabet: StringSet = tr.get_alphabet();
    let mut removeFromAlphabet: StringSet = StringSet::new();
    for s in transducerAlphabet.iter() {
        let alph: String = s.clone();
        // mirrors std::string::substr(0,3): the first (up to) three bytes
        let alphFirst3: String = {
            let n = std::cmp::min(3, alph.len());
            String::from_utf8_lossy(&alph.as_bytes()[..n]).into_owned()
        };

        //@operator.feature.value@ and @operator.feature@

        if alphFirst3 == "$P."
            || alphFirst3 == "$R."
            || alphFirst3 == "$U."
            || alphFirst3 == "$D."
            || alphFirst3 == "$C."
            || alphFirst3 == "$N."
            || alphFirst3 == "$p."
            || alphFirst3 == "$r."
            || alphFirst3 == "$u."
            || alphFirst3 == "$d."
            || alphFirst3 == "$c."
            || alphFirst3 == "$n."
        {
            let alph = alph.replace('$', "@");
            fakeFlagsToRealFlags.insert(s.clone(), alph);
            removeFromAlphabet.insert(s.clone());
        }
    }

    let mut retval: HfstTransducer = tr.clone();
    retval.substitute_substitutions(&fakeFlagsToRealFlags);
    retval.remove_from_alphabet_string_set(&removeFromAlphabet);
    retval
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.disjunct-vector-members-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.disjunct-vector-members-fn]
pub fn disjunctVectorMembers(trVector: &HfstTransducerVector) -> HfstTransducer {
    let mut retval: HfstTransducer = trVector[0].clone();
    for i in 1..trVector.len() {
        retval.disjunct(&trVector[i], true).optimize();
    }
    retval
}

//////////////////////////////////////
// Port of `libhfst/src/HfstXeroxRules.cc` lines 320..1500 (functions defined in
// that span). Sibling areas of `crate::hfst_xerox_rules` own everything outside
// this span (e.g. `decodeFlagDiacritics`, `Rule`, `ReplaceType`, the `replace*`
// interface functions); they reach this module via `use super::*`.

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.remove-markers-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.remove-markers-fn]
pub fn removeMarkers(tr: &HfstTransducer) -> HfstTransducer {
    let mut retval = tr.clone();

    let leftMarker: String = "@LM@".to_string();
    let rightMarker: String = "@RM@".to_string();

    retval
        .substitute_symbol_pair(
            &(leftMarker.clone(), leftMarker.clone()),
            &(
                "@_EPSILON_SYMBOL_@".to_string(),
                "@_EPSILON_SYMBOL_@".to_string(),
            ),
        )
        .optimize();
    retval
        .substitute_symbol_pair(
            &(rightMarker.clone(), rightMarker.clone()),
            &(
                "@_EPSILON_SYMBOL_@".to_string(),
                "@_EPSILON_SYMBOL_@".to_string(),
            ),
        )
        .optimize();

    retval.remove_from_alphabet_symbol(&leftMarker);
    retval.remove_from_alphabet_symbol(&rightMarker);

    retval.optimize();

    retval = decodeFlagDiacritics(&retval);

    retval
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
pub fn constraintComposition(t: &HfstTransducer, Constraint: &HfstTransducer) -> HfstTransducer {
    let mut retval = t.clone();
    retval.transform_weights(zero_weight);

    retval.input_project().optimize();

    let mut tmp = retval.clone();
    tmp.compose(Constraint, true).optimize();

    tmp.compose(&retval, true).optimize();
    tmp.output_project().optimize();
    retval.subtract(&tmp, true).optimize();

    //transform weights to zero
    retval.transform_weights(zero_weight);
    retval.compose(t, true).optimize();

    retval
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.insert-freely-all-the-brackets-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.insert-freely-all-the-brackets-fn]
pub fn insertFreelyAllTheBrackets(t: &mut HfstTransducer, optional: bool) {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let leftMarker: String = "@LM@".to_string();
    let rightMarker: String = "@RM@".to_string();
    let leftMarker2: String = "@LM2@".to_string();
    let rightMarker2: String = "@RM2@".to_string();

    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);
    TOK.add_multichar_symbol(&leftMarker2);
    TOK.add_multichar_symbol(&rightMarker2);

    let type_ = t.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    t.insert_freely_transducer(&leftBracket, false).optimize();
    t.insert_freely_transducer(&rightBracket, false).optimize();

    if !optional {
        let leftBracket2 = HfstTransducer::new_tokenized(&leftMarker2, &TOK, type_);
        let rightBracket2 = HfstTransducer::new_tokenized(&rightMarker2, &TOK, type_);

        t.insert_freely_transducer(&leftBracket2, false).optimize();
        t.insert_freely_transducer(&rightBracket2, false).optimize();
    }
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.expand-contexts-with-mapping-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.expand-contexts-with-mapping-fn]
pub fn expandContextsWithMapping(
    ContextVector: &HfstTransducerPairVector,
    mappingWithBracketsAndTmpBoundary: &HfstTransducer,
    identityExpanded: &HfstTransducer,
    replType: ReplaceType,
    optional: bool,
) -> HfstTransducer {
    let type_ = identityExpanded.get_type();

    let mut unionContextReplace = HfstTransducer::new_type(type_);

    let mut TOK = HfstTokenizer::new();
    // TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    // HfstTransducer epsilon("@_EPSILON_SYMBOL_@", TOK, type);

    for i in 0..ContextVector.len() {
        // Expand context with mapping
        // Cr' = (Rc .*) << Markers (<,>,|) .o. [I:I | <a:b>]*
        // Cr = Cr|Cr'
        // (same for left context)

        // Lc = (*. Lc) << {<,>}

        let identityPair = HfstTransducer::identity_pair(type_);
        let mut identityStar = identityPair.clone();
        identityStar.repeat_star();

        let mut firstContext = identityStar.clone();
        firstContext.concatenate(&ContextVector[i].0, true);
        firstContext.transform_weights(zero_weight);
        firstContext.optimize();

        insertFreelyAllTheBrackets(&mut firstContext, optional);

        // Rc =  (Rc .*) << {<,>}
        let mut secondContext = ContextVector[i].1.clone();
        secondContext.concatenate(&identityStar, true);
        secondContext.transform_weights(zero_weight);
        secondContext.optimize();
        insertFreelyAllTheBrackets(&mut secondContext, optional);

        /* RULE:    LC:        RC:
         * up        up        up
         * left        up        down
         * right    down    up
         * down        down    down
         */

        let mut leftContextExpanded = HfstTransducer::new_type(type_);
        let mut rightContextExpanded = HfstTransducer::new_type(type_);

        // both contexts are in upper language
        if replType == ReplaceType::REPL_UP {
            // compose them with [I:I | <a:b>]*
            leftContextExpanded = firstContext.clone();
            rightContextExpanded = secondContext.clone();

            leftContextExpanded.compose(identityExpanded, true);
            rightContextExpanded.compose(identityExpanded, true);
        }
        // left context is in lower language, right in upper ( // )
        if replType == ReplaceType::REPL_RIGHT {
            // compose them with [I:I | <a:b>]*

            // left compose opposite way
            leftContextExpanded = identityExpanded.clone();
            rightContextExpanded = secondContext.clone();

            leftContextExpanded.compose(&firstContext, true);
            rightContextExpanded.compose(identityExpanded, true);
        }
        // right context is in lower language, left in upper ( \\ )
        if replType == ReplaceType::REPL_LEFT {
            // compose them with [I:I | <a:b>]*
            leftContextExpanded = firstContext.clone();
            rightContextExpanded = identityExpanded.clone();

            leftContextExpanded.compose(identityExpanded, true);
            rightContextExpanded.compose(&secondContext, true);
        }
        if replType == ReplaceType::REPL_DOWN {
            // compose them with [I:I | <a:b>]*
            leftContextExpanded = identityExpanded.clone();
            rightContextExpanded = identityExpanded.clone();

            leftContextExpanded.compose(&firstContext, true);
            rightContextExpanded.compose(&secondContext, true);
        }

        leftContextExpanded.transform_weights(zero_weight);
        rightContextExpanded.transform_weights(zero_weight);
        leftContextExpanded.optimize();
        rightContextExpanded.optimize();

        firstContext.disjunct(&leftContextExpanded, true);
        firstContext.optimize();

        secondContext.disjunct(&rightContextExpanded, true);
        secondContext.optimize();

        // add boundary symbol before/after contexts
        let boundaryMarker: String = ".#.".to_string();
        TOK.add_multichar_symbol(&boundaryMarker);
        let boundary = HfstTransducer::new_tokenized(&boundaryMarker, &TOK, type_);

        identityStar.insert_to_alphabet_symbol(&boundaryMarker);

        // to firstContext
        let firstContextAlphabet = firstContext.get_alphabet();
        let mut hasBoundary = false;
        for s in firstContextAlphabet.iter() {
            if boundaryMarker == *s {
                hasBoundary = true;
            }
        }

        if hasBoundary == false {
            firstContext.insert_to_alphabet_symbol(&boundaryMarker);
            let mut tmp = boundary.clone();
            tmp.concatenate(&identityStar, true).optimize();
            tmp.concatenate(&firstContext, true);
            firstContext = tmp;
        }

        // to secondContext
        let secondContextAlphabet = secondContext.get_alphabet();
        hasBoundary = false;
        for s in secondContextAlphabet.iter() {
            if boundaryMarker == *s {
                hasBoundary = true;
            }
        }

        if hasBoundary == false {
            secondContext.insert_to_alphabet_symbol(&boundaryMarker);
            secondContext
                .concatenate(&identityStar, true)
                .concatenate(&boundary, true)
                .optimize();
        }

        // put mapping between (expanded) contexts
        let mut oneContextReplace = firstContext.clone();
        oneContextReplace
            .concatenate(mappingWithBracketsAndTmpBoundary, true)
            .concatenate(&secondContext, true);

        oneContextReplace.transform_weights(zero_weight);
        unionContextReplace.disjunct(&oneContextReplace, true);
        unionContextReplace.optimize();
    }
    unionContextReplace
}

/*
 * unconditional replace, in multiple contexts
 * first: (.* T<a:b>T .*) - [( .* L1 T<a:b>T R1 .*) u (.* L2 T<a:b>T R2 .*)...],
 *                         where .* = [I:I (+ {tmpMarker (T), <,>} in alphabet) | <a:b>]*
 * then: remove tmpMarker from transducer and alphabet, and do negation:
 *         .* - result from upper operations
 */

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.bracketed-replace-fn]
pub fn bracketedReplace(rule: &Rule, optional: bool) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    TOK.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    let leftMarker: String = "@LM@".to_string();
    let rightMarker: String = "@RM@".to_string();
    let tmpMarker: String = "@TMPM@".to_string();
    let leftMarker2: String = "@LM2@".to_string();
    let rightMarker2: String = "@RM2@".to_string();
    let newEpsilon: String = "$Epsilon$".to_string();

    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);
    TOK.add_multichar_symbol(&leftMarker2);
    TOK.add_multichar_symbol(&rightMarker2);
    TOK.add_multichar_symbol(&tmpMarker);
    TOK.add_multichar_symbol(&newEpsilon);
    TOK.add_multichar_symbol(".#.");

    //first, encode all flag diacritics
    let mut ruletmp = rule.clone();
    ruletmp.encodeFlags();

    let mappingPairVector: HfstTransducerPairVector = ruletmp.get_mapping();
    let ContextVector: HfstTransducerPairVector = ruletmp.get_context();
    let replType: ReplaceType = ruletmp.get_replType();

    let type_ = mappingPairVector[0].0.get_type();

    // Identity (normal)
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
    let mut mapping = HfstTransducer::new_type(type_);
    for i in 0..mappingPairVector.len() {
        let mut oneMappingPair = mappingPairVector[i].0.clone();

        //markup rules are already cross product in mappingPairVector[i].0 (second is empty)
        //so the cross product should not be done for markup rules
        if mappingPairVector[i].0.get_property("isMarkup") != "yes" {
            oneMappingPair.cross_product(&mappingPairVector[i].1, true);
        }

        // for removing .#. from the center
        let mut identityWithoutBoundary = identity.clone();
        identityWithoutBoundary.insert_to_alphabet_symbol(".#.");
        let mut removeHash = identityWithoutBoundary.clone();
        let boundary = HfstTransducer::new_tokenized(".#.", &TOK, type_);
        removeHash
            .concatenate(&boundary, true)
            .concatenate(&identityWithoutBoundary, true)
            .optimize();

        if i == 0 {
            // remove .#. from the center
            // center - (?* .#. ?*)
            oneMappingPair.subtract(&removeHash, false).optimize();
            oneMappingPair.remove_from_alphabet_symbol(".#.");
            mapping = oneMappingPair;
        } else {
            oneMappingPair.subtract(&removeHash, false).optimize();
            oneMappingPair.remove_from_alphabet_symbol(".#.");
            mapping.disjunct(&oneMappingPair, true).optimize();
        }
    }

    // In case of ? -> x replacement
    // If left side is empty, return identity transducer
    // If right side is empty, return identity transducer
    //    with alphabet from the left side
    let empty = HfstTransducer::new_type(type_);
    if mapping.compare(&empty, true) {
        mapping = identity.clone();
        if mappingPairVector[0].1.compare(&empty, true) {
            let transducerAlphabet = mappingPairVector[0].0.get_alphabet();
            for s in transducerAlphabet.iter() {
                mapping.insert_to_alphabet_symbol(s);
            }
        }
    }

    mapping.insert_to_alphabet_symbol(&leftMarker);
    mapping.insert_to_alphabet_symbol(&rightMarker);
    mapping.insert_to_alphabet_symbol(&tmpMarker);

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);
    let leftBracket2 = HfstTransducer::new_tokenized(&leftMarker2, &TOK, type_);
    let rightBracket2 = HfstTransducer::new_tokenized(&rightMarker2, &TOK, type_);
    let tmpBracket = HfstTransducer::new_tokenized(&tmpMarker, &TOK, type_);

    // Surround mapping with brackets
    let mut tmpMapping = leftBracket.clone();
    tmpMapping
        .concatenate(&mapping, true)
        .concatenate(&rightBracket, true)
        .optimize();

    let mut mappingWithBrackets = tmpMapping.clone();

    // Identity pair
    // for non-optional replacements
    if optional != true {
        // non - optional
        // mapping = <a:b> u <2a:a>2

        let mut mappingWithBrackets2 = leftBracket2.clone();
        let mut leftMappingUnion = mappingPairVector[0].0.clone();
        for i in 1..mappingPairVector.len() {
            leftMappingUnion
                .disjunct(&mappingPairVector[i].0, true)
                .optimize();
        }
        // needed in case of ? -> x replacement
        leftMappingUnion.insert_to_alphabet_symbol(&leftMarker2);
        leftMappingUnion.insert_to_alphabet_symbol(&rightMarker2);
        leftMappingUnion.insert_to_alphabet_symbol(&leftMarker);
        leftMappingUnion.insert_to_alphabet_symbol(&rightMarker);
        leftMappingUnion.insert_to_alphabet_symbol(&tmpMarker);

        mappingWithBrackets2
            .concatenate(&leftMappingUnion, true)
            .concatenate(&rightBracket2, true)
            .optimize();

        // mappingWithBrackets...... expanded
        mappingWithBrackets.insert_to_alphabet_symbol(&leftMarker2);
        mappingWithBrackets.insert_to_alphabet_symbol(&rightMarker2);
        mappingWithBrackets
            .disjunct(&mappingWithBrackets2, true)
            .optimize();
    }

    // Identity with bracketed mapping and marker symbols and TmpMarker in alphabet
    // [I:I | <a:b>]* (+ tmpMarker in alphabet)
    let mut identityExpanded = identityPair.clone();

    identityExpanded.insert_to_alphabet_symbol(&leftMarker);
    identityExpanded.insert_to_alphabet_symbol(&rightMarker);
    identityExpanded.insert_to_alphabet_symbol(&tmpMarker);

    if optional != true {
        identityExpanded.insert_to_alphabet_symbol(&leftMarker2);
        identityExpanded.insert_to_alphabet_symbol(&rightMarker2);
    }

    identityExpanded
        .disjunct(&mappingWithBrackets, true)
        .optimize();
    identityExpanded.repeat_star().optimize();

    // when there aren't any contexts, result is identityExpanded
    if ContextVector.len() == 1 {
        let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
        if ContextVector[0].0.compare(&epsilon, true) && ContextVector[0].1.compare(&epsilon, true)
        {
            identityExpanded.remove_from_alphabet_symbol(&tmpMarker);
            return identityExpanded;
        }
    }

    // Surround mapping with tmp boudaries
    let mut mappingWithBracketsAndTmpBoundary = tmpBracket.clone();
    mappingWithBracketsAndTmpBoundary
        .concatenate(&mappingWithBrackets, true)
        .concatenate(&tmpBracket, true)
        .optimize();

    // .* |<a:b>| :*
    let mut bracketedReplace = identityExpanded.clone();
    bracketedReplace
        .concatenate(&mappingWithBracketsAndTmpBoundary, true)
        .concatenate(&identityExpanded, true)
        .optimize();

    // Expand all contexts with mapping taking in consideration replace type
    // result is their union
    let unionContextReplace = expandContextsWithMapping(
        &ContextVector,
        &mappingWithBracketsAndTmpBoundary,
        &identityExpanded,
        replType,
        optional,
    );

    // subtract all mappings in contexts from replace without contexts
    let mut replaceWithoutContexts = bracketedReplace.clone();
    replaceWithoutContexts
        .subtract(&unionContextReplace, true)
        .optimize();

    // remove tmpMaprker
    replaceWithoutContexts
        .substitute_symbol_pair(
            &(tmpMarker.clone(), tmpMarker.clone()),
            &(
                "@_EPSILON_SYMBOL_@".to_string(),
                "@_EPSILON_SYMBOL_@".to_string(),
            ),
        )
        .optimize();
    replaceWithoutContexts.remove_from_alphabet_symbol(&tmpMarker);
    replaceWithoutContexts.optimize();

    identityExpanded.remove_from_alphabet_symbol(&tmpMarker);

    // final negation
    let mut uncondidtionalTr = identityExpanded.clone();
    uncondidtionalTr
        .subtract(&replaceWithoutContexts, true)
        .optimize();

    uncondidtionalTr
}

// Return the string "@N@" where N is the string representation of i.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.get-marker-string-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.get-marker-string-fn]
fn getMarkerString(i: u32) -> String {
    let oss: String = i.to_string();
    String::from("@") + &oss + &String::from("@")
}

// Return the number representation of N in string "@N@".
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.get-marker-number-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.get-marker-number-fn]
fn getMarkerNumber(str: &str) -> u32 {
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
pub fn parallelBracketedReplace(ruleVector: &Vec<Rule>, optional: bool) -> HfstTransducer {
    // For each parallel rule, we need to concatenate a special marker symbol
    // to its output side. This is needed so that overlapping mappings with
    // different weights and contexts are kept separate. If we have N rules,
    // we need marker symbols "@1@", "@2@", ... , "@N@" ("@0@" is reserved
    // for epsilon symbol). At the end, we must substitute any marker symbols
    // with epsilons.

    let mut marker_symbols: StringSet = StringSet::new(); // "@1@", "@2@", ... , "@N@"
    let mut marker_substitutions: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
    for i in 0..ruleVector.len() {
        let marker_string = getMarkerString((i + 1) as u32);
        marker_symbols.insert(marker_string.clone());
        marker_substitutions.insert(marker_string.clone(), internal_epsilon.to_string());
    }

    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let leftMarker: String = "@LM@".to_string();
    let rightMarker: String = "@RM@".to_string();

    let leftMarker2: String = "@LM2@".to_string();
    let rightMarker2: String = "@RM2@".to_string();

    let tmpMarker: String = "@TMPM@".to_string();
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);
    TOK.add_multichar_symbol(&leftMarker2);
    TOK.add_multichar_symbol(&rightMarker2);
    TOK.add_multichar_symbol(&tmpMarker);
    TOK.add_multichar_symbol(".#.");

    let type_ = ruleVector[0].get_mapping()[0].0.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);
    let leftBracket2 = HfstTransducer::new_tokenized(&leftMarker2, &TOK, type_);
    let rightBracket2 = HfstTransducer::new_tokenized(&rightMarker2, &TOK, type_);
    let tmpBracket = HfstTransducer::new_tokenized(&tmpMarker, &TOK, type_);

    // Identity pair (unknowns/identities must not be expanded to marker
    // symbols)
    let mut identityPair = HfstTransducer::identity_pair(type_);
    identityPair.insert_to_alphabet_set(&marker_symbols);

    let mut identity = identityPair.clone();
    // unknowns/identities must not be expanded to marker symbols
    identity.insert_to_alphabet_set(&marker_symbols);
    identity.repeat_star().optimize();

    let mut identityExpanded = identityPair.clone();
    identityExpanded.insert_to_alphabet_symbol(&leftMarker);
    identityExpanded.insert_to_alphabet_symbol(&rightMarker);
    identityExpanded.insert_to_alphabet_symbol(&leftMarker2);
    identityExpanded.insert_to_alphabet_symbol(&rightMarker2);
    identityExpanded.insert_to_alphabet_symbol(&tmpMarker);
    identityExpanded.insert_to_alphabet_set(&marker_symbols);
    // will be expanded with mappings

    // for removing .#. from the center
    let mut identityWithoutBoundary = identity.clone();
    identityWithoutBoundary.insert_to_alphabet_symbol(".#.");
    // (must not be expanded to marker symbols)
    identityWithoutBoundary.insert_to_alphabet_set(&marker_symbols);
    let mut removeHash = identityWithoutBoundary.clone();
    let boundary = HfstTransducer::new_tokenized(".#.", &TOK, type_);
    removeHash
        .concatenate(&boundary, true)
        .concatenate(&identityWithoutBoundary, true)
        .optimize();

    let mut mappingWithBracketsVector: HfstTransducerVector = Vec::new();
    let mut noContexts = true;

    // go through vector and do everything for each rule
    for i in 0..ruleVector.len() {
        let mut ruletmp = ruleVector[i].clone();
        ruletmp.encodeFlags();

        let mappingPairVector = ruletmp.get_mapping();
        let mut mapping = HfstTransducer::new_type(type_);
        for j in 0..mappingPairVector.len() {
            // i+1 because @0@ is epsilon..
            let marker_string = getMarkerString((i + 1) as u32);
            let marker = HfstTransducer::new_symbol(&marker_string, type_);
            let mut oneMappingPair = mappingPairVector[j].0.clone();
            // unknowns/identities must not be expanded to marker symbols
            oneMappingPair.insert_to_alphabet_set(&marker_symbols);
            let mut foo = mappingPairVector[j].1.clone();
            foo.insert_to_alphabet_set(&marker_symbols);
            oneMappingPair.cross_product(foo.concatenate(&marker, true), true);

            if j == 0 {
                // remove .#. from the center
                // center - (?* .#. ?*)
                oneMappingPair.subtract(&removeHash, false).optimize();
                oneMappingPair.remove_from_alphabet_symbol(".#.");
                mapping = oneMappingPair;
            } else {
                oneMappingPair.subtract(&removeHash, false).optimize();
                oneMappingPair.remove_from_alphabet_symbol(".#.");
                mapping.disjunct(&oneMappingPair, true).optimize();
            }
        }

        let contextVector = ruletmp.get_context();

        // when there aren't any contexts, result is identityExpanded
        if contextVector.len() == 1 {
            let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
            if !(contextVector[0].0.compare(&epsilon, true)
                && contextVector[0].1.compare(&epsilon, true))
            {
                noContexts = false;
            }
        }

        //////////////////////////////////////////////////////////////////
        // In case of ? -> x replacement
        // If left side is empty, return identity transducer
        // If right side is empty, return identity transducer
        //    with alphabet from the left side
        let empty = HfstTransducer::new_type(type_);

        if mapping.compare(&empty, true) {
            mapping = identity.clone();
            if mappingPairVector[0].1.compare(&empty, true) {
                let transducerAlphabet = mappingPairVector[0].0.get_alphabet();
                for s in transducerAlphabet.iter() {
                    mapping.insert_to_alphabet_symbol(s);
                }
            }
        }
        //////////////////////////////////////////////////////////////////

        mapping.insert_to_alphabet_symbol(&leftMarker);
        mapping.insert_to_alphabet_symbol(&rightMarker);
        mapping.insert_to_alphabet_symbol(&tmpMarker);

        // Surround mapping with brackets
        let mut mappingWithBrackets = leftBracket.clone();
        mappingWithBrackets
            .concatenate(&mapping, true)
            .concatenate(&rightBracket, true)
            .optimize();

        // non - optional
        // mapping = <a:b> u <2a:a>2
        if optional != true {
            // needed in case of ? -> x replacement
            mapping.insert_to_alphabet_symbol(&leftMarker2);
            mapping.insert_to_alphabet_symbol(&rightMarker2);
            mappingWithBrackets.insert_to_alphabet_symbol(&leftMarker2);
            mappingWithBrackets.insert_to_alphabet_symbol(&rightMarker2);

            let mut mappingProject = mapping.clone();
            mappingProject.input_project().optimize();

            let mut mappingWithBracketsNonOptional = leftBracket2.clone();

            mappingWithBracketsNonOptional
                .concatenate(&mappingProject, true)
                .concatenate(&rightBracket2, true)
                .optimize();
            // mappingWithBrackets...... expanded
            mappingWithBrackets
                .disjunct(&mappingWithBracketsNonOptional, true)
                .optimize();
        }

        identityExpanded
            .disjunct(&mappingWithBrackets, true)
            .optimize();
        mappingWithBracketsVector.push(mappingWithBrackets);
    }

    identityExpanded.repeat_star().optimize();

    // if none of the rules have contexts, return identityExpanded
    if noContexts {
        identityExpanded.remove_from_alphabet_symbol(&tmpMarker);
        // substitute markers with epsilons
        identityExpanded.substitute_symbols(&marker_substitutions);
        identityExpanded.remove_from_alphabet_set(&marker_symbols);
        return identityExpanded;
    }

    // if they have contexts, process them
    if ruleVector.len() != mappingWithBracketsVector.len() {
        crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "Vector sizes don't match");
    }

    let contextReplaceMap: std::collections::BTreeMap<
        String,
        crate::hfst_basic_transducer::HfstBasicTransducer,
    > = std::collections::BTreeMap::new();
    let _ = &contextReplaceMap;

    let mut unionContextReplace = HfstTransducer::new_type(type_);
    let mut bracketedReplace = HfstTransducer::new_type(type_);
    for i in 0..ruleVector.len() {
        let mut ruletmp = ruleVector[i].clone();
        ruletmp.encodeFlags();

        // Surround mapping with brackets with tmp boudaries
        let mut mappingWithBracketsAndTmpBoundary = tmpBracket.clone();
        mappingWithBracketsAndTmpBoundary
            .concatenate(&mappingWithBracketsVector[i], true)
            .concatenate(&tmpBracket, true)
            .optimize();
        // .* |<a:b>| :*
        let mut bracketedReplaceTmp = identityExpanded.clone();
        bracketedReplaceTmp
            .concatenate(&mappingWithBracketsAndTmpBoundary, true)
            .concatenate(&identityExpanded, true)
            .optimize();

        bracketedReplaceTmp.transform_weights(zero_weight);
        bracketedReplace
            .disjunct(&bracketedReplaceTmp, true)
            .optimize();

        //Create context part
        let mut unionContextReplaceTmp = HfstTransducer::new_type(type_);

        // For each context that uses the output side (REPL_DOWN,
        // REPL_LEFT, REPL_RIGHT) we must freely allow all markers that can
        // be generated by other rules.
        let mut cont = ruletmp.get_context();

        if ruletmp.get_replType() != ReplaceType::REPL_UP {
            for cont_it in cont.iter_mut() {
                for sit in marker_symbols.iter() {
                    if getMarkerNumber(sit) != i as u32 {
                        let marker_pair = (sit.clone(), sit.clone());
                        // 'false' makes sure harmonization is not done
                        cont_it.0.insert_freely_pair(&marker_pair, false);
                        cont_it.1.insert_freely_pair(&marker_pair, false);
                    }
                }
            }
        }

        unionContextReplaceTmp = expandContextsWithMapping(
            &cont,
            &mappingWithBracketsAndTmpBoundary,
            &identityExpanded,
            ruletmp.get_replType(),
            optional,
        );

        unionContextReplaceTmp.transform_weights(zero_weight);

        unionContextReplace
            .disjunct(&unionContextReplaceTmp, true)
            .optimize();
    }

    // subtract all mappings in contexts from replace without contexts
    let mut replaceWithoutContexts = bracketedReplace.clone();
    replaceWithoutContexts
        .subtract(&unionContextReplace, true)
        .optimize();

    // remove tmpMaprker
    replaceWithoutContexts
        .substitute_symbol_pair(
            &(tmpMarker.clone(), tmpMarker.clone()),
            &(
                "@_EPSILON_SYMBOL_@".to_string(),
                "@_EPSILON_SYMBOL_@".to_string(),
            ),
        )
        .optimize();
    replaceWithoutContexts.remove_from_alphabet_symbol(&tmpMarker);
    replaceWithoutContexts.optimize();

    identityExpanded.remove_from_alphabet_symbol(&tmpMarker);

    // final negation
    let mut uncondidtionalTr = identityExpanded.clone();
    uncondidtionalTr
        .subtract(&replaceWithoutContexts, true)
        .optimize();

    // substitute markers with epsilons
    uncondidtionalTr.substitute_symbols(&marker_substitutions);
    uncondidtionalTr.remove_from_alphabet_set(&marker_symbols);

    uncondidtionalTr
}

//---------------------------------
//    CONSTRAINTS
//---------------------------------

// (help function)
// returns: [ B:0 | 0:B | ?-B ]*
// which is used in some constraints
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.constraints-right-part-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.constraints-right-part-fn]
pub fn constraintsRightPart(type_: ImplementationType) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker: String = "@LM@".to_string();
    let rightMarker: String = "@RM@".to_string();
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    // Identity pair (normal)
    let identityPair = HfstTransducer::identity_pair(type_);

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    // Create Right Part
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();

    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
    let epsilonToLeftMark =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &leftMarker, &TOK, type_);
    let LeftMarkToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let _ = (&epsilonToLeftMark, &LeftMarkToEpsilon);

    let mut epsilonToBrackets = epsilon.clone();
    epsilonToBrackets.cross_product(&B, true);

    let mut bracketsToEpsilon = B.clone();
    bracketsToEpsilon.cross_product(&epsilon, true);

    let mut identityPairMinusBrackets = identityPair.clone();
    identityPairMinusBrackets.subtract(&B, true).optimize(); //.repeat_plus().optimize();

    let mut rightPart = epsilonToBrackets.clone();
    rightPart
        .disjunct(&bracketsToEpsilon, true)
        .disjunct(&identityPairMinusBrackets, true)
        .optimize()
        .repeat_star()
        .optimize();

    rightPart
}

// .#. ?* <:0 0:> ?* .#.
// filters out empty string
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.one-betterthan-none-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.one-betterthan-none-constraint-fn]
pub fn oneBetterthanNoneConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let type_ = uncondidtionalTr.get_type();
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    TOK.add_multichar_symbol(".#.");

    let leftMarker: String = "@LM@".to_string();
    let rightMarker: String = "@RM@".to_string();
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    // Identity (normal)
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    let leftBracketToZero =
        HfstTransducer::new_tokenized_pair(&leftMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let rightBracketToZero =
        HfstTransducer::new_tokenized_pair(&rightMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);

    let boundary = HfstTransducer::new_tokenized(".#.", &TOK, type_);
    let mut Constraint = boundary.clone();
    Constraint.concatenate(&identity, true);
    Constraint
        .concatenate(&leftBracketToZero, true)
        .concatenate(&rightBracketToZero, true)
        .concatenate(&boundary, true)
        .concatenate(&identity, true)
        .optimize();

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let retval = constraintComposition(uncondidtionalTr, &Constraint);

    retval
}

// .#. ?* <:0 [B:0]* [I-B] [ B:0 | 0:B | ?-B ]* .#.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.left-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.left-most-constraint-fn]
pub fn leftMostConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    TOK.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    TOK.add_multichar_symbol(".#.");

    let leftMarker: String = "@LM@".to_string();
    let rightMarker: String = "@RM@".to_string();
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    let type_ = uncondidtionalTr.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    // Identity (normal)
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let rightPart = constraintsRightPart(type_);

    // epsilon
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
    // B
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();
    // (B:0)*

    let mut bracketsToEpsilonStar = B.clone();
    bracketsToEpsilonStar
        .cross_product(&epsilon, true)
        .optimize()
        .repeat_star()
        .optimize();

    // (I-B) and (I-B)+
    let mut identityPairMinusBrackets = identityPair.clone();
    identityPairMinusBrackets.subtract(&B, true).optimize();

    let mut identityPairMinusBracketsPlus = identityPairMinusBrackets.clone();
    identityPairMinusBracketsPlus.repeat_plus().optimize();

    let LeftBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);

    let boundary = HfstTransducer::new_tokenized(".#.", &TOK, type_);

    let mut Constraint = boundary.clone();
    Constraint.concatenate(&identity, true);

    // ?* <:0 [B:0]* [I-B] [ B:0 | 0:B | ?-B ]*
    Constraint
        .concatenate(&LeftBracketToEpsilon, true)
        .concatenate(&bracketsToEpsilonStar, true)
        .concatenate(&identityPairMinusBrackets, true)
        .concatenate(&rightPart, true)
        .optimize();

    Constraint.concatenate(&boundary, true).optimize();

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let retval = constraintComposition(uncondidtionalTr, &Constraint);

    retval
}

// [ B:0 | 0:B | ?-B ]* [I-B]+  >:0 [ ?-B ]*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.right-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.right-most-constraint-fn]
pub fn rightMostConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    TOK.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");

    let leftMarker: String = "@LM@".to_string();
    let rightMarker: String = "@RM@".to_string();
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    let type_ = uncondidtionalTr.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    // Identity (normal)
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let rightPart = constraintsRightPart(type_);

    // epsilon
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
    // B
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();
    // (B:0)*
    let mut bracketsToEpsilonStar = B.clone();
    bracketsToEpsilonStar
        .cross_product(&epsilon, true)
        .optimize()
        .repeat_star()
        .optimize();

    // (I-B) and (I-B)+
    let mut identityPairMinusBrackets = identityPair.clone();
    identityPairMinusBrackets.subtract(&B, true).optimize();

    let mut identityPairMinusBracketsPlus = identityPairMinusBrackets.clone();
    identityPairMinusBracketsPlus.repeat_plus().optimize();

    let mut identityPairMinusBracketsStar = identityPairMinusBrackets.clone();
    identityPairMinusBracketsStar.repeat_star().optimize();

    let RightBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);

    let mut Constraint = rightPart.clone();
    // [ B:0 | 0:B | ?-B ]* [I-B]+  >:0 [ ?-B ]*

    Constraint
        .concatenate(&identityPairMinusBracketsPlus, true)
        .concatenate(&RightBracketToEpsilon, true)
        .concatenate(&identity, true)
        .optimize();

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let retval = constraintComposition(uncondidtionalTr, &Constraint);

    retval
}
// (unwrapped mod xerox_helpers_b {)

use std::collections::BTreeSet;

// Longest match
// it should be composed to left most transducer........
// ?* < [?-B]+ 0:> [ ? | 0:< | <:0 | 0:> | B ] [ B:0 | 0:B | ?-B ]*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.longest-match-left-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.longest-match-left-most-constraint-fn]
pub fn longestMatchLeftMostConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker = String::from("@LM@");
    let rightMarker = String::from("@RM@");
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    let type_: ImplementationType = uncondidtionalTr.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    // Identity
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    // epsilon
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
    // B
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();
    // (B:0)*
    let mut bracketsToEpsilonStar = B.clone();
    bracketsToEpsilonStar
        .cross_product(&epsilon, true)
        .optimize()
        .repeat_star()
        .optimize();

    // (I-B) and (I-B)+
    let mut identityPairMinusBrackets = identityPair.clone();
    identityPairMinusBrackets.subtract(&B, true).optimize();

    let mut identityPairMinusBracketsPlus = identityPairMinusBrackets.clone();
    identityPairMinusBracketsPlus.repeat_plus().optimize();

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let mut rightPart = HfstTransducer::new_type(type_);
    rightPart = constraintsRightPart(type_);

    let RightBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let epsilonToRightBracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &rightMarker, &TOK, type_);
    let LeftBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let epsilonToLeftBracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &leftMarker, &TOK, type_);

    //[ ? | 0:< | <:0 | 0:> | B ]
    //     HfstTransducer nonClosingBracketInsertion(identityPair);
    let mut nonClosingBracketInsertion = epsilonToLeftBracket.clone();
    nonClosingBracketInsertion
        //disjunct(epsilonToLeftBracket).
        .disjunct(&LeftBracketToEpsilon, true)
        .disjunct(&epsilonToRightBracket, true)
        .disjunct(&B, true)
        .optimize();
    //    printf("nonClosingBracketInsertion: \n");
    //    nonClosingBracketInsertion.write_in_att_format(stdout, 1);

    nonClosingBracketInsertion
        .concatenate(&identityPairMinusBracketsPlus, true)
        .optimize();

    let mut middlePart = identityPairMinusBrackets.clone();
    middlePart
        .disjunct(&nonClosingBracketInsertion, true)
        .optimize();

    // ?* < [?-B]+ 0:> [ ? | 0:< | <:0 | 0:> | B ] [?-B]+ [ B:0 | 0:B | ?-B ]*
    let mut Constraint = identity.clone();
    Constraint
        .concatenate(&leftBracket, true)
        .concatenate(&identityPairMinusBracketsPlus, true)
        .concatenate(&epsilonToRightBracket, true)
        //    concatenate(nonClosingBracketInsertion).
        //    concatenate(identityPairMinusBracketsPlus).
        .concatenate(&middlePart, true)
        .concatenate(&rightPart, true)
        .optimize();
    //printf("Constraint Longest Match: \n");
    //Constraint.write_in_att_format(stdout, 1);

    //uncondidtionalTr should be left most for the left most longest match
    let mut retval = HfstTransducer::new_type(type_);
    retval = constraintComposition(uncondidtionalTr, &Constraint);

    retval
}

// Longest match RIGHT most
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.longest-match-right-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.longest-match-right-most-constraint-fn]
pub fn longestMatchRightMostConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker = String::from("@LM@");
    let rightMarker = String::from("@RM@");
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    let type_: ImplementationType = uncondidtionalTr.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    // Identity
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    // epsilon
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
    // B
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();
    // (B:0)*
    let mut bracketsToEpsilonStar = B.clone();
    bracketsToEpsilonStar
        .cross_product(&epsilon, true)
        .optimize()
        .repeat_star()
        .optimize();

    // (I-B) and (I-B)+
    let mut identityPairMinusBrackets = identityPair.clone();
    identityPairMinusBrackets.subtract(&B, true).optimize();

    let mut identityPairMinusBracketsPlus = identityPairMinusBrackets.clone();
    identityPairMinusBracketsPlus.repeat_plus().optimize();

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let mut rightPart = HfstTransducer::new_type(type_);
    rightPart = constraintsRightPart(type_);

    let RightBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);

    let epsilonToRightBracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &rightMarker, &TOK, type_);
    let LeftBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let epsilonToLeftBracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &leftMarker, &TOK, type_);

    //[ ? | 0:< | >:0 | 0:> | B ]
    let mut nonClosingBracketInsertion = identityPair.clone();
    nonClosingBracketInsertion
        .disjunct(&epsilonToLeftBracket, true)
        .disjunct(&RightBracketToEpsilon, true)
        .disjunct(&epsilonToRightBracket, true)
        .disjunct(&B, true)
        .optimize();

    // [ B:0 | 0:B | ?-B ]* [?-B]+ [ ? | 0:< | <:0 | 0:> | B ] 0:< [?-B]+ > ?*

    let mut Constraint = rightPart.clone();
    Constraint
        .concatenate(&identityPairMinusBracketsPlus, true)
        .concatenate(&nonClosingBracketInsertion, true)
        .optimize()
        .concatenate(&epsilonToLeftBracket, true)
        .concatenate(&identityPairMinusBracketsPlus, true)
        .concatenate(&rightBracket, true)
        .concatenate(&identity, true)
        .optimize();
    //printf("Constraint Longest Match: \n");
    //Constraint.write_in_att_format(stdout, 1);

    //uncondidtionalTr should be left most for the left most longest match
    let mut retval = HfstTransducer::new_type(type_);
    retval = constraintComposition(uncondidtionalTr, &Constraint);

    retval
}

// Shortest match
// it should be composed to left most transducer........
// ?* < [?-B]+ >:0
// [?-B] or [ ? | 0:< | <:0 | >:0 | B ][?-B]+
// [ B:0 | 0:B | ?-B ]*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.shortest-match-left-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.shortest-match-left-most-constraint-fn]
pub fn shortestMatchLeftMostConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker = String::from("@LM@");
    let rightMarker = String::from("@RM@");
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    let type_: ImplementationType = uncondidtionalTr.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    // Identity
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let mut rightPart = HfstTransducer::new_type(type_);
    rightPart = constraintsRightPart(type_);

    // [?-B] and [?-B]+
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();
    let mut identityPairMinusBrackets = identityPair.clone();
    identityPairMinusBrackets.subtract(&B, true).optimize();
    let mut identityPairMinusBracketsPlus = identityPairMinusBrackets.clone();
    identityPairMinusBracketsPlus.repeat_plus().optimize();

    let RightBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let epsilonToRightBracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &rightMarker, &TOK, type_);
    let LeftBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let epsilonToLeftBracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &leftMarker, &TOK, type_);

    // [ 0:< | <:0 | >:0 | B ][?-B]+
    let mut nonClosingBracketInsertion = epsilonToLeftBracket.clone();
    nonClosingBracketInsertion
        //disjunct(epsilonToLeftBracket).
        .disjunct(&LeftBracketToEpsilon, true)
        .disjunct(&RightBracketToEpsilon, true)
        .disjunct(&B, true)
        .optimize();

    nonClosingBracketInsertion
        .concatenate(&identityPairMinusBracketsPlus, true)
        .optimize();

    let mut middlePart = identityPairMinusBrackets.clone();
    middlePart
        .disjunct(&nonClosingBracketInsertion, true)
        .optimize();

    //    printf("nonClosingBracketInsertion: \n");
    //    nonClosingBracketInsertion.write_in_att_format(stdout, 1);

    // ?* < [?-B]+ >:0
    // [?-B] or [ ? | 0:< | <:0 | >:0 | B ][?-B]+
    //[ B:0 | 0:B | ?-B ]*
    let mut Constraint = identity.clone();
    Constraint
        .concatenate(&leftBracket, true)
        .concatenate(&identityPairMinusBracketsPlus, true)
        .concatenate(&RightBracketToEpsilon, true)
        .concatenate(&middlePart, true)
        .optimize()
        .concatenate(&rightPart, true)
        .optimize();

    //printf("Constraint Shortest Match: \n");
    //Constraint.write_in_att_format(stdout, 1);

    //uncondidtionalTr should be left most for the left most shortest match
    let mut retval = HfstTransducer::new_type(type_);
    retval = constraintComposition(uncondidtionalTr, &Constraint);

    retval
}

// Shortest match
// it should be composed to left most transducer........
//[ B:0 | 0:B | ?-B ]*
// [?-B] or [?-B]+  [ ? | 0:> | >:0 | <:0 | B ]
// <:0 [?-B]+   > ?*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.shortest-match-right-most-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.shortest-match-right-most-constraint-fn]
pub fn shortestMatchRightMostConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker = String::from("@LM@");
    let rightMarker = String::from("@RM@");
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    let type_: ImplementationType = uncondidtionalTr.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    // Identity
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    // Create Right Part:  [ B:0 | 0:B | ?-B ]*
    let mut rightPart = HfstTransducer::new_type(type_);
    rightPart = constraintsRightPart(type_);

    // [?-B] and [?-B]+
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();
    let mut identityPairMinusBrackets = identityPair.clone();
    identityPairMinusBrackets.subtract(&B, true).optimize();
    let mut identityPairMinusBracketsPlus = identityPairMinusBrackets.clone();
    identityPairMinusBracketsPlus.repeat_plus().optimize();

    let RightBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let epsilonToRightBracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &rightMarker, &TOK, type_);
    let LeftBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let epsilonToLeftBracket =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &leftMarker, &TOK, type_);

    // [?-B]+ [ 0:> | >:0 | <:0 | B ]
    let mut nonClosingBracketInsertionTmp = epsilonToRightBracket.clone();
    nonClosingBracketInsertionTmp
        .disjunct(&RightBracketToEpsilon, true)
        .disjunct(&LeftBracketToEpsilon, true)
        .disjunct(&B, true)
        .optimize();
    let mut nonClosingBracketInsertion = identityPairMinusBracketsPlus.clone();
    nonClosingBracketInsertion
        .concatenate(&nonClosingBracketInsertionTmp, true)
        .optimize();

    let mut middlePart = identityPairMinusBrackets.clone();
    middlePart
        .disjunct(&nonClosingBracketInsertion, true)
        .optimize();

    //[ B:0 | 0:B | ?-B ]*
    // [?-B] or [?-B]+  [ ? | 0:> | >:0 | <:0 | B ]
    // <:0 [?-B]+   > ?*

    let mut Constraint = rightPart.clone();
    Constraint
        .concatenate(&middlePart, true)
        .concatenate(&LeftBracketToEpsilon, true)
        .concatenate(&identityPairMinusBracketsPlus, true)
        .concatenate(&rightBracket, true)
        .concatenate(&identity, true)
        .optimize();

    //printf("Constraint Shortest Match: \n");
    //Constraint.write_in_att_format(stdout, 1);

    //uncondidtionalTr should be left most for the left most longest match
    let mut retval = HfstTransducer::new_type(type_);
    retval = constraintComposition(uncondidtionalTr, &Constraint);

    retval
}

// ?* [ BL:0 (?-B)+ BR:0 ?* ]+
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.most-brackets-plus-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.most-brackets-plus-constraint-fn]
pub fn mostBracketsPlusConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker = String::from("@LM@");
    let rightMarker = String::from("@RM@");
    let leftMarker2 = String::from("@LM2@");
    let rightMarker2 = String::from("@RM2@");

    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);
    TOK.add_multichar_symbol(&leftMarker2);
    TOK.add_multichar_symbol(&rightMarker2);

    let type_: ImplementationType = uncondidtionalTr.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);
    let leftBracket2 = HfstTransducer::new_tokenized(&leftMarker2, &TOK, type_);
    let rightBracket2 = HfstTransducer::new_tokenized(&rightMarker2, &TOK, type_);

    // Identity (normal)
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    let mut identityPlus = identityPair.clone();
    identityPlus.repeat_plus().optimize();

    let mut identityStar = identityPair.clone();
    identityStar.repeat_star().optimize();

    // epsilon
    let epsilon = String::from("@_EPSILON_SYMBOL_@");

    // BL:0 ( <1 : 0, <2 : 0)
    let leftBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker, &epsilon, &TOK, type_);
    let leftBracket2ToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker2, &epsilon, &TOK, type_);
    let mut allLeftBracketsToEpsilon = leftBracketToEpsilon.clone();
    allLeftBracketsToEpsilon
        .disjunct(&leftBracket2ToEpsilon, true)
        .optimize();

    //    printf("allLeftBracketsToEpsilon: \n");
    //    allLeftBracketsToEpsilon.write_in_att_format(stdout, 1);

    // BR:0 ( >1 : 0, >2 : 0)
    let rightBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker, &epsilon, &TOK, type_);
    let rightBracket2ToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker2, &epsilon, &TOK, type_);
    let mut allRightBracketsToEpsilon = rightBracketToEpsilon.clone();
    allRightBracketsToEpsilon
        .disjunct(&rightBracket2ToEpsilon, true)
        .optimize();

    // B (B1 | B2)
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();
    B.disjunct(&leftBracket2, true).optimize();
    B.disjunct(&rightBracket2, true).optimize();

    // (? - B)+
    let mut identityPairMinusBracketsPlus = identityPair.clone();
    identityPairMinusBracketsPlus
        .subtract(&B, true)
        .optimize()
        .repeat_plus()
        .optimize();

    // repeatingPart ( BL:0 (?-B)+ BR:0 ?* )+
    let mut repeatingPart = allLeftBracketsToEpsilon.clone();
    repeatingPart
        .concatenate(&identityPairMinusBracketsPlus, true)
        .optimize();
    repeatingPart
        .concatenate(&allRightBracketsToEpsilon, true)
        .optimize();
    repeatingPart.concatenate(&identityStar, true).optimize();
    repeatingPart.repeat_plus().optimize();
    //printf("middlePart: \n");
    //middlePart.write_in_att_format(stdout, 1);

    let mut Constraint = identityStar.clone();
    Constraint.concatenate(&repeatingPart, true).optimize();
    //printf("Constraint: \n");
    //Constraint.write_in_att_format(stdout, 1);

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let mut retval = HfstTransducer::new_type(type_);
    retval = constraintComposition(uncondidtionalTr, &Constraint);

    //printf("After composition: \n");
    //retval.write_in_att_format(stdout, 1);

    retval
}

// ?* [ BL:0 (?-B)* BR:0 ?* ]+
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.most-brackets-star-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.most-brackets-star-constraint-fn]
pub fn mostBracketsStarConstraint(uncondidtionalTr: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker = String::from("@LM@");
    let rightMarker = String::from("@RM@");
    let leftMarker2 = String::from("@LM2@");
    let rightMarker2 = String::from("@RM2@");
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);
    TOK.add_multichar_symbol(&leftMarker2);
    TOK.add_multichar_symbol(&rightMarker2);

    let type_: ImplementationType = uncondidtionalTr.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    let leftBracket2 = HfstTransducer::new_tokenized(&leftMarker2, &TOK, type_);
    let rightBracket2 = HfstTransducer::new_tokenized(&rightMarker2, &TOK, type_);

    // Identity (normal)
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    let mut identityPlus = identityPair.clone();
    identityPlus.repeat_plus().optimize();

    let mut identityStar = identityPair.clone();
    identityStar.repeat_star().optimize();

    // epsilon
    let epsilon = String::from("@_EPSILON_SYMBOL_@");

    // BL:0 ( <1 : 0, <2 : 0)
    let leftBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker, &epsilon, &TOK, type_);
    let leftBracket2ToEpsilon =
        HfstTransducer::new_tokenized_pair(&leftMarker2, &epsilon, &TOK, type_);
    let mut allLeftBracketsToEpsilon = leftBracketToEpsilon.clone();
    allLeftBracketsToEpsilon
        .disjunct(&leftBracket2ToEpsilon, true)
        .optimize();

    //    printf("allLeftBracketsToEpsilon: \n");
    //    allLeftBracketsToEpsilon.write_in_att_format(stdout, 1);

    // BR:0 ( >1 : 0, >2 : 0)
    let rightBracketToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker, &epsilon, &TOK, type_);
    let rightBracket2ToEpsilon =
        HfstTransducer::new_tokenized_pair(&rightMarker2, &epsilon, &TOK, type_);
    let mut allRightBracketsToEpsilon = rightBracketToEpsilon.clone();
    allRightBracketsToEpsilon
        .disjunct(&rightBracket2ToEpsilon, true)
        .optimize();

    // B (B1 | B2)
    let mut B = leftBracket.clone();
    B.disjunct(&rightBracket, true).optimize();
    B.disjunct(&leftBracket2, true).optimize();
    B.disjunct(&rightBracket2, true).optimize();

    // (? - B)*
    let mut identityPairMinusBracketsStar = identityPair.clone();
    identityPairMinusBracketsStar
        .subtract(&B, true)
        .optimize()
        .repeat_star()
        .optimize();

    // repeatingPart [ BL:0 (?-B)* BR:0 ?* ]+
    let mut repeatingPart = allLeftBracketsToEpsilon.clone();
    repeatingPart
        .concatenate(&identityPairMinusBracketsStar, true)
        .optimize();
    repeatingPart
        .concatenate(&allRightBracketsToEpsilon, true)
        .optimize();
    repeatingPart.concatenate(&identityStar, true).optimize();
    repeatingPart.repeat_plus().optimize();
    //printf("middlePart: \n");
    //repeatingPart.write_in_att_format(stdout, 1);

    let mut Constraint = identityStar.clone();
    Constraint.concatenate(&repeatingPart, true).optimize();
    //printf("Constraint: \n");
    //Constraint.write_in_att_format(stdout, 1);

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t
    let mut retval = HfstTransducer::new_type(type_);
    retval = constraintComposition(uncondidtionalTr, &Constraint);

    //printf("After composition: \n");
    //retval.write_in_att_format(stdout, 1);
    retval
}

// ?* B2 ?*
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.remove-b2-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.remove-b2-constraint-fn]
pub fn removeB2Constraint(t: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker2 = String::from("@LM2@");
    let rightMarker2 = String::from("@RM2@");

    TOK.add_multichar_symbol(&leftMarker2);
    TOK.add_multichar_symbol(&rightMarker2);

    let type_: ImplementationType = t.get_type();

    let leftBracket2 = HfstTransducer::new_tokenized(&leftMarker2, &TOK, type_);
    let rightBracket2 = HfstTransducer::new_tokenized(&rightMarker2, &TOK, type_);

    // Identity (normal)
    let identityPair = HfstTransducer::identity_pair(type_);
    let mut identity = identityPair.clone();
    identity.repeat_star().optimize();

    let mut identityStar = identityPair.clone();
    identityStar.repeat_star().optimize();

    // B (B2)
    let mut B = leftBracket2.clone();
    B.disjunct(&rightBracket2, true).optimize();

    let mut Constraint = identityStar.clone();
    Constraint.concatenate(&B, true).optimize();
    Constraint.concatenate(&identityStar, true).optimize();

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    let mut retval = HfstTransducer::new_type(type_);
    retval = constraintComposition(t, &Constraint);

    retval.remove_from_alphabet(&leftMarker2);
    retval.remove_from_alphabet(&rightMarker2);

    //printf("Remove B2 After composition: \n");
    //retval.write_in_att_format(stdout, 1);

    retval
}

// to avoid repetition in empty replace rule
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.no-repetition-constraint-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.no-repetition-constraint-fn]
pub fn noRepetitionConstraint(t: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let leftMarker = String::from("@LM@");
    let rightMarker = String::from("@RM@");
    TOK.add_multichar_symbol(&leftMarker);
    TOK.add_multichar_symbol(&rightMarker);

    let leftMarker2 = String::from("@LM2@");
    let rightMarker2 = String::from("@RM2@");

    //if the transdcuer is optional, LM2 and RM2 are not there
    let mut optional = true;
    let transducerAlphabet: StringSet = t.get_alphabet();
    for s in transducerAlphabet.iter() {
        let alph = s.clone();
        if alph == leftMarker2 {
            optional = false;
            break;
        }
    }

    TOK.add_multichar_symbol(&leftMarker2);
    TOK.add_multichar_symbol(&rightMarker2);

    let type_: ImplementationType = t.get_type();

    let leftBracket = HfstTransducer::new_tokenized(&leftMarker, &TOK, type_);
    let rightBracket = HfstTransducer::new_tokenized(&rightMarker, &TOK, type_);

    let leftBracket2 = HfstTransducer::new_tokenized(&leftMarker2, &TOK, type_);
    let rightBracket2 = HfstTransducer::new_tokenized(&rightMarker2, &TOK, type_);

    let mut leftBrackets = leftBracket.clone();
    if !optional {
        leftBrackets.disjunct(&leftBracket2, true).optimize();
    }

    let mut rightBrackets = rightBracket.clone();
    if !optional {
        rightBrackets.disjunct(&rightBracket2, true).optimize();
    }
    // Identity (normal)
    let identityPair = HfstTransducer::identity_pair(type_);
    /*
    identityPair.insert_to_alphabet(leftMarker);
    identityPair.insert_to_alphabet(rightMarker);
    identityPair.insert_to_alphabet(leftMarker);
    identityPair.insert_to_alphabet(rightMarker2);
     */

    let mut identityStar = identityPair.clone();
    identityStar.repeat_star().optimize();

    let mut Constraint = identityStar.clone();
    Constraint
        .concatenate(&leftBrackets, true)
        .concatenate(&rightBrackets, true)
        .concatenate(&leftBrackets, true)
        .concatenate(&rightBrackets, true)
        .concatenate(&identityStar, true)
        .optimize();

    //// Compose with unconditional replace transducer
    // tmp = t.1 .o. Constr .o. t.1
    // (t.1 - tmp.2) .o. t

    //printf("...Constraint: \n");
    //Constraint.write_in_att_format(stdout, 1);

    let mut retval = HfstTransducer::new_type(type_);
    retval = constraintComposition(t, &Constraint);

    //    retval = removeB2Constraint(retval);

    retval
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
pub fn applyBoundaryMark(t: &HfstTransducer) -> HfstTransducer {
    let mut TOK = HfstTokenizer::new();
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    TOK.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    TOK.add_multichar_symbol("@TMP_UNKNOWN@");
    let type_: ImplementationType = t.get_type();

    let boundaryMarker = String::from(".#.");
    TOK.add_multichar_symbol(&boundaryMarker);
    let boundary = HfstTransducer::new_tokenized(&boundaryMarker, &TOK, type_);

    let mut identityPair = HfstTransducer::identity_pair(type_);
    identityPair.insert_to_alphabet(&boundaryMarker);
    // ? - .#.
    let mut identityMinusBoundary = identityPair.clone();
    identityMinusBoundary.subtract(&boundary, true).optimize();

    // (? - .#.)*
    let mut identityMinusBoundaryStar = identityMinusBoundary.clone();
    identityMinusBoundaryStar.repeat_star().optimize();

    // .#. (? - .#.)* .#.
    let mut boundaryAnythingBoundary = boundary.clone();
    boundaryAnythingBoundary
        .concatenate(&identityMinusBoundaryStar, true)
        .concatenate(&boundary, true)
        .optimize();

    // [0:.#. | ? - .#.]*
    let zeroToBoundary =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &boundaryMarker, &TOK, type_);
    let mut retval = zeroToBoundary.clone();
    retval
        .disjunct(&identityMinusBoundary, true)
        .optimize()
        .repeat_star()
        .optimize();

    //printf("retval .o. t: \n");
    //retval.write_in_att_format(stdout, 1);
    // [.#.:0 | ? - .#.]*
    let boundaryToZero =
        HfstTransducer::new_tokenized_pair(&boundaryMarker, "@_EPSILON_SYMBOL_@", &TOK, type_);
    let mut removeBoundary = boundaryToZero.clone();
    removeBoundary
        .disjunct(&identityMinusBoundary, true)
        .optimize()
        .repeat_star()
        .optimize();

    // apply boundary to the transducer
    // compose [0:.#. | ? - .#.]* .o. t
    let mut tr = t.clone();

    //tr.insert_to_alphabet(boundaryMarker);
    // substitutute unknowns with tmp symbol
    // this is necessary because of first composition
    tr.substitute("@_UNKNOWN_SYMBOL_@", "@TMP_UNKNOWN@", true, true);

    //printf("----first: ----\n");
    //tr.write_in_att_format(stdout, 1);

    retval.compose(&tr, true).optimize();

    //            printf("first composition: \n");
    //            retval.write_in_att_format(stdout, 1);

    // compose with .#. (? - .#.)* .#.
    retval.compose(&boundaryAnythingBoundary, true).optimize();

    //            printf("2. composition: \n");
    //            retval.write_in_att_format(stdout, 1);

    // compose with [.#.:0 | ? - .#.]*
    retval.compose(&removeBoundary, true).optimize();

    //            printf("3. composition: \n");
    //            retval.write_in_att_format(stdout, 1);

    // bring back unknown symbols
    retval.substitute("@TMP_UNKNOWN@", "@_UNKNOWN_SYMBOL_@", true, true);
    retval.remove_from_alphabet("@TMP_UNKNOWN@");

    // remove boundary from alphabet
    retval.remove_from_alphabet(&boundaryMarker);
    retval
}

//---------------------------------
//    INTERFACE HELPING FUNCTIONS
//---------------------------------

// used by hfst-regexp parser
// creates markup crossproduct and sets property of the first transducer in the mapping to "isMarkup" = "yes"
// the other transducer in the mapping is set to epsilon transducer
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.create-mapping-for-mark-up-replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.create-mapping-for-mark-up-replace-fn]
pub fn create_mapping_for_mark_up_replace(
    mappingPair: &HfstTransducerPair,
    marks: &HfstTransducerPair,
) -> HfstTransducerPair {
    let mut TOK = HfstTokenizer::new();
    let epsilon = String::from("@_EPSILON_SYMBOL_@");
    TOK.add_multichar_symbol(&epsilon);
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let type_: ImplementationType = mappingPair.0.get_type();

    let leftMark = marks.0.clone();
    let rightMark = marks.1.clone();

    let mut epsilonToLeftMark = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &TOK, type_);
    epsilonToLeftMark.cross_product(&leftMark, true).optimize();

    let mut epsilonToRightMark = HfstTransducer::new_tokenized(&epsilon, &TOK, type_);
    epsilonToRightMark
        .cross_product(&rightMark, true)
        .optimize();

    //Go through left part of every mapping pair
    // and concatenate: epsilonToLeftMark.leftMapping.epsilonToRightMark
    //then put it into right part of the new transducerPairVector
    let mut mappingCrossProduct = epsilonToLeftMark.clone();
    mappingCrossProduct
        .concatenate(&mappingPair.0, true)
        .concatenate(&epsilonToRightMark, true)
        .optimize();

    mappingCrossProduct.set_property("isMarkup", "yes");

    let epsilonTr = HfstTransducer::new_tokenized(&epsilon, &TOK, type_);
    let retval: HfstTransducerPair = (mappingCrossProduct, epsilonTr);

    retval
}

// replace up, left, right, down
pub fn replace_rule(rule: &Rule, optional: bool) -> HfstTransducer {
    let mut retval: HfstTransducer = bracketedReplace(rule, optional);

    //printf("---bracketed replace done---: \n");
    //retval.optimize().write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row

    retval = noRepetitionConstraint(&retval);

    //printf("-----noRepetitionConstraint-----: \n");
    //retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol, must be before mostBracketsStarConstraint
    retval = applyBoundaryMark(&retval);

    //printf("----after applyBoundaryMark: ----\n");
    //retval.write_in_att_format(stdout, 1);
    if !optional {
        //printf(" ----------  mostBracketsStarConstraint --------------\n");
        // Epenthesis rules behave differently if used mostBracketsPlusConstraint
        //retval = mostBracketsPlusConstraint(retval);
        retval = mostBracketsStarConstraint(&retval);
        //printf("after non optional: \n");
        //retval.write_in_att_format(stdout, 1);
    }
    retval = removeB2Constraint(&retval);
    retval = removeMarkers(&retval);
    //printf("after removeMarkers: \n");
    //retval.write_in_att_format(stdout, 1);
    retval
}

// for parallel rules
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-fn]
pub fn replace_rule_vector(ruleVector: &Vec<Rule>, optional: bool) -> HfstTransducer {
    // std::cerr << "replace"<< std::endl;

    let mut retval: HfstTransducer = HfstTransducer::new();
    // If there is only one rule in the vector, it is not parallel
    if ruleVector.len() == 1 {
        retval = bracketedReplace(&ruleVector[0], optional);
    } else {
        retval = parallelBracketedReplace(ruleVector, optional);
    }

    //std::cerr << "after bracketed replace"<< std::endl;
    //         printf("- bracketed replace -\n");
    //         retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = noRepetitionConstraint(&retval);

    //   printf("----after noRepetitionConstraint: ----\n");
    //   retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    //printf("----after applyBoundaryMark: ----\n");
    //retval.write_in_att_format(stdout, 1);

    if !optional {
        // Epenthesis rules behave differently if used mostBracketsPlusConstraint
        // retval = mostBracketsPlusConstraint(retval);
        retval = mostBracketsStarConstraint(&retval);
    }

    // printf("----after mostBracketsStarConstraint: ----\n");
    //  retval.write_in_att_format(stdout, 1);

    retval = removeB2Constraint(&retval);

    //printf("----after removeB2Constraint: ----\n");
    // retval.write_in_att_format(stdout, 1);

    retval = removeMarkers(&retval);

    //printf("----after removeMarkers: ----\n");
    //retval.write_in_att_format(stdout, 1);
    retval
}

// replace left
pub fn replace_left_rule(rule: &Rule, optional: bool) -> HfstTransducer {
    let mappingPairVector: HfstTransducerPairVector = rule.get_mapping();
    //HfstTransducer newMapping = rule.get_mapping();
    //newMapping.invert().optimize();

    let mut newMappingPairVector: HfstTransducerPairVector = HfstTransducerPairVector::new();
    for i in 0..mappingPairVector.len() {
        // in every mapping pair invert first and second
        //HfstTransducer newMapping = rule.get_mapping();
        let first: HfstTransducer = mappingPairVector[i].0.clone();
        let second: HfstTransducer = mappingPairVector[i].1.clone();
        newMappingPairVector.push((second, first));
    }

    let newRule: Rule = Rule::new_mapping_context_repl_type(
        &newMappingPairVector,
        &rule.get_context(),
        rule.get_replType(),
    );
    let mut retval: HfstTransducer = replace_rule(&newRule, optional);

    retval.invert().optimize();
    retval
}

// replace left parallel
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-left-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-left-fn]
pub fn replace_left_rule_vector(ruleVector: &Vec<Rule>, optional: bool) -> HfstTransducer {
    let mut leftRuleVector: Vec<Rule> = Vec::new();

    for i in 0..ruleVector.len() {
        let mappingPairVector: HfstTransducerPairVector = ruleVector[i].get_mapping();
        //HfstTransducer newMapping = rule.get_mapping();
        //newMapping.invert().optimize();

        let mut newMappingPairVector: HfstTransducerPairVector = HfstTransducerPairVector::new();
        for j in 0..mappingPairVector.len() {
            // in every mapping pair invert first and second
            //HfstTransducer newMapping = rule.get_mapping();
            let first: HfstTransducer = mappingPairVector[j].0.clone();
            let second: HfstTransducer = mappingPairVector[j].1.clone();
            newMappingPairVector.push((second, first));
        }

        let newRule: Rule = Rule::new_mapping_context_repl_type(
            &newMappingPairVector,
            &ruleVector[i].get_context(),
            ruleVector[i].get_replType(),
        );

        leftRuleVector.push(newRule);
    }

    let mut retval: HfstTransducer = replace_rule_vector(&leftRuleVector, optional);
    retval.invert().optimize();

    retval
}

// left to right
pub fn replace_leftmost_longest_match_rule(rule: &Rule) -> HfstTransducer {
    let mut uncondidtionalTr: HfstTransducer = bracketedReplace(rule, true);
    //uncondidtionalTr = bracketedReplace(rule, true);

    //printf("LM uncondidtionalTr: \n");
    //uncondidtionalTr.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    // it should be before leftMostConstraint
    uncondidtionalTr = noRepetitionConstraint(&uncondidtionalTr);

    let mut retval: HfstTransducer = leftMostConstraint(&uncondidtionalTr);

    //to remove empty strings
    retval = oneBetterthanNoneConstraint(&retval);

    // printf("leftMostConstraint: \n");
    // retval.write_in_att_format(stdout, 1);
    retval = longestMatchLeftMostConstraint(&retval);

    //printf("longestMatchLeftMostConstraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = removeB2Constraint(&retval);
    retval = removeMarkers(&retval);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    retval
}

// left to right
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-longest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-longest-match-fn]
pub fn replace_leftmost_longest_match_rule_vector(ruleVector: &Vec<Rule>) -> HfstTransducer {
    //printf("\n replace_leftmost_longest_match \n");

    let mut uncondidtionalTr: HfstTransducer = HfstTransducer::new();
    if ruleVector.len() == 1 {
        uncondidtionalTr = bracketedReplace(&ruleVector[0], true);
    } else {
        uncondidtionalTr = parallelBracketedReplace(ruleVector, true);
    }

    //printf("retval unconditional 1 \n");
    // uncondidtionalTr.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    // it should be before leftMostConstraint
    uncondidtionalTr = noRepetitionConstraint(&uncondidtionalTr);
    //printf("uncondidtionalTr epenthesis \n");
    //uncondidtionalTr.write_in_att_format(stdout, 1);

    let mut retval: HfstTransducer = leftMostConstraint(&uncondidtionalTr);

    //to remove empty strings
    retval = oneBetterthanNoneConstraint(&retval);

    retval = longestMatchLeftMostConstraint(&retval);
    //printf("retval longestMatchLeftMostConstraint \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = removeB2Constraint(&retval);

    //printf("retval removeB2Constraint \n");
    //retval.write_in_att_format(stdout, 1);

    retval = removeMarkers(&retval);

    //       printf("LM removeMarkers: \n");
    //        retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    // printf("LM applyBoundaryMark: \n");
    // retval.write_in_att_format(stdout, 1);

    retval
}

// right to left
pub fn replace_rightmost_longest_match_rule(rule: &Rule) -> HfstTransducer {
    let uncondidtionalTr: HfstTransducer = bracketedReplace(rule, true);
    //uncondidtionalTr = bracketedReplace(rule, true);

    let mut retval: HfstTransducer = rightMostConstraint(&uncondidtionalTr);
    //retval = rightMostConstraint(uncondidtionalTr);

    //printf("rightMostConstraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = longestMatchRightMostConstraint(&retval);

    //printf("longestMatchLeftMostConstraint: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = noRepetitionConstraint(&retval);
    // remove LM2, RM2
    retval = removeB2Constraint(&retval);

    retval = removeMarkers(&retval);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    retval
}

// right to left
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-longest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-longest-match-fn]
pub fn replace_rightmost_longest_match_rule_vector(ruleVector: &Vec<Rule>) -> HfstTransducer {
    let mut uncondidtionalTr: HfstTransducer = HfstTransducer::new();
    if ruleVector.len() == 1 {
        uncondidtionalTr = bracketedReplace(&ruleVector[0], true);
    } else {
        uncondidtionalTr = parallelBracketedReplace(ruleVector, true);
    }

    let mut retval: HfstTransducer = rightMostConstraint(&uncondidtionalTr);
    //retval = rightMostConstraint(uncondidtionalTr);

    //printf("rightMostConstraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = longestMatchRightMostConstraint(&retval);

    //printf("longestMatchLeftMostConstraint: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = noRepetitionConstraint(&retval);
    // remove LM2, RM2
    retval = removeB2Constraint(&retval);

    retval = removeMarkers(&retval);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    retval
}

pub fn replace_leftmost_shortest_match_rule(rule: &Rule) -> HfstTransducer {
    let mut uncondidtionalTr: HfstTransducer = bracketedReplace(rule, true);
    //    uncondidtionalTr = bracketedReplace(rule, true);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    //has to be before leftMostConstraint
    uncondidtionalTr = noRepetitionConstraint(&uncondidtionalTr);

    let mut retval: HfstTransducer = leftMostConstraint(&uncondidtionalTr);
    //to remove empty strings
    retval = oneBetterthanNoneConstraint(&retval);

    retval = shortestMatchLeftMostConstraint(&retval);

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = removeB2Constraint(&retval);

    retval = removeMarkers(&retval);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    retval
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-shortest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-shortest-match-fn]
pub fn replace_leftmost_shortest_match_rule_vector(ruleVector: &Vec<Rule>) -> HfstTransducer {
    let mut uncondidtionalTr: HfstTransducer = HfstTransducer::new();
    if ruleVector.len() == 1 {
        uncondidtionalTr = bracketedReplace(&ruleVector[0], true);
    } else {
        uncondidtionalTr = parallelBracketedReplace(ruleVector, true);
    }

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    uncondidtionalTr = noRepetitionConstraint(&uncondidtionalTr);

    let mut retval: HfstTransducer = leftMostConstraint(&uncondidtionalTr);

    //to remove empty strings
    retval = oneBetterthanNoneConstraint(&retval);

    retval = shortestMatchLeftMostConstraint(&retval);

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = removeB2Constraint(&retval);

    retval = removeMarkers(&retval);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    retval
}

pub fn replace_rightmost_shortest_match_rule(rule: &Rule) -> HfstTransducer {
    let uncondidtionalTr: HfstTransducer = bracketedReplace(rule, true);
    //uncondidtionalTr = bracketedReplace( rule, true);

    let mut retval: HfstTransducer = rightMostConstraint(&uncondidtionalTr);
    //retval = rightMostConstraint(uncondidtionalTr);
    retval = shortestMatchRightMostConstraint(&retval);

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = noRepetitionConstraint(&retval);
    // remove LM2, RM2
    retval = removeB2Constraint(&retval);

    retval = removeMarkers(&retval);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    retval
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-shortest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-shortest-match-fn]
pub fn replace_rightmost_shortest_match_rule_vector(ruleVector: &Vec<Rule>) -> HfstTransducer {
    let mut uncondidtionalTr: HfstTransducer = HfstTransducer::new();
    if ruleVector.len() == 1 {
        uncondidtionalTr = bracketedReplace(&ruleVector[0], true);
    } else {
        uncondidtionalTr = parallelBracketedReplace(ruleVector, true);
    }
    let mut retval: HfstTransducer = rightMostConstraint(&uncondidtionalTr);
    //retval = rightMostConstraint(uncondidtionalTr);
    retval = shortestMatchRightMostConstraint(&retval);

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = noRepetitionConstraint(&retval);
    // remove LM2, RM2
    retval = removeB2Constraint(&retval);

    retval = removeMarkers(&retval);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    retval
}

// replace up, left, right, down
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-epenthesis-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-epenthesis-fn]
pub fn replace_epenthesis_rule(rule: &Rule, optional: bool) -> HfstTransducer {
    replace_rule(rule, optional)
}

// replace up, left, right, down
pub fn replace_epenthesis_rule_vector(ruleVector: &Vec<Rule>, optional: bool) -> HfstTransducer {
    replace_rule_vector(ruleVector, optional)
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
pub fn restriction(_center: &HfstTransducer, context: &HfstTransducerPairVector) -> HfstTransducer {
    //check if the center is automata
    let mut c_proj1: HfstTransducer = _center.clone();
    c_proj1.input_project();
    let mut c_proj2: HfstTransducer = _center.clone();
    c_proj2.output_project();

    if !c_proj1.compare(_center, true) || !c_proj2.compare(_center, true) {
        HFST_THROW_MESSAGE!(
            TransducersAreNotAutomataException,
            "HfstXeroxRules::restriction"
        );
    }

    let type_: ImplementationType = _center.get_type();
    let restrictionMark: String = "@_D_@".to_string();

    let mut TOK: HfstTokenizer = HfstTokenizer::new();
    TOK.add_multichar_symbol(&restrictionMark);
    TOK.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mark: HfstTransducer =
        HfstTransducer::new_string_tokenizer_type(&restrictionMark, &TOK, type_);
    let epsilon: HfstTransducer =
        HfstTransducer::new_string_tokenizer_type("@_EPSILON_SYMBOL_@", &TOK, type_);

    // Identity
    let identityPair: HfstTransducer = HfstTransducer::identity_pair(type_);
    let mut identity: HfstTransducer = identityPair.clone();
    identity.repeat_star().optimize();

    let mut universalWithoutD: HfstTransducer = identity.clone();
    universalWithoutD.insert_to_alphabet_string(&restrictionMark);
    let mut universalWithoutDStar: HfstTransducer = universalWithoutD.clone();
    universalWithoutDStar.repeat_star().optimize();

    // NODU
    let mut noDUpper: HfstTransducer = HfstTransducer::new_string_string_tokenizer_type(
        "@_EPSILON_SYMBOL_@",
        &restrictionMark,
        &TOK,
        type_,
    );
    noDUpper
        .disjunct(&universalWithoutD, true)
        .repeat_star()
        .optimize();

    // NODL
    let mut noDLower: HfstTransducer = HfstTransducer::new_string_string_tokenizer_type(
        &restrictionMark,
        "@_EPSILON_SYMBOL_@",
        &TOK,
        type_,
    );
    noDLower
        .disjunct(&universalWithoutD, true)
        .repeat_star()
        .optimize();

    // 1. Surround center with marks
    // [ U* %<D%> CENTER %<D%> U* ]
    let mut center: HfstTransducer = _center.clone();
    center.insert_to_alphabet_string(&restrictionMark);

    let mut centerMarked: HfstTransducer = universalWithoutDStar.clone();
    centerMarked
        .concatenate(&mark, true)
        .concatenate(&center, true)
        .concatenate(&mark, true)
        .concatenate(&universalWithoutDStar, true)
        .optimize();

    // 2. Put mark in context
    // [ U* L1 %<D%> U* %<D%> R1 U* ]
    let mut contextMarked: HfstTransducer = HfstTransducer::new();
    for i in 0..context.len() {
        let mut lefContext: HfstTransducer = context[i].0.clone();
        lefContext.insert_to_alphabet_string(&restrictionMark);

        let mut rightContext: HfstTransducer = context[i].1.clone();
        rightContext.insert_to_alphabet_string(&restrictionMark);

        let mut RES: HfstTransducer = universalWithoutDStar.clone();
        RES.concatenate(&lefContext, true)
            .concatenate(&mark, true)
            .concatenate(&universalWithoutDStar, true)
            .concatenate(&mark, true)
            .concatenate(&rightContext, true)
            .concatenate(&universalWithoutDStar, true)
            .optimize();

        if i == 0 {
            contextMarked = RES;
        } else {
            contextMarked.disjunct(&RES, true).optimize();
        }
    }
    let mut centerMinusCtx: HfstTransducer = centerMarked.clone();
    centerMinusCtx.subtract(&contextMarked, true).optimize();

    let mut tmp: HfstTransducer = noDUpper.clone();
    tmp.compose(&centerMinusCtx, true)
        .compose(&noDLower, true)
        .optimize();

    let mut retval: HfstTransducer = universalWithoutDStar.clone();
    retval.subtract(&tmp, true).optimize();

    retval.remove_from_alphabet_string(&restrictionMark);

    // deals with boundary symbol
    retval = applyBoundaryMark(&retval);

    retval
}

// a < b
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.before-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.before-fn]
pub fn before(left: &HfstTransducer, right: &HfstTransducer) -> HfstTransducer {
    //check if the center is automata
    let mut l_proj1: HfstTransducer = left.clone();
    l_proj1.input_project();
    let mut l_proj2: HfstTransducer = left.clone();
    l_proj2.output_project();
    let mut r_proj1: HfstTransducer = right.clone();
    r_proj1.input_project();
    let mut r_proj2: HfstTransducer = right.clone();
    r_proj2.output_project();

    if !l_proj1.compare(left, true)
        || !l_proj2.compare(left, true)
        || !r_proj1.compare(right, true)
        || !r_proj2.compare(right, true)
    {
        HFST_THROW_MESSAGE!(
            TransducersAreNotAutomataException,
            "HfstXeroxRules::restriction"
        );
    }

    let type_: ImplementationType = left.get_type();

    // Identity
    let identityPair: HfstTransducer = HfstTransducer::identity_pair(type_);
    let mut identity: HfstTransducer = identityPair.clone();
    identity.repeat_star().optimize();

    let mut tmp: HfstTransducer = identity.clone();
    tmp.concatenate(right, true)
        .concatenate(&identity, true)
        .concatenate(left, true)
        .concatenate(&identity, true)
        .optimize();

    let mut retval: HfstTransducer = identity.clone();
    retval.subtract(&tmp, true).optimize();

    retval
}

// a > b
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.after-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.after-fn]
pub fn after(left: &HfstTransducer, right: &HfstTransducer) -> HfstTransducer {
    //check if the center is automata
    let mut l_proj1: HfstTransducer = left.clone();
    l_proj1.input_project();
    let mut l_proj2: HfstTransducer = left.clone();
    l_proj2.output_project();
    let mut r_proj1: HfstTransducer = right.clone();
    r_proj1.input_project();
    let mut r_proj2: HfstTransducer = right.clone();
    r_proj2.output_project();

    if !l_proj1.compare(left, true)
        || !l_proj2.compare(left, true)
        || !r_proj1.compare(right, true)
        || !r_proj2.compare(right, true)
    {
        HFST_THROW_MESSAGE!(
            TransducersAreNotAutomataException,
            "HfstXeroxRules::restriction"
        );
    }

    let type_: ImplementationType = left.get_type();

    // Identity
    let identityPair: HfstTransducer = HfstTransducer::identity_pair(type_);
    let mut identity: HfstTransducer = identityPair.clone();
    identity.repeat_star().optimize();

    let mut tmp: HfstTransducer = identity.clone();
    tmp.concatenate(left, true)
        .concatenate(&identity, true)
        .concatenate(right, true)
        .concatenate(&identity, true)
        .optimize();

    let mut retval: HfstTransducer = identity.clone();
    retval.subtract(&tmp, true).optimize();

    retval
}
