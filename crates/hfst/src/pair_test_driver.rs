//! The twolc pair-test engine behind `hfst-pair-test`, lifted out of the tool
//! that carried it: the compiled rule grammar and its known-symbol set, the
//! walk that decides whether one rule accepts a tokenized pair string, the
//! composition that works out how far a rejecting rule *did* get, and the twolc
//! string conventions the test cases are written in (the `%`-escaping a test
//! case uses, the `@#@` word boundaries every pair string is wrapped in, the
//! `__HFST_TWOLC_*` mangling a compiled rule's name carries).
//!
//! The tool keeps the parts that are its own: reading the pair-string or twolc
//! source file, recognising its comment and test-case markers, rendering the
//! `Rule X fails: ... HERE ---> ...` report, and the exit-code policy that
//! turns per-rule verdicts into the process's status. Nothing here prints or
//! exits — a failing rule is a returned verdict and a failing operation is a
//! [`crate::error::Result`].

use std::collections::BTreeSet;

use crate::error::Result;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_data_types::{StringPairVector, Symbol};
use crate::hfst_symbol_defs::{internal_epsilon, is_epsilon};
use crate::hfst_transducer::HfstTransducer;

// [spec:hfst:def:hfst-pair-test.basic-transducer-vector]
/// The compiled rules of a twolc grammar, in file order.
pub type BasicTransducerVector = Vec<HfstBasicTransducer>;
// [spec:hfst:def:hfst-pair-test.string-vector]
/// Rule names, and the raw test cases harvested in xerox mode.
pub type StringVector = Vec<String>;
// [spec:hfst:def:hfst-pair-test.symbol-set]
/// The symbols the grammar knows. A pair whose symbols are all absent from
/// this set may still be matched by a rule's identity arc.
pub type SymbolSet = BTreeSet<String>;

/// The word-boundary symbol a twolc rule expects at both ends of a pair
/// string, paired against epsilon on the output side.
const BOUNDARY: &str = "@#@";

/// A rule's verdict on one pair string, in the tool's exit-code encoding: `0`
/// is the outcome the test wanted, `1` the outcome it did not.
///
/// The encoding is deliberately not a `bool`: in a positive test a rule that
/// rejects scores 1, in a negative test a rule that rejects scores 0, and the
/// callers fold these into a process exit status directly.
pub type Verdict = i32;

// [spec:hfst:def:hfst-pair-test.get-target-fn]
// [spec:hfst:sem:hfst-pair-test.get-target-fn]
//
// The state a rule moves to on one pair, or 'u32::MAX' for "no transition".
fn get_target(
    isymbol: &str,
    osymbol: &str,
    s: HfstState,
    t: &HfstBasicTransducer,
    known_symbols: &SymbolSet,
) -> HfstState {
    t.pair_target_state(s, isymbol, osymbol, known_symbols)
        .unwrap_or(u32::MAX)
}

// [spec:hfst:def:hfst-pair-test.is-final-state-fn]
// [spec:hfst:sem:hfst-pair-test.is-final-state-fn]
fn is_final_state(s: HfstState, t: &HfstBasicTransducer) -> bool {
    t.is_final_state(s)
}

// [spec:hfst:def:hfst-pair-test.get-transducer-fn]
// [spec:hfst:sem:hfst-pair-test.get-transducer-fn]
//
// The single-path transducer that spells out one tokenized pair string.
fn get_transducer(
    tokenized_pair_string: &StringPairVector,
) -> Result<HfstTransducer<hfst_openfst::StdVectorFst>> {
    let mut t = HfstBasicTransducer::new();
    let mut s: HfstState = 0;
    for it in tokenized_pair_string.iter() {
        let target = t.add_state_new();
        let tr = HfstBasicTransition::new_symbols(
            target,
            it.0.clone(),
            it.1.clone(),
            0.0,
            t.coder_mut(),
        );
        t.add_transition(s, &tr, true);
        s = target;
    }
    t.set_final_weight(s, &0.0);
    HfstTransducer::new_from_basic(&t)
}

// [spec:hfst:def:hfst-pair-test.unescape-fn]
// [spec:hfst:sem:hfst-pair-test.unescape-fn]
/// The display form of a pair-string symbol: twolc writes epsilon as `0` and
/// the internal word boundary as `#`.
pub fn unescape(symbol: &str) -> String {
    if is_epsilon(symbol) {
        return "0".to_string();
    }
    if symbol == BOUNDARY {
        return "#".to_string();
    }
    symbol.to_string()
}

// replace every occurrence of substr in str with repl, in place.
fn replace_all_substr(substr: &str, repl: &str, str: &mut String) {
    let mut pos = 0;
    while let Some(found) = str[pos..].find(substr) {
        let at = pos + found;
        str.replace_range(at..at + substr.len(), repl);
        pos = at + repl.len();
    }
}

const PTPP: &str = "PAIR_TEST_PERC_PERC";
const PTPC: &str = "PAIR_TEST_PERC_COL";

// [spec:hfst:def:hfst-pair-test.backslash-escape-fn]
// [spec:hfst:sem:hfst-pair-test.backslash-escape-fn]
/// Re-escape a twolc test case for the pair-string tokenizer: twolc source
/// escapes special symbols with `%`, the tokenizer expects `\`.
pub fn backslash_escape(mut perc_escaped: String) -> String {
    replace_all_substr("%%", PTPP, &mut perc_escaped);
    replace_all_substr("%:", PTPC, &mut perc_escaped);
    replace_all_substr("%", "", &mut perc_escaped);
    replace_all_substr(PTPC, "\\:", &mut perc_escaped);
    replace_all_substr(PTPP, "%", &mut perc_escaped);
    perc_escaped
}

