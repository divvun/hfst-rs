//! The recursive AST evaluator: atoms, pairs, grouping, weights, unary and
//! binary operators, repetition, function calls, and file reads.

use nfst_xre::{ReadKind, UnaryOp, parse};

use super::lexer_support::strip_newline;
use super::*;
use crate::hfst_symbol_defs::{internal_epsilon, internal_identity, internal_unknown};
use crate::hfst_xerox_rules::{after, before};
use crate::virtual_flag_frontends::prepare_compose_flag_overlay;

// Classification of a ':' pair side. The nfst-xre parser only ever produces a
// halfarc atom (Symbol/Epsilon/Any/BoundaryMarker), a Curly, or a 'Group([E])'
// on each side of a Pair; 'None' from the classifier means "bracketed
// expression — evaluate it".
enum XrePairSide {
    Half(Symbol),
    Curly(Symbol),
}

fn xre_pair_side_kind(e: &SpannedXre) -> Option<XrePairSide> {
    match &e.value {
        XreExpr::Symbol(s) => Some(XrePairSide::Half(s.clone())),
        XreExpr::Epsilon => Some(XrePairSide::Half(Symbol::new(internal_epsilon))),
        XreExpr::Any => Some(XrePairSide::Half(Symbol::new(internal_unknown))),
        // nfst-xre actually emits '.#.' as Symbol(".#."); this arm is here for
        // completeness. Per the porting spec the boundary symbol is ".#.".
        XreExpr::BoundaryMarker => Some(XrePairSide::Half(Symbol::new(".#."))),
        XreExpr::Curly(s) => Some(XrePairSide::Curly(s.clone())),
        XreExpr::Pair { .. }
        | XreExpr::Weighted { .. }
        | XreExpr::ReadFile { .. }
        | XreExpr::FunctionCall { .. }
        | XreExpr::Group(_)
        | XreExpr::Optional(_)
        | XreExpr::BracketedDotted(_)
        | XreExpr::Unary(..)
        | XreExpr::Binary(..)
        | XreExpr::RepeatN(..)
        | XreExpr::RepeatNPlus(..)
        | XreExpr::RepeatNMinus(..)
        | XreExpr::RepeatNToK(..)
        | XreExpr::ContainmentWithWeight { .. }
        | XreExpr::Replace { .. }
        | XreExpr::Restriction { .. }
        | XreExpr::Substitute { .. } => None,
    }
}

