//! The walk that builds 'PmatchObject' nodes from the 'nfst-pmatch' AST.

use super::*;

// ===========================================================================
// nfst -> C++ enum mappers
// ===========================================================================

fn map_unop(op: nfst_pmatch::UnaryOp) -> PmatchUnaryOp {
    use nfst_pmatch::UnaryOp as N;
    match op {
        N::Star => PmatchUnaryOp::RepeatStar,
        N::Plus => PmatchUnaryOp::RepeatPlus,
        N::Reverse => PmatchUnaryOp::Reverse,
        N::Invert => PmatchUnaryOp::Invert,
        N::UpperProject => PmatchUnaryOp::InputProject,
        N::LowerProject => PmatchUnaryOp::OutputProject,
        N::Complement => PmatchUnaryOp::Complement,
        N::TermComplement => PmatchUnaryOp::TermComplement,
        N::Containment => PmatchUnaryOp::Containment,
        N::ContainmentOnce => PmatchUnaryOp::ContainmentOnce,
        N::ContainmentOpt => PmatchUnaryOp::ContainmentOptional,
    }
}
fn map_acceptor(a: nfst_pmatch::Acceptor) -> PmatchPredefined {
    use nfst_pmatch::Acceptor as N;
    match a {
        N::Alpha => PmatchPredefined::Alpha,
        N::UppercaseAlpha => PmatchPredefined::UppercaseAlpha,
        N::LowercaseAlpha => PmatchPredefined::LowercaseAlpha,
        N::Num => PmatchPredefined::Numeral,
        N::Punct => PmatchPredefined::Punctuation,
        N::Whitespace => PmatchPredefined::Whitespace,
    }
}
fn map_arrow(a: nfst_pmatch::ReplaceArrow) -> ReplaceArrow {
    use nfst_pmatch::ReplaceArrow as N;
    match a {
        N::Right => ReplaceArrow::E_REPLACE_RIGHT,
        N::OptionalRight => ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT,
        N::Left => ReplaceArrow::E_REPLACE_LEFT,
        N::OptionalLeft => ReplaceArrow::E_OPTIONAL_REPLACE_LEFT,
        N::LtrLongest => ReplaceArrow::E_LTR_LONGEST_MATCH,
        N::LtrShortest => ReplaceArrow::E_LTR_SHORTEST_MATCH,
        N::RtlLongest => ReplaceArrow::E_RTL_LONGEST_MATCH,
        N::RtlShortest => ReplaceArrow::E_RTL_SHORTEST_MATCH,
        // The pmatch replace grammar does not support <-> arrows; fall back.
        N::LeftRight => ReplaceArrow::E_REPLACE_RIGHT,
        N::OptionalLeftRight => ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT,
    }
}
fn map_mark(m: nfst_pmatch::ContextMark) -> ReplaceType {
    use nfst_pmatch::ContextMark as N;
    match m {
        N::UpperUpper => ReplaceType::REPL_UP,
        N::LowerUpper => ReplaceType::REPL_RIGHT,
        N::UpperLower => ReplaceType::REPL_LEFT,
        N::LowerLower => ReplaceType::REPL_DOWN,
    }
}
fn map_caseop(op: nfst_pmatch::CaseOp, side: Option<nfst_pmatch::CaseSide>) -> PmatchUnaryOp {
    use PmatchUnaryOp::*;
    use nfst_pmatch::CaseOp as Op;
    use nfst_pmatch::CaseSide as S;
    match (op, side) {
        (Op::Cap, None) => Cap,
        (Op::Cap, Some(S::Upper)) => CapUpper,
        (Op::Cap, Some(S::Lower)) => CapLower,
        (Op::OptCap, None) => OptCap,
        (Op::OptCap, Some(S::Upper)) => OptCapUpper,
        (Op::OptCap, Some(S::Lower)) => OptCapLower,
        (Op::ToLower, None) => ToLower,
        (Op::ToLower, Some(S::Upper)) => ToLowerUpper,
        (Op::ToLower, Some(S::Lower)) => ToLowerLower,
        (Op::ToUpper, None) => ToUpper,
        (Op::ToUpper, Some(S::Upper)) => ToUpperUpper,
        (Op::ToUpper, Some(S::Lower)) => ToUpperLower,
        (Op::OptToLower, None) => OptToLower,
        (Op::OptToLower, Some(S::Upper)) => OptToLowerUpper,
        (Op::OptToLower, Some(S::Lower)) => OptToLowerLower,
        (Op::OptToUpper, None) => OptToUpper,
        (Op::OptToUpper, Some(S::Upper)) => OptToUpperUpper,
        (Op::OptToUpper, Some(S::Lower)) => OptToUpperLower,
        (Op::AnyCase, None) => AnyCase,
        (Op::AnyCase, Some(S::Upper)) => AnyCaseUpper,
        (Op::AnyCase, Some(S::Lower)) => AnyCaseLower,
    }
}
// ===========================================================================
// PmatchObject node constructors (mirror the C++ 'new PmatchX(...)' calls; all
// share the 'PmatchObject()' base-field initialisation: name="", weight=0.0,
// line_defined=pmatchlineno (unavailable in the walk -> 0), my_timer=0,
// cache=NULL).
// ===========================================================================

