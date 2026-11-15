//! The operations on a transducer. Each changes the transducer it is called
//! on, as the `hfst` tool it is named after changes the transducer it reads;
//! the tool's options are its arguments.

use std::collections::HashMap;
use std::sync::Arc;

use hfst_embed::compile::{
    AffixGuessifyOptions, BinaryOptions, ComposeIntersectOptions, ComposeOptions, ConvertOptions,
    Count, DeterminizeOptions, DisjunctOptions, EditMetadataOptions, EliminateFlagsOptions,
    ExpandEquivalencesOptions, GuessDirection, GuessifyOptions, InsertFreelyOptions,
    KillPathsOptions, Level, MinimizeOptions, NameOptions, NamedOperand, NamedText, ProjectOptions,
    PruneAlphabetOptions, PushDirection, PushOptions, RepeatOptions, ReweightOptions,
    SubstituteOptions, Tape, Transducer as Machine,
};
use hfst_swift_runtime::GlueError;
use hfst_swift_runtime::error::invalid;

use crate::transducer::{Transducer, locked, unlocked};

/// The value `given` names in `table`, or the `invalid-argument` error for
/// the option `what`.
pub fn choice<T: Copy>(what: &str, given: &str, table: &[(&str, T)]) -> Result<T, GlueError> {
    if let Some((_, value)) = table.iter().find(|(known, _)| *known == given) {
        return Ok(*value);
    }
    let names: Vec<&str> = table.iter().map(|(name, _)| *name).collect();
    Err(invalid(format!(
        "{what} is one of {}, not {given}",
        names.join(", ")
    )))
}

const PUSH: [(&str, PushDirection); 2] = [
    ("initial", PushDirection::Initial),
    ("final", PushDirection::Final),
];

/// The functions `reweight` applies to a weight, as `hfst reweight
/// --function` names them.
pub const FUNCTIONS: [&str; 15] = [
    "cos", "sin", "tan", "acos", "asin", "atan", "cosh", "sinh", "tanh", "exp", "log", "log10",
    "sqrt", "floor", "ceil",
];

/// A count of repetitions, which the library keeps in 32 bits; `None` for
/// no bound.
fn count(what: &str, value: Option<i64>) -> Result<Count, GlueError> {
    match value {
        None => Ok(Count::Infinite),
        Some(n) if (0..=i64::from(u32::MAX)).contains(&n) => Ok(Count::Finite(n as u64)),
        Some(n) => Err(invalid(format!(
            "{what} is a whole number from 0 to {}, not {n}",
            u32::MAX
        ))),
    }
}

fn length(what: &str, value: i64) -> Result<usize, GlueError> {
    usize::try_from(value).map_err(|_| invalid(format!("{what} is 0 or more, not {value}")))
}

// [spec:hfst:req:swift.operations]
// [spec:hfst:req:swift.threads]
#[uniffi::export]
impl Transducer {
    pub fn determinize(&self, encode_weights: bool) -> Result<(), GlueError> {
        self.change(|t| t.determinize(&DeterminizeOptions { encode_weights }))
    }

    pub fn minimize(&self, encode_weights: bool) -> Result<(), GlueError> {
        self.change(|t| t.minimize(&MinimizeOptions { encode_weights }))
    }

    pub fn invert(&self) -> Result<(), GlueError> {
        self.change(Machine::invert)
    }

    pub fn reverse(&self) -> Result<(), GlueError> {
        self.change(Machine::reverse)
    }

    pub fn remove_epsilons(&self) -> Result<(), GlueError> {
        self.change(Machine::remove_epsilons)
    }

    pub fn prune_alphabet(&self, force: bool) -> Result<(), GlueError> {
        self.change(|t| t.prune_alphabet(&PruneAlphabetOptions { force }))
    }

    pub fn project(&self, project: String) -> Result<(), GlueError> {
        let project = choice(
            "project",
            &project,
            &[("input", Tape::Input), ("output", Tape::Output)],
        )?;
        self.change(|t| t.project(&ProjectOptions { project }))
    }

    pub fn push_weights(&self, push: String) -> Result<(), GlueError> {
        let push = choice("push", &push, &PUSH)?;
        self.change(|t| t.push_weights(&PushOptions { push }))
    }

    pub fn push_labels(&self, push: String) -> Result<(), GlueError> {
        let push = choice("push", &push, &PUSH)?;
        self.change(|t| t.push_labels(&PushOptions { push }))
    }

    pub fn realign(&self) -> Result<(), GlueError> {
        self.change(Machine::realign)
    }

