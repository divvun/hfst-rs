//! Looking a word up in a DHFST acceptor ([`crate::dhfst_acceptor`]) as
//! divvunspell's speller does when it checks a word against its lexicon.
//! Authored greenfield against `docs/spec/javascript.md`. The consumer of
//! record is divvunspell: `HfstSpeller::to_input_vec_lexicon`
//! (`src/speller/mod.rs`) turns the word into lexicon symbols, and
//! `SpellerWorker::accepting_weight` (`src/speller/worker.rs`) walks the
//! lexicon alone, taking free arcs as `lexicon_free_arc` does, flag
//! diacritics as `TreeNode::apply_operation` (`src/transducer/tree_node.rs`)
//! does, and regular arcs as `lexicon_consume` does. The symbols are read as
//! `TransducerAlphabetParser` (`src/transducer/hfst/alphabet.rs`) reads them.
//!
//! The word is split into extended grapheme clusters, and each cluster is the
//! symbol whose name is exactly its text, else the acceptor's
//! `@_UNKNOWN_SYMBOL_@`. A cluster that is neither leaves the word with no
//! path. Nothing else is tried: no longest match over multicharacter symbols,
//! no other normalisation form of a name, and no identity or default arc,
//! since the speller tries none of them.
//!
//! divvunspell walks every path depth first, which does not end on a cycle of
//! free arcs. This walk is best first over (state, input position, flag
//! state), takes each of those at most once, and stops at the first accepting
//! path it takes from the queue, so it ends on any acceptor and any word.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use icu::segmenter::GraphemeClusterSegmenter;

use crate::dhfst_acceptor::AcceptorReader;

/// What a flag diacritic does, as divvunspell's `FlagDiacriticOperator`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlagOperator {
    /// `P`: set the feature to the value.
    PositiveSet,
    /// `N`: set the feature to the value negated.
    NegativeSet,
    /// `R`: pass only if the feature has the value; with no value, any value.
    Require,
    /// `D`: pass only if the feature does not have the value; with no value,
    /// only if it has none.
    Disallow,
    /// `C`: unset the feature.
    Clear,
    /// `U`: pass and set the value if the feature is unset, has the value,
    /// or is negated to another value.
    Unify,
}

