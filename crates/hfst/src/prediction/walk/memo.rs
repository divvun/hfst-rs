//! What a walker learns of its generator, kept from one walk to the next:
//! what each state reaches without reading a symbol, the flag values paths
//! set, and what each state reaches for each purpose. All of it follows
//! from the generator and the unit grammar alone, never from the word, so a
//! walk that finds it known does not look again.

use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError, TryLockError};

use super::hash::IdMap;
use super::pass::{Flags, FlagsId, Purpose};
use super::reach::Reaches;
use crate::transducer::{SymbolNumber, TransitionTableIndex, Weight};

/// The most frontier entries, and the most answers, a memo keeps. A walk
/// that starts with more clears them, so the memory a walker holds stays
/// bounded however long it is used.
const MAX_ENTRIES: usize = 1_000_000;

/// A configuration one state reaches without reading a symbol, relative to
/// that state, from which it can read a symbol of some purpose. Its state
/// is where its readings leave from.
#[derive(Clone, Debug)]
pub(in crate::prediction) struct Entry {
    pub(super) flags: FlagsId,
    /// Where in the frontier's symbols the output written on the way is.
    pub(super) suffix: Range<u32>,
    pub(super) weight: Weight,
}

/// An arc from an entry of a frontier that reads a symbol of its purpose.
#[derive(Clone, Copy, Debug)]
pub(in crate::prediction) struct Reading {
    pub(super) symbol: SymbolNumber,
    pub(super) entry: u32,
    pub(super) target: TransitionTableIndex,
    pub(super) written: Option<SymbolNumber>,
    pub(super) weight: Weight,
}

/// The entries one state, with given flags, reaches for one purpose.
#[derive(Debug, Default)]
pub(in crate::prediction) struct Frontier {
    pub(super) entries: Vec<Entry>,
    /// The outputs of all the entries, one after another.
    pub(super) symbols: Vec<SymbolNumber>,
    /// The arcs of the entries that read a symbol of the purpose, in order
    /// of symbol, then of entry, then as the transducer lists them.
    pub(super) readings: Vec<Reading>,
}

impl Frontier {
    pub(super) fn suffix(&self, entry: &Entry) -> &[SymbolNumber] {
        let Range { start, end } = entry.suffix.clone();
        self.symbols
            .get(start as usize..end as usize)
            .unwrap_or(&[])
    }

    /// The arcs that read `symbol`, with the entries they leave from.
    pub(super) fn reading(&self, symbol: SymbolNumber) -> impl Iterator<Item = (&Reading, &Entry)> {
        let first = self.readings.partition_point(|read| read.symbol < symbol);
        self.readings[first..]
            .iter()
            .take_while(move |read| read.symbol == symbol)
            .filter_map(|read| Some((read, self.entries.get(read.entry as usize)?)))
    }

    /// The symbols some entry has arcs for, in order, each once.
    pub(super) fn symbols_read(&self) -> impl Iterator<Item = SymbolNumber> + '_ {
        let mut last = None;
        self.readings.iter().filter_map(move |read| {
            let new = last != Some(read.symbol);
            last = Some(read.symbol);
            new.then_some(read.symbol)
        })
    }
}

pub(in crate::prediction) type FrontierKey = (TransitionTableIndex, FlagsId, Purpose);

/// What a walk asks of a point that the memo keeps the answer to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::prediction) enum Question {
    /// Can it read a symbol of the purpose without reading another first?
    Reads(Purpose),
    /// Can it reach a final state without reading a symbol?
    Ends,
}

#[derive(Default)]
pub(in crate::prediction) struct Memo {
    pub(super) reaches: Reaches,
    pub(super) flags: Flags,
    pub(super) frontiers: IdMap<FrontierKey, Arc<Frontier>>,
    pub(super) answers: IdMap<(TransitionTableIndex, FlagsId, Question), bool>,
    /// How many entries the frontiers hold between them.
    pub(super) entries: usize,
    /// How many frontiers walks have found, kept or not.
    #[cfg(test)]
    pub(super) computed: usize,
}

impl Memo {
    pub(super) fn forget(&mut self, key: &FrontierKey) {
        if let Some(frontier) = self.frontiers.remove(key) {
            self.entries -= frontier.entries.len();
        }
    }
}

/// Where a walker keeps its memo between walks. A walk takes the memo and
/// puts it back when it ends. A walk that finds it taken, by a walk under
/// way on another thread or one that started this walk, starts its own.
#[derive(Default)]
pub(in crate::prediction) struct Shelf(Mutex<Option<Box<Memo>>>);

impl Shelf {
    pub(super) fn take(&self) -> Box<Memo> {
        let mut memo = match self.0.try_lock() {
            Ok(mut held) => held.take(),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().take(),
            Err(TryLockError::WouldBlock) => None,
        }
        .unwrap_or_default();
        if memo.entries > MAX_ENTRIES {
            memo.frontiers.clear();
            memo.entries = 0;
        }
        if memo.answers.len() > MAX_ENTRIES {
            memo.answers.clear();
        }
        memo
    }

    pub(super) fn put(&self, memo: Box<Memo>) {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        *held = Some(memo);
    }

    /// True when a memo is on the shelf and knows what `state` reaches
    /// without reading a symbol.
    #[cfg(test)]
    pub(super) fn knows_reach(&self, state: TransitionTableIndex) -> bool {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.as_ref().is_some_and(|memo| memo.reaches.knows(state))
    }

    /// How many frontiers the memo on the shelf holds.
    #[cfg(test)]
    pub(super) fn frontiers(&self) -> usize {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.as_ref().map_or(0, |memo| memo.frontiers.len())
    }

    /// How many frontiers walks with the memo on the shelf have found.
    #[cfg(test)]
    pub(super) fn computed(&self) -> usize {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.as_ref().map_or(0, |memo| memo.computed)
    }
}
