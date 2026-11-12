use std::collections::HashMap;
use std::rc::Rc;

use crate::error::Result;
use crate::prediction::units::UnitKey;
use crate::prediction::walk::{Candidate, Next, Walker, Word};
use crate::transducer::TransducerTablesInterface;

/// How the user reached a word in progress from the empty word: each unit
/// picked, with the letters typed before it was picked, then the letters
/// typed since. Walkers over the same generator, grammar and limits reach
/// the same word by the same steps.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(super) struct Steps {
    picks: Vec<(String, UnitKey)>,
    typed: String,
}

impl Steps {
    /// The letters typed since the last unit picked.
    pub(super) fn typed(&self) -> &str {
        &self.typed
    }

    /// The steps that pick `key` from here, with the letters typed so far.
    pub(super) fn then(&self, key: &UnitKey) -> Steps {
        let mut picks = self.picks.clone();
        picks.push((self.typed.clone(), key.clone()));
        Steps {
            picks,
            typed: String::new(),
        }
    }
}

/// What the walks found while one word is typed, by the steps they were
/// made at, so that each step is walked once however many variants type
/// the word and however often a variant looks there.
pub(super) struct Walks<'a, T: TransducerTablesInterface> {
    walker: Walker<'a, T>,
    found: HashMap<Steps, Rc<Next>>,
    /// The endings after each stem whose complete words were offered.
    endings: HashMap<Steps, Rc<Next>>,
}

impl<'a, T: TransducerTablesInterface> Walks<'a, T> {
    pub(super) fn new(walker: Walker<'a, T>) -> Self {
        Walks {
            walker,
            found: HashMap::new(),
            endings: HashMap::new(),
        }
    }

    /// The endings after `stem`, on offer at `steps`, which reach `picked`
    /// once it is picked. A stem a bound did not cut reaches the same
    /// configurations whatever letters were typed for it, so its endings are
    /// walked once for all of them.
    pub(super) fn endings(&mut self, steps: &Steps, stem: &Candidate, picked: &Word) -> Rc<Next> {
        let mut at = steps.then(&stem.key);
        if !picked.truncated()
            && let Some((typed, _)) = at.picks.last_mut()
        {
            typed.clear();
        }
        self.endings
            .entry(at)
            .or_insert_with(|| Rc::new(self.walker.endings(picked)))
            .clone()
    }

    /// What is on offer at `steps`, which reach `word`, as a session offers
    /// it: the units the generator accepts next, or once letters are typed,
    /// those entry offers for them.
    pub(super) fn offer(&mut self, steps: &Steps, word: &Word) -> Result<Rc<Next>> {
        if let Some(found) = self.found.get(steps) {
            return Ok(found.clone());
        }
        let next = Rc::new(if steps.typed.is_empty() {
            self.walker.next(word)
        } else {
            self.walker.entry(word, &steps.typed, None)?
        });
        self.found.insert(steps.clone(), next.clone());
        Ok(next)
    }

    /// Called between words, so that only one word's walks are held.
    pub(super) fn clear(&mut self) {
        self.found.clear();
        self.endings.clear();
    }

    #[cfg(test)]
    pub(super) fn walked(&self) -> usize {
        self.found.len() + self.endings.len()
    }

    #[cfg(test)]
    pub(super) fn walked_endings(&self) -> usize {
        self.endings.len()
    }
}

/// The word a user builds, as a session builds it: the word in progress
/// and the steps that reached it.
pub(super) struct Building {
    empty: Word,
    word: Word,
    steps: Steps,
}

impl Building {
    pub(super) fn new<T: TransducerTablesInterface>(walker: &Walker<'_, T>) -> Building {
        let empty = walker.start();
        Building {
            word: empty.clone(),
            empty,
            steps: Steps::default(),
        }
    }

    pub(super) fn word(&self) -> &Word {
        &self.word
    }

    pub(super) fn steps(&self) -> &Steps {
        &self.steps
    }

    /// A letter typed after the word's last unit. As a session does, it is
    /// refused where no stem can start.
    pub(super) fn type_letter<T: TransducerTablesInterface>(
        &mut self,
        walker: &Walker<'_, T>,
        letter: char,
    ) -> Result<()> {
        if !walker.stem_may_start(&self.word) {
            crate::bail!(
                FaultyStringInput,
                format!("letter {letter:?} is typed where no stem can start")
            );
        }
        self.steps.typed.push(letter);
        Ok(())
    }

    /// Append a unit on offer at the word's steps. The letters typed for
    /// it are spent.
    pub(super) fn pick(&mut self, candidate: &Candidate) {
        self.word = self.word.pick(candidate);
        self.steps = self.steps.then(&candidate.key);
    }

    pub(super) fn start_over(&mut self) {
        self.word = self.empty.clone();
        self.steps = Steps::default();
    }
}
