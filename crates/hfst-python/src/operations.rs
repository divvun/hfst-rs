//! The operations on a transducer. Each changes the transducer it is called
//! on, as the `hfst` tool it is named after changes the transducer it reads,
//! and returns None; the tool's options are its keyword-only arguments.

use hfst_embed::compile::{
    AffixGuessifyOptions, BinaryOptions, ComposeIntersectOptions, ComposeOptions, ConvertOptions,
    Count, DeterminizeOptions, DisjunctOptions, EditMetadataOptions, EliminateFlagsOptions,
    ExpandEquivalencesOptions, GuessDirection, GuessifyOptions, InsertFreelyOptions,
    KillPathsOptions, Level, MinimizeOptions, NameOptions, NamedOperand, NamedText, ProjectOptions,
    PruneAlphabetOptions, PushDirection, PushOptions, RepeatOptions, ReweightOptions,
    SubstituteOptions, Tape, Transducer as Machine,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyMapping;

use crate::args::Value;
use crate::call::without_gil;
use crate::error::hfst_error;
use crate::transducer::{Transducer, locked, unlocked};

/// The value `table` gives `given`, or the ValueError a parameter `name` of
/// `call` raises for a value outside it.
pub fn choice<T: Copy>(call: &str, name: &str, given: &str, table: &[(&str, T)]) -> PyResult<T> {
    if let Some((_, value)) = table.iter().find(|(known, _)| *known == given) {
        return Ok(*value);
    }
    let names: Vec<String> = table.iter().map(|(n, _)| format!("'{n}'")).collect();
    Err(PyValueError::new_err(format!(
        "{call} argument '{name}' must be one of {}, not '{given}'",
        names.join(", ")
    )))
}

const PUSH: [(&str, PushDirection); 2] = [
    ("initial", PushDirection::Initial),
    ("final", PushDirection::Final),
];

/// The functions `hfst reweight --function` applies to a weight.
pub const FUNCTIONS: [&str; 15] = [
    "cos", "sin", "tan", "acos", "asin", "atan", "cosh", "sinh", "tanh", "exp", "log", "log10",
    "sqrt", "floor", "ceil",
];

/// A bound of `repeat`: a whole number of times, or `math.inf`.
fn count(name: &str, value: f64) -> PyResult<Count> {
    if value == f64::INFINITY {
        return Ok(Count::Infinite);
    }
    // The library counts repetitions in 32 bits.
    if value.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&value) {
        return Ok(Count::Finite(value as u64));
    }
    Err(PyValueError::new_err(format!(
        "repeat() argument '{name}' must be a whole number from 0 to {}, or math.inf, not {value}",
        u32::MAX
    )))
}

fn pairs(call: &str, name: &str, value: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mapping = value
        .cast::<PyMapping>()
        .map_err(|_| Value { call, name, value }.wrong_type("a mapping of str to str"))?;
    let mut pairs = Vec::new();
    for item in mapping.items()?.iter() {
        pairs.push(item.extract::<(String, String)>()?);
    }
    Ok(pairs)
}

// [spec:hfst:req:python.operations]
// [spec:hfst:req:python.threads+1]
#[pymethods]
impl Transducer {
    /// Determinizes the transducer, as `hfst determinize`. `encode_weights`
    /// treats each weight as part of its arc's label, as `--encode-weights`.
    #[pyo3(signature = (*, encode_weights = false))]
    fn determinize(&self, py: Python<'_>, encode_weights: bool) -> PyResult<()> {
        let options = DeterminizeOptions { encode_weights };
        self.change(py, |t| t.determinize(&options))
    }

    /// Minimizes the transducer, as `hfst minimize`.
    #[pyo3(signature = (*, encode_weights = false))]
    fn minimize(&self, py: Python<'_>, encode_weights: bool) -> PyResult<()> {
        let options = MinimizeOptions { encode_weights };
        self.change(py, |t| t.minimize(&options))
    }

    /// Swaps the input and output sides, as `hfst invert`.
    fn invert(&self, py: Python<'_>) -> PyResult<()> {
        self.change(py, Machine::invert)
    }

    /// Reverses every path, as `hfst reverse`.
    fn reverse(&self, py: Python<'_>) -> PyResult<()> {
        self.change(py, Machine::reverse)
    }

