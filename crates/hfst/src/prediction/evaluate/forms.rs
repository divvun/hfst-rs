//! The simulated user of word-form completion alone: with units off, it
//! types the word's own letters and picks the word when a counted form shown
//! is the word, capitals aside (`prediction.seen-forms`).

use super::corpus::is_character;
use super::report::Typed;
use super::user::same_text;
use crate::prediction::model::FormScores;

/// Type one word with the counted forms `scores` offers after the words
/// before it, `shown` at a time. The user looks before each letter,
/// starting with no letter typed, and picks the word, for one tap, when it
/// is among the forms shown; otherwise it types the next letter. A word
/// never shown is typed in full. The word is judged first or shown at its
/// look after the first letter, or at the look before it when it is picked
/// there; a word of one letter is judged with that letter typed, which only
/// judges, since typing it has written the word.
// [spec:hfst:def:prediction-evaluation.keystrokes]
// [spec:hfst:sem:prediction.seen-forms]
pub(super) fn complete_form(scores: &FormScores<'_, '_>, shown: usize, surface: &str) -> Typed {
    let letters: Vec<char> = surface.chars().collect();
    let mut typed = Typed::default();
    let mut judged = None;
    for at in 0..=letters.len() {
        let prefix: String = letters[..at].iter().collect();
        let offered = scores.offered(&prefix);
        let best = scores.best(&offered, Some(shown));
        let hit = |id: u32| scores.text(id).is_some_and(|form| same_text(form, surface));
        let first = best.first().is_some_and(|&(id, _)| hit(id));
        let found = best.iter().any(|&(id, _)| hit(id));
        if at == 1 || (at == 0 && found) {
            judged = Some((first, found));
        }
        let Some(&letter) = letters.get(at) else {
            break;
        };
        if found {
            typed.taps += 1;
            typed.looked_for.extend(judged);
            return typed;
        }
        typed.taps += usize::from(is_character(letter));
    }
    typed.in_full = true;
    typed.looked_for.extend(judged);
    typed
}
