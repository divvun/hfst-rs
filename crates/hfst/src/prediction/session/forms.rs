//! Counted word forms in a session: picking one, and writing it
//! (`prediction.seen-forms`).

use super::Session;
use crate::error::Result;
use crate::prediction::walk::{CandidateOrder, SeenForm};
use crate::transducer::TransducerTablesInterface;

impl<T: TransducerTablesInterface> Session<'_, T> {
    /// Pick the counted form `form`, on offer before the word's first unit
    /// for the letters typed, capitals folded. The word is then complete,
    /// and the letters typed are spent. `order` must be the one the
    /// suggestions were ranked by.
    // [spec:hfst:sem:prediction.seen-forms]
    pub fn pick_form(&mut self, order: &dyn CandidateOrder, form: &str) -> Result<()> {
        let refused = || {
            crate::err!(
                FaultyStringInput,
                format!("word form {form:?} is not offered here")
            )
        };
        if self.picked_form.is_some() || !self.word().units().is_empty() {
            return Err(refused());
        }
        let spelled = self.script.typed(&self.typed).ok_or_else(refused)?;
        let (seen, _) = order
            .seen_forms(&self.before, &spelled)
            .and_then(|seen| seen.find(form))
            .ok_or_else(refused)?;
        self.picked_form = Some(seen);
        self.typed.clear();
        self.offered = None;
        Ok(())
    }

    /// The counted form picked for the word, if one is.
    pub fn picked_form(&self) -> Option<&SeenForm> {
        self.picked_form.as_ref()
    }

    /// True when the word can be committed: a counted form is picked, or
    /// the units picked make a complete word.
    pub fn is_complete(&self) -> bool {
        self.picked_form.is_some() || self.word().is_complete()
    }

    /// A counted form in the script the word is committed in.
    // [spec:hfst:req:prediction.script-edges]
    pub(super) fn written_form(&self, form: &str) -> Result<String> {
        self.script.written(form).ok_or_else(|| {
            crate::err!(
                Hfst,
                format!("the output transliterator has no conversion for {form:?}")
            )
        })
    }
}