    /// Removes the epsilon arcs, as `hfst remove-epsilons`.
    fn remove_epsilons(&self, py: Python<'_>) -> PyResult<()> {
        self.change(py, Machine::remove_epsilons)
    }

    /// Drops the symbols no arc uses from the alphabet, as
    /// `hfst prune-alphabet`. `force` prunes even when an arc uses the
    /// unknown or identity symbol, as `--force`.
    #[pyo3(signature = (*, force = false))]
    fn prune_alphabet(&self, py: Python<'_>, force: bool) -> PyResult<()> {
        let options = PruneAlphabetOptions { force };
        self.change(py, |t| t.prune_alphabet(&options))
    }

    /// Makes the transducer the identity on one side's language, as
    /// `hfst project`: `"input"` or `"output"`.
    #[pyo3(signature = (*, project = "output"))]
    fn project(&self, py: Python<'_>, project: &str) -> PyResult<()> {
        let project = choice(
            "project()",
            "project",
            project,
            &[("input", Tape::Input), ("output", Tape::Output)],
        )?;
        let options = ProjectOptions { project };
        self.change(py, |t| t.project(&options))
    }

    /// Pushes the weights towards the initial state or the final states, as
    /// `hfst push-weights`.
    #[pyo3(signature = (*, push = "final"))]
    fn push_weights(&self, py: Python<'_>, push: &str) -> PyResult<()> {
        let push = choice("push_weights()", "push", push, &PUSH)?;
        let options = PushOptions { push };
        self.change(py, |t| t.push_weights(&options))
    }

    /// Pushes the labels towards the initial state or the final states, as
    /// `hfst push-labels`.
    #[pyo3(signature = (*, push = "final"))]
    fn push_labels(&self, py: Python<'_>, push: &str) -> PyResult<()> {
        let push = choice("push_labels()", "push", push, &PUSH)?;
        let options = PushOptions { push };
        self.change(py, |t| t.push_labels(&options))
    }

    /// Realigns the symbols of each path, as `hfst realign`.
    fn realign(&self, py: Python<'_>) -> PyResult<()> {
        self.change(py, Machine::realign)
    }

    /// Makes the transducer repeat at least `from_` and at most `to` times,
    /// as `hfst repeat --from --to`. Each is a whole number or `math.inf`,
    /// and `to` None is `math.inf`.
    #[pyo3(signature = (*, from_ = 0.0, to = None))]
    fn repeat(&self, py: Python<'_>, from_: f64, to: Option<f64>) -> PyResult<()> {
        let options = RepeatOptions {
            from: count("from_", from_)?,
            to: count("to", to.unwrap_or(f64::INFINITY))?,
        };
        self.change(py, |t| t.repeat(&options))
    }

    /// Inserts `symbol_pair`, `IN:OUT` with `@0@` for the epsilon, freely
    /// anywhere along each path, as `hfst insert-freely`. `harmonise`
    /// expands the identity and unknown symbols by the pair's symbols.
    #[pyo3(signature = (*, symbol_pair, harmonise = false))]
    fn insert_freely(&self, py: Python<'_>, symbol_pair: String, harmonise: bool) -> PyResult<()> {
        let options = InsertFreelyOptions {
            symbol_pair,
            harmonise,
        };
        self.change(py, |t| t.insert_freely(&options))
    }

    /// Compiles the flag diacritics of the feature `flag`, or of every
    /// feature, into the transducer's paths, as `hfst eliminate-flags`.
    #[pyo3(signature = (*, flag = None))]
    fn eliminate_flags(&self, py: Python<'_>, flag: Option<String>) -> PyResult<()> {
        let options = EliminateFlagsOptions { flag };
        self.change(py, |t| t.eliminate_flags(&options))
    }

    /// Prepares the transducer for conversion to optimized lookup, as
    /// `hfst preprocess-for-optimized-lookup-format`.
    fn preprocess_for_optimized_lookup_format(&self, py: Python<'_>) -> PyResult<()> {
        self.change(py, Machine::preprocess_for_optimized_lookup_format)
    }