    /// Repeats at least `from` and at most `to` times; `to` `None` for no
    /// bound.
    pub fn repeat(&self, from: Option<i64>, to: Option<i64>) -> Result<(), GlueError> {
        let options = RepeatOptions {
            from: count("from", from)?,
            to: count("to", to)?,
        };
        self.change(|t| t.repeat(&options))
    }

    pub fn insert_freely(&self, symbol_pair: String, harmonise: bool) -> Result<(), GlueError> {
        let options = InsertFreelyOptions {
            symbol_pair,
            harmonise,
        };
        self.change(|t| t.insert_freely(&options))
    }

    pub fn eliminate_flags(&self, flag: Option<String>) -> Result<(), GlueError> {
        self.change(|t| t.eliminate_flags(&EliminateFlagsOptions { flag }))
    }

    pub fn preprocess_for_optimized_lookup_format(&self) -> Result<(), GlueError> {
        self.change(Machine::preprocess_for_optimized_lookup_format)
    }

    pub fn affix_guessify(&self, direction: String, weight: f32) -> Result<(), GlueError> {
        let direction = choice(
            "direction",
            &direction,
            &[
                ("prefix", GuessDirection::GuessPrefix),
                ("suffix", GuessDirection::GuessSuffix),
            ],
        )?;
        self.change(|t| t.affix_guessify(&AffixGuessifyOptions { direction, weight }))
    }

    pub fn expand_equivalences(
        &self,
        from: Option<String>,
        to: Option<String>,
        tsv: Option<String>,
        level: String,
    ) -> Result<(), GlueError> {
        let level = choice(
            "level",
            &level,
            &[
                ("first", Level::First),
                ("second", Level::Second),
                ("both", Level::Both),
            ],
        )?;
        let options = ExpandEquivalencesOptions {
            from,
            to,
            tsv,
            level,
        };
        self.change(|t| t.expand_equivalences(&options))
    }

    pub fn kill_paths(&self, symbol: Option<String>, tsv: Option<String>) -> Result<(), GlueError> {
        self.change(|t| t.kill_paths(&KillPathsOptions { symbol, tsv }))
    }

