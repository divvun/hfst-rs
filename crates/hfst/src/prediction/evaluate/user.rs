use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use super::Variant;
use super::corpus::{Target, characters, is_character};
use super::report::Typed;
use super::walks::{Building, Steps, Walks};
use crate::error::Result;
use crate::prediction::model::Model;
use crate::prediction::units::{Label, SymbolClass, Unit, UnitKey, UnitKind};
use crate::prediction::walk::{
    Candidate, Ranking, Suggest, Suggestion, WalkLimits, Walker, WholeWord, Word,
};
use crate::transducer::{Transducer, TransducerTablesInterface, Weight};

/// The simulated user of unit prediction. It types one word after another,
/// knowing each word's target, and builds each as a session would.
pub(super) struct UnitTypist<'a, 'm, T: TransducerTablesInterface> {
    building: Building,
    eyes: Eyes<'a, 'm, T>,
}

/// What the user reads off the keyboard.
pub(super) struct Eyes<'a, 'm, T: TransducerTablesInterface> {
    walker: Walker<'a, T>,
    model: &'m Model<'m>,
    variant: &'m Variant,
}

/// The user's search for one unit of the word.
#[derive(Default)]
pub(super) struct Slot {
    /// True for a stem or a prefix, which entry offers for letters typed.
    pub(super) entry: bool,
    /// Letters typed since the last unit.
    pub(super) typed: usize,
    /// For a unit that entry does not match, the start of the word that
    /// suggestions' previews must start with: what the generator has written
    /// of the word, then the letters typed. It only grows while the unit is
    /// looked for.
    pub(super) filter: Option<String>,
    /// Each read once for the slot: the word and what is on offer stay as
    /// they are while the unit is looked for.
    pub(super) previews: HashMap<UnitKey, Option<String>>,
    /// Suggestions ruled out by what their paths have written: each of
    /// those disagrees with the filter, and so with every longer one. Their
    /// previews are never read.
    pub(super) ruled_out: HashSet<UnitKey>,
}

enum Outcome {
    Picked,
    /// The pick wrote the whole word.
    Written,
    GaveUp,
}

/// What a key the user presses picks: a unit, a stem and its ending, or a
/// counted form, which writes the word.
enum Chosen {
    Unit(Candidate),
    Word(WholeWord),
    Form,
}

/// A key the user presses, and what it costs.
struct Choice {
    chosen: Chosen,
    taps: usize,
    writes_word: bool,
}

/// Where the user looks from: the word in progress, the steps that reached
/// it, and what the model ranks the next unit, and counted forms, after.
pub(super) struct View<'w> {
    pub(super) word: &'w Word,
    pub(super) steps: &'w Steps,
    pub(super) ranking: &'w [UnitKey],
    /// The forms of the words before, capitals folded, oldest first.
    pub(super) before: &'w [String],
}

/// What one look at the keyboard found.
struct Look {
    first: bool,
    shown: bool,
    choice: Option<Choice>,
    truncated: bool,
    /// False when, for a unit entry does not offer, no candidate is the
    /// unit or writes the word. Letters typed only narrow what is shown, so
    /// none of them can help.
    reachable: bool,
}

impl<'a, 'm, T: TransducerTablesInterface> UnitTypist<'a, 'm, T> {
    pub(super) fn new(
        generator: &'a Transducer<T>,
        grammar: &'a crate::prediction::units::UnitGrammar,
        limits: WalkLimits,
        model: &'m Model<'m>,
        variant: &'m Variant,
    ) -> Result<Self> {
        let walker = Walker::new(generator, grammar, limits)?;
        Ok(UnitTypist {
            building: Building::new(&walker),
            eyes: Eyes::new(walker, model, variant),
        })
    }