// ====================== core recursive evaluator ===========================
impl<B: AlgebraBackend> XreCompiler<B> {
    // [spec:hfst:def:xre-utils.hfst.xre.compile-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.compile-fn]
    pub(crate) fn eval(&mut self, e: &SpannedXre) -> crate::error::Result<HfstTransducer<B>> {
        // Anchor any diagnostic emitted while evaluating this node at its span.
        self.current_span = e.span.range.clone();
        Ok(match &e.value {
            // ---- atoms (LABEL: HALFARC) ----
            XreExpr::Symbol(s) => self.label_from_halfarc(s)?,
            XreExpr::Epsilon => self.label_from_halfarc(internal_epsilon)?,
            XreExpr::Any => self.label_from_halfarc(internal_unknown)?,
            XreExpr::BoundaryMarker => self.label_from_halfarc(".#.")?,
            XreExpr::Curly(c) => self.xfst_curly_label_to_transducer(c, c)?,

            // ---- pair ('upper:lower') ----
            XreExpr::Pair { upper, lower } => self.eval_pair(upper, lower)?,

            // ---- grouping ----
            XreExpr::Group(inner) => {
                // REGEXP11: [ REGEXP2 ] -> optimize().
                let mut t = self.eval(inner)?;
                t.optimize_with_config(&self.opt_cfg())?;
                t
            }
            XreExpr::Optional(inner) => {
                // REGEXP11: ( REGEXP2 ) -> optionalize().
                let mut t = self.eval(inner)?;
                t.optionalize()?;
                t
            }
            XreExpr::BracketedDotted(opt) => match opt {
                // '[. E .]' as a bare expression behaves as grouping; '[..]' is
                // epsilon (it only carries replace semantics in mapping
                // position, which is handled by MappingSide::Dotted).
                Some(inner) => self.eval(inner)?,
                None => HfstTransducer::new_symbol(internal_epsilon)?,
            },

            // ---- weighted ('E::w') ----
            XreExpr::Weighted { expr, weight } => {
                let mut t = self.eval(expr)?;
                t.set_final_weights(*weight as f32, true)?;
                // '[E]::w' optimizes after weighting; bare 'LABEL::w' does not.
                if matches!(expr.value, XreExpr::Group(_)) {
                    t.optimize_with_config(&self.opt_cfg())?;
                }
                t
            }

            // ---- operators ----
            XreExpr::Unary(op, inner) => self.eval_unary(*op, inner)?,
            XreExpr::Binary(op, l, r) => self.eval_binary(*op, l, r)?,

            // ---- repetition ----
            XreExpr::RepeatN(inner, n) => {
                let mut t = self.eval(inner)?;
                t.repeat_n(*n)?;
                t
            }
            XreExpr::RepeatNPlus(inner, n) => {
                // REGEXP9: repeat_n_plus($2 + 1).
                let mut t = self.eval(inner)?;
                t.repeat_n_plus(n.wrapping_add(1))?;
                t
            }
            XreExpr::RepeatNMinus(inner, n) => {
                // REGEXP9: repeat_n_minus($2 - 1).
                let mut t = self.eval(inner)?;
                t.repeat_n_minus(n.wrapping_sub(1))?;
                t
            }
            XreExpr::RepeatNToK(inner, n, k) => {
                let mut t = self.eval(inner)?;
                t.repeat_n_to_k(*n, *k)?;
                t
            }

            // ---- containment with explicit weight ('$::w E') ----
            XreExpr::ContainmentWithWeight { expr, weight } => {
                let t = self.eval(expr)?;
                if !t.is_automaton()? {
                    crate::bail!(Hfst, "Containment with weight only works with automata");
                }
                self.contains_with_weight(&t, *weight as f32)?
            }

            // ---- function call ----
            XreExpr::FunctionCall { name, args } => self.eval_function_call(name, args)?,

            // ---- rules (built in 'rules') and file reads ----
            XreExpr::Replace { arrow, rules } => self.eval_replace(*arrow, rules)?,
            XreExpr::Restriction { body, contexts } => self.eval_restriction(body, contexts)?,
            XreExpr::Substitute { haystack, what } => self.eval_substitute(haystack, what)?,
            XreExpr::ReadFile { kind, path } => self.eval_read_file(*kind, path)?,
        })
    }

    // LABEL: HALFARC. '?' (internal_unknown) becomes a single identity arc;
    // anything else is definition-expanded (gated on expand_definitions).
    fn label_from_halfarc(&self, sym: &str) -> crate::error::Result<HfstTransducer<B>> {
        if sym == internal_unknown {
            HfstTransducer::new_symbol(internal_identity)
        } else {
            self.expand_definition_sym(sym)
        }
    }

