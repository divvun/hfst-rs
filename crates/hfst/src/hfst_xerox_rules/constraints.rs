//! The constraints composed onto a bracketed replace to pick leftmost,
//! rightmost, longest, shortest, and most-bracketed matches, plus the
//! boundary-marker application.

use super::*;

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
