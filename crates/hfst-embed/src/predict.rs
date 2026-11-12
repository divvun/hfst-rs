//! Predictive text: a predictor reads a language's generator, unit grammar
//! and model from bytes, and a session builds words with it a unit at a time
//! (`docs/spec/prediction.md`).
//!
//! The library's walker borrows the generator and grammar it walks, and a
//! binding cannot keep a borrow from one call to the next. So a predictor
//! owns what it read, a session owns the words it built, and each call walks
//! with a walker made for that call. Making one costs a pass over the
//! generator's alphabet, a small part of what a walk costs.

use std::sync::Arc;

use hfst::prediction::entry::Relaxation;
use hfst::prediction::model::Model;
use hfst::prediction::realise::{Script, Transliterate, realise};
pub use hfst::prediction::units::UnitKind;
use hfst::prediction::units::{Label, UnitGrammar, UnitKey};
pub use hfst::prediction::walk::WalkLimits;
use hfst::prediction::walk::{Candidate as Offered, Next, Suggest, Suggestion, Walker, Word};
use hfst::transducer::{
    OlMember, Transducer, TransducerTablesInterface, UnweightedTables, WeightedTables,
};

use crate::Package;
use crate::error::{Code, Error};
use crate::stream;

/// What a candidate on offer is: one unit, or a complete word, which is a
/// stem and one of its endings picked together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CandidateKind {
    Unit(UnitKind),
    Word,
}

/// Every kind of candidate, so a binding can list the names [`kind_name`]
/// gives.
pub const CANDIDATE_KINDS: [CandidateKind; 7] = [
    CandidateKind::Unit(UnitKind::Stem),
    CandidateKind::Unit(UnitKind::Prefix),
    CandidateKind::Unit(UnitKind::Boundary),
    CandidateKind::Unit(UnitKind::Ending),
    CandidateKind::Unit(UnitKind::Enclitic),
    CandidateKind::Unit(UnitKind::Filter),
    CandidateKind::Word,
];

/// The name both packages give a kind of candidate. It does not change
/// between releases, so callers may test it.
pub fn kind_name(kind: CandidateKind) -> &'static str {
    match kind {
        CandidateKind::Unit(UnitKind::Stem) => "stem",
        CandidateKind::Unit(UnitKind::Prefix) => "prefix",
        CandidateKind::Unit(UnitKind::Boundary) => "boundary",
        CandidateKind::Unit(UnitKind::Ending) => "ending",
        CandidateKind::Unit(UnitKind::Enclitic) => "enclitic",
        CandidateKind::Unit(UnitKind::Filter) => "filter",
        CandidateKind::Word => "word",
    }
}

/// An optimized-lookup transducer, in the table layout its header names.
enum Tables {
    Weighted(Transducer<WeightedTables>),
    Unweighted(Transducer<UnweightedTables>),
}

impl Tables {
    fn relaxation(&self) -> &dyn Relaxation {
        match self {
            Tables::Weighted(transducer) => transducer,
            Tables::Unweighted(transducer) => transducer,
        }
    }

    fn transliterator(&self) -> &dyn Transliterate {
        match self {
            Tables::Weighted(transducer) => transducer,
            Tables::Unweighted(transducer) => transducer,
        }
    }
}

/// Binds `$walker` to a walker over the predictor's generator, in the table
/// layout it was read in, for `$body`.
macro_rules! walking {
    ($predictor:expr, $walker:ident => $body:expr) => {
        match &$predictor.generator {
            Tables::Weighted(generator) => {
                let $walker = $predictor.walker(generator);
                $body
            }
            Tables::Unweighted(generator) => {
                let $walker = $predictor.walker(generator);
                $body
            }
        }
    };
}