    /// Makes the transducer a guesser of the analyses of unknown words by
    /// their affixes, as `hfst affix-guessify`: by their suffixes or their
    /// prefixes, each guessed character costing `weight`.
    #[pyo3(signature = (*, direction = "suffix", weight = 1.0))]
    fn affix_guessify(&self, py: Python<'_>, direction: &str, weight: f32) -> PyResult<()> {
        let direction = choice(
            "affix_guessify()",
            "direction",
            direction,
            &[
                ("prefix", GuessDirection::GuessPrefix),
                ("suffix", GuessDirection::GuessSuffix),
            ],
        )?;
        let options = AffixGuessifyOptions { direction, weight };
        self.change(py, |t| t.affix_guessify(&options))
    }

    /// Lets each symbol stand for the symbols it is equivalent to, as
    /// `hfst expand-equivalences`: `from_` for `to`, or each line of `tsv`,
    /// `FROM<TAB>TO<TAB>TO...`, on the `level` side.
    #[pyo3(signature = (*, from_ = None, to = None, tsv = None, level = "first"))]
    fn expand_equivalences(
        &self,
        py: Python<'_>,
        from_: Option<String>,
        to: Option<String>,
        tsv: Option<String>,
        level: &str,
    ) -> PyResult<()> {
        let level = choice(
            "expand_equivalences()",
            "level",
            level,
            &[
                ("first", Level::First),
                ("second", Level::Second),
                ("both", Level::Both),
            ],
        )?;
        let options = ExpandEquivalencesOptions {
            from: from_,
            to,
            tsv,
            level,
        };
        self.change(py, |t| t.expand_equivalences(&options))
    }

    /// Removes every arc carrying `symbol`, or any symbol `tsv` lists one a
    /// line, as `hfst kill-paths`.
    #[pyo3(signature = (*, symbol = None, tsv = None))]
    fn kill_paths(
        &self,
        py: Python<'_>,
        symbol: Option<String>,
        tsv: Option<String>,
    ) -> PyResult<()> {
        let options = KillPathsOptions { symbol, tsv };
        self.change(py, |t| t.kill_paths(&options))
    }