    /// Type one word, given the keys of the units before it and the forms
    /// of the words before it, from an empty word. A word the user cannot
    /// finish through units, or a counted form, costs its characters, as if
    /// typed in full from its start. `walks` holds what was walked for the
    /// word so far, by this user or another.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    pub(super) fn type_word(
        &mut self,
        surface: &str,
        target: &Target,
        context: &[UnitKey],
        before: &[String],
        walks: &mut Walks<'_, T>,
    ) -> Result<Typed> {
        let mut typed = Typed::default();
        let built = self.build(surface, target, (context, before), walks, &mut typed)?;
        self.building.start_over();
        if built {
            return Ok(typed);
        }
        Ok(Typed {
            taps: characters(surface),
            in_full: true,
            ..typed
        })
    }

    fn build(
        &mut self,
        surface: &str,
        target: &Target,
        context: (&[UnitKey], &[String]),
        walks: &mut Walks<'_, T>,
        typed: &mut Typed,
    ) -> Result<bool> {
        for (unit, key) in target.units.iter().zip(&target.keys) {
            if self.eyes.writes(self.building.word(), surface) {
                return Ok(true);
            }
            match self.slot(surface, unit, key, context, walks, typed)? {
                Outcome::Picked => {}
                Outcome::Written => return Ok(true),
                Outcome::GaveUp => return Ok(false),
            }
        }
        Ok(self.building.word().is_complete())
    }

    /// Look, then pick or type a letter, until the unit is picked or no
    /// letter is left to type. Where counted forms are offered and the word
    /// has no unit yet, the letters typed are the word's own
    /// ([`as_written`]), and entry matches units against them; otherwise
    /// they are the unit's, so a stem is entered by its lemma. A unit is
    /// judged first or shown at the user's first look for it; a stem, which
    /// entry offers only for typed letters, after its first letter, or at
    /// the look before it when a counted form picked there writes the word.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    fn slot(
        &mut self,
        surface: &str,
        unit: &Unit,
        key: &UnitKey,
        context: (&[UnitKey], &[String]),
        walks: &mut Walks<'_, T>,
        typed: &mut Typed,
    ) -> Result<Outcome> {
        let entry_letters = self.eyes.entry_letters(unit);
        let entry_letters = if self.eyes.variant.forms && self.building.word().units().is_empty() {
            Some(as_written(
                surface,
                entry_letters.as_deref().unwrap_or_default(),
            ))
        } else {
            entry_letters
        };
        let judged_at = usize::from(unit.kind == UnitKind::Stem);
        let mut slot = Slot {
            entry: entry_letters.is_some(),
            ..Slot::default()
        };
        let mut judged = (false, false);
        let outcome = loop {
            let look = self.look(surface, key, context, &mut slot, walks)?;
            typed.truncated |= look.truncated;
            if slot.typed == judged_at || (slot.typed < judged_at && look.choice.is_some()) {
                judged = (look.first, look.shown);
            }
            if let Some(choice) = look.choice {
                typed.taps += choice.taps;
                match &choice.chosen {
                    Chosen::Unit(candidate) => self.building.pick(candidate),
                    Chosen::Word(word) => {
                        self.building.pick(word.stem());
                        self.building.pick(word.ending());
                    }
                    Chosen::Form => {}
                }
                break if choice.writes_word {
                    Outcome::Written
                } else {
                    Outcome::Picked
                };
            }
            if !look.reachable {
                break Outcome::GaveUp;
            }
            let letter = match &entry_letters {
                Some(letters) => letters.get(slot.typed).copied(),
                None => self
                    .eyes
                    .next_letter(self.building.word(), surface, &mut slot),
            };
            let Some(letter) = letter else {
                break Outcome::GaveUp;
            };
            if entry_letters.is_some() {
                self.building.type_letter(&self.eyes.walker, letter)?;
            }
            typed.taps += usize::from(is_character(letter));
            slot.typed += 1;
        };
        typed.looked_for.push(judged);
        Ok(outcome)
    }

    fn look(
        &self,
        surface: &str,
        key: &UnitKey,
        (context, before): (&[UnitKey], &[String]),
        slot: &mut Slot,
        walks: &mut Walks<'_, T>,
    ) -> Result<Look> {
        let word = self.building.word();
        let grammar = self.eyes.walker.grammar();
        let mut ranking = context.to_vec();
        ranking.extend(word.units().iter().map(|unit| grammar.key(unit)));
        let next = walks.offer(self.building.steps(), word)?;
        let reachable = slot.entry
            || slot.typed > 0
            || next
                .candidates
                .iter()
                .any(|candidate| &candidate.key == key)
            || next.candidates.iter().any(|candidate| {
                matches!(candidate.unit.kind, UnitKind::Ending | UnitKind::Enclitic)
                    && self.eyes.writes(&word.pick(candidate), surface)
            });
        let view = View {
            word,
            steps: self.building.steps(),
            ranking: &ranking,
            before,
        };
        let shown = self.eyes.shown(&view, &next.candidates, slot, walks)?;
        let hit = |suggestion: &Suggestion<'_>| match suggestion {
            Suggestion::Unit(candidate) => &candidate.key == key,
            Suggestion::Word(whole) => same_text(whole.surface(), surface),
            Suggestion::Form(seen) => same_text(&seen.form, surface),
        };
        Ok(Look {
            reachable,
            first: shown.first().is_some_and(hit),
            shown: shown.iter().any(hit),
            choice: self
                .eyes
                .choose(word, shown, &next.candidates, key, surface),
            truncated: next.truncated,
        })
    }
}