/// What a language may add to its generator, grammar and model. Each
/// transducer is the bytes of one optimized-lookup transducer.
#[derive(Clone, Copy, Debug, Default)]
pub struct PredictorOptions<'a> {
    /// From typed letters to the letters they may stand for, so that letters
    /// typed without their diacritics still match.
    pub relaxation: Option<&'a [u8]>,
    /// Applied to typed letters before they are matched, for a language
    /// typed in a script its generator does not write.
    pub input_transliterator: Option<&'a [u8]>,
    /// Applied to the word a session commits.
    pub output_transliterator: Option<&'a [u8]>,
    pub limits: WalkLimits,
    /// How many of its highest-ranked endings complete each stem on offer
    /// as a complete word (`prediction.whole-words`). 0 offers none.
    pub whole_words: usize,
}

/// A language's generator, unit grammar and model, read and checked against
/// one another. Nothing writes to it after it is built, so sessions on
/// several threads may share it through an [`Arc`].
pub struct Predictor {
    generator: Tables,
    grammar: UnitGrammar,
    model: Vec<u8>,
    /// How many units of earlier words a session keeps: as many as the
    /// model ranks after.
    context_len: usize,
    relaxation: Option<Tables>,
    input: Option<Tables>,
    output: Option<Tables>,
    limits: WalkLimits,
    whole_words: usize,
}

impl Predictor {
    /// Reads the generator, an optimized-lookup transducer from analyses to
    /// words, the unit grammar's TOML and the model, each from the bytes of
    /// its file. A grammar that does not fit the generator's alphabet and a
    /// model built with another grammar are refused here, as malformed
    /// files are, with `invalid-format`. A transducer in a format the
    /// packages do not read is refused with `unsupported-format`, and bytes
    /// holding more than one with `transducer-count`.
    pub fn new(
        generator: &[u8],
        grammar: &[u8],
        model: Vec<u8>,
        options: PredictorOptions<'_>,
        package: &Package,
    ) -> Result<Predictor, Error> {
        let generator = read_transducer("the generator", generator, package)?;
        let grammar = UnitGrammar::from_toml(grammar).map_err(invalid)?;
        // Making a walker checks the grammar against the generator's alphabet.
        let fits = match &generator {
            Tables::Weighted(generator) => {
                Walker::new(generator, &grammar, options.limits).map(|_| ())
            }
            Tables::Unweighted(generator) => {
                Walker::new(generator, &grammar, options.limits).map(|_| ())
            }
        };
        fits.map_err(invalid)?;
        let order = Model::from_bytes(&model, &grammar)
            .map_err(invalid)?
            .order();
        let optional = |what: &str, bytes: Option<&[u8]>| {
            bytes
                .map(|bytes| read_transducer(what, bytes, package))
                .transpose()
        };
        Ok(Predictor {
            relaxation: optional("the relaxation", options.relaxation)?,
            input: optional("the input transliterator", options.input_transliterator)?,
            output: optional("the output transliterator", options.output_transliterator)?,
            generator,
            grammar,
            model,
            context_len: order.saturating_sub(1),
            limits: options.limits,
            whole_words: options.whole_words,
        })
    }

    /// The face of a class: its most frequent unit keys in the model, most
    /// frequent first. Empty for a class the model has no unit of.
    pub fn face(&self, class: &str) -> Vec<String> {
        self.model()
            .face(class)
            .iter()
            .map(|key| key.as_str().to_owned())
            .collect()
    }

    fn walker<'p, T: TransducerTablesInterface>(
        &'p self,
        generator: &'p Transducer<T>,
    ) -> Walker<'p, T> {
        Walker::new(generator, &self.grammar, self.limits)
            .expect("the grammar was checked against this generator when the predictor was built")
    }

    fn model(&self) -> Model<'_> {
        Model::from_bytes(&self.model, &self.grammar)
            .expect("the model was read with this grammar when the predictor was built")
    }

    fn script(&self) -> Script<'_> {
        Script {
            input: self.input.as_ref().map(Tables::transliterator),
            output: self.output.as_ref().map(Tables::transliterator),
        }
    }

    fn relaxation(&self) -> Option<&dyn Relaxation> {
        self.relaxation.as_ref().map(Tables::relaxation)
    }
}