// [spec:hfst:def:hfst-pair-test.demangle-fn]
// [spec:hfst:sem:hfst-pair-test.demangle-fn]
/// Recover a rule's source name from the form `hfst-twolc` stored it in.
///
/// Deliberately *not* [`crate::twolc::unescape_name`], which spells the same
/// two replacements as a pair of single-pass `str::replace` calls: this one
/// rescans from the start after every deletion, so a name in which removing a
/// marker splices a fresh marker together loses that one too. The cases differ
/// only on names contrived to straddle a marker, but they do differ.
pub fn demangle_rule_name(mut name: String) -> String {
    let space_subst = "__HFST_TWOLC_SPACE";
    let name_subst = "__HFST_TWOLC_RULE_NAME=";

    while let Some(pos) = name.find(name_subst) {
        name.replace_range(pos..pos + name_subst.len(), "");
    }

    while let Some(pos) = name.find(space_subst) {
        name.replace_range(pos..pos + space_subst.len(), " ");
    }

    name
}

/// Wrap a tokenized pair string in the word boundaries a twolc rule matches
/// against, in place.
pub fn add_word_boundaries(tokenized_pair_string: &mut StringPairVector) {
    tokenized_pair_string.insert(
        0,
        (
            Symbol::new_static(BOUNDARY),
            Symbol::new_static(internal_epsilon),
        ),
    );
    tokenized_pair_string.push((
        Symbol::new_static(BOUNDARY),
        Symbol::new_static(internal_epsilon),
    ));
}

/// A compiled twolc grammar to test pair strings against: the rule
/// transducers, their demangled names, and the symbol set that decides when an
/// identity arc may stand in for an unlisted symbol.
#[derive(Default)]
pub struct PairTestGrammar {
    transducers: BasicTransducerVector,
    names: StringVector,
    known_symbols: SymbolSet,
}

impl PairTestGrammar {
    /// An empty grammar, to be filled with [`PairTestGrammar::push_rule`].
    pub fn new() -> PairTestGrammar {
        PairTestGrammar::default()
    }

    /// Add one compiled rule under `name`, which is demangled on the way in.
    pub fn push_rule(&mut self, rule: HfstBasicTransducer, name: String) {
        self.transducers.push(rule);
        self.names.push(demangle_rule_name(name));
    }

    // [spec:hfst:def:hfst-pair-test.get-symbols-fn]
    // [spec:hfst:sem:hfst-pair-test.get-symbols-fn]
    /// Seed the known-symbol set from the first rule.
    ///
    /// One rule is enough because `hfst-twolc` harmonizes every rule in a
    /// grammar over the same alphabet; an empty grammar leaves the set empty.
    pub fn define_known_symbols(&mut self) {
        if let Some(first) = self.transducers.first() {
            self.known_symbols
                .extend(first.symbols_used().into_iter().map(String::from));
        }
    }

    /// Whether the grammar holds no rules.
    pub fn is_empty(&self) -> bool {
        self.transducers.is_empty()
    }

    /// How many rules the grammar holds.
    pub fn len(&self) -> usize {
        self.transducers.len()
    }

    /// The symbols seeded by [`PairTestGrammar::define_known_symbols`].
    pub fn known_symbols(&self) -> &SymbolSet {
        &self.known_symbols
    }

    /// The demangled name of rule `index`.
    pub fn name(&self, index: usize) -> &str {
        &self.names[index]
    }

    /// Walk `tokenized_pair_string` through rule `index`.
    ///
    /// The rule accepts when every pair has a transition and the walk ends in
    /// a final state. `positive` names what the caller wanted: under a
    /// positive test acceptance scores `0`, under a negative test rejection
    /// does.
    pub fn test_rule(
        &self,
        index: usize,
        tokenized_pair_string: &StringPairVector,
        positive: bool,
    ) -> Verdict {
        let t = &self.transducers[index];
        let mut s: HfstState = 0;
        for it in tokenized_pair_string.iter() {
            s = get_target(&it.0, &it.1, s, t, &self.known_symbols);
            if s == u32::MAX {
                if positive {
                    return 1;
                } else {
                    return 0;
                }
            }
        }

        if is_final_state(s, t) && positive {
            0
        } else if positive {
            1
        } else if !is_final_state(s, t) {
            0
        } else {
            1
        }
    }

    /// How many leading pairs of `tokenized_pair_string` rule `index` can
    /// still recognize — the point a failure report puts its `HERE --->`
    /// marker at.
    ///
    /// The prefix is not read off the rule directly: the pair string is
    /// composed with the rule (after projecting to its input side) and
    /// minimized first, so the answer accounts for the whole rule's
    /// contribution rather than one greedy path through it.
    pub fn recognized_prefix_length(
        &self,
        index: usize,
        tokenized_pair_string: &StringPairVector,
    ) -> Result<usize> {
        let mut str_transducer = get_transducer(tokenized_pair_string)?;
        let rule: HfstTransducer<hfst_openfst::StdVectorFst> =
            HfstTransducer::new_from_basic(&self.transducers[index])?;
        str_transducer.input_project()?;
        str_transducer.compose(&rule, true)?;
        str_transducer.minimize()?;
        let recognizer = HfstBasicTransducer::new_from_transducer(&str_transducer);

        let mut s: HfstState = 0;
        let mut idx = 0;
        while idx < tokenized_pair_string.len() {
            let it = &tokenized_pair_string[idx];
            s = get_target(&it.0, &it.1, s, &recognizer, &self.known_symbols);
            if s == u32::MAX {
                break;
            }
            idx += 1;
        }
        Ok(idx)
    }
}
