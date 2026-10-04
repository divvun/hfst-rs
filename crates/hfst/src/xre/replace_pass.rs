//! The replace-pass compiler: one parallel optional replace rule `P`,
//! compiled as the relation of `?* P ?*` without determinising it.
//!
//! A Giella speller's strings files each hold one such rule, often with
//! thousands of `L (->) R::w` mappings. Compiled the classic way, `?* P ?*`
//! is a determinised machine that grows with every overlap between left-hand
//! sides. The same relation is a few pass states, each with a loop that
//! copies one symbol, stepping by epsilon into a minimised union of the
//! rules' cross-products and back. A speller determinises lazily while it
//! searches, so it never needs the expanded form.

use std::collections::BTreeMap;
use std::ops::Range;

use nfst_xre::{
    ContextMark, MappingKind, MappingPair, MappingSide, ReplaceArrow, ReplaceContexts, ReplaceRule,
};

use super::*;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::internal_epsilon;

/// A state of the pass automaton.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PassState {
    /// No rewrite made yet.
    Start,
    /// At least one rewrite made, and more may follow.
    Middle,
    /// The last rewrite made; only copying remains.
    End,
}

/// Where in a pass a rule may rewrite, one entry per context of the rule:
/// whether that context needs the rewrite to be the pass's first (`.#. _`)
/// and whether it needs it to be the last (`_ .#.`). A rule without
/// contexts has one entry that needs neither.
struct Placement(Vec<(bool, bool)>);

impl Placement {
    // [spec:hfst:req:xre-replace-pass.boundary-contexts]
    /// Whether a rewrite that is (or is not) the first and (or is not) the
    /// last of its pass satisfies one of the rule's contexts. Inside `?* P
    /// ?*` the boundary `.#.` is the edge of P's own domain, so `_ .#.`
    /// admits exactly the pass's last rewrite and `.#. _` its first.
    fn allows(&self, first: bool, last: bool) -> bool {
        self.0
            .iter()
            .any(|&(needs_first, needs_last)| (first || !needs_first) && (last || !needs_last))
    }

    /// Whether being the pass's first rewrite changes what the rule may do.
    fn depends_on_first(&self) -> bool {
        self.allows(true, false) != self.allows(false, false)
            || self.allows(true, true) != self.allows(false, true)
    }

    /// The pass edges a rewrite by this rule labels. Every legal sequence of
    /// rewrites follows exactly one path: from `Start` the first rewrite goes
    /// to `Middle` when more may follow it and to `End` when it must be the
    /// only one; from `Middle` a rewrite stays when more may follow it and
    /// goes to `End` when it must be the last. Without `track_first` no rule
    /// cares about being first, `Start` and `Middle` behave alike, and
    /// `Start` stands for both.
    fn edges(&self, track_first: bool) -> Vec<(PassState, PassState)> {
        use PassState::{End, Middle, Start};
        let mut edges = Vec::new();
        let middle = if track_first {
            if self.allows(true, false) {
                edges.push((Start, Middle));
            } else if self.allows(true, true) {
                edges.push((Start, End));
            }
            Middle
        } else {
            Start
        };
        if self.allows(false, false) {
            edges.push((middle, middle));
        } else if self.allows(false, true) {
            edges.push((middle, End));
        }
        edges
    }
}

/// The rule bodies of a pass, keyed by the edge between pass states each
/// one labels.
type Bodies<B> = BTreeMap<(PassState, PassState), HfstTransducer<B>>;

/// One `,,`-separated rule of the pass: its placement and the cross-product
/// of each of its `,`-separated mappings.
struct PassRule<B: AlgebraBackend> {
    placement: Placement,
    mappings: Vec<HfstTransducer<B>>,
}

/// The rules of a root that is one replace rule, looking through grouping
/// brackets; `None` for any other root.
fn replace_rules(root: &SpannedXre) -> Option<&[ReplaceRule]> {
    if let XreExpr::Group(inner) = &root.value {
        return replace_rules(inner);
    }
    if let XreExpr::Replace { rules, .. } = &root.value {
        return Some(rules);
    }
    None
}

/// Whether a context side is the boundary `.#.` and nothing else.
fn is_boundary(side: &SpannedXre) -> bool {
    if let XreExpr::Group(inner) = &side.value {
        return is_boundary(inner);
    }
    matches!(&side.value, XreExpr::BoundaryMarker)
        || matches!(&side.value, XreExpr::Symbol(s) if s.as_str() == ".#.")
}