/// What a session offers next, as a keyboard shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// The units and complete words to show, best first: those the model
    /// scores after the context first, then the rest by the generator's
    /// weight.
    pub candidates: Vec<Candidate>,
    /// How many units are on offer, shown or not. Complete words are not
    /// counted: only those that may be shown are drawn.
    pub total: usize,
    /// A stem can start here, so typed letters offer stems.
    pub stem_next: bool,
    /// A bound cut the walk short, so units may be missing.
    pub truncated: bool,
}

/// A unit or a complete word on offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// What [`Session::pick`] takes to pick it. A complete word's key is its
    /// stem's key then its ending's key.
    pub key: String,
    pub kind: CandidateKind,
    /// What a keyboard shows: the grammar's label, or for an ending without
    /// one and for a complete word, the word it completes.
    pub label: String,
    /// Picking it makes the word complete, so it can be committed.
    pub completes: bool,
}

/// A session's words and letters, apart from its predictor and its cached
/// offer, so a call can read them while it fills the cache.
struct State {
    empty: Word,
    /// The word after each unit picked, so backspace steps back without
    /// walking again.
    picked: Vec<Word>,
    typed: String,
    /// The keys of the latest units of earlier words, oldest first.
    context: Vec<UnitKey>,
}

impl State {
    fn word(&self) -> &Word {
        self.picked.last().unwrap_or(&self.empty)
    }

    /// The context, then the keys of the word's own units.
    fn ranking_context(&self, grammar: &UnitGrammar) -> Vec<UnitKey> {
        let mut keys = self.context.clone();
        keys.extend(self.word().units().iter().map(|unit| grammar.key(unit)));
        keys
    }

    /// The units on offer: those the generator accepts next, or, once
    /// letters are typed, those whose labels the letters match after the
    /// input transliterator. Letters it cannot convert match nothing.
    fn offer<T: TransducerTablesInterface>(
        &self,
        predictor: &Predictor,
        walker: &Walker<'_, T>,
    ) -> Result<Next, Error> {
        let word = self.word();
        if self.typed.is_empty() {
            return Ok(walker.next(word));
        }
        let Some(spelled) = predictor.script().typed(&self.typed) else {
            return Ok(Next {
                candidates: Vec::new(),
                forms: walker.forms(word),
                stem_next: true,
                truncated: false,
            });
        };
        Ok(walker.entry(word, &spelled, predictor.relaxation())?)
    }

    /// The units of `next` to show, with the complete words of the stems
    /// among them, ranked after the context and the word.
    fn shown<T: TransducerTablesInterface>(
        &self,
        predictor: &Predictor,
        walker: &Walker<'_, T>,
        next: &Next,
        limit: Option<usize>,
    ) -> Offer {
        let context = self.ranking_context(&predictor.grammar);
        let settings = Suggest {
            whole_words: predictor.whole_words,
            limit,
        };
        let word = self.word();
        let candidates = walker
            .suggest(
                word,
                &next.candidates,
                &context,
                &predictor.model(),
                settings,
            )
            .into_iter()
            .map(|suggestion| match suggestion {
                Suggestion::Unit(offered) => {
                    let picked = word.pick(offered);
                    Candidate {
                        key: offered.key.as_str().to_owned(),
                        kind: CandidateKind::Unit(offered.unit.kind),
                        label: label(walker, offered, &picked),
                        completes: picked.is_complete(),
                    }
                }
                Suggestion::Word(whole) => Candidate {
                    key: whole.key(),
                    kind: CandidateKind::Word,
                    label: whole.surface().to_owned(),
                    completes: true,
                },
            })
            .collect();
        Offer {
            candidates,
            total: next.candidates.len(),
            stem_next: next.stem_next,
            truncated: next.truncated,
        }
    }
}

/// The grammar's label for a unit. An ending without one is shown as the
/// word it completes, in the generator's spelling: the output
/// transliterator runs only on the word committed.
fn label<T: TransducerTablesInterface>(
    walker: &Walker<'_, T>,
    offered: &Offered,
    picked: &Word,
) -> String {
    match walker.grammar().label(&offered.unit) {
        Label::Text(text) => text,
        Label::CompleteWord => walker
            .forms(picked)
            .into_iter()
            .next()
            .map_or_else(|| offered.key.as_str().to_owned(), |form| form.surface),
    }
}

