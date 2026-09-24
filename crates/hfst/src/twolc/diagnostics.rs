//! Source-anchored twolc diagnostics that point at the pieces at fault, say
//! why, and suggest what to write instead.

use super::*;
use crate::diag::Diagnostic;

/// How many example pairs a help line lists before summarising the rest.
const EXAMPLE_PAIRS: usize = 8;

/// Where a pair was written: the whole pair and each side. A bare set name
/// used as a pair gives all three the same span.
#[derive(Clone)]
pub(super) struct PairSite {
    pub(super) pair: Range<usize>,
    pub(super) upper: Range<usize>,
    pub(super) lower: Range<usize>,
}

impl PairSite {
    pub(super) fn whole(span: &Range<usize>) -> Self {
        Self {
            pair: span.clone(),
            upper: span.clone(),
            lower: span.clone(),
        }
    }
}

/// The pairs a set-naming rule centre declared because the Alphabet did not.
pub(super) struct ImpliedPairs {
    pub(super) span: Range<usize>,
    pub(super) upper: Symbol,
    pub(super) lower: Symbol,
    pub(super) added: Vec<SymbolPair>,
}

/// Where the empty pair set was used. A rule centre gets one more note: C++
/// hfst-twolc dropped such a rule silently, so a grammar that built there
/// fails here for the first time.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum PairUse {
    Centre,
    Context,
}

impl<B: AlgebraBackend> TwolcCompiler<B> {
    /// Render `d`. Silent mode hides warnings, never errors.
    pub(super) fn emit(&self, d: Diagnostic) {
        if d.is_error() || !self.silent {
            d.emit(&self.source_name, &self.source);
        }
    }

    /// Report a pair `input:output` with a set-name side that matches no
    /// declared pair.
    pub(super) fn report_empty_pair_set(
        &self,
        cfg: &OstConfig,
        input: &str,
        output: &str,
        site: &PairSite,
        used: PairUse,
    ) {
        let shown = if input == output {
            Rule::<B>::get_print_name(input)
        } else {
            format!(
                "{}:{}",
                Rule::<B>::get_print_name(input),
                Rule::<B>::get_print_name(output)
            )
        };
        let mut d = Diagnostic::error(format!("The pair set {shown} is empty."))
            .label(site.pair.clone(), "no declared pair matches this");
        for set in [input, output] {
            if let (Some(span), Some(members)) = (self.set_spans.get(set), self.sets.get(set)) {
                d = d.label(
                    span.clone(),
                    format!(
                        "{} is defined here, with {}",
                        Rule::<B>::get_print_name(set),
                        symbols(members.len())
                    ),
                );
            }
            if input == output {
                break;
            }
        }
        d = match used {
            // A centre declares its own pairs, so it is empty only when none
            // of them can exist.
            PairUse::Centre => d
                .note(
                    "None of its pairs can exist: a diacritic pairs only with \
                     itself, and an empty set has no members.",
                )
                .note(
                    "C++ hfst-twolc dropped a rule like this without a word, so \
                     it has never had any effect.",
                ),
            PairUse::Context => d.note(format!(
                "{} A set in a context stands for the declared pairs it covers; \
                 it does not declare any. A pair written out in full anywhere \
                 in the grammar is declared automatically.",
                self.why_empty(input, output)
            )),
        };
        self.emit(d.help(self.help_for_empty(cfg, input, output)));
    }

    /// Warn that a set-naming rule centre declared pairs the Alphabet lacks.
    pub(super) fn report_implied_pairs(&self, implied: &ImpliedPairs) {
        let shown = format!(
            "{}:{}",
            Rule::<B>::get_print_name(&implied.upper),
            Rule::<B>::get_print_name(&implied.lower)
        );
        let listed = example_list(implied.added.iter().map(|(x, y)| {
            format!(
                "{}:{}",
                Rule::<B>::get_print_name(x),
                Rule::<B>::get_print_name(y)
            )
        }));
        let count = match implied.added.len() {
            1 => "1 pair".to_string(),
            n => format!("{n} pairs"),
        };
        self.emit(
            Diagnostic::warning(format!(
                "The rule centre {shown} declares {count} the Alphabet does not list."
            ))
            .label(implied.span.clone(), format!("declares {count}"))
            .note(
                "A set in a rule centre stands for one subrule per pair, like a \
                 where-variable, and a rule declares the pairs it controls.",
            )
            .help(format!(
                "List them in the Alphabet to silence this warning: {listed}"
            )),
        );
    }

