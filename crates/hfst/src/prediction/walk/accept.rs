//! Whether a transducer accepts a sequence of input symbols, decided as the
//! walk reads a generator: closing over epsilon and flag arcs with flag
//! diacritics applied as lookup applies them, following only the arcs that
//! lead to the next symbol, and stopping at the first final configuration.
//! A lookup instead writes every output on the way, which on a generator
//! that writes a morpheme before reading its tags means every morpheme that
//! may follow.

use std::cell::RefCell;

use super::hash::IdSet;
use super::machine::Machine;
use super::pass::{Flags, FlagsId};
use super::reach::Reaches;
use crate::transducer::{
    NO_SYMBOL_NUMBER, SymbolNumber, Transducer, TransducerTablesInterface, TransitionTableIndex,
};

/// A configuration with its output left out, which acceptance never needs.
type Point = (TransitionTableIndex, FlagsId);

/// What the acceptor has learnt of the transducer, kept from one call to
/// the next since a corpus asks about the same states again and again.
#[derive(Default)]
struct Known {
    reaches: Reaches,
    flags: Flags,
}

/// Decides whether a transducer accepts input symbol sequences. The
/// transducer is borrowed and read in place.
pub struct Acceptor<'a, T: TransducerTablesInterface> {
    machine: Machine<'a, T>,
    known: RefCell<Known>,
}

impl<'a, T: TransducerTablesInterface> Acceptor<'a, T> {
    pub fn new(transducer: &'a Transducer<T>) -> Acceptor<'a, T> {
        Acceptor {
            machine: Machine::new(transducer),
            known: RefCell::new(Known::default()),
        }
    }

    /// False when the transducer has a default symbol, whose arcs lookup
    /// takes where no other arc reads a symbol and the acceptor never does.
    pub fn reads_like_lookup(&self) -> bool {
        self.machine
            .transducer()
            .get_alphabet()
            .get_default_symbol()
            == NO_SYMBOL_NUMBER
    }

    /// `text` cut into input symbols, the longest at each place, as lookup
    /// cuts it, or none when some character begins no input symbol.
    pub fn tokenize(&self, text: &str) -> Option<Vec<SymbolNumber>> {
        let encoder = self.machine.transducer().get_encoder();
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        let mut symbols = Vec::new();
        let mut at = 0;
        while bytes[at] != 0 {
            symbols.push(encoder.find_key(&bytes, &mut at)?);
        }
        Some(symbols)
    }

    /// True when some path reads exactly `input` and ends in a final state,
    /// every flag diacritic on it holding.
    pub fn accepts(&self, input: &[SymbolNumber]) -> bool {
        let mut known = self.known.borrow_mut();
        let Known { reaches, flags } = &mut *known;
        let start = flags.intern(self.machine.neutral_flags());
        let mut points = vec![(0, start)];
        for &symbol in input {
            let mut reached: IdSet<Point> = IdSet::default();
            let mut next = Vec::new();
            for (state, flag_values) in self.close(&points, Some(symbol), reaches, flags) {
                for arc in self.machine.symbol_arcs(state, symbol) {
                    let point = (self.machine.arc(arc).target, flag_values);
                    if reached.insert(point) {
                        next.push(point);
                    }
                }
            }
            if next.is_empty() {
                return false;
            }
            points = next;
        }
        self.close(&points, None, reaches, flags)
            .iter()
            .any(|&(state, _)| self.machine.final_weight(state).is_some())
    }

    /// The points `points` reach over epsilon and flag arcs that can still
    /// lead to reading `symbol`, or to a final state when there is none.
    /// When looking for a final state it stops at the first.
    fn close(
        &self,
        points: &[Point],
        symbol: Option<SymbolNumber>,
        reaches: &mut Reaches,
        flags: &mut Flags,
    ) -> Vec<Point> {
        let machine = &self.machine;
        let mut wanted = |state: TransitionTableIndex| {
            let reach = reaches.of(machine, state);
            match symbol {
                Some(symbol) => reach.reads(symbol),
                None => reach.can_end(),
            }
        };
        let mut seen: IdSet<Point> = IdSet::default();
        let mut agenda: Vec<Point> = points.to_vec();
        let mut closed = Vec::new();
        while let Some(point) = agenda.pop() {
            if !seen.insert(point) || !wanted(point.0) {
                continue;
            }
            closed.push(point);
            if symbol.is_none() && machine.final_weight(point.0).is_some() {
                break;
            }
            for arc in machine.epsilon_arcs(point.0) {
                let arc = machine.arc(arc);
                if let Some(flag_values) = flags.after(machine, point.1, arc.input) {
                    agenda.push((arc.target, flag_values));
                }
            }
        }
        closed
    }
}
