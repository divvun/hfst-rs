//! What each operation tool does to one transducer, or to a pair: the
//! operation, with the options that change its result, and the name and
//! formula the tool gives what it writes. A tool reads its streams and calls
//! these on each transducer it reads; a program that calls them on the same
//! transducers gets what the tool writes.
//!
//! Every function changes the transducer it is given, without copying it.
//! The ones that build a new transducer, as kill-paths and expand-equivalences
//! do, move the old one in and the new one back.

use crate::backend::{AlgebraBackend, Backend};
use crate::expand_equivalences::FsaLevel;
use crate::guessify_fst::GuessDirection;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::{PushType, StringPair};
use crate::hfst_transducer::{EngineConfig, FlagDiacriticOverlay, HfstTransducer};
use crate::tools::metadata::{
    ToolTransducer, hfst_set_formula_binary, hfst_set_formula_unary, hfst_set_name_binary,
    hfst_set_name_unary,
};

type Result<T> = crate::error::Result<T>;

fn stamp(t: &mut impl ToolTransducer, name: &str, formula: Option<&str>) {
    hfst_set_name_unary(t, name);
    if let Some(formula) = formula {
        hfst_set_formula_unary(t, formula);
    }
}

/// `t` moved out, with an empty transducer of its type left behind, for an
/// operation that builds its result from the transducer it consumes.
fn take<B: AlgebraBackend>(t: &mut HfstTransducer<B>) -> HfstTransducer<B> {
    std::mem::take(t)
}

// -----------------------------------------------------------------------------
// One transducer
// -----------------------------------------------------------------------------