    fn why_empty(&self, input: &str, output: &str) -> String {
        let count = |s: &str| symbols(self.sets.get(s).map_or(1, Vec::len));
        let upper = Rule::<B>::get_print_name(input);
        let lower = Rule::<B>::get_print_name(output);
        match (
            self.sets.contains_key(input),
            self.sets.contains_key(output),
        ) {
            (true, true) if input == output => {
                format!("No declared pair has both sides in {upper}.")
            }
            (true, true) => format!(
                "No declared pair has its upper side in {upper} ({}) \
                 and its lower side in {lower} ({}).",
                count(input),
                count(output)
            ),
            (true, false) if output == TWOLC_UNKNOWN => format!(
                "No symbol in {upper} ({}) is the upper side of a declared pair.",
                count(input)
            ),
            (true, false) => format!(
                "No symbol in {upper} ({}) is declared with {lower} as its lower side.",
                count(input)
            ),
            (false, true) if input == TWOLC_UNKNOWN => format!(
                "No symbol in {lower} ({}) is the lower side of a declared pair.",
                count(output)
            ),
            (false, true) => format!(
                "{upper} is not declared with any symbol in {lower} ({}) as its lower side.",
                count(output)
            ),
            (false, false) => format!("No declared pair matches {upper}:{lower}."),
        }
    }

    fn help_for_empty(&self, cfg: &OstConfig, input: &str, output: &str) -> String {
        let generic = "Declare at least one pair it should cover in the Alphabet.";
        if input == TWOLC_UNKNOWN || output == TWOLC_UNKNOWN {
            return generic.to_string();
        }
        // A bare set name means its identity pairs; otherwise every pairing
        // of the two sides.
        let examples: Vec<String> = if input == output {
            self.set_of(input)
                .iter()
                .map(|m| {
                    format!(
                        "{}:{}",
                        Rule::<B>::get_print_name(m),
                        Rule::<B>::get_print_name(m)
                    )
                })
                .collect()
        } else {
            let lower = self.set_of(output);
            self.set_of(input)
                .iter()
                .flat_map(|x| {
                    lower
                        .iter()
                        .filter(move |y| x == *y || !cfg.diacritics.contains(x.as_str()))
                        .map(move |y| {
                            format!(
                                "{}:{}",
                                Rule::<B>::get_print_name(x),
                                Rule::<B>::get_print_name(y)
                            )
                        })
                })
                .collect()
        };
        if examples.is_empty() {
            return format!("{generic} A diacritic pairs only with itself.");
        }
        format!(
            "Declare the pairs it should cover in the Alphabet: {}",
            example_list(examples.into_iter())
        )
    }

    /// Warn that a diacritic's partner in a pair is ignored.
    pub(super) fn report_diacritic_pair(&self, input: &str, output: &str, site: &PairSite) {
        let (d, o) = (
            Rule::<B>::get_print_name(input),
            Rule::<B>::get_print_name(output),
        );
        self.emit(
            Diagnostic::warning(format!("Diacritic {d} in pair {d}:{o} will correspond 0."))
                .label(site.upper.clone(), format!("{d} is declared a diacritic"))
                .label(site.lower.clone(), "so this side is ignored"),
        );
    }

