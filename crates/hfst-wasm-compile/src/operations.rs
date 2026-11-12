//! The operations on a transducer. Each changes the transducer it is called
//! on, as the `hfst` tool it is named after changes the transducer it reads,
//! with the options `crate::operation_options` reads. The wrapper's method
//! returns the transducer.

use hfst_embed::Code;
use hfst_embed::compile::{
    ComposeIntersectOptions, NamedOperand, NamedText, SubstituteOptions, Transducer as Machine,
};
use hfst_wasm_shared::GlueError;
use wasm_bindgen::prelude::*;

use crate::list::TransducerList;
use crate::operation_options::{self as parse, Substitution};
use crate::options::Options;
use crate::transducer::Transducer;

// [spec:hfst:req:javascript.compile-module]
#[wasm_bindgen]
impl Transducer {
    pub fn determinize(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::determinize(&Options::new("determinize", options)?)?;
        Ok(self.changing()?.determinize(&options)?)
    }

    pub fn minimize(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::minimize(&Options::new("minimize", options)?)?;
        Ok(self.changing()?.minimize(&options)?)
    }

    pub fn invert(&self) -> Result<(), GlueError> {
        Ok(self.changing()?.invert()?)
    }

    pub fn reverse(&self) -> Result<(), GlueError> {
        Ok(self.changing()?.reverse()?)
    }

    #[wasm_bindgen(js_name = removeEpsilons)]
    pub fn remove_epsilons(&self) -> Result<(), GlueError> {
        Ok(self.changing()?.remove_epsilons()?)
    }

    #[wasm_bindgen(js_name = pruneAlphabet)]
    pub fn prune_alphabet(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::prune_alphabet(&Options::new("pruneAlphabet", options)?)?;
        Ok(self.changing()?.prune_alphabet(&options)?)
    }

    pub fn project(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::project(&Options::new("project", options)?)?;
        Ok(self.changing()?.project(&options)?)
    }

    #[wasm_bindgen(js_name = pushWeights)]
    pub fn push_weights(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::push(&Options::new("pushWeights", options)?)?;
        Ok(self.changing()?.push_weights(&options)?)
    }

    #[wasm_bindgen(js_name = pushLabels)]
    pub fn push_labels(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::push(&Options::new("pushLabels", options)?)?;
        Ok(self.changing()?.push_labels(&options)?)
    }

    pub fn realign(&self) -> Result<(), GlueError> {
        Ok(self.changing()?.realign()?)
    }

    pub fn repeat(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::repeat(&Options::new("repeat", options)?)?;
        Ok(self.changing()?.repeat(&options)?)
    }

    #[wasm_bindgen(js_name = insertFreely)]
    pub fn insert_freely(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::insert_freely(&Options::new("insertFreely", options)?)?;
        Ok(self.changing()?.insert_freely(&options)?)
    }

    #[wasm_bindgen(js_name = eliminateFlags)]
    pub fn eliminate_flags(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::eliminate_flags(&Options::new("eliminateFlags", options)?)?;
        Ok(self.changing()?.eliminate_flags(&options)?)
    }

    #[wasm_bindgen(js_name = preprocessForOptimizedLookupFormat)]
    pub fn preprocess_for_optimized_lookup_format(&self) -> Result<(), GlueError> {
        Ok(self.changing()?.preprocess_for_optimized_lookup_format()?)
    }

    #[wasm_bindgen(js_name = affixGuessify)]
    pub fn affix_guessify(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::affix_guessify(&Options::new("affixGuessify", options)?)?;
        Ok(self.changing()?.affix_guessify(&options)?)
    }

    #[wasm_bindgen(js_name = expandEquivalences)]
    pub fn expand_equivalences(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::expand_equivalences(&Options::new("expandEquivalences", options)?)?;
        Ok(self.changing()?.expand_equivalences(&options)?)
    }

    #[wasm_bindgen(js_name = killPaths)]
    pub fn kill_paths(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::kill_paths(&Options::new("killPaths", options)?)?;
        Ok(self.changing()?.kill_paths(&options)?)
    }

    pub fn reweight(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::reweight(&Options::new("reweight", options)?)?;
        Ok(self.changing()?.reweight(&options)?)
    }

