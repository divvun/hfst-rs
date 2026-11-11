//! Writing the word: the generator's surface form for the units picked, and
//! the two places a language written in another script converts
//! (`prediction.realise`, `prediction.script-edges`).

use super::walk::{Walker, Word};
use crate::error::Result;
use crate::hfst_flag_diacritics::FdOperation;
use crate::lookup_state::LookupState;
use crate::transducer::{Transducer, TransducerTablesInterface};

pub trait Transliterate {
    /// The lowest-weight result, or none when the text has no conversion.
    fn transliterate(&self, text: &str) -> Option<String>;
}

/// An optimized-lookup transducer converts by ordinary lookup. Equal weights
/// are settled by the result's text, so the answer does not depend on the
/// order lookup found them in.
impl<T: TransducerTablesInterface> Transliterate for Transducer<T> {
    fn transliterate(&self, text: &str) -> Option<String> {
        let mut state = LookupState::new(self);
        state
            .lookup_fd(text, -1, 0.0)
            .into_iter()
            .map(|path| {
                let written: String = path
                    .second
                    .iter()
                    .filter(|symbol| !FdOperation::is_diacritic(symbol))
                    .map(|symbol| symbol.as_str())
                    .collect();
                (path.first, written)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
            .map(|(_, written)| written)
    }
}

/// The transliterators of a language its generator does not write the
/// script of. These two edges are the only places script is converted: the
/// units, the model and the walk see the generator's own spelling.
// [spec:hfst:req:prediction.script-edges]
#[derive(Clone, Copy, Default)]
pub struct Script<'a> {
    /// Applied to typed letters before entry matching.
    pub input: Option<&'a dyn Transliterate>,
    /// Applied to the committed word after the generator writes it.
    pub output: Option<&'a dyn Transliterate>,
}

impl Script<'_> {
    /// Typed letters in the generator's spelling, or none when the input
    /// transliterator has no conversion for them.
    // [spec:hfst:req:prediction.script-edges]
    pub fn typed(&self, letters: &str) -> Option<String> {
        match self.input {
            Some(input) => input.transliterate(letters),
            None => Some(letters.to_owned()),
        }
    }

    /// A word the generator wrote, in the script it is committed in.
    // [spec:hfst:req:prediction.script-edges]
    pub fn written(&self, surface: &str) -> Option<String> {
        match self.output {
            Some(output) => output.transliterate(surface),
            None => Some(surface.to_owned()),
        }
    }
}

/// The word a complete word in progress commits: the lowest-weight surface
/// form the generator writes for its units, passed through the output
/// transliterator when there is one.
// [spec:hfst:req:prediction.realise]
pub fn realise<T: TransducerTablesInterface>(
    walker: &Walker<'_, T>,
    word: &Word,
    script: &Script<'_>,
) -> Result<String> {
    let Some(form) = walker.forms(word).into_iter().next() else {
        crate::bail!(
            StateIsNotFinal,
            "the word in progress is not complete, so the generator writes no form for it"
        );
    };
    script.written(&form.surface).ok_or_else(|| {
        crate::err!(
            Hfst,
            format!(
                "the output transliterator has no conversion for {:?}",
                form.surface
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use crate::prediction::walk::fixtures_test::*;

    // [spec:hfst:req:prediction.realise/test]
    #[test]
    fn commits_the_lightest_form_the_generator_writes() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let word = word_after(&walker, "girji", &["+N+Sg+Nom"]);
        assert_eq!(
            realise(&walker, &word, &Script::default()).unwrap(),
            "girji"
        );
        let asked = word_after(&walker, "girji", &["+N+Sg+Nom", "+Qst"]);
        assert_eq!(
            realise(&walker, &asked, &Script::default()).unwrap(),
            "girjigo"
        );
    }

    // [spec:hfst:req:prediction.realise/test]
    #[test]
    fn an_incomplete_word_is_not_realised() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let stem = word_after(&walker, "girji", &[]);
        let refused = realise(&walker, &stem, &Script::default())
            .expect_err("an incomplete word has no form");
        assert_eq!(refused.kind, ErrorKind::StateIsNotFinal);
    }

    // [spec:hfst:req:prediction.realise/test]
    // [spec:hfst:req:prediction.script-edges/test]
    #[test]
    fn the_output_transliterator_runs_after_the_generator() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let capitals = regex("[ g:G | i | r | j | a | t ]*");
        let script = Script {
            input: None,
            output: Some(&capitals),
        };
        let word = word_after(&walker, "girji", &["+N+Pl+Nom"]);
        assert_eq!(realise(&walker, &word, &script).unwrap(), "Girjat");
        let unconvertible = word_after(&walker, "girji", &["+N+Sg+Nom", "+Qst"]);
        assert!(realise(&walker, &unconvertible, &script).is_err());
    }

    // [spec:hfst:req:prediction.script-edges/test]
    #[test]
    fn typed_letters_convert_before_entry_and_nowhere_else() {
        let lower = regex("[ G:g | I:i | R:r ]*");
        let script = Script {
            input: Some(&lower),
            output: None,
        };
        assert_eq!(script.typed("GIR").as_deref(), Some("gir"));
        assert_eq!(script.typed("GX"), None);
        assert_eq!(script.written("girji").as_deref(), Some("girji"));
        assert_eq!(Script::default().typed("GIR").as_deref(), Some("GIR"));
    }

    #[test]
    fn a_transliterator_takes_its_lightest_result() {
        let choice = regex("[ a:b::1 | a:c::0.5 ]");
        assert_eq!(choice.transliterate("a").as_deref(), Some("c"));
        assert_eq!(choice.transliterate("b"), None);
    }
}