fn arrow_text(arrow: ReplaceArrow) -> &'static str {
    match arrow {
        ReplaceArrow::Right => "->",
        ReplaceArrow::OptionalRight => "(->)",
        ReplaceArrow::Left => "<-",
        ReplaceArrow::OptionalLeft => "(<-)",
        ReplaceArrow::LeftRight => "<->",
        ReplaceArrow::OptionalLeftRight => "(<->)",
        ReplaceArrow::LtrLongest => "@->",
        ReplaceArrow::LtrShortest => "@>",
        ReplaceArrow::RtlLongest => "->@",
        ReplaceArrow::RtlShortest => ">@",
    }
}

fn mark_text(mark: ContextMark) -> &'static str {
    match mark {
        ContextMark::UpperUpper => "||",
        ContextMark::LowerUpper => "//",
        ContextMark::UpperLower => "\\\\",
        ContextMark::LowerLower => "\\/",
    }
}

fn side_span(side: &MappingSide) -> Option<Range<usize>> {
    match side {
        MappingSide::Expr(e) | MappingSide::Dotted(Some(e)) => Some(e.span.range.clone()),
        MappingSide::Dotted(None) => None,
    }
}

/// The source span of one mapping, from its left-hand side to its last
/// written piece.
fn mapping_span(mp: &MappingPair) -> Range<usize> {
    let rest: Vec<Option<Range<usize>>> = match &mp.kind {
        MappingKind::Plain { lower } => vec![side_span(lower)],
        MappingKind::Markup { pre, post } => vec![
            pre.as_ref().and_then(side_span),
            post.as_ref().and_then(side_span),
        ],
    };
    let mut spans = std::iter::once(side_span(&mp.upper)).chain(rest).flatten();
    let first = spans.next().unwrap_or(0..0);
    spans.fold(first, |acc, s| acc.start.min(s.start)..acc.end.max(s.end))
}

/// The span of a rule's first mapping, where a refusal about the rule as a
/// whole is anchored.
fn rule_span(rule: &ReplaceRule) -> Range<usize> {
    rule.mappings.first().map(mapping_span).unwrap_or(0..0)
}