    // The ':' productions from LABEL / REGEXP11, dispatched on the kinds of the
    // two sides. Cross-product orderings (including the '{c}:[F]' swap that the
    // grammar performs at xre_parse.yy:1001) are preserved verbatim.
    fn eval_pair(
        &mut self,
        upper: &SpannedXre,
        lower: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        Ok(
            match (xre_pair_side_kind(upper), xre_pair_side_kind(lower)) {
                (Some(XrePairSide::Half(a)), Some(XrePairSide::Half(b))) => {
                    self.xfst_label_to_transducer(&a, &b)?
                }
                (Some(XrePairSide::Half(a)), Some(XrePairSide::Curly(c))) => {
                    let mut up = self.xfst_label_to_transducer(&a, &a)?;
                    let lo = self.xfst_curly_label_to_transducer(&c, &c)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (Some(XrePairSide::Curly(c)), Some(XrePairSide::Half(a))) => {
                    let mut up = self.xfst_curly_label_to_transducer(&c, &c)?;
                    let lo = self.xfst_label_to_transducer(&a, &a)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (Some(XrePairSide::Curly(c1)), Some(XrePairSide::Curly(c2))) => {
                    self.xfst_curly_label_to_transducer(&c1, &c2)?
                }
                (Some(XrePairSide::Half(a)), None) => {
                    // HALFARC : [F]  -> expand_definition(a) x eval(F)
                    let mut up = self.expand_definition_sym(&a)?;
                    let lo = self.eval(lower)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (None, Some(XrePairSide::Half(b))) => {
                    // [E] : HALFARC  -> eval(E) x expand_definition(b)
                    let mut up = self.eval(upper)?;
                    let lo = self.expand_definition_sym(&b)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (Some(XrePairSide::Curly(c)), None) => {
                    // {c} : [F]  -> grammar computes eval(F).cross_product(curly).
                    let cur = self.xfst_curly_label_to_transducer(&c, &c)?;
                    let mut lo = self.eval(lower)?;
                    lo.cross_product(&cur, false)?;
                    lo
                }
                (None, Some(XrePairSide::Curly(c))) => {
                    // [E] : {c}  -> eval(E) x curly
                    let mut up = self.eval(upper)?;
                    let lo = self.xfst_curly_label_to_transducer(&c, &c)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (None, None) => {
                    // [E] : [F]  -> eval(E) x eval(F)
                    let mut up = self.eval(upper)?;
                    let lo = self.eval(lower)?;
                    up.cross_product(&lo, false)?;
                    up
                }
            },
        )
    }

    // REGEXP8/9/10 unary operators.
    fn eval_unary(
        &mut self,
        op: UnaryOp,
        inner: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        Ok(match op {
            UnaryOp::Star => {
                let mut t = self.eval(inner)?;
                t.repeat_star()?;
                t
            }
            UnaryOp::Plus => {
                let mut t = self.eval(inner)?;
                t.repeat_plus()?;
                t
            }
            UnaryOp::Reverse => {
                let mut t = self.eval(inner)?;
                t.reverse()?;
                t
            }
            UnaryOp::Invert => {
                let mut t = self.eval(inner)?;
                t.invert()?;
                t
            }
            UnaryOp::UpperProject => {
                let mut t = self.eval(inner)?;
                t.input_project()?;
                t
            }
            UnaryOp::LowerProject => {
                let mut t = self.eval(inner)?;
                t.output_project()?;
                t
            }
            UnaryOp::Complement => {
                // ~A : [? | flags(A)]* - A, only for automata. A's flag
                // diacritics enter the identity universe as ORDINARY symbols so
                // subtract harmonization cannot erase them
                // ([spec:hfst:sem:xre-compiler.hfst.xre.complement-compilation-fn]):
                // this DIVERGES from upstream C++ XRE ([?:?]* - A, which swallows
                // flags) to match the Xerox transcript and HfstTransducer::negate().
                let a = self.eval(inner)?;
                if !a.is_automaton()? {
                    crate::bail!(Hfst, "Complement operator ~ is defined only for automata");
                }
                let mut complement = HfstTransducer::identity_with_flags_of(&a)?;
                complement.repeat_star()?;
                complement.optimize_with_config(&self.opt_cfg())?;
                complement.subtract(&a, true)?;
                complement.prune_alphabet(false)?;
                complement
            }
            UnaryOp::TermComplement => {
                // \A : [? | flags(A)] - A. Same flag-ordinary universe as ~A but
                // without the star, so \A matches any SINGLE symbol other than A
                // ([spec:hfst:sem:xre-compiler.hfst.xre.term-complement-compilation-fn]):
                // this DIVERGES from upstream C++ XRE ([?] - A, which swallows
                // flags) to match the Xerox transcript (hfst/hfst#349).
                let a = self.eval(inner)?;
                let mut any = HfstTransducer::identity_with_flags_of(&a)?;
                any.subtract(&a, true)?;
                any.prune_alphabet(false)?;
                any
            }
            UnaryOp::Containment => {
                // $A : transducers fall back to simple containment; automata use
                // the weighted-rule path with weight 0.
                let a = self.eval(inner)?;
                if !a.is_automaton()? {
                    self.contains(&a)?
                } else {
                    self.contains_with_weight(&a, 0.0)?
                }
            }
            UnaryOp::ContainmentOnce => {
                let a = self.eval(inner)?;
                self.contains_once(&a)?
            }
            UnaryOp::ContainmentOpt => {
                let a = self.eval(inner)?;
                self.contains_once_optional(&a)?
            }
        })
    }

    // REGEXP2/3/5/6/7 binary operators.
    fn eval_binary(
        &mut self,
        op: BinaryOp,
        l: &SpannedXre,
        r: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        Ok(match op {
            BinaryOp::Compose => {
                let mut left = self.eval(l)?;
                let mut right = self.eval(r)?;
                let config = self.opt_cfg();
                let overlay = prepare_compose_flag_overlay(
                    &mut left,
                    &mut right,
                    self.harmonize_flags,
                    &config,
                )?;
                left.compose_with_config_and_flag_overlay(
                    &right,
                    self.harmonize,
                    &config,
                    overlay.as_ref(),
                )?;
                left.optimize_with_config(&config)?;
                left
            }
            BinaryOp::CrossProduct => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.cross_product(&right, false)?;
                left.optimize_with_config(&self.opt_cfg())?;
                left
            }
            BinaryOp::LenientCompose => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.lenient_composition(&right, false)?;
                left.optimize_with_config(&self.opt_cfg())?;
                left
            }
            BinaryOp::MergeRight => {
                // .m>. : merge left into right.
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                let mut res = self.merge_first_to_second(&mut left, right)?;
                res.optimize_with_config(&self.opt_cfg())?;
                res
            }
            BinaryOp::MergeLeft => {
                // .<m. : merge right into left.
                let left = self.eval(l)?;
                let mut right = self.eval(r)?;
                let mut res = self.merge_first_to_second(&mut right, left)?;
                res.optimize_with_config(&self.opt_cfg())?;
                res
            }
            BinaryOp::Before => {
                let left = self.eval(l)?;
                let right = self.eval(r)?;
                before(&left, &right)?
            }
            BinaryOp::After => {
                let left = self.eval(l)?;
                let right = self.eval(r)?;
                after(&left, &right)?
            }
            BinaryOp::Union => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.disjunct(&right, self.harmonize)?;
                left
            }
            BinaryOp::Intersect => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.intersect(&right, self.harmonize)?;
                left.optimize_with_config(&self.opt_cfg())?;
                left.prune_alphabet(false)?;
                left
            }
            BinaryOp::Subtract => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.subtract(&right, self.harmonize)?;
                left.prune_alphabet(false)?;
                left
            }
            BinaryOp::UpperPriorityUnion => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.priority_union(&right)?;
                left
            }
            BinaryOp::LowerPriorityUnion => {
                // invert both, priority_union, invert back.
                let mut left = self.eval(l)?;
                let mut right = self.eval(r)?;
                right.invert()?;
                left.invert()?;
                left.priority_union(&right)?;
                left.invert()?;
                left
            }
            BinaryOp::Concatenate => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.concatenate(&right, self.harmonize)?;
                left
            }
            BinaryOp::Ignoring => {
                // harmonize (force), then insert_freely without harmonization.
                let mut left = self.eval(l)?;
                let mut right = self.eval(r)?;
                left.harmonize(&mut right, true)?;
                left.insert_freely(&right, false)?;
                left
            }
            // Operators the C++ grammar rejects with xreerror + YYABORT.
            BinaryOp::Shuffle => crate::bail!(Hfst, "No shuffle"),
            BinaryOp::UpperSubtract => crate::bail!(Hfst, "No upper minus"),
            BinaryOp::LowerSubtract => crate::bail!(Hfst, "No lower minus"),
            BinaryOp::IgnoreInternally => {
                crate::bail!(Hfst, "No ignoring internally")
            }
            BinaryOp::LeftQuotient => crate::bail!(Hfst, "No left quotient"),
        })
    }

    // LABEL: FUNCTION REGEXP_LIST ')'. Because eval is &self, the function
    // arguments are registered in a cloned, augmented compiler (the C++ flow
    // define_function_args -> recursive parse -> undefine_function_args, but
    // re-entrant).
    fn eval_function_call(
        &mut self,
        name: &str,
        args: &[SpannedXre],
    ) -> crate::error::Result<HfstTransducer<B>> {
        let arg_trs: Vec<HfstTransducer<B>> =
            args.iter()
                .map(|a| self.eval(a))
                .collect::<crate::error::Result<Vec<HfstTransducer<B>>>>()?;
        let n_args = arg_trs.len();

        // is_valid_function_call: defined + correct arity.
        let expected = match self.function_arguments.get(name) {
            Some(n) => *n,
            None => {
                crate::bail!(Hfst, format!("No such function defined: '{}'", name))
            }
        };
        if !self.function_definitions.contains_key(name) {
            crate::bail!(Hfst, format!("No such function defined: '{}'", name));
        }
        if expected as usize != n_args {
            crate::bail!(
                Hfst,
                format!(
                    "Wrong number of arguments: function '{}' expects {}, {} given",
                    name, expected, n_args
                )
            );
        }

        // define_function_args: definitions["@name N@"] = arg (1-based).
        let mut sub_defs = self.definitions.clone();
        for (i, arg) in arg_trs.into_iter().enumerate() {
            sub_defs.insert(Symbol::from(format!("@{}{}@", name, i + 1)), arg);
        }
        let mut sub = XreCompiler {
            definitions: sub_defs,
            function_definitions: self.function_definitions.clone(),
            function_arguments: self.function_arguments.clone(),
            list_definitions: self.list_definitions.clone(),
            verbose: self.verbose,
            expand_definitions: self.expand_definitions,
            harmonize: self.harmonize,
            harmonize_flags: self.harmonize_flags,
            minimize_result: self.minimize_result,
            flag_is_epsilon: self.flag_is_epsilon,
            xerox_composition: self.xerox_composition,
            encode_weights: self.encode_weights,
            contains_only_comments: false,
            source: self.source.clone(),
            source_name: self.source_name.clone(),
            current_span: self.current_span.clone(),
        };

        // get_function_xre + recursive compile.
        let body = self
            .function_definitions
            .get(name)
            .cloned()
            .expect("function definition present (checked above)");
        Ok(match parse(&body) {
            Ok(expr) => {
                let mut t = sub.eval(&expr)?;
                t.optimize_with_config(&self.opt_cfg())?;
                t
            }
            Err(e) => {
                // The span is relative to the stored function body, not the
                // caller's source, so anchor the snippet in the body text.
                for d in &e.diagnostics {
                    crate::diag::emit(
                        &format!("<function {}>", name),
                        &body,
                        d.span.range.clone(),
                        crate::diag::Severity::Error,
                        &d.message,
                    );
                }
                crate::bail!(Hfst, format!("Could not parse body of function '{}'", name))
            }
        })
    }
}

