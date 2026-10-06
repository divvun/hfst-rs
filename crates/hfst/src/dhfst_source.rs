//! The source of a DHFST error model ([`crate::dhfst`]): an error model as
//! plain states and arcs, read from a weighted optimized-lookup transducer
//! exactly as divvunspell's suggestion search reads that transducer from its
//! file. Authored greenfield against `docs/spec/port/back-ends/dhfst/dhfst.md`;
//! the writer is [`crate::dhfst_writer`].

use std::collections::HashMap;

use crate::dhfst::DEFAULT_BASE;
use crate::hfst_transducer::HfstTransducer;
use crate::transducer::{
    NO_SYMBOL_NUMBER, NO_TABLE_INDEX, TRANSITION_TARGET_TABLE_START, Transducer, WeightedTables,
};

fn unsupported(detail: impl std::fmt::Display) -> crate::error::Error {
    crate::err!(
        Hfst,
        format!("the error model cannot be written as DHFST: {detail}")
    )
}

/// One arc of the source model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceArc {
    /// input symbol
    pub input: u16,
    /// output symbol
    pub output: u16,
    /// target state
    pub target: u32,
    /// weight
    pub weight: f32,
}

/// One state of the source model.
#[derive(Clone, Debug, Default)]
pub struct SourceState {
    /// final weight, if final
    pub final_weight: Option<f32>,
    /// arcs, in any order
    pub arcs: Vec<SourceArc>,
}

/// An error model as plain states and arcs, state 0 being the start, each
/// state's arcs sorted by `(input, output, target, weight)` with exact
/// duplicates dropped.
#[derive(Clone, Debug)]
pub struct SourceModel {
    pub(crate) symbols: Vec<String>,
    pub(crate) states: Vec<SourceState>,
    pub(crate) duplicate_arcs: u64,
}

impl SourceModel {
    /// A model from plain states, state 0 being the start. Arcs are sorted and
    /// exact duplicates dropped; targets, symbols and weights are checked.
    // [spec:hfst:def:dhfst.source-model]
    pub fn new(
        symbols: Vec<String>,
        mut states: Vec<SourceState>,
    ) -> crate::error::Result<SourceModel> {
        check_symbol_count(symbols.len())?;
        if symbols[0] != "@_EPSILON_SYMBOL_@" {
            return Err(unsupported("symbol 0 must be @_EPSILON_SYMBOL_@"));
        }
        if states.is_empty() {
            return Err(unsupported("the model has no states"));
        }
        let n_states = states.len();
        let mut duplicate_arcs = 0u64;
        for (q, state) in states.iter_mut().enumerate() {
            check_state(q, state, symbols.len(), n_states)?;
            let before = state.arcs.len();
            state
                .arcs
                .sort_by_key(|a| (a.input, a.output, a.target, a.weight.to_bits()));
            state
                .arcs
                .dedup_by_key(|a| (a.input, a.output, a.target, a.weight.to_bits()));
            duplicate_arcs += (before - state.arcs.len()) as u64;
        }
        Ok(SourceModel {
            symbols,
            states,
            duplicate_arcs,
        })
    }

    /// Read every state reachable from the start of a weighted
    /// optimized-lookup transducer exactly as divvunspell's suggestion search
    /// reads that transducer from its file: the same arcs, no more and no
    /// fewer, and the states numbered in the order the search first meets
    /// them. Refuses a model with flag diacritic arcs, which the search never
    /// follows, and a symbol table divvunspell would refuse to load.
    // [spec:hfst:def:dhfst.source-model]
    // [spec:hfst:sem:dhfst.source-model]
    pub fn from_olw(
        t: &HfstTransducer<Transducer<WeightedTables>>,
    ) -> crate::error::Result<SourceModel> {
        let ol = OlView::new(&t.fst)?;
        let n_symbols = ol.symbols.len();
        check_symbol_count(n_symbols)?;

        let mut ids: HashMap<u32, u32> = HashMap::new();
        let mut order: Vec<u32> = vec![0];
        ids.insert(0, 0);
        let mut states: Vec<SourceState> = Vec::new();
        // Each state's arcs are gathered here and then copied out at their
        // exact length: a model can have tens of millions of arcs, and growth
        // slack on every state would add up.
        let mut arcs: Vec<SourceArc> = Vec::new();
        let mut cursor = 0usize;
        while cursor < order.len() {
            let address = order[cursor];
            cursor += 1;
            if let Some(symbol) = ol.first_flag_arc(address) {
                return Err(unsupported(format!(
                    "state at {address} has a flag diacritic arc ({}); the suggestion search never follows those",
                    ol.symbols[symbol as usize]
                )));
            }
            arcs.clear();
            for input in 0..n_symbols as u16 {
                ol.for_each_arc(address, input, |output, target, weight| {
                    let next_id = order.len() as u32;
                    let id = *ids.entry(target).or_insert_with(|| {
                        order.push(target);
                        next_id
                    });
                    arcs.push(SourceArc {
                        input,
                        output,
                        target: id,
                        weight,
                    });
                });
            }
            let final_weight = if ol.is_final(address) {
                ol.final_weight(address)
            } else {
                None
            };
            states.push(SourceState {
                final_weight,
                arcs: arcs.to_vec(),
            });
        }
        SourceModel::new(ol.symbols.iter().map(|s| s.to_string()).collect(), states)
    }