    #[wasm_bindgen(js_name = setName)]
    pub fn set_name(&self, name: String, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::set_name(name, &Options::new("setName", options)?)?;
        self.changing()?.set_name(&options);
        Ok(())
    }

    #[wasm_bindgen(js_name = editMetadata)]
    pub fn edit_metadata(&self, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::edit_metadata(&Options::new("editMetadata", options)?)?;
        Ok(self.changing()?.edit_metadata(&options)?)
    }

    /// The model form generator `hfst guessify` writes after the guesser, or
    /// nothing with `doNotCompileGenerator`.
    pub fn guessify(&self, options: &JsValue) -> Result<Option<Transducer>, GlueError> {
        let options = parse::guessify(&Options::new("guessify", options)?)?;
        Ok(self.changing()?.guessify(&options)?.map(Transducer::new))
    }

    /// Replaces labels, as `hfst substitute`: `fromLabel` by `toLabel`, or
    /// each line of `fromFile`.
    pub fn substitute(&self, options: &JsValue) -> Result<(), GlueError> {
        let substitution = parse::substitute(&Options::new("substitute", options)?)?;
        self.substitute_by(None, substitution)
    }

    /// Replaces `fromLabel` by `to`, as `hfst substitute --to-transducer`.
    #[wasm_bindgen(js_name = substituteTransducer)]
    pub fn substitute_transducer(
        &self,
        to: &Transducer,
        options: &JsValue,
    ) -> Result<(), GlueError> {
        let substitution = parse::substitute(&Options::new("substitute", options)?)?;
        self.substitute_by(Some(to), substitution)
    }

    pub fn compose(&self, other: &Transducer, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::compose(&Options::new("compose", options)?)?;
        self.change_with(other, |t, other| t.compose(other, &options))
    }

    pub fn concatenate(&self, other: &Transducer, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::binary(&Options::new("concatenate", options)?)?;
        self.change_with(other, |t, other| t.concatenate(other, &options))
    }

    pub fn conjunct(&self, other: &Transducer, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::binary(&Options::new("conjunct", options)?)?;
        self.change_with(other, |t, other| t.conjunct(other, &options))
    }

    pub fn subtract(&self, other: &Transducer, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::binary(&Options::new("subtract", options)?)?;
        self.change_with(other, |t, other| t.subtract(other, &options))
    }

    pub fn disjunct(&self, other: &Transducer, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::disjunct(&Options::new("disjunct", options)?)?;
        self.change_with(other, |t, other| t.disjunct(other, &options))
    }

    #[wasm_bindgen(js_name = priorityDisjunct)]
    pub fn priority_disjunct(
        &self,
        other: &Transducer,
        options: &JsValue,
    ) -> Result<(), GlueError> {
        let options = parse::conversion(&Options::new("priorityDisjunct", options)?)?;
        self.change_with(other, |t, other| t.priority_disjunct(other, &options))
    }

    pub fn shuffle(&self, other: &Transducer, options: &JsValue) -> Result<(), GlueError> {
        let options = parse::conversion(&Options::new("shuffle", options)?)?;
        self.change_with(other, |t, other| t.shuffle(other, &options))
    }

    /// Composes the transducer, a lexicon, with the intersection of `rules`,
    /// as `hfst compose-intersect`.
    #[wasm_bindgen(js_name = composeIntersect)]
    pub fn compose_intersect(
        &self,
        rules: &TransducerList,
        options: &JsValue,
    ) -> Result<(), GlueError> {
        let options = parse::compose_intersect(&Options::new("composeIntersect", options)?)?;
        self.compose_intersect_with(rules, &options)
    }
}

fn in_use() -> GlueError {
    GlueError::new(
        Code::InvalidArgument,
        "a transducer is in use by a call that has not returned",
    )
}