impl FlagOperator {
    fn from_byte(byte: u8) -> Option<FlagOperator> {
        match byte {
            b'P' => Some(FlagOperator::PositiveSet),
            b'N' => Some(FlagOperator::NegativeSet),
            b'R' => Some(FlagOperator::Require),
            b'D' => Some(FlagOperator::Disallow),
            b'C' => Some(FlagOperator::Clear),
            b'U' => Some(FlagOperator::Unify),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FlagOperation {
    operator: FlagOperator,
    feature: usize,
    value: i32,
}

/// What a flag diacritic does to a feature holding `current`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlagOutcome {
    Blocked,
    Unchanged,
    Set(i32),
}

impl FlagOperation {
    /// The test and update of divvunspell's `TreeNode::apply_operation`.
    fn outcome(self, current: i32) -> FlagOutcome {
        let pass = |ok: bool| {
            if ok {
                FlagOutcome::Unchanged
            } else {
                FlagOutcome::Blocked
            }
        };
        match self.operator {
            FlagOperator::PositiveSet => FlagOutcome::Set(self.value),
            FlagOperator::NegativeSet => FlagOutcome::Set(-self.value),
            FlagOperator::Require if self.value == 0 => pass(current != 0),
            FlagOperator::Require => pass(current == self.value),
            FlagOperator::Disallow if self.value == 0 => pass(current == 0),
            FlagOperator::Disallow => pass(current != self.value),
            FlagOperator::Clear => FlagOutcome::Set(0),
            FlagOperator::Unify
                if current == 0
                    || current == self.value
                    || (current < 0 && -current != self.value) =>
            {
                FlagOutcome::Set(self.value)
            }
            FlagOperator::Unify => FlagOutcome::Blocked,
        }
    }
}

/// An acceptor's symbols as divvunspell's alphabet parser reads them: the
/// symbol each piece of input text is, the unknown symbol, and what each flag
/// diacritic does.
#[derive(Clone, Debug, Default)]
pub(crate) struct LexiconAlphabet {
    by_text: BTreeMap<String, u16>,
    unknown: Option<u16>,
    flags: Vec<Option<FlagOperation>>,
    features: usize,
}

impl LexiconAlphabet {
    /// The alphabet of an acceptor whose symbols are `names`. A name longer
    /// than one byte that starts and ends with `@` is never input text: it is
    /// a flag diacritic when it is at least five bytes long with `.` as its
    /// third byte, and otherwise epsilon, the unknown symbol, or a symbol no
    /// input reaches. When two symbols have one name the later one is the
    /// input's. Features and values are numbered in the order they first
    /// appear, and each epsilon name numbers the empty value afresh. Symbol 0
    /// is epsilon, so a flag with no value has value 0, which is also what a
    /// feature holds before any flag sets it.
    pub(crate) fn new(names: &[String]) -> LexiconAlphabet {
        let mut alphabet = LexiconAlphabet {
            flags: vec![None; names.len()],
            ..LexiconAlphabet::default()
        };
        let mut features: BTreeMap<String, usize> = BTreeMap::new();
        let mut values: BTreeMap<String, i32> = BTreeMap::new();
        let mut next_value = 0i32;
        for (symbol, name) in names.iter().enumerate() {
            let number =
                u16::try_from(symbol).expect("parsing refuses more symbols than a u16 numbers");
            let bytes = name.as_bytes();
            if !(bytes.len() > 1 && name.starts_with('@') && name.ends_with('@')) {
                alphabet.by_text.insert(name.clone(), number);
            } else if bytes.len() >= 5 && bytes[2] == b'.' {
                // Parsing refuses a flag-shaped name with another operator.
                let Some(operator) = FlagOperator::from_byte(bytes[1]) else {
                    continue;
                };
                let mut chunks = name.split('.').skip(1);
                let mut part = || -> String {
                    chunks
                        .next()
                        .unwrap_or("")
                        .chars()
                        .filter(|c| *c != '@')
                        .collect()
                };
                let (feature, value) = (part(), part());
                let n_features = features.len();
                let feature = *features.entry(feature).or_insert(n_features);
                let value = *values.entry(value).or_insert_with(|| {
                    next_value += 1;
                    next_value - 1
                });
                alphabet.flags[symbol] = Some(FlagOperation {
                    operator,
                    feature,
                    value,
                });
            } else if name == "@_EPSILON_SYMBOL_@" {
                values.insert(String::new(), next_value);
                next_value += 1;
            } else if name == "@_UNKNOWN_SYMBOL_@" {
                alphabet.unknown = Some(number);
            }
        }
        alphabet.features = features.len();
        alphabet
    }

    /// The symbol named by `cluster`, else the unknown symbol.
    fn symbol(&self, cluster: &str) -> Option<u16> {
        self.by_text.get(cluster).copied().or(self.unknown)
    }

    fn flag(&self, symbol: u16) -> Option<FlagOperation> {
        self.flags.get(symbol as usize).copied().flatten()
    }
}

/// The symbols of `word`, one per extended grapheme cluster, or `None` when
/// some cluster is no symbol.
//
// The segmenter follows the Unicode version icu ships, and divvunspell's
// unic_segment 0.9 follows Unicode 10, so the two can split differently on
// clusters that later versions changed, such as emoji sequences and Indic
// conjuncts.
fn input_symbols(alphabet: &LexiconAlphabet, word: &str) -> Option<Vec<u16>> {
    let bounds: Vec<usize> = GraphemeClusterSegmenter::new().segment_str(word).collect();
    bounds
        .windows(2)
        .filter(|pair| pair[0] < pair[1])
        .map(|pair| alphabet.symbol(&word[pair[0]..pair[1]]))
        .collect()
}

/// Flag states, each a value per feature, numbered as they are first reached.
struct FlagStates {
    values: Vec<Vec<i32>>,
    numbers: BTreeMap<Vec<i32>, u32>,
}

impl FlagStates {
    /// The start state, every feature unset, is number 0.
    fn new(features: usize) -> FlagStates {
        let start = vec![0; features];
        FlagStates {
            values: vec![start.clone()],
            numbers: BTreeMap::from([(start, 0)]),
        }
    }

    /// The state after `operation` from state `state`, or `None` when the
    /// flag blocks the path.
    fn apply(&mut self, state: u32, operation: FlagOperation) -> Option<u32> {
        let values = &self.values[state as usize];
        let next = match operation.outcome(values[operation.feature]) {
            FlagOutcome::Blocked => return None,
            FlagOutcome::Unchanged => return Some(state),
            FlagOutcome::Set(value) if value == values[operation.feature] => return Some(state),
            FlagOutcome::Set(value) => {
                let mut next = values.clone();
                next[operation.feature] = value;
                next
            }
        };
        if let Some(number) = self.numbers.get(&next) {
            return Some(*number);
        }
        let number =
            u32::try_from(self.values.len()).expect("fewer flag states than a u32 numbers");
        self.values.push(next.clone());
        self.numbers.insert(next, number);
        Some(number)
    }
}

/// What the walk takes next from its queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    /// A path that has consumed the whole input and ended in a final state.
    Accept,
    /// A state reached with `position` symbols of the input consumed.
    Visit {
        state: u32,
        position: usize,
        flags: u32,
    },
}

/// A step and the weight of the path to it, ordered for a max-heap so that
/// the lightest comes out first and, at equal weight, an accepting path.
#[derive(Clone, Copy, Debug)]
struct Queued {
    weight: f32,
    step: Step,
}

impl Ord for Queued {
    fn cmp(&self, other: &Queued) -> Ordering {
        other
            .weight
            .total_cmp(&self.weight)
            .then_with(|| other.step.cmp(&self.step))
    }
}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Queued) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Queued {
    fn eq(&self, other: &Queued) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Queued {}

impl AcceptorReader<'_> {
    /// The least weight of a path that accepts `word`, final weight included,
    /// or `None` when no path does: the weight divvunspell's speller finds for
    /// the word as typed, checking it against this acceptor alone.
    ///
    /// A lexicon's weights are costs and not negative, and then the first
    /// accepting path the walk takes is the lightest. A negative weight can
    /// make a path lighter than one the walk has already taken, and the walk
    /// does not go back for it.
    // [spec:hfst:req:javascript.dhfst+1]
    pub fn lookup(&self, word: &str) -> Option<f32> {
        let alphabet = self.lexicon_alphabet();
        let input = input_symbols(alphabet, word)?;
        let mut flag_states = FlagStates::new(alphabet.features);
        let mut taken: BTreeSet<(u32, usize, u32)> = BTreeSet::new();
        let mut queue = BinaryHeap::from([Queued {
            weight: 0.0,
            step: Step::Visit {
                state: 0,
                position: 0,
                flags: 0,
            },
        }]);
        while let Some(Queued { weight, step }) = queue.pop() {
            let (state, position, flags) = match step {
                Step::Accept => return Some(weight),
                Step::Visit {
                    state,
                    position,
                    flags,
                } => (state, position, flags),
            };
            if !taken.insert((state, position, flags)) {
                continue;
            }
            let visit = |state, position, flags, weight| Queued {
                weight,
                step: Step::Visit {
                    state,
                    position,
                    flags,
                },
            };
            if position == input.len()
                && let Some(final_weight) = self.final_weight(state)
            {
                queue.push(Queued {
                    weight: weight + final_weight,
                    step: Step::Accept,
                });
            }
            self.for_each_free_arc(state, |symbol, target, arc_weight| {
                let next_flags = if symbol == 0 {
                    Some(flags)
                } else {
                    alphabet
                        .flag(symbol)
                        .and_then(|operation| flag_states.apply(flags, operation))
                };
                if let Some(next_flags) = next_flags {
                    queue.push(visit(target, position, next_flags, weight + arc_weight));
                }
            });
            if let Some(&symbol) = input.get(position) {
                self.for_each_arc(state, symbol, |target, arc_weight| {
                    queue.push(visit(target, position + 1, flags, weight + arc_weight));
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dhfst_acceptor_writer::tests::{LEXICON, lexicon, written};

    fn alphabet(names: &[&str]) -> LexiconAlphabet {
        let names: Vec<String> = names.iter().map(|name| name.to_string()).collect();
        LexiconAlphabet::new(&names)
    }

    fn flag(operator: FlagOperator, feature: usize, value: i32) -> Option<FlagOperation> {
        Some(FlagOperation {
            operator,
            feature,
            value,
        })
    }

    // [spec:hfst:req:javascript.dhfst+1/test]
    #[test]
    fn symbols_are_read_as_divvunspell_reads_them() {
        let read = alphabet(&[
            "@_EPSILON_SYMBOL_@",
            "@R.CASE@",
            "@P.CASE.NOM@",
            "@N.NUM.SG@",
            "@U.CASE.NOM@",
            "@_UNKNOWN_SYMBOL_@",
            "a",
            "@@",
            "a",
        ]);
        assert_eq!(read.flag(1), flag(FlagOperator::Require, 0, 0));
        assert_eq!(read.flag(2), flag(FlagOperator::PositiveSet, 0, 1));
        assert_eq!(read.flag(3), flag(FlagOperator::NegativeSet, 1, 2));
        assert_eq!(read.flag(4), flag(FlagOperator::Unify, 0, 1));
        assert_eq!((read.features, read.flag(6)), (2, None));
        assert_eq!(
            read.symbol("a"),
            Some(8),
            "the later of two symbols named a"
        );
        assert_eq!(read.symbol("@@"), Some(5), "an @ name is not input text");
        assert_eq!(read.symbol("q"), Some(5));
        assert_eq!(alphabet(&["@_EPSILON_SYMBOL_@", "a"]).symbol("q"), None);
    }

    // [spec:hfst:req:javascript.dhfst+1/test]
    #[test]
    fn flags_pass_and_set_as_divvunspell_does() {
        use FlagOperator::{Clear, Disallow, NegativeSet, PositiveSet, Require, Unify};
        use FlagOutcome::{Blocked, Set, Unchanged};
        let cases = [
            (PositiveSet, 1, 2, Set(1)),
            (NegativeSet, 1, 0, Set(-1)),
            (Require, 0, 0, Blocked),
            (Require, 0, -1, Unchanged),
            (Require, 1, 1, Unchanged),
            (Require, 1, -1, Blocked),
            (Disallow, 0, 0, Unchanged),
            (Disallow, 0, 2, Blocked),
            (Disallow, 1, 1, Blocked),
            (Disallow, 1, -1, Unchanged),
            (Clear, 0, 2, Set(0)),
            (Unify, 1, 0, Set(1)),
            (Unify, 1, 1, Set(1)),
            (Unify, 1, -1, Blocked),
            (Unify, 1, -2, Set(1)),
            (Unify, 1, 2, Blocked),
        ];
        for (operator, value, current, outcome) in cases {
            let operation = FlagOperation {
                operator,
                feature: 0,
                value,
            };
            assert_eq!(
                operation.outcome(current),
                outcome,
                "{operation:?} on {current}"
            );
        }
    }

    // [spec:hfst:req:javascript.dhfst+1/test]
    #[test]
    fn the_lightest_path_wins_through_epsilons_and_flags() {
        let bytes = written(&lexicon(LEXICON), 1).bytes;
        let reader = AcceptorReader::parse(&bytes).expect("the written file loads");
        for (word, weight) in [
            ("ac", Some(4.0)),
            ("c", Some(3.5)),
            ("cg", Some(2.125)),
            ("acg", Some(2.625)),
            ("bd", Some(2.75)),
            ("e", Some(3.75)),
            ("f", Some(2.5)),
            ("a", None),
            ("", None),
        ] {
            assert_eq!(reader.lookup(word), weight, "{word:?}");
        }
    }
}