    /// Total arcs.
    pub fn arc_count(&self) -> u64 {
        self.states.iter().map(|s| s.arcs.len() as u64).sum()
    }

    /// Symbol names, `@_EPSILON_SYMBOL_@` first.
    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }

    /// States; arc targets index this.
    pub fn states(&self) -> &[SourceState] {
        &self.states
    }

    /// Arcs dropped because an identical arc (same state, pair, target and
    /// weight) was already there.
    pub fn duplicate_arcs(&self) -> u64 {
        self.duplicate_arcs
    }
}

fn check_symbol_count(n_symbols: usize) -> crate::error::Result<()> {
    if n_symbols == 0 || n_symbols > DEFAULT_BASE as usize {
        return Err(unsupported(format!("{n_symbols} symbols is out of range")));
    }
    Ok(())
}

/// A state's weights and symbols: no NaN weight, no final weight of minus
/// infinity, no symbol outside the alphabet, no target past `n_states`.
fn check_state(
    q: usize,
    state: &SourceState,
    n_symbols: usize,
    n_states: usize,
) -> crate::error::Result<()> {
    for arc in &state.arcs {
        if arc.weight.is_nan() {
            return Err(unsupported(format!("state {q} has a NaN weight")));
        }
        if arc.input as usize >= n_symbols || arc.output as usize >= n_symbols {
            return Err(unsupported(format!(
                "state {q} has an arc {}:{} outside the alphabet",
                arc.input, arc.output
            )));
        }
        if arc.target as usize >= n_states {
            return Err(unsupported(format!(
                "state {q} has an arc to missing state {}",
                arc.target
            )));
        }
    }
    if state
        .final_weight
        .is_some_and(|w| w.is_nan() || w == f32::NEG_INFINITY)
    {
        return Err(unsupported(format!(
            "state {q} has an invalid final weight"
        )));
    }
    Ok(())
}

/// Whether a symbol name is a flag diacritic as divvunspell's alphabet
/// parser tells one: `@`, an operator, `.`, at least five bytes, `@` last.
/// `Err` for a name of that shape whose operator divvunspell does not know,
/// since it then refuses to load the whole model.
pub(crate) fn flag_name(name: &str) -> crate::error::Result<bool> {
    let shaped = name.len() >= 5
        && name.starts_with('@')
        && name.ends_with('@')
        && name.as_bytes().get(2) == Some(&b'.');
    if shaped && !matches!(name.get(1..2), Some("P" | "N" | "R" | "D" | "C" | "U")) {
        return Err(unsupported(format!(
            "symbol {name} looks like a flag diacritic but its operator is not one of P N R D C U, so divvunspell cannot load the model"
        )));
    }
    Ok(shaped)
}

/// The transition-table half of the optimized-lookup address space.
pub(crate) const TARGET_TABLE: u32 = TRANSITION_TARGET_TABLE_START;

/// The weighted optimized-lookup tables as divvunspell's HFST reader walks
/// them: the same cursor moves, bounded by the header's table sizes, with
/// `0xFFFF` and `0xFFFFFFFF` read as "no symbol" and "no target".
pub(crate) struct OlView<'a> {
    t: &'a Transducer<WeightedTables>,
    pub(crate) symbols: &'a [crate::hfst_data_types::Symbol],
    pub(crate) index_size: u32,
    pub(crate) target_size: u32,
    flags: Vec<bool>,
}