impl<'a, 'm, T: TransducerTablesInterface> Eyes<'a, 'm, T> {
    pub(super) fn new(walker: Walker<'a, T>, model: &'m Model<'m>, variant: &'m Variant) -> Self {
        Eyes {
            walker,
            model,
            variant,
        }
    }

    /// The letters entry matches a stem or a prefix by: a stem's own
    /// letters, which may differ from the word's surface, and a prefix's
    /// label. None for a unit entry does not offer.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    fn entry_letters(&self, unit: &Unit) -> Option<Vec<char>> {
        let grammar = self.walker.grammar();
        match unit.kind {
            UnitKind::Stem => Some(
                unit.symbols
                    .iter()
                    .filter(|symbol| grammar.classify(symbol) == SymbolClass::Letter)
                    .flat_map(|symbol| symbol.as_str().chars())
                    .collect(),
            ),
            UnitKind::Prefix | UnitKind::Filter => match grammar.label(unit) {
                Label::Text(text) => Some(text.chars().collect()),
                Label::CompleteWord => Some(Vec::new()),
            },
            UnitKind::Boundary | UnitKind::Ending | UnitKind::Enclitic => None,
        }
    }

    /// The N suggestions shown: the first in the model's order, with
    /// complete words beside the stems when the variant offers them, and
    /// before the word's first unit the counted forms that start with the
    /// letters typed when it offers those. Once letters are typed for a
    /// unit entry does not offer, which is never a stem, only the
    /// candidates whose preview starts with the slot's filter count. Their
    /// previews are read in the model's order, and none after the Nth that
    /// fits.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    // [spec:hfst:sem:prediction.whole-words]
    // [spec:hfst:sem:prediction.seen-forms]
    pub(super) fn shown<'n>(
        &self,
        view: &View<'_>,
        offered: &'n [Candidate],
        slot: &mut Slot,
        walks: &mut Walks<'_, T>,
    ) -> Result<Vec<Suggestion<'n>>> {
        if slot.filter.is_none() {
            let settings = Suggest {
                whole_words: self.variant.whole_words,
                limit: Some(self.variant.shown),
                forms: self.variant.forms,
                units: true,
                unit_weight: self.variant.unit_weight,
            };
            let ranking = Ranking {
                order: self.model,
                context: view.ranking,
                before: view.before,
                typed: view.steps.typed(),
            };
            return self.walker.suggest_with(
                view.word,
                offered,
                &ranking,
                settings,
                |stem, picked| Ok(walks.endings(view.steps, stem, picked)),
            );
        }
        let pairs: Vec<(UnitKey, Weight)> = offered
            .iter()
            .map(|candidate| (candidate.key.clone(), candidate.weight))
            .collect();
        let mut shown = Vec::new();
        for index in self.model.rank(view.ranking, &pairs) {
            if shown.len() == self.variant.shown {
                break;
            }
            let candidate = &offered[index];
            if self.fits(view, candidate, slot, walks)? {
                shown.push(Suggestion::Unit(candidate));
            }
        }
        Ok(shown)
    }

    /// True when there is no filter, or the candidate's preview starts with
    /// it. A preview is read at most once for a slot, and only when what
    /// the candidate's paths have written agrees with the filter: a preview
    /// starts with what one of them has written.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    fn fits(
        &self,
        view: &View<'_>,
        candidate: &Candidate,
        slot: &mut Slot,
        walks: &mut Walks<'_, T>,
    ) -> Result<bool> {
        let Some(filter) = &slot.filter else {
            return Ok(true);
        };
        if slot.ruled_out.contains(&candidate.key) {
            return Ok(false);
        }
        let preview = match slot.previews.entry(candidate.key.clone()) {
            Entry::Occupied(read) => read.into_mut(),
            Entry::Vacant(unread) => {
                let picked = view.word.pick(candidate);
                let starts = self.walker.preview_starts(&picked);
                if !starts.iter().any(|start| agree(start, filter)) {
                    slot.ruled_out.insert(candidate.key.clone());
                    return Ok(false);
                }
                unread.insert(self.preview(view, &picked, candidate, walks)?)
            }
        };
        Ok(preview
            .as_deref()
            .is_some_and(|preview| starts_with(preview, filter)))
    }

    /// The preview of `picked`, the word with `candidate` picked, from the
    /// walk `walks` holds for it.
    fn preview(
        &self,
        view: &View<'_>,
        picked: &Word,
        candidate: &Candidate,
        walks: &mut Walks<'_, T>,
    ) -> Result<Option<String>> {
        let mut context = view.ranking.to_vec();
        context.push(candidate.key.clone());
        if picked.is_complete() {
            return Ok(self.walker.preview(picked, &context, self.model));
        }
        let next = walks.offer(&view.steps.then(&candidate.key), picked)?;
        Ok(self
            .walker
            .preview_from(picked, &next, &context, self.model))
    }

    /// The cheapest key that is the word's next unit or that writes the
    /// whole word. Of keys that cost the same, one that writes the word is
    /// taken first, then the earlier one: suggestions, then face keys.
    fn choose(
        &self,
        word: &Word,
        shown: Vec<Suggestion<'_>>,
        offered: &[Candidate],
        key: &UnitKey,
        surface: &str,
    ) -> Option<Choice> {
        let mut options: Vec<(Suggestion<'_>, usize)> = shown
            .into_iter()
            .map(|suggestion| (suggestion, 1))
            .collect();
        options.extend(
            self.face_keys(word, offered, key)
                .into_iter()
                .map(|(candidate, taps)| (Suggestion::Unit(candidate), taps)),
        );
        let mut best: Option<Choice> = None;
        for (suggestion, taps) in options {
            let (writes_word, next_unit) = match &suggestion {
                Suggestion::Unit(candidate) => (
                    self.writes(&word.pick(candidate), surface),
                    &candidate.key == key,
                ),
                Suggestion::Word(whole) => (same_text(whole.surface(), surface), false),
                Suggestion::Form(seen) => (same_text(&seen.form, surface), false),
            };
            let better = best.as_ref().is_none_or(|best| {
                taps < best.taps || (taps == best.taps && writes_word && !best.writes_word)
            });
            if better && (writes_word || next_unit) {
                best = Some(Choice {
                    chosen: match suggestion {
                        Suggestion::Unit(candidate) => Chosen::Unit(candidate.clone()),
                        Suggestion::Word(whole) => Chosen::Word(*whole),
                        Suggestion::Form(_) => Chosen::Form,
                    },
                    taps,
                    writes_word,
                });
            }
        }
        best
    }

    /// The offered candidates on the keyboard's faces, with what each costs:
    /// the face of the word's class, and the target's own face when that is
    /// another class's, one switch away.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    fn face_keys<'n>(
        &self,
        word: &Word,
        offered: &'n [Candidate],
        key: &UnitKey,
    ) -> Vec<(&'n Candidate, usize)> {
        let Some(faces) = self.variant.faces else {
            return Vec::new();
        };
        let face = |class: &str| -> Vec<(UnitKey, usize)> {
            self.model
                .face(class)
                .into_iter()
                .take(faces.keys + faces.long_press)
                .enumerate()
                .map(|(at, key)| (key, if at < faces.keys { 1 } else { 2 }))
                .collect()
        };
        let shown_class = self.walker.grammar().word_class(word.units());
        let mut keys = shown_class.map(face).unwrap_or_default();
        if let Some(class) = self.model.class_of(key)
            && Some(class) != shown_class
        {
            keys.extend(
                face(class)
                    .into_iter()
                    .filter(|(on_face, _)| on_face == key)
                    .map(|(on_face, taps)| (on_face, taps + 1)),
            );
        }
        keys.into_iter()
            .filter_map(|(on_face, taps)| {
                offered
                    .iter()
                    .find(|candidate| candidate.key == on_face)
                    .map(|candidate| (candidate, taps))
            })
            .collect()
    }

    /// True when the word is complete and the generator writes it as the
    /// user's word, capitals aside.
    fn writes(&self, word: &Word, surface: &str) -> bool {
        word.is_complete()
            && self
                .walker
                .forms(word)
                .first()
                .is_some_and(|form| same_text(&form.surface, surface))
    }

    /// The next letter of the word after what the generator has written of
    /// it and the letters typed since the last unit, and the slot's filter
    /// with that letter added.
    fn next_letter(&self, word: &Word, surface: &str, slot: &mut Slot) -> Option<char> {
        let written = match &slot.filter {
            Some(filter) => filter.chars().count(),
            None => self.written(word, surface),
        };
        let letter = surface.chars().nth(written)?;
        slot.filter = Some(surface.chars().take(written + 1).collect());
        Some(letter)
    }

    /// How many letters of the word the generator has written on every path
    /// the word reaches: the longest start of the word that the output of
    /// each of its configurations starts with. A language that writes a
    /// prefix only once a later unit is picked, as Plains Cree's person
    /// prefixes, has written none of the word, and the letters typed are
    /// matched from its start.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    fn written(&self, word: &Word, surface: &str) -> usize {
        let alphabet = self.walker.generator().get_alphabet();
        let mut common: Option<Vec<char>> = None;
        for configuration in self.walker.configurations(word) {
            let mut output = String::new();
            for &symbol in configuration.output() {
                output.push_str(alphabet.string_from_symbol(symbol).as_str());
            }
            common = Some(match common {
                None => output.chars().collect(),
                Some(mut common) => {
                    let agree = common
                        .iter()
                        .zip(output.chars())
                        .take_while(|&(&a, b)| a == b)
                        .count();
                    common.truncate(agree);
                    common
                }
            });
        }
        common
            .unwrap_or_default()
            .into_iter()
            .zip(surface.chars())
            .take_while(|&(a, b)| same_letter(a, b))
            .count()
    }
}