/// A word built a unit at a time over one predictor, the letters typed after
/// its last unit, and the units of the words committed before it
/// (`prediction.session`).
pub struct Session {
    predictor: Arc<Predictor>,
    state: State,
    /// What is on offer for the word and letters as they are, once walked.
    offered: Option<Next>,
}

impl Session {
    /// A session with an empty word and no context.
    pub fn new(predictor: Arc<Predictor>) -> Session {
        let empty = walking!(predictor, walker => walker.start());
        Session {
            predictor,
            state: State {
                empty,
                picked: Vec::new(),
                typed: String::new(),
                context: Vec::new(),
            },
            offered: None,
        }
    }

    /// Letters typed after the word's last unit. They are refused, with
    /// `invalid-argument`, where no stem can start.
    pub fn type_letters(&mut self, letters: &str) -> Result<(), Error> {
        let predictor = &*self.predictor;
        if !walking!(predictor, walker => walker.stem_may_start(self.state.word())) {
            return Err(Error::new(
                Code::InvalidArgument,
                format!("letters {letters:?} are typed where no stem can start"),
            ));
        }
        self.state.typed.push_str(letters);
        self.offered = None;
        Ok(())
    }

    /// The letters typed after the word's last unit, as typed.
    pub fn typed(&self) -> &str {
        &self.state.typed
    }

    /// The units on offer, and where stems are on offer their complete
    /// words, the first `limit` of them when there is a limit, ranked by the
    /// model after the context and the word's own units. With complete
    /// words, give a limit: without one every stem on offer is completed,
    /// at a walk for each.
    pub fn candidates(&mut self, limit: Option<usize>) -> Result<Offer, Error> {
        let predictor = &*self.predictor;
        walking!(predictor, walker => {
            let next = match self.offered.take() {
                Some(next) => next,
                None => self.state.offer(predictor, &walker)?,
            };
            let next = self.offered.insert(next);
            Ok(self.state.shown(predictor, &walker, next, limit))
        })
    }

    /// Appends the unit on offer with `key` to the word, or the stem and
    /// the ending of the complete word with `key`, whose stem is on offer,
    /// spending the letters typed for it. A key not on offer is refused with
    /// `invalid-argument`.
    // [spec:hfst:sem:prediction.whole-words]
    pub fn pick(&mut self, key: &str) -> Result<(), Error> {
        let predictor = &*self.predictor;
        let next = match self.offered.take() {
            Some(next) => next,
            None => walking!(predictor, walker => self.state.offer(predictor, &walker))?,
        };
        let word = self.state.word();
        let found = match next
            .candidates
            .iter()
            .find(|candidate| candidate.key.as_str() == key)
        {
            Some(candidate) => vec![word.pick(candidate)],
            None => walking!(predictor, walker => {
                walker.whole_word(word, &next.candidates, key).map(|whole| {
                    let stem = word.pick(whole.stem());
                    let complete = stem.pick(whole.ending());
                    vec![stem, complete]
                })
            })
            .unwrap_or_default(),
        };
        if found.is_empty() {
            self.offered = Some(next);
            return Err(Error::new(
                Code::InvalidArgument,
                format!("unit or complete word {key:?} is not on offer here"),
            ));
        }
        self.state.picked.extend(found);
        self.state.typed.clear();
        Ok(())
    }

    /// Removes the last letter typed, or the last unit picked when no letter
    /// is typed after it. False when there is nothing to remove.
    pub fn backspace(&mut self) -> bool {
        let removed = self.state.typed.pop().is_some() || self.state.picked.pop().is_some();
        if removed {
            self.offered = None;
        }
        removed
    }

    /// True when the word can be committed.
    pub fn is_complete(&self) -> bool {
        self.state.word().is_complete()
    }