    /// Changes the weights, as `hfst reweight`: each weight in bounds
    /// becomes `function(weight * multiplier + addition)`, on the arcs and
    /// final states `symbol`, `input_symbol` and `output_symbol` select, or
    /// by the rules of `tsv`, `SYMBOL<TAB>WEIGHT` a line. `upper_bound` None
    /// sets no upper bound.
    #[pyo3(signature = (
        *,
        addition = 0.0,
        multiplier = 1.0,
        function = None,
        lower_bound = 0.0,
        upper_bound = None,
        input_symbol = None,
        output_symbol = None,
        symbol = None,
        end_states_only = false,
        arcs_only = false,
        tsv = None,
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a keyword-only option of hfst reweight"
    )]
    fn reweight(
        &self,
        py: Python<'_>,
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
    ) -> PyResult<()> {
        if let Some(name) = &function {
            let table: Vec<(&str, ())> = FUNCTIONS.iter().map(|f| (*f, ())).collect();
            choice("reweight()", "function", name, &table)?;
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
        self.change(py, |t| t.reweight(&options))
    }

    /// Sets the transducer's name, as `hfst name --name`, cut to
    /// `truncate_length` bytes unless that is 0. The tool is `name`, which
    /// is the attribute that reads the name.
    #[pyo3(signature = (name, /, *, truncate_length = 0))]
    fn set_name(&self, py: Python<'_>, name: String, truncate_length: usize) -> PyResult<()> {
        let options = NameOptions {
            name,
            truncate_length,
        };
        self.change(py, |t| {
            t.set_name(&options);
            Ok(())
        })
    }

    /// Adds or replaces each property of `add`, as `hfst edit-metadata
    /// --add NAME=VALUE`, each value cut to `truncate_length` bytes unless
    /// that is 0.
    #[pyo3(signature = (*, add, truncate_length = 0))]
    fn edit_metadata(
        &self,
        py: Python<'_>,
        add: &Bound<'_, PyAny>,
        truncate_length: usize,
    ) -> PyResult<()> {
        let options = EditMetadataOptions {
            add: pairs("edit_metadata()", "add", add)?,
            truncate_length,
        };
        self.change(py, |t| t.edit_metadata(&options))
    }

    /// Makes the transducer, a morphological analyser, a guesser of the
    /// analyses of unknown words, as `hfst guessify`: weighted optimized
    /// lookup that `guess` takes. Returns the model form generator the tool
    /// writes after the guesser, or None with `do_not_compile_generator`.
    #[pyo3(signature = (*, default_penalty = 1.0, do_not_compile_generator = false))]
    fn guessify(
        &self,
        py: Python<'_>,
        default_penalty: f32,
        do_not_compile_generator: bool,
    ) -> PyResult<Option<Transducer>> {
        let options = GuessifyOptions {
            default_penalty,
            do_not_compile_generator,
        };
        let generator = self.change(py, |t| t.guessify(&options))?;
        Ok(generator.map(Transducer::new))
    }

    /// Replaces labels, as `hfst substitute`: `from_label` by `to_label` or
    /// by `to_transducer`, or each `FROM<TAB>TO` line of `from_file`, the
    /// text of a relabel file, in turn with `in_order`. A label is a symbol
    /// or a colon pair, `@0@` for the epsilon. `file_name` is the name of
    /// the file the tool would read `to_transducer` or `from_file` from,
    /// which names the result. `compose` composes symbol-for-symbol
    /// substitutions in rather than rewriting arcs.
    #[pyo3(signature = (
        *,
        from_label = None,
        to_label = None,
        to_transducer = None,
        from_file = None,
        file_name = "",
        in_order = false,
        do_not_convert = false,
        compose = false,
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a keyword-only option of hfst substitute"
    )]
    fn substitute(
        &self,
        py: Python<'_>,
        from_label: Option<String>,
        to_label: Option<String>,
        to_transducer: Option<Bound<'_, Transducer>>,
        from_file: Option<String>,
        file_name: &str,
        in_order: bool,
        do_not_convert: bool,
        compose: bool,
    ) -> PyResult<()> {
        let to = to_transducer.as_ref().map(Bound::get);
        let done = without_gil(py, || {
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
                    name: file_name,
                }),
                from_file: from_file.map(|text| NamedText {
                    name: file_name.to_owned(),
                    text,
                }),
                in_order,
                do_not_convert,
                compose,
            };
            changed.substitute(&options)
        })?;
        done.map_err(|error| hfst_error(py, error))
    }

    /// Composes `other` after the transducer, as `hfst compose`. Flag
    /// diacritics are harmonized unless `do_not_harmonize`; with
    /// `xerox_composition` they are ordinary symbols, and with
    /// `flag_is_epsilon` a one-sided one is an epsilon. Transducers of two
    /// algebra types are converted to one as the tool converts them, unless
    /// `do_not_convert`. `t.compose(t)` composes the transducer with itself
    /// as it was.
    #[pyo3(signature = (
        other,
        /,
        *,
        do_not_harmonize = false,
        harmonize_flags = false,
        xerox_composition = false,
        flag_is_epsilon = false,
        do_not_convert = false,
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a keyword-only option of hfst compose"
    )]
    fn compose(
        &self,
        py: Python<'_>,
        other: &Bound<'_, Transducer>,
        do_not_harmonize: bool,
        harmonize_flags: bool,
        xerox_composition: bool,
        flag_is_epsilon: bool,
        do_not_convert: bool,
    ) -> PyResult<()> {
        let options = ComposeOptions {
            do_not_harmonize,
            harmonize_flags,
            xerox_composition,
            flag_is_epsilon,
            do_not_convert,
        };
        self.change_with(py, other.get(), |t, other| t.compose(other, &options))
    }

    /// Concatenates `other` after the transducer, as `hfst concatenate`.
    #[pyo3(signature = (other, /, *, do_not_harmonize = false, harmonize_flags = false, do_not_convert = false))]
    fn concatenate(
        &self,
        py: Python<'_>,
        other: &Bound<'_, Transducer>,
        do_not_harmonize: bool,
        harmonize_flags: bool,
        do_not_convert: bool,
    ) -> PyResult<()> {
        let options = BinaryOptions {
            do_not_harmonize,
            harmonize_flags,
            do_not_convert,
        };
        self.change_with(py, other.get(), |t, other| t.concatenate(other, &options))
    }

    /// Intersects the transducer with `other`, as `hfst conjunct`.
    #[pyo3(signature = (other, /, *, do_not_harmonize = false, harmonize_flags = false, do_not_convert = false))]
    fn conjunct(
        &self,
        py: Python<'_>,
        other: &Bound<'_, Transducer>,
        do_not_harmonize: bool,
        harmonize_flags: bool,
        do_not_convert: bool,
    ) -> PyResult<()> {
        let options = BinaryOptions {
            do_not_harmonize,
            harmonize_flags,
            do_not_convert,
        };
        self.change_with(py, other.get(), |t, other| t.conjunct(other, &options))
    }

    /// Subtracts `other` from the transducer, as `hfst subtract`.
    #[pyo3(signature = (other, /, *, do_not_harmonize = false, harmonize_flags = false, do_not_convert = false))]
    fn subtract(
        &self,
        py: Python<'_>,
        other: &Bound<'_, Transducer>,
        do_not_harmonize: bool,
        harmonize_flags: bool,
        do_not_convert: bool,
    ) -> PyResult<()> {
        let options = BinaryOptions {
            do_not_harmonize,
            harmonize_flags,
            do_not_convert,
        };
        self.change_with(py, other.get(), |t, other| t.subtract(other, &options))
    }

    /// Unites the transducer with `other`, as `hfst disjunct`.
    #[pyo3(signature = (other, /, *, do_not_harmonize = false, do_not_convert = false))]
    fn disjunct(
        &self,
        py: Python<'_>,
        other: &Bound<'_, Transducer>,
        do_not_harmonize: bool,
        do_not_convert: bool,
    ) -> PyResult<()> {
        let options = DisjunctOptions {
            do_not_harmonize,
            do_not_convert,
        };
        self.change_with(py, other.get(), |t, other| t.disjunct(other, &options))
    }

    /// Unites the transducer with the paths of `other` whose input it does
    /// not read, as `hfst priority-disjunct`.
    #[pyo3(signature = (other, /, *, do_not_convert = false))]
    fn priority_disjunct(
        &self,
        py: Python<'_>,
        other: &Bound<'_, Transducer>,
        do_not_convert: bool,
    ) -> PyResult<()> {
        let options = ConvertOptions { do_not_convert };
        self.change_with(py, other.get(), |t, other| {
            t.priority_disjunct(other, &options)
        })
    }

    /// Shuffles the transducer with `other`, as `hfst shuffle`.
    #[pyo3(signature = (other, /, *, do_not_convert = false))]
    fn shuffle(
        &self,
        py: Python<'_>,
        other: &Bound<'_, Transducer>,
        do_not_convert: bool,
    ) -> PyResult<()> {
        let options = ConvertOptions { do_not_convert };
        self.change_with(py, other.get(), |t, other| t.shuffle(other, &options))
    }

    /// Composes the transducer, a lexicon, with the intersection of `rules`,
    /// as `hfst compose-intersect`. The result is named after the lexicon,
    /// or `lexicon_name` when it has no name, and `rules_name`, as the tool
    /// names it after its files.
    #[pyo3(signature = (
        rules,
        /,
        *,
        invert = false,
        fast = false,
        encode_weights = false,
        harmonize = false,
        lexicon_name = "",
        rules_name = "",
        do_not_convert = false,
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a keyword-only option of hfst compose-intersect"
    )]
    fn compose_intersect(
        &self,
        py: Python<'_>,
        rules: Vec<Bound<'_, Transducer>>,
        invert: bool,
        fast: bool,
        encode_weights: bool,
        harmonize: bool,
        lexicon_name: &str,
        rules_name: &str,
        do_not_convert: bool,
    ) -> PyResult<()> {
        let options = ComposeIntersectOptions {
            invert,
            fast,
            encode_weights,
            harmonize,
            lexicon_name: lexicon_name.to_owned(),
            rules_name: rules_name.to_owned(),
            do_not_convert,
        };
        let rules: Vec<&Transducer> = rules.iter().map(Bound::get).collect();
        let done = without_gil(py, || {
            let (changed, read) = locked(Some(self), &rules);
            let mut changed = changed.ok_or_else(unlocked)?;
            // A rule that is the lexicon itself is read as it was before.
            let itself = if rules.iter().any(|rule| read.get(rule).is_none()) {
                Some(changed.copy()?)
            } else {
                None
            };
            let rules: Vec<&Machine> = rules
                .iter()
                .filter_map(|rule| read.get(rule).or(itself.as_ref()))
                .collect();
            changed.compose_intersect(&rules, &options)
        })?;
        done.map_err(|error| hfst_error(py, error))
    }
}