pub(super) fn as_obj<B: AlgebraBackend + 'static, T: PmatchObject<B> + 'static>(
    p: Rc<T>,
) -> ObjRef<B> {
    p
}
fn pmb_symbol<B: AlgebraBackend + 'static>(sym: Symbol) -> ObjRef<B> {
    Rc::new(PmatchSymbol {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        sym,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_string<B: AlgebraBackend + 'static>(string: Symbol, multichar: bool) -> ObjRef<B> {
    Rc::new(PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        string,
        multichar,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_question_mark<B: AlgebraBackend + 'static>() -> ObjRef<B> {
    Rc::new(PmatchQuestionMark {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_empty<B: AlgebraBackend + 'static>() -> ObjRef<B> {
    Rc::new(PmatchEmpty {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_epsilon_arc<B: AlgebraBackend + 'static>() -> ObjRef<B> {
    Rc::new(PmatchEpsilonArc {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_acceptor<B: AlgebraBackend + 'static>(set: PmatchPredefined) -> ObjRef<B> {
    Rc::new(PmatchAcceptor {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        set,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_unary<B: AlgebraBackend + 'static>(op: PmatchUnaryOp, root: ObjRef<B>) -> ObjRef<B> {
    Rc::new(PmatchUnaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        op,
        root,
    })
}
fn pmb_binary<B: AlgebraBackend + 'static>(
    op: PmatchBinaryOp,
    left: ObjRef<B>,
    right: ObjRef<B>,
) -> ObjRef<B> {
    Rc::new(PmatchBinaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        op,
        left,
        right,
    })
}
fn pmb_ternary<B: AlgebraBackend + 'static>(
    op: PmatchTernaryOp,
    left: ObjRef<B>,
    middle: ObjRef<B>,
    right: ObjRef<B>,
) -> ObjRef<B> {
    Rc::new(PmatchTernaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        op,
        left,
        middle,
        right,
    })
}
fn pmb_numeric<B: AlgebraBackend + 'static>(
    op: PmatchNumericOp,
    root: ObjRef<B>,
    values: Vec<i32>,
) -> ObjRef<B> {
    Rc::new(PmatchNumericOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        op,
        root,
        values,
    })
}
fn pmb_object_pair<B: AlgebraBackend + 'static>(left: ObjRef<B>, right: ObjRef<B>) -> PairRef<B> {
    Rc::new(PmatchObjectPair { left, right })
}
fn pmb_markup_container<B: AlgebraBackend + 'static>(
    left_of_arrow: ObjRef<B>,
    left: ObjRef<B>,
    right: ObjRef<B>,
) -> PairRef<B> {
    Rc::new(PmatchMarkupContainer {
        left,
        right,
        left_of_arrow,
    })
}
fn pmb_tc<B: AlgebraBackend + 'static>(t: HfstTransducer<B>) -> Rc<PmatchTransducerContainer<B>> {
    Rc::new(PmatchTransducerContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        t,
    })
}
// ===========================================================================
// Small build helpers
// ===========================================================================