    /// `upper_bound` `None` sets no upper bound.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is an option of hfst reweight"
    )]
    pub fn reweight(
        &self,
        addition: f32,
        multiplier: f32,
        function: Option<String>,
        lower_bound: f32,
        upper_bound: Option<f32>,
        input_symbol: Option<String>,
        output_symbol: Option<String>,
        symbol: Option<String>,
        end_states_only: bool,
        arcs_only: bool,
        tsv: Option<String>,
    ) -> Result<(), GlueError> {
        if let Some(name) = &function {
            let table: Vec<(&str, ())> = FUNCTIONS.iter().map(|f| (*f, ())).collect();
            choice("function", name, &table)?;
        }
        let defaults = ReweightOptions::default();
        let options = ReweightOptions {
            addition,
            multiplier,
            function,
            lower_bound,
            upper_bound: upper_bound.unwrap_or(defaults.upper_bound),
            input_symbol,
            output_symbol,
            symbol,
            end_states_only,
            arcs_only,
            tsv,
        };
        self.change(|t| t.reweight(&options))
    }

    pub fn set_name(&self, name: String, truncate_length: i64) -> Result<(), GlueError> {
        let options = NameOptions {
            name,
            truncate_length: length("truncateLength", truncate_length)?,
        };
        self.change(|t| {
            t.set_name(&options);
            Ok(())
        })
    }

    pub fn edit_metadata(
        &self,
        add: HashMap<String, String>,
        truncate_length: i64,
    ) -> Result<(), GlueError> {
        let mut add: Vec<(String, String)> = add.into_iter().collect();
        // A dictionary has no order, so the properties are added in the
        // order of their names, which sets each the same whatever the
        // order the caller's dictionary keeps.
        add.sort();
        let options = EditMetadataOptions {
            add,
            truncate_length: length("truncateLength", truncate_length)?,
        };
        self.change(|t| t.edit_metadata(&options))
    }

    /// The model form generator the tool writes after the guesser, or `None`
    /// with `do_not_compile_generator`.
    pub fn guessify(
        &self,
        default_penalty: f32,
        do_not_compile_generator: bool,
    ) -> Result<Option<Arc<Transducer>>, GlueError> {
        let options = GuessifyOptions {
            default_penalty,
            do_not_compile_generator,
        };
        Ok(self.change(|t| t.guessify(&options))?.map(Transducer::new))
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each is an option of hfst substitute"
    )]
    pub fn substitute(
        &self,
        from_label: Option<String>,
        to_label: Option<String>,
        to_transducer: Option<Arc<Transducer>>,
        from_file: Option<String>,
        file_name: String,
        in_order: bool,
        do_not_convert: bool,
        compose: bool,
    ) -> Result<(), GlueError> {
        let to = to_transducer.as_deref();
        let read: Vec<&Transducer> = to.into_iter().collect();
        let (changed, read) = locked(Some(self), &read);
        let mut changed = changed.ok_or_else(unlocked)?;
        // A transducer substituted into itself is read as it was before.
        let itself = match to {
            Some(to) if read.get(to).is_none() => Some(changed.copy()?),
            Some(_) | None => None,
        };
        let replacement = to.and_then(|to| read.get(to).or(itself.as_ref()));
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
        Ok(changed.substitute(&options)?)
    }

    pub fn compose(
        &self,
        other: Arc<Transducer>,
        do_not_harmonize: bool,
        harmonize_flags: bool,
        xerox_composition: bool,
        flag_is_epsilon: bool,
        do_not_convert: bool,
    ) -> Result<(), GlueError> {
        let options = ComposeOptions {
            do_not_harmonize,
            harmonize_flags,
            xerox_composition,
            flag_is_epsilon,
            do_not_convert,
        };
        self.change_with(&other, |t, other| t.compose(other, &options))
    }

    pub fn concatenate(
        &self,
        other: Arc<Transducer>,
        do_not_harmonize: bool,
        harmonize_flags: bool,
        do_not_convert: bool,
    ) -> Result<(), GlueError> {
        let options = BinaryOptions {
            do_not_harmonize,
            harmonize_flags,
            do_not_convert,
        };
        self.change_with(&other, |t, other| t.concatenate(other, &options))
    }

    pub fn conjunct(
        &self,
        other: Arc<Transducer>,
        do_not_harmonize: bool,
        harmonize_flags: bool,
        do_not_convert: bool,
    ) -> Result<(), GlueError> {
        let options = BinaryOptions {
            do_not_harmonize,
            harmonize_flags,
            do_not_convert,
        };
        self.change_with(&other, |t, other| t.conjunct(other, &options))
    }

    pub fn subtract(
        &self,
        other: Arc<Transducer>,
        do_not_harmonize: bool,
        harmonize_flags: bool,
        do_not_convert: bool,
    ) -> Result<(), GlueError> {
        let options = BinaryOptions {
            do_not_harmonize,
            harmonize_flags,
            do_not_convert,
        };
        self.change_with(&other, |t, other| t.subtract(other, &options))
    }

    pub fn disjunct(
        &self,
        other: Arc<Transducer>,
        do_not_harmonize: bool,
        do_not_convert: bool,
    ) -> Result<(), GlueError> {
        let options = DisjunctOptions {
            do_not_harmonize,
            do_not_convert,
        };
        self.change_with(&other, |t, other| t.disjunct(other, &options))
    }

    pub fn priority_disjunct(
        &self,
        other: Arc<Transducer>,
        do_not_convert: bool,
    ) -> Result<(), GlueError> {
        let options = ConvertOptions { do_not_convert };
        self.change_with(&other, |t, other| t.priority_disjunct(other, &options))
    }

    pub fn shuffle(&self, other: Arc<Transducer>, do_not_convert: bool) -> Result<(), GlueError> {
        let options = ConvertOptions { do_not_convert };
        self.change_with(&other, |t, other| t.shuffle(other, &options))
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each is an option of hfst compose-intersect"
    )]
    pub fn compose_intersect(
        &self,
        rules: Vec<Arc<Transducer>>,
        invert: bool,
        fast: bool,
        encode_weights: bool,
        harmonize: bool,
        lexicon_name: String,
        rules_name: String,
        do_not_convert: bool,
    ) -> Result<(), GlueError> {
        let options = ComposeIntersectOptions {
            invert,
            fast,
            encode_weights,
            harmonize,
            lexicon_name,
            rules_name,
            do_not_convert,
        };
        let rules: Vec<&Transducer> = rules.iter().map(Arc::as_ref).collect();
        let (changed, read) = locked(Some(self), &rules);
        let mut changed = changed.ok_or_else(unlocked)?;
        // compose_intersect(t, [t]): the lexicon among its own rules stands,
        // as a rule, for itself before the change.
        let itself = if rules.iter().any(|rule| read.get(rule).is_none()) {
            Some(changed.copy()?)
        } else {
            None
        };
        let rules: Vec<&Machine> = rules
            .iter()
            .filter_map(|rule| read.get(rule).or(itself.as_ref()))
            .collect();
        Ok(changed.compose_intersect(&rules, &options)?)
    }
}
