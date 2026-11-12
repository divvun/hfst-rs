//! Word forms the model counted, offered beside the units before a word's
//! first unit (`prediction.seen-forms`).

use crate::prediction::units::UnitKey;

/// A word form the model counted, offered as a complete word.
// [spec:hfst:sem:prediction.seen-forms]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenForm {
    /// The form with capitals folded: its label, its key and what it writes.
    pub form: String,
    /// The unit keys of the reading the model counted it with most often,
    /// which become the word's units for the context once it is picked.
    pub units: Vec<UnitKey>,
}

/// The counted forms on offer for the letters typed, scored after the forms
/// of the words before.
pub trait SeenForms {
    /// The first `limit` of them, best first, each with its score; all of
    /// them when there is no limit.
    fn best(&self, limit: Option<usize>) -> Vec<(SeenForm, Option<f64>)>;

    /// The one that is `surface` with capitals folded, with its score; none
    /// when no such form is on offer.
    fn find(&self, surface: &str) -> Option<(SeenForm, Option<f64>)>;
}