    /// Warn about a symbol no section declares, and suggest the declared name
    /// it most likely meant.
    pub(super) fn report_undeclared_symbol(
        &self,
        symbol: &str,
        span: Range<usize>,
        declared: &BTreeSet<Symbol>,
        definitions: &BTreeSet<Symbol>,
    ) {
        let shown = Rule::<B>::get_print_name(symbol);
        let d = Diagnostic::warning(format!(
            "Symbol '{shown}' is not declared in the Alphabet; treating it as a literal symbol."
        ))
        .label(span, "not declared");
        let help = match nearest_declared(symbol, declared) {
            Some(near) => {
                let kind = if self.sets.contains_key(near.as_str()) {
                    "the set"
                } else if definitions.contains(near) {
                    "the definition"
                } else {
                    "the symbol"
                };
                format!("Did you mean {kind} {}?", Rule::<B>::get_print_name(near))
            }
            None => format!("If '{shown}' is meant, declare it in the Alphabet."),
        };
        self.emit(d.help(help));
    }

    /// Report why a rule or definition failed to compile, unless the failure
    /// was already reported where it happened.
    pub(super) fn report_failure(&self, e: &crate::error::Error, span: &Range<usize>, what: &str) {
        if e.kind == crate::error::ErrorKind::EmptySymbolPairSet {
            return;
        }
        self.emit(
            Diagnostic::error(format!("{what} could not be compiled: {e}"))
                .label(span.clone(), "while compiling this"),
        );
    }

    /// Render the parser's diagnostics, notes included.
    pub(super) fn report_parse_errors(&self, diagnostics: &[nfst_twolc::Diagnostic]) {
        for pd in diagnostics {
            let mut d = Diagnostic::error(pd.message.clone()).label(pd.span.range.clone(), "here");
            for n in &pd.notes {
                d = d.note(n.clone());
            }
            self.emit(d);
        }
    }
}

/// The first few pairs, then a count of the rest.
fn example_list(pairs: impl Iterator<Item = String>) -> String {
    let pairs: Vec<String> = pairs.collect();
    let mut shown = pairs[..pairs.len().min(EXAMPLE_PAIRS)].join(" ");
    if pairs.len() > EXAMPLE_PAIRS {
        shown.push_str(&format!(" and {} more", pairs.len() - EXAMPLE_PAIRS));
    }
    shown
}

fn symbols(n: usize) -> String {
    if n == 1 {
        "1 symbol".to_string()
    } else {
        format!("{n} symbols")
    }
}

/// The declared name `symbol` most likely stands for: the same word in a
/// different case, else one a single edit away. Internal markers never count.
fn nearest_declared<'a>(symbol: &str, declared: &'a BTreeSet<Symbol>) -> Option<&'a Symbol> {
    let visible = || {
        declared
            .iter()
            .filter(|d| !d.as_str().starts_with("__HFST_TWOLC_") && !d.as_str().starts_with("@_"))
    };
    let folded = symbol.to_lowercase();
    visible()
        .find(|d| d.as_str().to_lowercase() == folded)
        .or_else(|| {
            if symbol.chars().count() < 3 {
                return None;
            }
            visible().find(|d| within_one_edit(symbol, d.as_str()))
        })
}

/// Whether `a` becomes `b` by one insertion, deletion or substitution.
fn within_one_edit(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (short, long) = if a.len() <= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    if long.len() - short.len() > 1 {
        return false;
    }
    let prefix = short
        .iter()
        .zip(long.iter())
        .take_while(|(x, y)| x == y)
        .count();
    if prefix == short.len() {
        return long.len() > short.len();
    }
    let rest = if short.len() == long.len() {
        prefix + 1
    } else {
        prefix
    };
    short[rest..] == long[prefix + 1..]
}

#[cfg(test)]
mod tests {
    use super::within_one_edit;

    #[test]
    fn one_edit_covers_insert_delete_and_substitute() {
        assert!(within_one_edit("Vow", "Vowl"));
        assert!(within_one_edit("Vowl", "Vow"));
        assert!(within_one_edit("Cns", "Cnz"));
        assert!(within_one_edit("Cns", "Cs"));
        assert!(!within_one_edit("Cns", "Cns"));
        assert!(!within_one_edit("Vow", "Cons"));
        assert!(!within_one_edit("abc", "abcde"));
    }
}