impl Transducer {
    /// `hfst substitute`, putting in `to` when it is given. A transducer
    /// substituted into itself is read as it was before.
    pub(crate) fn substitute_by(
        &self,
        to: Option<&Transducer>,
        substitution: Substitution,
    ) -> Result<(), GlueError> {
        let itself = match to {
            Some(to) if self.is(&to.machine) => Some(self.reading()?.copy()?),
            Some(_) | None => None,
        };
        let other = match to {
            Some(to) if itself.is_none() => Some(to.reading()?),
            Some(_) | None => None,
        };
        let replacement: Option<&Machine> = itself.as_ref().or(other.as_deref());
        let Substitution {
            from_label,
            to_label,
            from_file,
            file_name,
            in_order,
            do_not_convert,
            compose,
        } = substitution;
        let options = SubstituteOptions {
            from_label,
            to_label,
            to_transducer: replacement.map(|transducer| NamedOperand {
                transducer,
                name: &file_name,
            }),
            from_file: from_file.map(|text| NamedText {
                name: file_name.clone(),
                text,
            }),
            in_order,
            do_not_convert,
            compose,
        };
        Ok(self.changing()?.substitute(&options)?)
    }

    /// `hfst compose-intersect` with the rules of `rules`. A rule that is the
    /// lexicon itself is read as it was before.
    pub(crate) fn compose_intersect_with(
        &self,
        rules: &TransducerList,
        options: &ComposeIntersectOptions,
    ) -> Result<(), GlueError> {
        let itself = if rules.items().iter().any(|rule| self.is(rule)) {
            Some(self.reading()?.copy()?)
        } else {
            None
        };
        let others = rules
            .items()
            .iter()
            .filter(|rule| !self.is(rule))
            .map(|rule| rule.try_borrow().map_err(|_| in_use()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut others = others.iter();
        let read: Vec<&Machine> = rules
            .items()
            .iter()
            .filter_map(|rule| {
                if self.is(rule) {
                    itself.as_ref()
                } else {
                    others.next().map(|r| &**r)
                }
            })
            .collect();
        Ok(self.changing()?.compose_intersect(&read, options)?)
    }
}

#[cfg(test)]
mod tests {
    use hfst_embed::compile::ComposeOptions;

    use super::*;
    use crate::options::{Value, given};
    use crate::tests::{machine, paths};

    fn substitution(fields: &[(&str, Value)]) -> Substitution {
        parse::substitute(&given("substitute", fields)).expect("options")
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn a_transducer_composes_with_itself_as_it_was() {
        let t = Transducer::new(machine("a:b | b:c"));
        t.change_with(&t, |m, other| m.compose(other, &ComposeOptions::default()))
            .expect("composes with itself");
        assert_eq!(paths(&t), ["a:c"]);
        let u = Transducer::new(machine("c:d"));
        t.change_with(&u, |m, other| m.compose(other, &ComposeOptions::default()))
            .expect("composes with another");
        assert_eq!(paths(&t), ["a:d"]);
        assert_eq!(paths(&u), ["c:d"], "the other is not changed");
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn a_transducer_substitutes_into_itself_as_it_was() {
        let t = Transducer::new(machine("a b"));
        t.substitute_by(Some(&t), substitution(&[("fromLabel", "b".into())]))
            .expect("substitutes");
        assert_eq!(paths(&t), ["aab"]);
        let labels = Transducer::new(machine("a b"));
        let relabel = [("fromLabel", "a".into()), ("toLabel", "c".into())];
        labels
            .substitute_by(None, substitution(&relabel))
            .expect("relabels");
        assert_eq!(paths(&labels), ["cb"]);
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn a_rule_that_is_the_lexicon_reads_unchanged() {
        let lexicon = Transducer::new(machine("a | b"));
        let mut rules = TransducerList::new();
        rules.push(&lexicon);
        let options = parse::compose_intersect(&given("composeIntersect", &[])).expect("none");
        lexicon
            .compose_intersect_with(&rules, &options)
            .expect("composes with itself");
        assert_eq!(paths(&lexicon), ["a", "b"]);
        let other = Transducer::new(machine("a"));
        let mut rules = TransducerList::new();
        rules.push(&other);
        lexicon
            .compose_intersect_with(&rules, &options)
            .expect("composes with a rule");
        assert_eq!(paths(&lexicon), ["a"]);
    }

    #[test]
    fn a_busy_transducer_is_refused() {
        let t = Transducer::new(machine("a"));
        let held = t.changing().expect("free");
        assert_eq!(
            t.reading().err().map(|e| e.code()),
            Some("invalid-argument")
        );
        drop(held);
        assert!(t.reading().is_ok(), "free once the change is over");
    }
}