impl<'a> OlView<'a> {
    pub(crate) fn new(t: &'a Transducer<WeightedTables>) -> crate::error::Result<OlView<'a>> {
        let header = t.get_header();
        let symbols = t.get_symbol_table();
        if symbols.len() != header.symbol_count() as usize {
            return Err(unsupported(format!(
                "the symbol table has {} symbols, the header says {}",
                symbols.len(),
                header.symbol_count()
            )));
        }
        let flags = symbols
            .iter()
            .map(|s| flag_name(s))
            .collect::<crate::error::Result<Vec<bool>>>()?;
        Ok(OlView {
            t,
            symbols,
            index_size: header.index_table_size(),
            target_size: header.target_table_size(),
            flags,
        })
    }

    pub(crate) fn is_flag(&self, symbol: u16) -> bool {
        self.flags.get(symbol as usize).copied().unwrap_or(false)
    }

    pub(crate) fn index_input(&self, i: u32) -> Option<u16> {
        (i < self.index_size)
            .then(|| self.t.get_index_input(i))
            .filter(|s| *s != NO_SYMBOL_NUMBER)
    }

    pub(crate) fn index_target(&self, i: u32) -> Option<u32> {
        (i < self.index_size)
            .then(|| self.t.get_index_target(i))
            .filter(|v| *v != NO_TABLE_INDEX)
    }

    pub(crate) fn trans_input(&self, i: u32) -> Option<u16> {
        (i < self.target_size)
            .then(|| self.t.get_transition_input(i))
            .filter(|s| *s != NO_SYMBOL_NUMBER)
    }

    pub(crate) fn trans_output(&self, i: u32) -> Option<u16> {
        (i < self.target_size)
            .then(|| self.t.get_transition_output(i))
            .filter(|s| *s != NO_SYMBOL_NUMBER)
    }

    pub(crate) fn trans_target(&self, i: u32) -> Option<u32> {
        (i < self.target_size)
            .then(|| self.t.get_transition_target(i))
            .filter(|v| *v != NO_TABLE_INDEX)
    }

    pub(crate) fn trans_weight(&self, i: u32) -> Option<f32> {
        (i < self.target_size).then(|| self.t.get_transition_weight(i))
    }

    /// Whether index-table entry `i` marks a final state: no symbol, and a
    /// target field that is not "no target".
    pub(crate) fn index_is_final(&self, i: u32) -> bool {
        self.index_input(i).is_none() && self.index_target(i).is_some()
    }

    /// The final weight index-table entry `i` holds in its target field.
    pub(crate) fn index_final_weight(&self, i: u32) -> Option<f32> {
        (i < self.index_size).then(|| f32::from_bits(self.t.get_index_target(i)))
    }

    /// Whether transition-table record `i` marks a final state: no symbols,
    /// and target 1.
    pub(crate) fn trans_is_final(&self, i: u32) -> bool {
        self.trans_input(i).is_none()
            && self.trans_output(i).is_none()
            && self.trans_target(i) == Some(1)
    }

    pub(crate) fn is_final(&self, address: u32) -> bool {
        if address >= TARGET_TABLE {
            self.trans_is_final(address - TARGET_TABLE)
        } else {
            self.index_is_final(address)
        }
    }

    pub(crate) fn final_weight(&self, address: u32) -> Option<f32> {
        if address >= TARGET_TABLE {
            self.trans_weight(address - TARGET_TABLE)
        } else {
            self.index_final_weight(address)
        }
    }

    /// Whether the cursor at `i` (one past a state) has arcs on `symbol`.
    pub(crate) fn has_transitions(&self, i: u32, symbol: u16) -> bool {
        if i >= TARGET_TABLE {
            self.trans_input(i - TARGET_TABLE) == Some(symbol)
        } else {
            self.index_input(i.wrapping_add(symbol as u32)) == Some(symbol)
        }
    }

    /// The first transition of `state` on `symbol`, as a transition-table
    /// offset.
    pub(crate) fn next(&self, state: u32, symbol: u16) -> Option<u32> {
        if state >= TARGET_TABLE {
            Some(state - TARGET_TABLE + 1)
        } else {
            self.index_target(state.wrapping_add(symbol as u32 + 1))
                .map(|v| v.wrapping_sub(TARGET_TABLE))
        }
    }

    /// The arcs of `state` on `input`, as divvunspell's
    /// `Transducer::for_each_arc` hands them over.
    pub(crate) fn for_each_arc<V: FnMut(u16, u32, f32)>(
        &self,
        state: u32,
        input: u16,
        mut visit: V,
    ) {
        if !self.has_transitions(state.wrapping_add(1), input) {
            return;
        }
        let Some(mut next) = self.next(state, input) else {
            return;
        };
        while self.trans_input(next) == Some(input) {
            if let (Some(output), Some(target), Some(weight)) = (
                self.trans_output(next),
                self.trans_target(next),
                self.trans_weight(next),
            ) {
                visit(output, target, weight);
            }
            next = next.wrapping_add(1);
        }
    }

    /// The symbol of the first flag diacritic arc in the epsilon run of the
    /// state at `address`, if it has one.
    fn first_flag_arc(&self, address: u32) -> Option<u16> {
        let i = address.wrapping_add(1);
        let free = if i >= TARGET_TABLE {
            self.trans_input(i - TARGET_TABLE)
                .is_some_and(|s| s == 0 || self.is_flag(s))
        } else {
            self.index_input(i) == Some(0)
        };
        let mut next = self.next(address, 0).filter(|_| free)?;
        while let Some(symbol) = self.trans_input(next) {
            if symbol != 0 && !self.is_flag(symbol) {
                return None;
            }
            if self.is_flag(symbol) {
                return Some(symbol);
            }
            next = next.wrapping_add(1);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols() -> Vec<String> {
        ["@_EPSILON_SYMBOL_@", "a", "b"].map(String::from).to_vec()
    }

    fn one_arc(input: u16, output: u16, target: u32, weight: f32) -> SourceState {
        SourceState {
            final_weight: Some(0.0),
            arcs: vec![SourceArc {
                input,
                output,
                target,
                weight,
            }],
        }
    }

    fn refusal(symbols: Vec<String>, states: Vec<SourceState>) -> String {
        match SourceModel::new(symbols, states) {
            Ok(_) => panic!("the model should be refused"),
            Err(e) => e.to_string(),
        }
    }

    // [spec:hfst:def:dhfst.source-model/test]
    #[test]
    fn refuses_models_divvunspell_cannot_read() {
        let mut no_epsilon = symbols();
        no_epsilon[0] = "x".into();
        assert!(refusal(no_epsilon, vec![SourceState::default()]).contains("symbol 0"));
        assert!(refusal(symbols(), Vec::new()).contains("no states"));
        assert!(refusal(symbols(), vec![one_arc(1, 2, 0, f32::NAN)]).contains("NaN"));
        assert!(refusal(symbols(), vec![one_arc(1, 3, 0, 1.0)]).contains("outside the alphabet"));
        assert!(refusal(symbols(), vec![one_arc(1, 2, 1, 1.0)]).contains("missing state 1"));
        let minus_infinity = SourceState {
            final_weight: Some(f32::NEG_INFINITY),
            arcs: Vec::new(),
        };
        assert!(refusal(symbols(), vec![minus_infinity]).contains("final weight"));
    }

    // [spec:hfst:sem:dhfst.source-model/test]
    #[test]
    fn sorts_arcs_and_drops_exact_duplicates() {
        let mut state = one_arc(2, 1, 0, 1.0);
        state.arcs.push(SourceArc {
            input: 1,
            output: 2,
            target: 0,
            weight: 1.0,
        });
        state.arcs.push(state.arcs[0]);
        let model = SourceModel::new(symbols(), vec![state]).expect("the model is well formed");
        assert_eq!(model.duplicate_arcs(), 1);
        let pairs: Vec<(u16, u16)> = model.states()[0]
            .arcs
            .iter()
            .map(|a| (a.input, a.output))
            .collect();
        assert_eq!(pairs, vec![(1, 2), (2, 1)]);
    }

    // [spec:hfst:sem:dhfst.source-model/test]
    #[test]
    fn tells_flag_diacritics_as_divvunspell_does() {
        for (name, flag) in [
            ("@P.CASE.NOM@", true),
            ("@U.X@", true),
            ("@_UNKNOWN_SYMBOL_@", false),
            ("@P@", false),
            ("a", false),
        ] {
            assert_eq!(flag_name(name).ok(), Some(flag), "{name}");
        }
        assert!(flag_name("@Q.CASE.NOM@").is_err());
    }
}