/// The union of `mappings`, joined as a balanced tree: every harmonizing
/// union converts both operands, so folding thousands of mappings into one
/// growing machine would cost quadratic time.
fn union_of<B: AlgebraBackend>(
    mappings: &[&HfstTransducer<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    match mappings {
        [] => return Ok(HfstTransducer::new()),
        [only] => return Ok((*only).clone()),
        _ => {}
    }
    let (left, right) = mappings.split_at(mappings.len() / 2);
    let mut union = union_of(left)?;
    union.disjunct(&union_of(right)?, true)?;
    Ok(union)
}

/// Whether some path from the initial state that only takes arcs `follow`
/// accepts reaches a final state.
fn reaches_final(
    graph: &HfstBasicTransducer,
    follow: impl Fn(&HfstBasicTransition) -> bool,
) -> bool {
    let mut seen = vec![false; graph.state_vector.len()];
    let mut stack = vec![0u32];
    while let Some(s) = stack.pop() {
        if std::mem::replace(&mut seen[s as usize], true) {
            continue;
        }
        if graph.is_final_state(s) {
            return true;
        }
        for t in &graph.state_vector[s as usize] {
            if follow(t) {
                stack.push(t.get_target_state());
            }
        }
    }
    false
}

impl<B: AlgebraBackend> XreCompiler<B> {
    /// Report a refusal at `span` and count it.
    fn refuse(&self, refusals: &mut usize, span: Range<usize>, msg: &str) {
        self.diag_at(
            span,
            crate::diag::Severity::Error,
            &format!("--replace-pass: {msg}"),
        );
        *refusals += 1;
    }

    // [spec:hfst:req:xre-replace-pass.relation]
    // [spec:hfst:req:xre-replace-pass.refusals]
    /// Compile `root`, which must be one parallel optional replace rule `P`,
    /// as one pass: the weighted relation of `?* P ?*`. Every rule and
    /// mapping is checked before anything is built, and each refusal is
    /// reported at its own source position.
    pub(super) fn eval_replace_pass(
        &mut self,
        root: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut refusals = 0;
        let Some(rules) = replace_rules(root) else {
            self.refuse(
                &mut refusals,
                root.span.range.clone(),
                "this expression is not a replace rule; a pass is compiled from one parallel optional replace rule",
            );
            crate::bail!(Hfst, "--replace-pass: the expression is not a replace rule");
        };
        let mut pass_rules = Vec::new();
        for (index, rule) in rules.iter().enumerate() {
            if let Some(pass_rule) = self.pass_rule(index + 1, rule, &mut refusals) {
                pass_rules.push(pass_rule);
            }
        }
        if refusals > 0 {
            crate::bail!(
                Hfst,
                format!("--replace-pass: {refusals} problem(s) in the replace rule")
            );
        }
        self.assemble_pass(&pass_rules)
    }

    /// Check and build rule number `index` (1-based, counting `,,`-separated
    /// rules). `None` once any of its problems has been reported.
    fn pass_rule(
        &mut self,
        index: usize,
        rule: &ReplaceRule,
        refusals: &mut usize,
    ) -> Option<PassRule<B>> {
        let placement = self.placement(index, rule, refusals);
        let mut mappings = Vec::new();
        let mut complete = placement.is_some();
        for (m, mp) in rule.mappings.iter().enumerate() {
            match self.pass_mapping(index, m + 1, mp, refusals) {
                Some(mapping) => mappings.push(mapping),
                None => complete = false,
            }
        }
        let placement = placement?;
        complete.then_some(PassRule {
            placement,
            mappings,
        })
    }

    /// The placement a rule's contexts give it. Only the pass boundary may
    /// appear in a context, on the upper side (`||`).
    fn placement(
        &self,
        index: usize,
        rule: &ReplaceRule,
        refusals: &mut usize,
    ) -> Option<Placement> {
        let Some(ReplaceContexts { mark, items }) = &rule.contexts else {
            return Some(Placement(vec![(false, false)]));
        };
        if *mark != ContextMark::UpperUpper {
            let span = items
                .iter()
                .flat_map(|c| [c.left.as_ref(), c.right.as_ref()])
                .flatten()
                .map(|e| e.span.range.clone())
                .next()
                .unwrap_or_else(|| rule_span(rule));
            self.refuse(
                refusals,
                span,
                &format!(
                    "rule {index} uses the context mark '{}'; a pass allows only '||'",
                    mark_text(*mark)
                ),
            );
            return None;
        }
        let before = *refusals;
        let mut entries = Vec::new();
        for item in items {
            let mut needs = [false, false];
            for (need, side) in needs.iter_mut().zip([&item.left, &item.right]) {
                let Some(side) = side else { continue };
                if is_boundary(side) {
                    *need = true;
                } else {
                    self.refuse(
                        refusals,
                        side.span.range.clone(),
                        &format!(
                            "rule {index} has a context other than the pass boundary; a pass allows only '_ .#.', '.#. _' and '.#. _ .#.'"
                        ),
                    );
                }
            }
            entries.push((needs[0], needs[1]));
        }
        (*refusals == before).then_some(Placement(entries))
    }

    /// Check and build mapping `m` of rule `index`: its cross-product, or
    /// `None` once its problems have been reported.
    fn pass_mapping(
        &mut self,
        index: usize,
        m: usize,
        mp: &MappingPair,
        refusals: &mut usize,
    ) -> Option<HfstTransducer<B>> {
        let span = mapping_span(mp);
        let which = format!("mapping {m} of rule {index}");
        if mp.arrow != ReplaceArrow::OptionalRight {
            self.refuse(
                refusals,
                span.clone(),
                &format!(
                    "{which} uses '{}'; a pass compiles only the optional arrow '(->)'",
                    arrow_text(mp.arrow)
                ),
            );
        }
        if matches!(mp.kind, MappingKind::Markup { .. }) {
            self.refuse(
                refusals,
                span.clone(),
                &format!("{which} is a markup mapping; a pass compiles only plain 'A (->) B'"),
            );
        }
        if mp.arrow != ReplaceArrow::OptionalRight || matches!(mp.kind, MappingKind::Markup { .. })
        {
            return None;
        }
        match self.mapping_relation(mp) {
            Ok(mapping) => Some(mapping),
            Err(problem) => {
                self.refuse(refusals, span, &format!("{which} {problem}"));
                None
            }
        }
    }

    /// `L .x. R` for a plain mapping, through the same evaluation the classic
    /// replace compile gives its mappings, or what makes it unusable in a
    /// pass. A flag diacritic is refused: the classic compile copies one only
    /// inside the rule's own domain, a distinction a pass does not draw.
    fn mapping_relation(&mut self, mp: &MappingPair) -> Result<HfstTransducer<B>, String> {
        let compile_error = |e: crate::error::Error| format!("does not compile: {e}");
        let (mut upper, lower) = self.build_mapping_pair(mp).map_err(compile_error)?;
        for side in [&upper, &lower] {
            let alphabet = side.get_alphabet().map_err(compile_error)?;
            if let Some(flag) = alphabet.iter().find(|s| FdOperation::is_diacritic(s)) {
                return Err(format!(
                    "uses the flag diacritic '{flag}'; a pass does not support flag diacritics"
                ));
            }
        }
        upper.cross_product(&lower, true).map_err(compile_error)?;
        if upper.get_alphabet().map_err(compile_error)?.contains(".#.") {
            return Err("contains the boundary '.#.', which belongs only in a context".to_string());
        }
        let graph = upper.to_basic().map_err(compile_error)?;
        if !reaches_final(&graph, |_| true) {
            return Err(
                "can never apply: its left- or right-hand side matches nothing".to_string(),
            );
        }
        let coder = graph.coder();
        if reaches_final(&graph, |t| {
            t.get_input_symbol(coder).as_str() == internal_epsilon
        }) {
            return Err(
                "has a left-hand side that matches the empty string; a pass rewrites only non-empty strings"
                    .to_string(),
            );
        }
        Ok(upper)
    }

    /// The rule bodies of a pass, keyed by the edge they label, and the
    /// copying loop's labels as a transducer. Each body is the union of its
    /// mappings' cross-products, minimised. The loop is harmonized with every
    /// body: the first round grows its alphabet to all of theirs, the second
    /// expands each body's unknown and identity arcs against that whole
    /// alphabet, as the classic compile's harmonizing operations would.
    fn pass_bodies(
        &self,
        rules: &[PassRule<B>],
    ) -> crate::error::Result<(HfstTransducer<B>, Bodies<B>)> {
        use PassState::{Middle, Start};
        let track_first = rules.iter().any(|r| r.placement.depends_on_first());
        let mut labels: BTreeMap<(PassState, PassState), Vec<&HfstTransducer<B>>> = BTreeMap::new();
        for rule in rules {
            for edge in rule.placement.edges(track_first) {
                labels.entry(edge).or_default().extend(&rule.mappings);
            }
        }
        let mut bodies = BTreeMap::new();
        for (edge, mappings) in labels {
            bodies.insert(edge, union_of(&mappings)?);
        }
        // `Middle` exists only after a first rewrite that leads to it.
        if !bodies.contains_key(&(Start, Middle)) {
            bodies.retain(|(from, _), _| *from != Middle);
        }
        let mut copy = HfstTransducer::<B>::identity_pair();
        for _ in 0..2 {
            for body in bodies.values_mut() {
                copy.harmonize(body, true)?;
            }
        }
        for body in bodies.values_mut() {
            body.optimize_with_config(&self.opt_cfg())?;
        }
        Ok((copy, bodies))
    }

    // [spec:hfst:req:xre-replace-pass.shape]
    /// Build the pass automaton: one state per [`PassState`] in use, each
    /// final with a loop that copies any one symbol, and for each edge the
    /// minimised union of the cross-products that label it, entered and left
    /// by epsilon. The loops are never determinised against the rules.
    fn assemble_pass(&self, rules: &[PassRule<B>]) -> crate::error::Result<HfstTransducer<B>> {
        let (copy, bodies) = self.pass_bodies(rules)?;
        let copy = copy.to_basic()?;
        let copy_labels: Vec<_> = copy.state_vector[0]
            .iter()
            .map(|t| {
                (
                    t.get_input_symbol(copy.coder()),
                    t.get_output_symbol(copy.coder()),
                )
            })
            .collect();
        let mut alphabet = copy.get_alphabet().clone();
        let mut graph = HfstBasicTransducer::new();
        let mut states: BTreeMap<PassState, u32> = BTreeMap::from([(PassState::Start, 0)]);
        for &(from, to) in bodies.keys() {
            for p in [from, to] {
                states.entry(p).or_insert_with(|| graph.add_state_new());
            }
        }
        for &s in states.values() {
            for (i, o) in &copy_labels {
                let t = HfstBasicTransition::new_symbols(
                    s,
                    i.clone(),
                    o.clone(),
                    0.0,
                    graph.coder_mut(),
                );
                graph.add_transition(s, &t, true);
            }
            graph.set_final_weight(s, &0.0);
        }
        for ((from, to), body) in &bodies {
            alphabet.extend(body.get_alphabet()?);
            graph.insert_transducer(states[from], states[to], &body.to_basic()?);
        }
        graph.add_symbols_to_alphabet_set(&alphabet);
        HfstTransducer::new_from_basic(&graph)
    }
}