/// The word's own letters, as the user types them where counted forms are
/// offered. Capitals are shift state, so while the word's letters agree with
/// the unit's, each is typed in the unit's case, which entry matches.
// [spec:hfst:def:prediction-evaluation.keystrokes]
fn as_written(surface: &str, unit: &[char]) -> Vec<char> {
    let mut agreed = true;
    surface
        .chars()
        .enumerate()
        .map(|(at, letter)| match unit.get(at) {
            Some(&own) if agreed && same_letter(own, letter) => own,
            Some(_) | None => {
                agreed = false;
                letter
            }
        })
        .collect()
}

/// Capitals are a keyboard's shift state, not other letters. They are
/// folded as [`crate::prediction::model::fold`] folds the forms the model
/// counts.
fn same_letter(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

fn starts_with(text: &str, start: &str) -> bool {
    let mut letters = text.chars();
    start.chars().all(|wanted| {
        letters
            .next()
            .is_some_and(|letter| same_letter(letter, wanted))
    })
}

/// True when one of the texts starts with the other, capitals aside.
fn agree(a: &str, b: &str) -> bool {
    a.chars().zip(b.chars()).all(|(a, b)| same_letter(a, b))
}

pub(super) fn same_text(a: &str, b: &str) -> bool {
    a.chars().count() == b.chars().count() && starts_with(a, b)
}