// STRINGLIKE: QUOTED_LITERAL -> PmatchString, CURLY_LITERAL -> PmatchString
// (multichar), SYMBOL -> PmatchSymbol (no used_definitions / empty-check).
fn build_stringlike<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    e: &nfst_pmatch::SpannedExpr,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::PmatchExpr as PE;
    Ok(match &e.value {
        PE::QuotedLiteral(s) => pmb_string(s.clone(), false),
        PE::CurlyLiteral(s) => pmb_string(s.clone(), true),
        PE::Symbol(s) => pmb_symbol(s.clone()),
        PE::Literal(_)
        | PE::Epsilon
        | PE::Any
        | PE::BoundaryMarker
        | PE::Acceptor(_)
        | PE::CharacterRange { .. }
        | PE::Binary(..)
        | PE::Unary(..)
        | PE::Group(_)
        | PE::Optional(_)
        | PE::BracketedDotted(_)
        | PE::Pair { .. }
        | PE::Weighted { .. }
        | PE::RepeatN(..)
        | PE::RepeatNPlus(..)
        | PE::RepeatNMinus(..)
        | PE::RepeatNToK(..)
        | PE::Replace { .. }
        | PE::Restriction { .. }
        | PE::Ins(_)
        | PE::EndTag(_)
        | PE::Capture(_)
        | PE::Tag { .. }
        | PE::With { .. }
        | PE::Counter(_)
        | PE::CaseOp { .. }
        | PE::DefineWrapper(_)
        | PE::Explode(_)
        | PE::Implode(_)
        | PE::Like { .. }
        | PE::Lst(_)
        | PE::Exc(_)
        | PE::Sigma(_)
        | PE::Interpolate(_)
        | PE::Substitute(..)
        | PE::Uncompose(..)
        | PE::Lc(_)
        | PE::Rc(_)
        | PE::Nlc(_)
        | PE::Nrc(_)
        | PE::OrContext(_)
        | PE::AndContext(_)
        | PE::Call { .. }
        | PE::ReadFile { .. }
        | PE::ReadLexc(_)
        | PE::ReadVec(_) => build_object(ctx, e)?,
    })
}
// CONCATENATED_STRING_LIST: right-folded Concatenate of STRINGLIKEs.
fn build_concatenated_string_list<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    items: &[nfst_pmatch::SpannedExpr],
) -> crate::error::Result<ObjRef<B>> {
    let mut iter = items.iter().rev();
    let last = iter.next().expect("non-empty string list");
    let mut acc = build_stringlike(ctx, last)?;
    for it in iter {
        acc = pmb_binary(PmatchBinaryOp::Concatenate, build_stringlike(ctx, it)?, acc);
    }
    Ok(acc)
}
// MappingSide -> object: Expr -> build, Dotted([..]) -> PmatchEpsilonArc,
// Dotted([. E .]) -> build.
fn side_to_obj<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    side: &nfst_pmatch::MappingSide,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::MappingSide as MS;
    Ok(match side {
        MS::Expr(e) => build_object(ctx, e)?,
        MS::Dotted(None) => pmb_epsilon_arc(),
        MS::Dotted(Some(e)) => build_object(ctx, e)?,
    })
}
// READ_FROM productions: eagerly load the named file into a container.
fn build_read_file<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    kind: nfst_pmatch::ReadKind,
    path: &str,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::ReadKind as RK;
    let filepath = path_from_filename(ctx, path);
    match kind {
        RK::Binary => {
            let mut instream = crate::hfst_input_stream::HfstInputStream::new_filename(&filepath)?;
            let read = instream.read()?.into_typed::<B>()?;
            instream.close();
            Ok(as_obj(pmb_tc(read)))
        }
        RK::Text => Ok(as_obj(pmb_tc(read_text(filepath, false)?))),
        RK::Spaced => Ok(as_obj(pmb_tc(read_spaced_text(filepath)?))),
        RK::Prolog => match std::fs::File::open(&filepath) {
            Err(_) => {
                error!("File cannot be opened.");
                Ok(as_obj(pmb_tc(HfstTransducer::new())))
            }
            Ok(f) => {
                let mut reader = std::io::BufReader::new(f);
                let mut linecount: u32 = 0;
                let tmp =
                    crate::hfst_basic_transducer::HfstBasicTransducer::read_in_prolog_format_file(
                        &mut reader,
                        &mut linecount,
                    )?;
                let mut t = Box::new(HfstTransducer::new_from_basic_transducer(&tmp));
                t.minimize()?;
                Ok(as_obj(pmb_tc(*t)))
            }
        },
        RK::Regex => {
            let mut regex = String::new();
            if let Ok(contents) = std::fs::read_to_string(&filepath) {
                for line in contents.lines() {
                    regex.push_str(line);
                }
            }
            if regex.is_empty() {
                error!("Failed to read regex from {}.", filepath);
            }
            let mut xre_compiler = crate::xre::XreCompiler::new();
            let compiled = xre_compiler.compile(&regex).unwrap_or_default();
            Ok(as_obj(pmb_tc(compiled)))
        }
    }
}
/// Build a 'PmatchObject*' AST node from an 'nfst-pmatch' expression node.
pub fn build_object<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    e: &nfst_pmatch::SpannedExpr,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::PmatchExpr as PE;
    Ok(match &e.value {
        // ---- atoms ---------------------------------------------------------
        PE::Symbol(s) => {
            let sym = s.clone();
            if sym.is_empty() {
                pmb_empty()
            } else {
                ctx.used_definitions_insert(sym.clone());
                pmb_symbol(sym)
            }
        }
        PE::Literal(s) => pmb_string(s.clone(), false),
        PE::QuotedLiteral(s) => pmb_string(s.clone(), false),
        PE::CurlyLiteral(s) => pmb_string(s.clone(), true),
        PE::Epsilon => pmb_string(
            Symbol::from(crate::hfst_symbol_defs::internal_epsilon),
            false,
        ),
        PE::BoundaryMarker => pmb_string(Symbol::from("@BOUNDARY@"), false),
        PE::Any => pmb_question_mark(),
        PE::Acceptor(a) => pmb_acceptor(map_acceptor(*a)),
        PE::CharacterRange { from, to } => {
            let raw = format!("\"{}-{}\"", from, to);
            as_obj(parse_range(ctx, &raw)?)
        }

        // ---- operators -----------------------------------------------------
        PE::Binary(op, l, r) => build_binary_object(ctx, *op, l, r)?,
        PE::Unary(op, inner) => pmb_unary(map_unop(*op), build_object(ctx, inner)?),

        // ---- grouping / weight / pair --------------------------------------
        PE::Group(inner) => build_object(ctx, inner)?,
        PE::Optional(inner) => pmb_unary(PmatchUnaryOp::Optionalize, build_object(ctx, inner)?),
        PE::BracketedDotted(inner) => match inner {
            Some(b) => build_object(ctx, b)?,
            None => pmb_epsilon_arc(),
        },
        PE::Pair { upper, lower } => pmb_binary(
            PmatchBinaryOp::CrossProduct,
            build_object(ctx, upper)?,
            build_object(ctx, lower)?,
        ),
        PE::Weighted { expr, weight } => {
            let mut obj = build_object(ctx, expr)?;
            let new_weight = obj.get_weight() + *weight;
            Rc::get_mut(&mut obj)
                .expect("freshly built node is uniquely owned")
                .set_weight(new_weight);
            obj
        }

        // ---- catenate N ----------------------------------------------------
        PE::RepeatN(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatN,
            build_object(ctx, inner)?,
            vec![*n as i32],
        ),
        PE::RepeatNPlus(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatNPlus,
            build_object(ctx, inner)?,
            vec![*n as i32 + 1],
        ),
        PE::RepeatNMinus(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatNMinus,
            build_object(ctx, inner)?,
            vec![*n as i32 - 1],
        ),
        PE::RepeatNToK(inner, n, k) => pmb_numeric(
            PmatchNumericOp::RepeatNToK,
            build_object(ctx, inner)?,
            vec![*n as i32, *k as i32],
        ),

        // ---- replacement / restriction -------------------------------------
        PE::Replace { arrow, rules } => build_replace(ctx, *arrow, rules)?,
        PE::Restriction { body, contexts } => build_restriction(ctx, body, contexts)?,

        // ---- pmatch-specific constructs, contexts, calls, file references ---
        PE::Ins(_)
        | PE::EndTag(_)
        | PE::Capture(_)
        | PE::Tag { .. }
        | PE::With { .. }
        | PE::Counter(_)
        | PE::CaseOp { .. }
        | PE::DefineWrapper(_)
        | PE::Explode(_)
        | PE::Implode(_)
        | PE::Like { .. }
        | PE::Lst(_)
        | PE::Exc(_)
        | PE::Sigma(_)
        | PE::Interpolate(_)
        | PE::Substitute(..)
        | PE::Uncompose(..)
        | PE::Lc(_)
        | PE::Rc(_)
        | PE::Nlc(_)
        | PE::Nrc(_)
        | PE::OrContext(_)
        | PE::AndContext(_)
        | PE::Call { .. }
        | PE::ReadFile { .. }
        | PE::ReadLexc(_)
        | PE::ReadVec(_) => build_pmatch_construct(ctx, e)?,
    })
}
// Binary operators, mapped onto 'PmatchBinaryOp'.
fn build_binary_object<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    op: nfst_pmatch::BinaryOp,
    l: &nfst_pmatch::SpannedExpr,
    r: &nfst_pmatch::SpannedExpr,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::BinaryOp as NBinOp;
    Ok(match op {
        NBinOp::Concatenate => pmb_binary(
            PmatchBinaryOp::Concatenate,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::Compose => pmb_binary(
            PmatchBinaryOp::Compose,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::LenientCompose => pmb_binary(
            PmatchBinaryOp::LenientCompose,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::CrossProduct => pmb_binary(
            PmatchBinaryOp::CrossProduct,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::MergeRight => pmb_binary(
            PmatchBinaryOp::Merge,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::MergeLeft => {
            // .<m. swaps the operands: Merge($3, $1).
            let lo = build_object(ctx, l)?;
            let ro = build_object(ctx, r)?;
            pmb_binary(PmatchBinaryOp::Merge, ro, lo)
        }
        NBinOp::Before => pmb_binary(
            PmatchBinaryOp::Before,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::After => pmb_binary(
            PmatchBinaryOp::After,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::Shuffle => pmb_binary(
            PmatchBinaryOp::Shuffle,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::Union => pmb_binary(
            PmatchBinaryOp::Disjunct,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::Intersect => pmb_binary(
            PmatchBinaryOp::Intersect,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::Subtract => pmb_binary(
            PmatchBinaryOp::Subtract,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::UpperSubtract => pmb_binary(
            PmatchBinaryOp::UpperSubtract,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::LowerSubtract => pmb_binary(
            PmatchBinaryOp::LowerSubtract,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::UpperPriorityUnion => pmb_binary(
            PmatchBinaryOp::UpperPriorityUnion,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::LowerPriorityUnion => pmb_binary(
            PmatchBinaryOp::LowerPriorityUnion,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::Ignoring => pmb_binary(
            PmatchBinaryOp::InsertFreely,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::IgnoreInternally => pmb_binary(
            PmatchBinaryOp::IgnoreInternally,
            build_object(ctx, l)?,
            build_object(ctx, r)?,
        ),
        NBinOp::LeftQuotient => {
            warn!("Left quotient not implemented");
            pmb_empty()
        }
    })
}
// A replace-rule list: one 'PmatchReplaceRuleContainer' per parallel rule.
fn build_replace<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    arrow: nfst_pmatch::ReplaceArrow,
    rules: &[nfst_pmatch::PmatchReplaceRule],
) -> crate::error::Result<ObjRef<B>> {
    let mapped_arrow = map_arrow(arrow);
    let mut rule_ptrs: Vec<Rc<PmatchReplaceRuleContainer<B>>> = Vec::new();
    for rule in rules.iter() {
        // MAPPINGPAIR_VECTOR -> mapping pairs
        let mut mapping: MappingPairVector<B> = Vec::new();
        for mp in rule.mappings.iter() {
            use nfst_pmatch::MappingKind as MK;
            let pair: PairRef<B> = match &mp.kind {
                MK::Plain { lower } => {
                    pmb_object_pair(side_to_obj(ctx, &mp.upper)?, side_to_obj(ctx, lower)?)
                }
                MK::Markup { pre, post } => {
                    let loa = side_to_obj(ctx, &mp.upper)?;
                    let lom = match pre {
                        Some(s) => side_to_obj(ctx, s)?,
                        None => pmb_epsilon_arc(),
                    };
                    let rom = match post {
                        Some(s) => side_to_obj(ctx, s)?,
                        None => pmb_epsilon_arc(),
                    };
                    pmb_markup_container(loa, lom, rom)
                }
            };
            mapping.push(pair);
        }
        // CONTEXTS_WITH_MARK -> context pairs + type
        let (rtype, context): (ReplaceType, MappingPairVector<B>) = match &rule.contexts {
            Some(ctxs) => {
                let mut context: MappingPairVector<B> = Vec::new();
                for c in ctxs.items.iter() {
                    let l = match &c.left {
                        Some(e) => build_object(ctx, e)?,
                        None => pmb_epsilon_arc(),
                    };
                    let r = match &c.right {
                        Some(e) => build_object(ctx, e)?,
                        None => pmb_epsilon_arc(),
                    };
                    context.push(pmb_object_pair(l, r));
                }
                (map_mark(ctxs.mark), context)
            }
            None => (ReplaceType::REPL_UP, Vec::new()),
        };
        rule_ptrs.push(Rc::new(PmatchReplaceRuleContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            arrow: mapped_arrow,
            ty: rtype,
            mapping,
            context,
        }));
    }
    Ok(as_obj(Rc::new(PmatchParallelRulesContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        arrow: mapped_arrow,
        rules: rule_ptrs,
    })))
}
// A restriction: the body and its left/right context pairs.
fn build_restriction<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    body: &nfst_pmatch::SpannedExpr,
    contexts: &[nfst_pmatch::RestrContext],
) -> crate::error::Result<ObjRef<B>> {
    let left = build_object(ctx, body)?;
    let mut ctxs: MappingPairVector<B> = Vec::new();
    for rc in contexts.iter() {
        let l: ObjRef<B> = match &rc.left {
            Some(e) => build_object(ctx, e)?,
            None => {
                if rc.right.is_some() {
                    pmb_epsilon_arc()
                } else {
                    pmb_empty()
                }
            }
        };
        let r: ObjRef<B> = match &rc.right {
            Some(e) => build_object(ctx, e)?,
            None => {
                if rc.left.is_some() {
                    pmb_epsilon_arc()
                } else {
                    pmb_empty()
                }
            }
        };
        ctxs.push(pmb_object_pair(l, r));
    }
    Ok(as_obj(Rc::new(PmatchRestrictionContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        left,
        contexts: ctxs,
    })))
}
// The pmatch-specific constructs, context conditions, function calls and
// file references. 'build_object' dispatches here and builds the rest.
fn build_pmatch_construct<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    e: &nfst_pmatch::SpannedExpr,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::PmatchExpr as PE;
    Ok(match &e.value {
        // ---- pmatch-specific constructs ------------------------------------
        PE::Ins(name) => build_insertion(ctx, name),
        PE::EndTag(name) => {
            let retval = as_obj(make_end_tag(ctx, name.to_string())?);
            ctx.need_delimiters = true;
            retval
        }
        PE::Capture(name) => {
            let retval = as_obj(make_capture_tag(ctx, name.to_string())?);
            let captured = make_captured_tag(ctx, name.to_string())?;
            if ctx.definitions_contains(name) {
                warn(format!(
                    "definition of {} on line {} shadows earlier definition\n",
                    name, 0
                ));
            }
            ctx.definitions_insert(name.to_string(), as_obj(captured));
            ctx.need_delimiters = true;
            retval
        }
        PE::Tag { body, name } => {
            // AddDelimiters(Concatenate(body, make_end_tag(ctx, name)))
            let cat = pmb_binary(
                PmatchBinaryOp::Concatenate,
                build_object(ctx, body)?,
                as_obj(make_end_tag(ctx, name.to_string())?),
            );
            pmb_unary(PmatchUnaryOp::AddDelimiters, cat)
        }
        PE::With { body, name, value } => {
            // Concatenate(Concatenate(entry, body), exit)
            let entry = make_with_tag_entry(name.to_string(), value.to_string());
            let exit = make_with_tag_exit(name.to_string());
            let inner = pmb_binary(PmatchBinaryOp::Concatenate, entry, build_object(ctx, body)?);
            pmb_binary(PmatchBinaryOp::Concatenate, inner, exit)
        }
        PE::Counter(name) => as_obj(make_counter(ctx, name.to_string())?),
        PE::CaseOp { op, side, body } => {
            pmb_unary(map_caseop(*op, *side), build_object(ctx, body)?)
        }
        PE::DefineWrapper(inner) => {
            pmb_unary(PmatchUnaryOp::AddDelimiters, build_object(ctx, inner)?)
        }
        PE::Explode(items) => pmb_unary(
            PmatchUnaryOp::Explode,
            build_concatenated_string_list(ctx, items)?,
        ),
        PE::Implode(items) => pmb_unary(
            PmatchUnaryOp::Implode,
            build_concatenated_string_list(ctx, items)?,
        ),
        PE::Like {
            args,
            threshold,
            unlike,
        } => build_like(ctx, args, *threshold, *unlike)?,
        PE::Lst(inner) => pmb_unary(PmatchUnaryOp::MakeList, build_object(ctx, inner)?),
        PE::Exc(inner) => pmb_unary(PmatchUnaryOp::MakeExcList, build_object(ctx, inner)?),
        PE::Sigma(inner) => pmb_unary(PmatchUnaryOp::MakeSigma, build_object(ctx, inner)?),
        PE::Interpolate(items) => {
            // FUNCALL_ARGLIST is in reverse source order; replicate.
            let argvec: Vec<ObjRef<B>> = items
                .iter()
                .rev()
                .map(|it| build_object(ctx, it))
                .collect::<crate::error::Result<_>>()?;
            as_obj(Rc::new(PmatchBuiltinFunction {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                args: argvec,
                ty: PmatchBuiltin::Interpolate,
            }))
        }
        PE::Substitute(a, b, c) => pmb_ternary(
            PmatchTernaryOp::Substitute,
            build_object(ctx, a)?,
            build_object(ctx, b)?,
            build_object(ctx, c)?,
        ),
        PE::Uncompose(a, b, c) => {
            let left = build_stringlike(ctx, a)?;
            let middle = build_stringlike(ctx, b)?;
            let right = build_stringlike(ctx, c)?;
            let middle_str = Symbol::from(middle.as_string(ctx).unwrap_or_default());
            ctx.uncomposed_insert(middle_str.clone());
            ctx.used_definitions_insert(middle_str);
            let right_str = Symbol::from(right.as_string(ctx).unwrap_or_default());
            ctx.uncomposed_insert(right_str.clone());
            ctx.used_definitions_insert(right_str);
            pmb_ternary(PmatchTernaryOp::Uncompose, left, middle, right)
        }

        // ---- context conditions --------------------------------------------
        PE::Lc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::LC, build_object(ctx, inner)?);
            ctx.need_delimiters = true;
            retval
        }
        PE::Rc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::RC, build_object(ctx, inner)?);
            ctx.need_delimiters = true;
            retval
        }
        PE::Nlc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::NLC, build_object(ctx, inner)?);
            ctx.need_delimiters = true;
            retval
        }
        PE::Nrc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::NRC, build_object(ctx, inner)?);
            ctx.need_delimiters = true;
            retval
        }
        PE::OrContext(items) => {
            let mut result: Option<ObjRef<B>> = None;
            for it in items.iter() {
                let obj = build_object(ctx, it)?;
                result = match result {
                    None => Some(obj),
                    Some(prev) => Some(pmb_binary(PmatchBinaryOp::Disjunct, prev, obj)),
                };
            }
            // Zero the counter for making minimization guards for disjuncted
            // negative contexts.
            ctx.zero_minimization_guard();
            ctx.need_delimiters = true;
            result.unwrap_or_else(|| pmb_empty())
        }
        PE::AndContext(items) => {
            let mut result: Option<ObjRef<B>> = None;
            for it in items.iter() {
                let obj = build_object(ctx, it)?;
                result = match result {
                    None => Some(obj),
                    Some(prev) => Some(pmb_binary(PmatchBinaryOp::Concatenate, prev, obj)),
                };
            }
            ctx.need_delimiters = true;
            result.unwrap_or_else(|| pmb_empty())
        }

        // ---- function call -------------------------------------------------
        PE::Call { name, args } => {
            let sym = name.clone();
            let result = if !ctx.function_names_contains(name) {
                error!("Function {} hasn't been defined", sym);
                pmb_string(Symbol::default(), false)
            } else {
                let fun = symbol_from_global_context(ctx, &sym)
                    .expect("a defined function name is bound in global definitions");
                let argvec: Vec<ObjRef<B>> = args
                    .iter()
                    .rev()
                    .map(|a| build_object(ctx, a))
                    .collect::<crate::error::Result<_>>()?;
                as_obj(Rc::new(PmatchFuncall {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    args: argvec,
                    fun,
                }))
            };
            ctx.used_definitions_insert(sym);
            result
        }

        // ---- file references -----------------------------------------------
        PE::ReadFile { kind, path } => build_read_file(ctx, *kind, path)?,
        PE::ReadLexc(path) => {
            let filepath = path_from_filename(ctx, path);
            as_obj(pmb_tc(HfstTransducer::read_lexc(&filepath, ctx.verbose)?))
        }
        PE::ReadVec(path) => {
            let filepath = path_from_filename(ctx, path);
            read_vec(ctx, filepath);
            pmb_empty()
        }
        PE::Symbol(_)
        | PE::Literal(_)
        | PE::QuotedLiteral(_)
        | PE::CurlyLiteral(_)
        | PE::Epsilon
        | PE::Any
        | PE::BoundaryMarker
        | PE::Acceptor(_)
        | PE::CharacterRange { .. }
        | PE::Binary(..)
        | PE::Unary(..)
        | PE::Group(_)
        | PE::Optional(_)
        | PE::BracketedDotted(_)
        | PE::Pair { .. }
        | PE::Weighted { .. }
        | PE::RepeatN(..)
        | PE::RepeatNPlus(..)
        | PE::RepeatNMinus(..)
        | PE::RepeatNToK(..)
        | PE::Replace { .. }
        | PE::Restriction { .. } => unreachable!("built by build_object"),
    })
}
// Ins(name): an insertion arc, or under --flatten the named definition.
fn build_insertion<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    name: &Symbol,
) -> ObjRef<B> {
    if !ctx.flatten {
        if !ctx.definitions_contains(name) {
            ctx.unsatisfied_insertions_insert(name.clone());
        }
        let retval = pmb_string(Symbol::from(get_Ins_transition(name)), false);
        ctx.inserted_names_insert(name.clone());
        ctx.used_definitions_insert(name.clone());
        retval
    } else if ctx.definitions_contains(name) {
        ctx.definitions_get(name)
            .expect("definitions_contains verified above")
    } else {
        error!(
            "Insertion of {} is undefined and --ctx.flatten is in use",
            name
        );
        pmb_empty()
    }
}
// Like() and Unlike() over the loaded word vectors.
fn build_like<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    args: &[Symbol],
    threshold: Option<u32>,
    unlike: bool,
) -> crate::error::Result<ObjRef<B>> {
    // The C++ ARGLIST is in reverse source order; replicate.
    let rargs: Vec<String> = args.iter().rev().map(|a| a.to_string()).collect();
    let nwords = threshold.unwrap_or(10);
    Ok(if unlike {
        if rargs.len() < 2 {
            error!(
                "Unlike() operation takes exactly 2 arguments, got {}",
                rargs.len()
            );
            pmb_empty()
        } else {
            compile_like_arc(ctx, rargs[1].clone(), rargs[0].clone(), nwords, true)?
        }
    } else {
        match rargs.len() {
            0 => compile_like_arc_word(ctx, String::new(), 10)?,
            1 => compile_like_arc_word(ctx, rargs[0].clone(), nwords)?,
            _ => compile_like_arc(ctx, rargs[0].clone(), rargs[1].clone(), nwords, false)?,
        }
    })
}
// EXPRESSION1: EXPRESSION2 END_OF_WEIGHTED_EXPRESSION { weight += w; wrap in
// AddDelimiters if need_delimiters; reset need_delimiters. } The trailing
// weight is folded into a 'Weighted' node by nfst, so only the delimiter wrap
// remains here.
fn build_expression1<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    body: &nfst_pmatch::SpannedExpr,
) -> crate::error::Result<ObjRef<B>> {
    let obj = build_object(ctx, body)?;
    let result = if ctx.need_delimiters {
        pmb_unary(PmatchUnaryOp::AddDelimiters, obj)
    } else {
        obj
    };
    ctx.need_delimiters = false;
    Ok(result)
}
// PMATCH DEFINITION verbose timer report.
fn report_defined<B: AlgebraBackend + 'static>(ctx: &mut PmatchEvalContext<B>, name: &str) {
    if ctx.verbose {
        let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
        ctx.timer = clock();
        debug!("defined {} in {:.2} seconds", name, duration);
    }
}
// PMATCH DEFINITION { shadow check + insert }.
fn insert_definition<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    name: String,
    obj: ObjRef<B>,
) {
    if ctx.definitions_contains(&name) {
        warn(format!(
            "definition of {} on line {} shadows earlier definition\n",
            name, 0
        ));
    }
    ctx.definitions_insert(name, obj);
}
/// Apply one top-level 'nfst-pmatch' statement (definition / def-ins /
/// regex-top / set-variable / list-definition / read-vec), populating the
/// 'hfst::pmatch' globals.
pub fn build_statement<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    s: &nfst_pmatch::Spanned<nfst_pmatch::PmatchStatement>,
) -> crate::error::Result<()> {
    use nfst_pmatch::PmatchStatement as PS;
    use nfst_pmatch::VariableValue;
    match &s.value {
        PS::Define { name, params, body } => match params {
            None => {
                let mut obj = build_expression1(ctx, body)?;
                // Under --flatten, `Ins(X)` inlines by returning X's shared
                // definition node (see PE::Ins), so a `Define A Ins(X)` body can
                // hand us an object that already aliases an existing definition
                // (Rc strong count > 1). Only rename when we uniquely own it;
                // an aliased inline keeps its source name — used solely for
                // eval-stack labels — instead of panicking in Rc::get_mut or
                // mutating the shared node out from under the other definition.
                if let Some(node) = Rc::get_mut(&mut obj) {
                    node.set_name(name.to_string());
                }
                report_defined(ctx, name);
                insert_definition(ctx, name.to_string(), obj);
            }
            Some(args) => {
                let root = build_expression1(ctx, body)?;
                // The C++ ARGLIST is in reverse source order; replicate.
                let mut fun = Rc::new(PmatchFunction {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    args: args.iter().rev().cloned().collect(),
                    root,
                });
                Rc::get_mut(&mut fun)
                    .expect("freshly built node is uniquely owned")
                    .name = name.to_string();
                ctx.function_names_insert(name.clone());
                report_defined(ctx, name);
                insert_definition(ctx, name.to_string(), as_obj(fun));
            }
        },
        PS::DefIns { name, body } => {
            let mut body_obj = build_expression1(ctx, body)?;
            Rc::get_mut(&mut body_obj)
                .expect("freshly built node is uniquely owned")
                .set_name(name.to_string());
            ctx.def_insed_expressions_insert(name.to_string(), body_obj);
            let def_value = pmb_string(Symbol::from(get_Ins_transition(name)), false);
            report_defined(ctx, name);
            insert_definition(ctx, name.to_string(), def_value);
        }
        PS::RegexTop { body } => {
            let mut obj = build_expression1(ctx, body)?;
            Rc::get_mut(&mut obj)
                .expect("freshly built node is uniquely owned")
                .set_name("TOP".to_string());
            report_defined(ctx, "TOP");
            insert_definition(ctx, "TOP".to_string(), obj);
        }
        PS::SetVariable { name, value } => {
            let v = match value {
                VariableValue::Symbol(s) => s.to_string(),
                VariableValue::Epsilon => "0".to_string(),
            };
            ctx.variables_insert(name.to_string(), v);
        }
        PS::ListDefinition { name, body } => {
            // DEFINED_LIST: the name lands on the inner body, the stored value
            // is the MakeSigma wrapper.
            let mut inner = build_expression1(ctx, body)?;
            Rc::get_mut(&mut inner)
                .expect("freshly built node is uniquely owned")
                .set_name(name.to_string());
            let value = pmb_unary(PmatchUnaryOp::MakeSigma, inner);
            report_defined(ctx, name);
            insert_definition(ctx, name.to_string(), value);
        }
        PS::ReadVec { path } => {
            let filepath = path_from_filename(ctx, path);
            read_vec(ctx, filepath);
        }
    }
    Ok(())
}