    /// Writes the word, keeps its units as context and starts an empty word.
    /// An incomplete word, or letters typed with no unit picked for them,
    /// are refused with `invalid-argument`.
    pub fn commit(&mut self) -> Result<String, Error> {
        if !self.state.typed.is_empty() {
            return Err(Error::new(
                Code::InvalidArgument,
                format!(
                    "letters {:?} are typed but no unit is picked for them",
                    self.state.typed
                ),
            ));
        }
        let predictor = &*self.predictor;
        let written = walking!(predictor, walker => {
            realise(&walker, self.state.word(), &predictor.script())
        })?;
        let grammar = &predictor.grammar;
        let keys: Vec<UnitKey> = self
            .state
            .word()
            .units()
            .iter()
            .map(|unit| grammar.key(unit))
            .collect();
        let context = &mut self.state.context;
        context.extend(keys);
        let excess = context.len().saturating_sub(predictor.context_len);
        context.drain(..excess);
        self.state.picked.clear();
        self.offered = None;
        Ok(written)
    }

    /// The word completed by the ending the model ranks first, or the word
    /// itself when it is complete, in the generator's spelling. None when no
    /// ending is in reach. With no letters typed, the preview and
    /// [`Session::candidates`] share one walk, whichever comes first.
    pub fn preview(&mut self) -> Option<String> {
        let predictor = &*self.predictor;
        let order = predictor.model();
        let context = self.state.ranking_context(&predictor.grammar);
        let word = self.state.word();
        walking!(predictor, walker => {
            if self.state.typed.is_empty() {
                let next = self.offered.get_or_insert_with(|| walker.next(word));
                walker.preview_from(word, next, &context, &order)
            } else {
                walker.preview(word, &context, &order)
            }
        })
    }

    /// The class of the word: that of its latest unit that has one.
    pub fn word_class(&self) -> Option<&str> {
        self.predictor.grammar.word_class(self.state.word().units())
    }

    /// The face of the word's class, as [`Predictor::face`] gives it. Empty
    /// for a word with no class.
    pub fn face(&self) -> Vec<String> {
        self.word_class()
            .map_or_else(Vec::new, |class| self.predictor.face(class))
    }

    /// The keys of the latest units of committed words, oldest first.
    pub fn context(&self) -> impl Iterator<Item = &str> {
        self.state.context.iter().map(UnitKey::as_str)
    }
}

/// A library error with a grammar or a model, which are files the
/// predictor could not use.
fn invalid(error: hfst::error::Error) -> Error {
    Error::new(Code::InvalidFormat, error.to_string())
}

/// The one optimized-lookup transducer in `bytes`. `what` names it in the
/// error, since a predictor reads up to four.
fn read_transducer(what: &str, bytes: &[u8], package: &Package) -> Result<Tables, Error> {
    let named = |error: Error| Error::new(error.code(), format!("{what}: {}", error.message()));
    if bytes.is_empty() {
        return Err(named(stream::empty()));
    }
    let member = stream::member(bytes, package).map_err(named)?;
    if !member.format.is_read() {
        return Err(named(stream::unsupported(
            &format!("a transducer in the {} format", member.format.name()),
            package,
        )));
    }
    let mut rest = member.payload;
    // The library reports a short read as the transducer having the wrong
    // type, so a read that used up the bytes is called what it is.
    let read = OlMember::read_from(&mut rest, None).map_err(|error| {
        named(if rest.is_empty() {
            Error::new(
                Code::InvalidFormat,
                format!(
                    "the stream ends inside an optimized-lookup transducer, so the bytes are truncated ({error})"
                ),
            )
        } else {
            stream::read_error(error)
        })
    })?;
    if !rest.is_empty() {
        return Err(Error::new(
            Code::TransducerCount,
            format!("{what} is one transducer, and the bytes hold more"),
        ));
    }
    Ok(match read {
        OlMember::Weighted(transducer) => Tables::Weighted(transducer),
        OlMember::Unweighted(transducer) => Tables::Unweighted(transducer),
    })
}

#[cfg(test)]
mod tests;
