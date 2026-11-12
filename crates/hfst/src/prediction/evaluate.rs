//! The harness that measures prediction: a simulated user types held-out
//! analysed text, and the keystrokes it saves are reported side by side for
//! every variant scored (`docs/spec/prediction-evaluation.md`).

mod corpus;
mod external;
mod forms;
mod report;
mod run;
mod user;
mod walks;

#[cfg(test)]
mod corpus_tests;
#[cfg(test)]
mod external_tests;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod forms_tests;
#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod user_tests;

pub use corpus::{Corpus, Fold, Target, Token, characters, is_character};
pub use external::Recording;
pub use report::{Report, Row, Tally};
pub use run::Harness;

use super::model::ModelSettings;
use super::walk::WalkLimits;

/// One way of predicting, scored on the same text as the others in a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    pub name: String,
    /// How the variant's models are built. Variants with equal settings
    /// share their models.
    pub model: ModelSettings,
    /// How many suggestions the keyboard shows: N.
    pub shown: usize,
    pub faces: Option<Faces>,
    /// How many of its highest-ranked endings complete each stem on offer
    /// as a complete word (`prediction.whole-words`). 0 offers none.
    pub whole_words: usize,
    /// Offer the word forms the model counted (`prediction.seen-forms`).
    pub forms: bool,
    /// Offer units, and complete words built from them. Without them the
    /// user completes counted forms alone, as on an ordinary keyboard.
    pub units: bool,
    /// What the score of a unit or a complete word built from units is
    /// multiplied by beside counted forms (`prediction-model.form-score`),
    /// from 0 to 1.
    pub unit_weight: f64,
}

impl Variant {
    /// A variant of units alone, with no faces and no complete words.
    pub fn units(name: &str, model: ModelSettings, shown: usize) -> Variant {
        Variant {
            name: name.to_owned(),
            model,
            shown,
            faces: None,
            whole_words: 0,
            forms: false,
            units: true,
            unit_weight: 1.0,
        }
    }
}

/// Which units of a class's face (`prediction-model.faces`) a keyboard
/// shows, and how many taps each costs. The face shown is that of the
/// word's class; the face of another class is one tap away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Faces {
    /// The face's first units, one tap each.
    pub keys: usize,
    /// The units after those, behind a long press of two taps each.
    pub long_press: usize,
}

/// The settings of a run that every variant shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunSettings {
    /// The number of parts the text is split into, k: one model is built
    /// for each part, from the others.
    pub folds: usize,
    pub limits: WalkLimits,
    /// How many threads type the folds and variants at once. Each holds
    /// its own walks, so memory grows with it. The default is what the
    /// machine can run at once.
    pub threads: usize,
}

impl Default for RunSettings {
    // [spec:hfst:req:prediction-evaluation.held-out]
    fn default() -> Self {
        RunSettings {
            folds: 10,
            limits: WalkLimits::default(),
            threads: std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
        }
    }
}