// [spec:hfst:req:embed.tools-in-library]
pub fn determinize<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    encode_weights: bool,
) -> Result<()> {
    t.determinize_with_config(&EngineConfig {
        encode_weights,
        ..EngineConfig::default()
    })?;
    stamp(t, "determinize", Some("\u{2336}"));
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
pub fn minimize<B: AlgebraBackend>(t: &mut HfstTransducer<B>, encode_weights: bool) -> Result<()> {
    t.minimize_with_config(&EngineConfig {
        encode_weights,
        ..EngineConfig::default()
    })?;
    stamp(t, "minimize", Some("M"));
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
pub fn invert<B: AlgebraBackend>(t: &mut HfstTransducer<B>) -> Result<()> {
    t.invert()?;
    stamp(t, "invert", Some("\u{207b}\u{00b9}"));
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
pub fn reverse<B: AlgebraBackend>(t: &mut HfstTransducer<B>) -> Result<()> {
    t.reverse()?;
    stamp(t, "reverse", Some("\u{21c6}"));
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
pub fn remove_epsilons<B: AlgebraBackend>(t: &mut HfstTransducer<B>) -> Result<()> {
    t.remove_epsilons()?;
    stamp(t, "remove-epsilons", Some("Id"));
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst prune-alphabet`: the symbols no arc uses leave the alphabet. Unless
/// `force`, only when no arc uses the unknown or identity symbol.
pub fn prune_alphabet<B: AlgebraBackend>(t: &mut HfstTransducer<B>, force: bool) -> Result<()> {
    t.prune_alphabet(force)?;
    stamp(t, "prune-alphabet", None);
    Ok(())
}

/// A side of a transducer: its input, upper or first tape, or its output,
/// lower or second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tape {
    Input,
    Output,
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst project`: the identity on the language of `tape`.
pub fn project<B: AlgebraBackend>(t: &mut HfstTransducer<B>, tape: Tape) -> Result<()> {
    match tape {
        Tape::Input => {
            t.input_project()?;
            stamp(t, "project-1st", Some("\u{00b9}"));
        }
        Tape::Output => {
            t.output_project()?;
            stamp(t, "project-2nd", Some("\u{00b2}"));
        }
    }
    Ok(())
}

/// Which end weights or labels are pushed towards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushDirection {
    /// Towards the initial state: start, initial or begin.
    Initial,
    /// Towards the final states: end or final.
    Final,
}

impl PushDirection {
    fn push_type(self) -> PushType {
        match self {
            PushDirection::Initial => PushType::TO_INITIAL_STATE,
            PushDirection::Final => PushType::TO_FINAL_STATE,
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
pub fn push_weights<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    direction: PushDirection,
) -> Result<()> {
    t.push_weights(direction.push_type())?;
    let name = match direction {
        PushDirection::Initial => "push-weights-i",
        PushDirection::Final => "push-weights-f",
    };
    stamp(t, name, Some("Id"));
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
pub fn push_labels<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    direction: PushDirection,
) -> Result<()> {
    t.push_labels(direction.push_type())?;
    let name = match direction {
        PushDirection::Initial => "push-labels-i",
        PushDirection::Final => "push-labels-f",
    };
    stamp(t, name, Some("Id"));
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
pub fn realign<B: AlgebraBackend>(t: &mut HfstTransducer<B>) -> Result<()> {
    t.realign()?;
    stamp(t, "realign", Some("Id"));
    Ok(())
}

/// How many times `hfst repeat` repeats a transducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repetition {
    /// Any number of times, none included: `--from` and `--to` both
    /// infinite.
    Star,
    /// At least this many times, with no upper bound.
    AtLeast(u32),
    /// At least `from` and at most `to` times.
    Between { from: u32, to: u32 },
}

/// Why `hfst repeat` refuses its bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepetitionError {
    /// The lower bound is above the upper one.
    Reversed { from: u64, to: u64 },
    /// The lower bound is infinite and the upper one is not.
    FromInfinity { to: u64 },
}

impl std::fmt::Display for RepetitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepetitionError::Reversed { from, to } => {
                writeln!(f, "Cannot repeat from {} to {} times", from, to)
            }
            RepetitionError::FromInfinity { to } => {
                writeln!(f, "Cannot repeat from infinity to {} times", to)
            }
        }
    }
}

impl Repetition {
    // [spec:hfst:req:embed.tools-in-library]
    /// The repetition `hfst repeat --from FROM --to TO` asks for, each bound
    /// `None` when it is infinite. The tool reads an absent `--from` as 0 and
    /// an absent `--to` as infinite, and a bound as the 32-bit count it
    /// truncates to.
    pub fn from_bounds(
        from: Option<u64>,
        to: Option<u64>,
    ) -> std::result::Result<Self, RepetitionError> {
        match (from, to) {
            (Some(from), Some(to)) if from > to => Err(RepetitionError::Reversed { from, to }),
            (Some(from), Some(to)) => Ok(Repetition::Between {
                from: from as u32,
                to: to as u32,
            }),
            (Some(from), None) => Ok(Repetition::AtLeast(from as u32)),
            (None, None) => Ok(Repetition::Star),
            (None, Some(to)) => Err(RepetitionError::FromInfinity { to }),
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
pub fn repeat<B: AlgebraBackend>(t: &mut HfstTransducer<B>, repetition: Repetition) -> Result<()> {
    match repetition {
        Repetition::Between { from, to } => {
            t.repeat_n_to_k(from, to)?;
            stamp(
                t,
                &format!("repeat-{}-to-{}", from, to),
                Some(&format!("_{}^{}", from, to)),
            );
        }
        Repetition::Star => {
            t.repeat_star()?;
            stamp(t, "repeat-star", Some("\u{22c6}"));
        }
        Repetition::AtLeast(from) => {
            t.repeat_n_plus(from)?;
            stamp(
                t,
                &format!("repeat-{}-plus", from),
                Some(&format!("_{}^\u{221e}", from)),
            );
        }
    }
    Ok(())
}

/// Why `hfst insert-freely` refuses its `--symbol-pair`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolPairError {
    Empty,
    /// No colon separates the two symbols.
    NoColon(String),
}

impl std::fmt::Display for SymbolPairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolPairError::Empty => write!(
                f,
                "argument of source label option is empty;\nif you REALLY want to replace epsilons with something, use @0@ or {}",
                crate::hfst_symbol_defs::internal_epsilon
            ),
            SymbolPairError::NoColon(label) => {
                write!(f, "symbol pair '{label}' has no colon; give it as SYM:SYM")
            }
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// The pair `hfst insert-freely --symbol-pair` reads from `label`: two
/// symbols separated by a colon, the whole of it `@0@` for the epsilon.
pub fn insert_freely_pair(label: &str) -> std::result::Result<StringPair, SymbolPairError> {
    let read = if label == "@0@" {
        crate::hfst_symbol_defs::internal_epsilon
    } else {
        label
    };
    if read.is_empty() {
        return Err(SymbolPairError::Empty);
    }
    crate::hfst_symbol_defs::label_to_stringpair(read)
        .ok_or_else(|| SymbolPairError::NoColon(label.to_string()))
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst insert-freely`: `pair` may occur anywhere. With `harmonize`, the
/// identity and unknown symbols of `t` are expanded by the pair's symbols.
pub fn insert_freely<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    pair: &StringPair,
    harmonize: bool,
) -> Result<()> {
    t.insert_freely_pair(pair, harmonize)?;
    stamp(t, "insert-freely", Some("Id"));
    Ok(())
}

/// Why `hfst eliminate-flags` failed.
#[derive(Debug)]
pub enum EliminateFlagsError {
    /// The flag feature to eliminate does not occur in the transducer.
    NoSuchFeature(String),
    Operation(crate::error::Error),
}

impl std::fmt::Display for EliminateFlagsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EliminateFlagsError::NoSuchFeature(feature) => write!(
                f,
                "flag feature {} does not occur in the transducer\nonly the flag feature must be given, no value or operator",
                feature
            ),
            EliminateFlagsError::Operation(e) => write!(f, "{}", e),
        }
    }
}

impl From<crate::error::Error> for EliminateFlagsError {
    fn from(e: crate::error::Error) -> Self {
        EliminateFlagsError::Operation(e)
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst eliminate-flags`: every flag diacritic, or only those of the
/// feature `flag`, made redundant by expanding the paths they constrain.
pub fn eliminate_flags<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    flag: Option<&str>,
) -> std::result::Result<(), EliminateFlagsError> {
    match flag {
        None => {
            t.eliminate_flags()?;
        }
        Some(feature) => {
            if t.eliminate_flag(feature).is_err() {
                return Err(EliminateFlagsError::NoSuchFeature(feature.to_string()));
            }
        }
    }
    stamp(t, "eliminate-flags", Some("Id"));
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst preprocess-for-optimized-lookup-format`: epsilons removed, the
/// states renumbered, and epsilons removed again. `rebuilding` hears when the
/// renumbering starts.
pub fn preprocess_for_optimized_lookup_format<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    rebuilding: &mut dyn FnMut(),
) -> Result<()> {
    t.remove_epsilons()?;
    rebuilding();
    let original: HfstBasicTransducer = t.to_basic()?;
    let replication = original.renumber_states();
    *t = HfstTransducer::new_from_basic(&replication)?;
    stamp(t, "fu", Some("FU"));
    t.remove_epsilons()?;
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst affix-guessify`: a guesser for the affixes `t` knows, in
/// `direction`, each arc outside a known affix weighing `weight`.
pub fn affix_guessify<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    direction: GuessDirection,
    weight: f32,
) -> Result<()> {
    *t = crate::guessify_fst::affix_guessify(t, direction, weight)?;
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst expand-equivalences`: each `(from, to)` pair lets `to` stand where
/// `from` does, on the tape `level` names.
pub fn expand_equivalences<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    pairs: &[(String, String)],
    level: FsaLevel,
) -> Result<()> {
    *t = crate::expand_equivalences::expand_equivalences(take(t), pairs, level)?;
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst kill-paths`: every arc carrying one of `symbols` removed, as
/// [`crate::tools::kill_paths::kill_paths`] does it.
pub fn kill_paths<'s, B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    symbols: impl IntoIterator<Item = &'s str>,
    each: &mut dyn FnMut(&str),
) -> Result<()> {
    *t = crate::tools::kill_paths::kill_paths(take(t), symbols, each)?;
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst name --name NAME`: the name, cut to at most `truncate` bytes when
/// it is not 0, a partial character at the cut read as U+FFFD.
pub fn name<B: Backend>(t: &mut HfstTransducer<B>, name: &str, truncate: usize) {
    if truncate == 0 {
        t.set_name(name);
    } else {
        t.set_name(&truncated(name, truncate));
    }
}

/// `value` cut to `len` bytes as `hfst_strndup` cuts it, 0 for no cut.
fn truncated(value: &str, len: usize) -> String {
    if len == 0 {
        return value.to_string();
    }
    let n = len.min(value.len());
    String::from_utf8_lossy(&value.as_bytes()[..n]).into_owned()
}

/// Why `hfst edit-metadata` refused a property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataError {
    /// `character-encoding` set to anything but UTF-8.
    UnsupportedEncoding,
}

impl std::fmt::Display for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetadataError::UnsupportedEncoding => write!(
                f,
                "Cannot set `character-encoding' to unsupported value;\nconsider recoding sources of automaton"
            ),
        }
    }
}

/// What `hfst edit-metadata` warns of when a property it sets changes how the
/// file reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataWarning {
    /// The `type` property, which does not change the transducer's type.
    Type,
    /// The `version` property, which changes how the header is parsed.
    Version,
}