// ===== integration shims: deferred eval_read_file =====
impl<B: AlgebraBackend> XreCompiler<B> {
    /// '@bin'/'@txt'/'@stxt'/'@pl'/'@re' file-load evaluation. Ports the
    /// xre_parse.yy READ_BIN/READ_TEXT/READ_SPACED/READ_PROLOG/READ_RE actions.
    fn eval_read_file(
        &mut self,
        kind: ReadKind,
        path: &str,
    ) -> crate::error::Result<HfstTransducer<B>> {
        use crate::hfst_basic_transducer::HfstBasicTransducer;
        match kind {
            // READ_BIN: HfstInputStream instream(path); new HfstTransducer(instream);
            // t.convert(format). The stream reader returns the runtime sum
            // ('AnyTransducer'); 'B' here carries only 'AlgebraBackend' (the
            // facade seeds compilers without 'FromAnyTransducer'), so extract
            // the typed value through the interchange transducer — exactly
            // 'into_typed's general (convert) arm — keeping the facade
            // metadata like the C++ convert did.
            ReadKind::Binary => {
                let mut instream = crate::hfst_input_stream::HfstInputStream::new_filename(path)?;
                let any = instream.read()?;
                instream.close();
                let src_type = any.get_type();
                if src_type != B::TYPE {
                    // C++ hfst-xfst THROWS TransducerTypeMismatchException for a
                    // cross-backend `read regex @"file"` ("loading automata in
                    // different formats (OpenFst, foma) is not supported in XFST
                    // scripts"); we are more permissive and convert through the
                    // basic transducer. That is not a problem, but the
                    // conversion should never be silent: log it at info level
                    // (the tracing level filter decides visibility).
                    let mut line = format!(
                        "converting transducer type from {} to {} when reading from file '{}'",
                        crate::hfst_data_types::implementation_type_to_format(src_type),
                        crate::hfst_data_types::implementation_type_to_format(B::TYPE),
                        path
                    );
                    if !crate::hfst_transducer::is_safe_conversion(src_type, B::TYPE) {
                        line.push_str(" (loss of information is possible)");
                    }
                    tracing::info!("{}", line);
                }
                let net = any.to_basic()?;
                let mut retval: HfstTransducer<B> = HfstTransducer::new_from_basic(&net)?;
                retval.name = any.get_name();
                retval.props = any.get_properties().clone();
                Ok(retval)
            }
            // READ_TEXT / READ_SPACED: tokenize each line and disjunct it into a
            // basic transducer, then build a transducer of the compiler format and
            // optimize. READ_TEXT uses the multichar tokenizer; READ_SPACED splits
            // on spaces.
            ReadKind::Text | ReadKind::Spaced => {
                let Ok(contents) = std::fs::read_to_string(path) else {
                    crate::bail!(Hfst, format!("File cannot be opened: '{}'", path));
                };
                let mut tmp = HfstBasicTransducer::new();
                let tok = crate::hfst_tokenizer::HfstTokenizer::new();
                for raw in contents.lines() {
                    let line = strip_newline(raw);
                    let spv = if kind == ReadKind::Spaced {
                        crate::hfst_tokenizer::HfstTokenizer::tokenize_space_separated(&line)
                    } else {
                        tok.tokenize(&line, false)
                    };
                    tmp.disjunct_path(&spv, 0.0);
                }
                let mut retval = HfstTransducer::new_from_basic(&tmp)?;
                retval.optimize_with_config(&self.opt_cfg())?;
                Ok(retval)
            }
            // READ_PROLOG: read_in_prolog_format then build of the compiler format.
            ReadKind::Prolog => {
                let f = match std::fs::File::open(path) {
                    Ok(f) => f,
                    Err(_) => crate::bail!(Hfst, format!("File cannot be opened: '{}'", path)),
                };
                let mut reader = std::io::BufReader::new(f);
                let mut linecount: u32 = 0;
                let tmp = HfstBasicTransducer::read_in_prolog_format(&mut reader, &mut linecount)?;
                let mut retval = HfstTransducer::new_from_basic(&tmp)?;
                retval.optimize_with_config(&self.opt_cfg())?;
                Ok(retval)
            }
            // READ_RE: read the file content and re-compile it as a regex (the C++
            // spins up a fresh scanner; the ported compiler re-parses the string).
            ReadKind::Regex => {
                let Ok(contents) = std::fs::read_to_string(path) else {
                    crate::bail!(Hfst, format!("File cannot be opened: '{}'", path));
                };
                self.compile(&contents)
                    .ok_or_else(|| crate::err!(Hfst, "read-regex: regex did not compile"))
            }
        }
    }
}