impl std::fmt::Display for MetadataWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetadataWarning::Type => write!(
                f,
                "Changing `type' metadata will not change type of transducer in file;\nhaving wrong type may cause breakage, use with caution"
            ),
            MetadataWarning::Version => write!(
                f,
                "Changing `version' changes parsing semantics for header;\nuse with caution"
            ),
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst edit-metadata --add`: each property set in key order, its value cut
/// to `truncate` bytes when that is not 0. `warn` hears of each property the
/// tool warns about before it is set.
pub fn edit_metadata<'p, B: Backend>(
    t: &mut HfstTransducer<B>,
    properties: impl IntoIterator<Item = (&'p str, &'p str)>,
    truncate: usize,
    warn: &mut dyn FnMut(MetadataWarning),
) -> std::result::Result<(), MetadataError> {
    for (key, value) in properties {
        match key {
            "type" => warn(MetadataWarning::Type),
            "version" => warn(MetadataWarning::Version),
            "character-encoding" if !(value == "utf-8" || value == "UTF-8") => {
                return Err(MetadataError::UnsupportedEncoding);
            }
            _ => {}
        }
        t.set_property(key, &truncated(value, truncate));
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// Two transducers
// -----------------------------------------------------------------------------

/// The second transducer of a binary operation: borrowed from the caller, or
/// owned. The operations that harmonize flag diacritics change both
/// transducers, and a borrowed one is copied only then.
pub enum Operand<'a, B: Backend> {
    Borrowed(&'a HfstTransducer<B>),
    Owned(HfstTransducer<B>),
}

impl<'a, B: Backend> Operand<'a, B> {
    pub fn get(&self) -> &HfstTransducer<B> {
        match self {
            Operand::Borrowed(t) => t,
            Operand::Owned(t) => t,
        }
    }

    /// The operand to change, copied first when it is borrowed.
    pub fn to_mut(&mut self) -> Result<&mut HfstTransducer<B>> {
        if let Operand::Borrowed(t) = self {
            *self = Operand::Owned(HfstTransducer::new_copy(t)?);
        }
        match self {
            Operand::Owned(t) => Ok(t),
            Operand::Borrowed(_) => crate::bail!(Fatal, "a borrowed operand was just copied"),
        }
    }

    /// The operand as an owned transducer, copied when it is borrowed.
    pub fn into_owned(self) -> Result<HfstTransducer<B>> {
        match self {
            Operand::Borrowed(t) => HfstTransducer::new_copy(t),
            Operand::Owned(t) => Ok(t),
        }
    }
}

/// What a binary tool warns of about its operands, before it combines them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryWarning {
    /// The operands have flag diacritics, and harmonizing them was not asked
    /// for. Carries the tool's text, which is not the same in every tool.
    FlagsNotHarmonized(&'static str),
    /// The transducer subtracted has flag diacritics, so the difference may
    /// be wrong. `hfst subtract` warns of this whatever its options.
    SubtrahendHasFlags,
}

/// Options every binary tool takes.
#[derive(Clone, Copy, Debug)]
pub struct BinaryOptions {
    /// Harmonize the alphabets, expanding each operand's unknown and
    /// identity symbols by the other's symbols. `--do-not-harmonize` turns it
    /// off.
    pub harmonize: bool,
    /// Rename the flag diacritics so the operands' flags do not interact:
    /// `--harmonize-flags`.
    pub harmonize_flags: bool,
}

impl Default for BinaryOptions {
    fn default() -> Self {
        BinaryOptions {
            harmonize: true,
            harmonize_flags: false,
        }
    }
}

/// How an algebra operation that can treat flags virtually prepares them.
#[derive(Clone, Copy)]
enum FlagPreparation {
    /// Compose, intersect and subtract: a virtual overlay where the backend
    /// has one, otherwise harmonized with renamed flags inserted.
    Overlay { supported: bool },
    /// Concatenate: harmonized, renamed flags not inserted.
    Rename,
}

/// The flag handling the binary tools share. `needs_flags` says whether the
/// operands' flags call for harmonizing; without `harmonize_flags`, `warn`
/// hears `text` instead.
fn prepare_flags<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &mut Operand<'_, B>,
    needs_flags: bool,
    harmonize_flags: bool,
    preparation: FlagPreparation,
    text: &'static str,
    warn: &mut dyn FnMut(BinaryWarning),
) -> Result<Option<FlagDiacriticOverlay>> {
    if !needs_flags {
        return Ok(None);
    }
    if !harmonize_flags {
        warn(BinaryWarning::FlagsNotHarmonized(text));
        return Ok(None);
    }
    match preparation {
        FlagPreparation::Overlay { supported: true } => first
            .prepare_flag_diacritics_for_operation(second.to_mut()?)
            .map(Some),
        FlagPreparation::Overlay { supported: false } => {
            first.harmonize_flag_diacritics(second.to_mut()?, true)?;
            Ok(None)
        }
        FlagPreparation::Rename => {
            first.harmonize_flag_diacritics(second.to_mut()?, false)?;
            Ok(None)
        }
    }
}

/// Name `first`, the result, after both operands as a binary tool does.
fn stamp_binary<B: Backend>(
    first: &mut HfstTransducer<B>,
    second: &HfstTransducer<B>,
    name: &str,
    formula: &str,
) {
    hfst_set_name_binary(first, second, name);
    hfst_set_formula_binary(first, second, formula);
}

/// What `hfst compose` takes besides the binary options.
#[derive(Clone, Copy, Debug, Default)]
pub struct ComposeOptions {
    pub binary: BinaryOptions,
    /// Flag diacritics are ordinary symbols in composition:
    /// `--xerox-composition`.
    pub xerox_composition: bool,
    /// A one-sided flag diacritic is an epsilon: `--xfst flag-is-epsilon`.
    pub flag_is_epsilon: bool,
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst compose`: `first` composed with `second`. `memory_limit` bounds the
/// working memory of the backends that can spill, as `--memory-limit` does;
/// it changes how the result is reached, never the result.
pub fn compose<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &mut Operand<'_, B>,
    options: &ComposeOptions,
    memory_limit: Option<u64>,
    warn: &mut dyn FnMut(BinaryWarning),
) -> Result<()> {
    let needs_flags = first.has_flag_diacritics() || second.get().has_flag_diacritics();
    let overlay = prepare_flags(
        first,
        second,
        needs_flags,
        options.binary.harmonize_flags,
        FlagPreparation::Overlay {
            supported: B::SUPPORTS_VIRTUAL_FLAG_COMPOSE,
        },
        "At least one of the arguments contains flag diacritics. Use -F to harmonize them.",
        warn,
    )?;
    let config = EngineConfig {
        flag_is_epsilon_in_composition: options.flag_is_epsilon,
        xerox_composition: options.xerox_composition,
        compose_memory_limit_bytes: memory_limit,
        ..EngineConfig::default()
    };
    first.compose_with_config_and_flag_overlay(
        second.get(),
        options.binary.harmonize,
        &config,
        overlay.as_ref(),
    )?;
    stamp_binary(first, second.get(), "compose", "\u{2218}");
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
pub fn concatenate<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &mut Operand<'_, B>,
    options: &BinaryOptions,
    warn: &mut dyn FnMut(BinaryWarning),
) -> Result<()> {
    let needs_flags = first.has_flag_diacritics() && second.get().has_flag_diacritics();
    prepare_flags(
        first,
        second,
        needs_flags,
        options.harmonize_flags,
        FlagPreparation::Rename,
        "The arguments contain flag diacritics. Use -F to harmonize them.",
        warn,
    )?;
    first.concatenate(second.get(), options.harmonize)?;
    stamp_binary(first, second.get(), "concatenate", "\u{22c5}");
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
// [spec:hfst:req:virtual-flag-algebra.intersection]
/// `hfst conjunct`, the intersection.
pub fn conjunct<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &mut Operand<'_, B>,
    options: &BinaryOptions,
    warn: &mut dyn FnMut(BinaryWarning),
) -> Result<()> {
    let needs_flags = first.has_flag_diacritics() || second.get().has_flag_diacritics();
    let overlay = prepare_flags(
        first,
        second,
        needs_flags,
        options.harmonize_flags,
        FlagPreparation::Overlay {
            supported: B::SUPPORTS_VIRTUAL_FLAG_INTERSECTION,
        },
        "At least one of the argumentes contains flag diacritics. Use -F to harmonize them.",
        warn,
    )?;
    first.intersect_with_flag_overlay(second.get(), options.harmonize, overlay.as_ref())?;
    stamp_binary(first, second.get(), "intersect", "\u{2229}");
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst disjunct`, the union. `--harmonize-flags` is not one of its
/// options, so `options.harmonize_flags` is not read.
pub fn disjunct<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &HfstTransducer<B>,
    options: &BinaryOptions,
) -> Result<()> {
    first.disjunct(second, options.harmonize)?;
    stamp_binary(first, second, "union", "\u{222a}");
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst priority-disjunct`: the union in which `first`'s paths win where
/// both have an input. It harmonizes nothing, whatever the options.
pub fn priority_disjunct<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &HfstTransducer<B>,
) -> Result<()> {
    first.priority_union(second)?;
    stamp_binary(first, second, "union", "\u{222a}");
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst shuffle`, of two automata.
pub fn shuffle<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &HfstTransducer<B>,
) -> Result<()> {
    first.shuffle(second, true)?;
    stamp_binary(first, second, "shuffle", "shuffle");
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library]
/// `hfst subtract`: the paths of `first` that are not in `second`.
pub fn subtract<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &mut Operand<'_, B>,
    options: &BinaryOptions,
    warn: &mut dyn FnMut(BinaryWarning),
) -> Result<()> {
    let second_has_flags = second.get().has_flag_diacritics();
    if second_has_flags {
        warn(BinaryWarning::SubtrahendHasFlags);
    }
    let needs_flags = first.has_flag_diacritics() && second_has_flags;
    let overlay = prepare_flags(
        first,
        second,
        needs_flags,
        options.harmonize_flags,
        FlagPreparation::Overlay {
            supported: B::SUPPORTS_VIRTUAL_FLAG_SUBTRACTION,
        },
        "The argumentes contain flag diacritics. Use -F to harmonize them.",
        warn,
    )?;
    first.subtract_with_flag_overlay(second.get(), options.harmonize, overlay.as_ref())?;
    stamp_binary(first, second.get(), "subtract", "\u{2212}");
    Ok(())
}
