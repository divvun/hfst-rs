//! The operations of the compile surface: each operation tool as a method
//! that changes the transducer it is called on, with the tool's options as a
//! typed value. Each calls the library function the tool calls.
//!
//! What a tool warns of goes to `tracing` at the warn level, under the
//! target `hfst_embed::compile`, which both packages pass on to their host's
//! logging.

use hfst::backend::AlgebraBackend;
use hfst::expand_equivalences::read_tsv_extensions;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use hfst::substitute_driver::SubstituteReporter;
use hfst::tools::compose_intersect::ComposeIntersectWarning;
use hfst::tools::conversion::conversion_needed;
use hfst::tools::format::format_description;
use hfst::tools::operations::{self as lib, BinaryWarning};
use hfst::tools::reweight::{Reweighting, finish_reweighted, reweight_by_tsv, weight_function};
use hfst::tools::substitute::{Substitution, relabel_entries, substitute_label};
use hfst_openfst::StdVectorFst;

use super::transducer::{Operand, Transducer};
use crate::error::{Code, Error};

pub use hfst::expand_equivalences::FsaLevel as Level;
pub use hfst::guessify_fst::GuessDirection;
pub use hfst::tools::operations::{PushDirection, Repetition, Tape};

fn refuses_optimized_lookup(tool: &str) -> Error {
    Error::new(
        Code::TypeMismatch,
        format!(
            "hfst {tool} cannot process transducers that are in optimized lookup format; \
             convert it to openfst-tropical or foma first"
        ),
    )
}

fn warn(message: &str) {
    tracing::warn!(target: "hfst_embed::compile", "{}", message);
}

fn binary_warning(w: BinaryWarning) {
    match w {
        BinaryWarning::FlagsNotHarmonized(text) => warn(text),
        BinaryWarning::SubtrahendHasFlags => warn(
            "the transducer subtracted contains flag diacritics; the result of subtraction may be incorrect",
        ),
    }
}

// [spec:hfst:req:embed.no-panic]
/// Run `$body` with `$t` the typed transducer of an algebra type, or refuse
/// optimized lookup as `$tool` does. It runs on the stack a compile is
/// given, which the algebra's recursion, such as minimizing along a long
/// path, is measured against.
macro_rules! algebra {
    ($self:expr, $tool:expr, $t:ident => $body:expr) => {
        hfst::depth::on_compile_stack(|| match &mut $self.inner {
            AnyTransducer::Tropical($t) => $body,
            AnyTransducer::Foma($t) => $body,
            AnyTransducer::OlW(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_) => {
                Err(refuses_optimized_lookup($tool))
            }
        })
    };
}

/// Run `$body` with `$t` the typed transducer, whatever its type.
macro_rules! any_type {
    ($self:expr, $t:ident => $body:expr) => {
        match &mut $self.inner {
            AnyTransducer::Tropical($t) => $body,
            AnyTransducer::Foma($t) => $body,
            AnyTransducer::OlW($t) => $body,
            AnyTransducer::OlU($t) => $body,
            AnyTransducer::Thfst($t) => $body,
        }
    };
}

fn is_optimized_lookup(t: &AnyTransducer) -> bool {
    match t {
        AnyTransducer::OlW(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_) => true,
        AnyTransducer::Tropical(_) | AnyTransducer::Foma(_) => false,
    }
}

/// The second operand of `tool`, as the binary tools take one: refused in
/// optimized lookup; when its type and `first`'s differ, one of the two
/// converted to the other's type as [`conversion_needed`] says, or refused
/// when `do_not_convert`. `itself` and `converted` hold an operand made here.
fn binary_operand<'o>(
    first: &mut AnyTransducer,
    other: Operand<'o>,
    itself: &'o mut Option<AnyTransducer>,
    converted: &'o mut Option<AnyTransducer>,
    tool: &str,
    noun: &str,
    do_not_convert: bool,
) -> Result<&'o AnyTransducer, Error> {
    let second: &'o AnyTransducer = match other {
        Operand::Other(t) => &t.inner,
        Operand::Itself => itself.insert(first.copy()?),
    };
    if is_optimized_lookup(first) || is_optimized_lookup(second) {
        return Err(refuses_optimized_lookup(tool));
    }
    let (first_type, second_type) = (first.get_type(), second.get_type());
    let Some(note) = conversion_needed(first_type, second_type) else {
        return Ok(second);
    };
    if do_not_convert {
        return Err(Error::new(
            Code::TypeMismatch,
            format!(
                "formats {} and {} are not compatible for {} (do_not_convert was asked for)",
                format_description(first_type),
                format_description(second_type),
                noun
            ),
        ));
    }
    let target = note.target();
    if target == first_type {
        Ok(converted.insert(second.converted(target, "")?))
    } else {
        *first = first.converted(target, "")?;
        Ok(second)
    }
}

// [spec:hfst:req:embed.no-panic]
/// Run a binary operation: `$body` with `$f` the typed transducer and `$s`
/// the operand of the same type, as a library [`lib::Operand`] when the
/// operation may change it (`mut $s`), else as a typed reference. It runs on
/// the stack a compile is given, as [`algebra`] does.
macro_rules! binary {
    ($self:expr, $other:expr, $tool:expr, $noun:expr, $do_not_convert:expr, $f:ident, mut $s:ident => $body:expr) => {
        binary!(@run $self, $other, $tool, $noun, $do_not_convert, $f, s => {
            let mut $s = lib::Operand::Borrowed(s);
            $body
        })
    };
    ($self:expr, $other:expr, $tool:expr, $noun:expr, $do_not_convert:expr, $f:ident, $s:ident => $body:expr) => {
        binary!(@run $self, $other, $tool, $noun, $do_not_convert, $f, $s => $body)
    };
    (@run $self:expr, $other:expr, $tool:expr, $noun:expr, $do_not_convert:expr, $f:ident, $s:ident => $body:expr) => {
        hfst::depth::on_compile_stack(|| {
        let mut itself = None;
        let mut converted = None;
        let second = binary_operand(
            &mut $self.inner,
            $other,
            &mut itself,
            &mut converted,
            $tool,
            $noun,
            $do_not_convert,
        )?;
        match (&mut $self.inner, second) {
            (AnyTransducer::Tropical($f), AnyTransducer::Tropical($s)) => $body,
            (AnyTransducer::Foma($f), AnyTransducer::Foma($s)) => $body,
            _ => Err(Error::new(
                Code::Internal,
                "the operands did not convert to one type",
            )),
        }
        })
    };
}

/// `hfst determinize`'s options.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeterminizeOptions {
    pub encode_weights: bool,
}

/// `hfst minimize`'s options.
#[derive(Clone, Copy, Debug, Default)]
pub struct MinimizeOptions {
    pub encode_weights: bool,
}

/// `hfst project`'s options.
#[derive(Clone, Copy, Debug)]
pub struct ProjectOptions {
    pub project: Tape,
}

impl Default for ProjectOptions {
    /// The tool's default, the output tape.
    fn default() -> Self {
        ProjectOptions {
            project: Tape::Output,
        }
    }
}

/// `hfst push-weights`' and `hfst push-labels`' options.
#[derive(Clone, Copy, Debug)]
pub struct PushOptions {
    pub push: PushDirection,
}

impl Default for PushOptions {
    /// The tools' default, towards the final states.
    fn default() -> Self {
        PushOptions {
            push: PushDirection::Final,
        }
    }
}

/// `hfst prune-alphabet`'s options.
#[derive(Clone, Copy, Debug, Default)]
pub struct PruneAlphabetOptions {
    /// Prune even when an arc uses the unknown or identity symbol, as
    /// `--force`; the default is `--safe`.
    pub force: bool,
}

/// A bound of `hfst repeat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    Finite(u64),
    Infinite,
}

/// `hfst repeat`'s options.
#[derive(Clone, Copy, Debug)]
pub struct RepeatOptions {
    pub from: Count,
    pub to: Count,
}

impl Default for RepeatOptions {
    /// The tool's defaults: at least 0 and at most infinitely many times.
    fn default() -> Self {
        RepeatOptions {
            from: Count::Finite(0),
            to: Count::Infinite,
        }
    }
}

/// `hfst insert-freely`'s options.
#[derive(Clone, Debug, Default)]
pub struct InsertFreelyOptions {
    /// The pair to insert, as `--symbol-pair` takes it: `IN:OUT`, `@0@` for
    /// the epsilon.
    pub symbol_pair: String,
    /// Expand the identity and unknown symbols by the pair's symbols, as
    /// `--harmonise`.
    pub harmonise: bool,
}

/// `hfst eliminate-flags`' options.
#[derive(Clone, Debug, Default)]
pub struct EliminateFlagsOptions {
    /// Eliminate only the flags of this feature; `None` eliminates them all.
    pub flag: Option<String>,
}

/// `hfst kill-paths`' options: one of the two is required.
#[derive(Clone, Debug, Default)]
pub struct KillPathsOptions {
    /// The symbol whose arcs to remove.
    pub symbol: Option<String>,
    /// The text of a `--tsv` file listing the symbols, one a line.
    pub tsv: Option<String>,
}

/// `hfst reweight`'s options.
#[derive(Clone, Debug)]
pub struct ReweightOptions {
    pub addition: f32,
    pub multiplier: f32,
    /// One of the functions `--function` names: cos, sin, tan, acos, asin,
    /// atan, cosh, sinh, tanh, exp, log, log10, sqrt, floor or ceil.
    pub function: Option<String>,
    pub lower_bound: f32,
    pub upper_bound: f32,
    pub input_symbol: Option<String>,
    pub output_symbol: Option<String>,
    pub symbol: Option<String>,
    pub end_states_only: bool,
    pub arcs_only: bool,
    /// The text of a `--tsv` rules file, `SYMBOL<TAB>WEIGHT` a line.
    pub tsv: Option<String>,
}

impl Default for ReweightOptions {
    fn default() -> Self {
        let rule = Reweighting::default();
        ReweightOptions {
            addition: rule.addition,
            multiplier: rule.multiplier,
            function: None,
            lower_bound: rule.lower_bound,
            upper_bound: rule.upper_bound,
            input_symbol: None,
            output_symbol: None,
            symbol: None,
            end_states_only: false,
            arcs_only: false,
            tsv: None,
        }
    }
}

/// `hfst name --name`'s options.
#[derive(Clone, Debug, Default)]
pub struct NameOptions {
    pub name: String,
    /// Cut the name to this many bytes; 0 does not cut it.
    pub truncate_length: usize,
}

/// `hfst edit-metadata --add`'s options.
#[derive(Clone, Debug, Default)]
pub struct EditMetadataOptions {
    /// Each property to add or replace, as `--add NAME=VALUE`.
    pub add: Vec<(String, String)>,
    /// Cut each value to this many bytes; 0 does not cut it.
    pub truncate_length: usize,
}

/// `hfst expand-equivalences`' options: `from` and `to` together, or `tsv`.
#[derive(Clone, Debug)]
pub struct ExpandEquivalencesOptions {
    pub from: Option<String>,
    pub to: Option<String>,
    /// The text of a `--tsv` file, `FROM<TAB>TO<TAB>TO...` a line.
    pub tsv: Option<String>,
    pub level: Level,
}

impl Default for ExpandEquivalencesOptions {
    fn default() -> Self {
        ExpandEquivalencesOptions {
            from: None,
            to: None,
            tsv: None,
            level: Level::First,
        }
    }
}

/// A file's text with the name the tool would know it by, which the tools
/// that read one put in the name they give their result.
#[derive(Clone, Debug, Default)]
pub struct NamedText {
    pub name: String,
    pub text: String,
}

/// A transducer with the name of the file the tool would have read it from.
#[derive(Clone, Copy)]
pub struct NamedOperand<'a> {
    pub transducer: &'a Transducer,
    pub name: &'a str,
}

/// `hfst substitute`'s options: `from_label` with `to_label` or
/// `to_transducer`, or `from_file`.
#[derive(Clone, Default)]
pub struct SubstituteOptions<'a> {
    /// A symbol or a colon pair, `@0@` for the epsilon.
    pub from_label: Option<String>,
    pub to_label: Option<String>,
    pub to_transducer: Option<NamedOperand<'a>>,
    /// A relabel file, `FROM<TAB>TO` a line.
    pub from_file: Option<NamedText>,
    /// With `from_file`, apply each line in turn.
    pub in_order: bool,
    pub do_not_convert: bool,
    /// Compose symbol-for-symbol substitutions in rather than rewriting
    /// arcs.
    pub compose: bool,
}

/// `hfst affix-guessify`'s options.
#[derive(Clone, Copy, Debug)]
pub struct AffixGuessifyOptions {
    pub direction: GuessDirection,
    pub weight: f32,
}

impl Default for AffixGuessifyOptions {
    fn default() -> Self {
        AffixGuessifyOptions {
            direction: GuessDirection::GuessSuffix,
            weight: 1.0,
        }
    }
}

/// `hfst guessify`'s options.
#[derive(Clone, Copy, Debug)]
pub struct GuessifyOptions {
    pub default_penalty: f32,
    pub do_not_compile_generator: bool,
}

impl Default for GuessifyOptions {
    fn default() -> Self {
        GuessifyOptions {
            default_penalty: 1.0,
            do_not_compile_generator: false,
        }
    }
}

/// `hfst compose`'s options.
#[derive(Clone, Copy, Debug, Default)]
pub struct ComposeOptions {
    pub do_not_harmonize: bool,
    pub harmonize_flags: bool,
    /// Flag diacritics are ordinary symbols in composition.
    pub xerox_composition: bool,
    /// A one-sided flag diacritic is an epsilon, as `--xfst flag-is-epsilon`.
    pub flag_is_epsilon: bool,
    pub do_not_convert: bool,
}

/// The options of `hfst concatenate`, `hfst conjunct` and `hfst subtract`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BinaryOptions {
    pub do_not_harmonize: bool,
    pub harmonize_flags: bool,
    pub do_not_convert: bool,
}

/// `hfst disjunct`'s options.
#[derive(Clone, Copy, Debug, Default)]
pub struct DisjunctOptions {
    pub do_not_harmonize: bool,
    pub do_not_convert: bool,
}

/// The options of `hfst priority-disjunct` and `hfst shuffle`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ConvertOptions {
    pub do_not_convert: bool,
}

/// `hfst compose-intersect`'s options.
#[derive(Clone, Debug, Default)]
pub struct ComposeIntersectOptions {
    pub invert: bool,
    pub fast: bool,
    pub encode_weights: bool,
    pub harmonize: bool,
    /// The name the result takes the lexicon by when it has none, as the
    /// tool takes its file's.
    pub lexicon_name: String,
    /// The name the result takes the rules by, as the tool takes their
    /// file's.
    pub rules_name: String,
    pub do_not_convert: bool,
}

/// The substitution engine's progress lines, which a package does not show.
struct Silent;

impl SubstituteReporter for Silent {
    fn verbose(&self, msg: &str) {
        tracing::debug!(target: "hfst_embed::compile", "{}", msg.trim_end());
    }
}

impl Transducer {
    // [spec:hfst:req:embed.operations]
    pub fn determinize(&mut self, options: &DeterminizeOptions) -> Result<(), Error> {
        algebra!(self, "determinize", t => Ok(lib::determinize(t, options.encode_weights)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn minimize(&mut self, options: &MinimizeOptions) -> Result<(), Error> {
        algebra!(self, "minimize", t => Ok(lib::minimize(t, options.encode_weights)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn invert(&mut self) -> Result<(), Error> {
        algebra!(self, "invert", t => Ok(lib::invert(t)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn reverse(&mut self) -> Result<(), Error> {
        algebra!(self, "reverse", t => Ok(lib::reverse(t)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn remove_epsilons(&mut self) -> Result<(), Error> {
        algebra!(self, "remove-epsilons", t => Ok(lib::remove_epsilons(t)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn prune_alphabet(&mut self, options: &PruneAlphabetOptions) -> Result<(), Error> {
        algebra!(self, "prune-alphabet", t => Ok(lib::prune_alphabet(t, options.force)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn project(&mut self, options: &ProjectOptions) -> Result<(), Error> {
        algebra!(self, "project", t => Ok(lib::project(t, options.project)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn push_weights(&mut self, options: &PushOptions) -> Result<(), Error> {
        algebra!(self, "push-weights", t => Ok(lib::push_weights(t, options.push)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn push_labels(&mut self, options: &PushOptions) -> Result<(), Error> {
        algebra!(self, "push-labels", t => Ok(lib::push_labels(t, options.push)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn realign(&mut self) -> Result<(), Error> {
        algebra!(self, "realign", t => Ok(lib::realign(t)?))
    }

    // [spec:hfst:req:embed.operations]
    /// `hfst repeat`. A lower bound above the upper, or an infinite lower
    /// bound with a finite upper one, is refused as the tool refuses it.
    pub fn repeat(&mut self, options: &RepeatOptions) -> Result<(), Error> {
        let repetition = match (options.from, options.to) {
            (Count::Finite(from), Count::Finite(to)) if from > to => {
                return Err(Error::new(
                    Code::InvalidArgument,
                    format!("Cannot repeat from {} to {} times", from, to),
                ));
            }
            (Count::Infinite, Count::Finite(to)) => {
                return Err(Error::new(
                    Code::InvalidArgument,
                    format!("Cannot repeat from infinity to {} times", to),
                ));
            }
            (Count::Finite(from), Count::Finite(to)) => Repetition::Between {
                from: from as u32,
                to: to as u32,
            },
            (Count::Finite(from), Count::Infinite) => Repetition::AtLeast(from as u32),
            (Count::Infinite, Count::Infinite) => Repetition::Star,
        };
        algebra!(self, "repeat", t => Ok(lib::repeat(t, repetition)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn insert_freely(&mut self, options: &InsertFreelyOptions) -> Result<(), Error> {
        let pair = lib::insert_freely_pair(&options.symbol_pair)
            .map_err(|e| Error::new(Code::InvalidArgument, e.to_string()))?;
        algebra!(self, "insert-freely", t => Ok(lib::insert_freely(t, &pair, options.harmonise)?))
    }

    // [spec:hfst:req:embed.operations]
    pub fn eliminate_flags(&mut self, options: &EliminateFlagsOptions) -> Result<(), Error> {
        algebra!(self, "eliminate-flags", t => {
            lib::eliminate_flags(t, options.flag.as_deref()).map_err(|e| match e {
                lib::EliminateFlagsError::Operation(e) => Error::from(e),
                e @ lib::EliminateFlagsError::NoSuchFeature(_) => Error::new(Code::InvalidArgument, e.to_string()),
            })
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn preprocess_for_optimized_lookup_format(&mut self) -> Result<(), Error> {
        algebra!(self, "preprocess-for-optimized-lookup-format", t => {
            Ok(lib::preprocess_for_optimized_lookup_format(t, &mut || {})?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn affix_guessify(&mut self, options: &AffixGuessifyOptions) -> Result<(), Error> {
        algebra!(self, "affix-guessify", t => {
            Ok(lib::affix_guessify(t, options.direction, options.weight)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    /// `hfst expand-equivalences`. Without `from` and `to` or `tsv`, there
    /// is nothing to expand by and the call is refused, as the tool refuses
    /// it.
    pub fn expand_equivalences(
        &mut self,
        options: &ExpandEquivalencesOptions,
    ) -> Result<(), Error> {
        let pairs = match (&options.from, &options.to, &options.tsv) {
            (Some(_), _, Some(_)) | (_, Some(_), Some(_)) => {
                return Err(Error::new(
                    Code::InvalidArgument,
                    "Only one of -a, -T or -f and -t may be given",
                ));
            }
            (Some(from), Some(to), None) => vec![(from.clone(), to.clone())],
            (None, Some(_), None) => {
                return Err(Error::new(Code::InvalidArgument, "option -t requires -f"));
            }
            (Some(_), None, None) => {
                return Err(Error::new(Code::InvalidArgument, "option -f requires -t"));
            }
            (None, None, Some(tsv)) => read_tsv_extensions(tsv.as_bytes()).map_err(|e| {
                Error::new(
                    Code::InvalidArgument,
                    format!("line {}: {}", e.line, e.message),
                )
            })?,
            (None, None, None) => {
                return Err(Error::new(
                    Code::InvalidArgument,
                    "Must give extension specification file with either -a or -t.",
                ));
            }
        };
        algebra!(self, "expand-equivalences", t => {
            Ok(lib::expand_equivalences(t, &pairs, options.level)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn kill_paths(&mut self, options: &KillPathsOptions) -> Result<(), Error> {
        let symbols = match (&options.symbol, &options.tsv) {
            (_, Some(tsv)) => hfst::tools::kill_paths::kill_symbols(tsv),
            (Some(symbol), None) => vec![symbol.clone()],
            (None, None) => {
                return Err(Error::new(
                    Code::InvalidArgument,
                    "Either --symbol or --tsv-file is required",
                ));
            }
        };
        algebra!(self, "kill-paths", t => {
            Ok(lib::kill_paths(t, symbols.iter().map(String::as_str), &mut |_| {})?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn reweight(&mut self, options: &ReweightOptions) -> Result<(), Error> {
        if options.arcs_only && options.end_states_only {
            return Err(Error::new(
                Code::InvalidArgument,
                "Options '--arcs-only' and '--end-states-only' cannot be used at the same time",
            ));
        }
        let function = match &options.function {
            None => Reweighting::default().function,
            Some(name) => weight_function(name).ok_or_else(|| {
                Error::new(
                    Code::InvalidArgument,
                    format!("Unrecognised function {}", name),
                )
            })?,
        };
        let rule = Reweighting {
            addition: options.addition,
            multiplier: options.multiplier,
            function,
            lower_bound: options.lower_bound,
            upper_bound: options.upper_bound,
            symbol: options.symbol.clone(),
            input_symbol: options.input_symbol.clone(),
            output_symbol: options.output_symbol.clone(),
            ends_only: options.end_states_only,
            arcs_only: options.arcs_only,
        };
        if matches!(self.inner, AnyTransducer::Foma(_)) {
            warn("Weighting is not supported in this automaton type;weights will be discarded");
        }
        algebra!(self, "reweight", t => {
            match &options.tsv {
                None => rule.apply(t)?,
                Some(tsv) => reweight_by_tsv(t, &rule, tsv, &mut |_| {}).map_err(|e| match e {
                    hfst::tools::reweight::ReweightError::Operation(e) => Error::from(e),
                    hfst::tools::reweight::ReweightError::Tsv { line, error } => {
                        Error::new(Code::InvalidArgument, format!("line {}: {}", line, error))
                    }
                })?,
            }
            Ok(finish_reweighted(t)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    /// `hfst name --name`, which any type takes.
    pub fn set_name(&mut self, options: &NameOptions) {
        any_type!(self, t => lib::name(t, &options.name, options.truncate_length))
    }

    // [spec:hfst:req:embed.operations]
    /// `hfst edit-metadata --add`, which any type takes.
    pub fn edit_metadata(&mut self, options: &EditMetadataOptions) -> Result<(), Error> {
        // The tool sets them in key order, the last value of a key winning.
        let properties: std::collections::BTreeMap<&str, &str> = options
            .add
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        any_type!(self, t => {
            lib::edit_metadata(t, properties.iter().map(|(k, v)| (*k, *v)), options.truncate_length, &mut |w| {
                warn(&w.to_string())
            })
            .map_err(|e| Error::new(Code::InvalidArgument, e.to_string()))
        })
    }

    // [spec:hfst:req:embed.operations]
    /// `hfst guessify`: this transducer becomes the guesser, in weighted
    /// optimized lookup, and the model form generator the tool writes after
    /// it comes back unless `do_not_compile_generator`.
    pub fn guessify(&mut self, options: &GuessifyOptions) -> Result<Option<Transducer>, Error> {
        if options.default_penalty < 0.0 {
            return Err(Error::new(
                Code::InvalidArgument,
                format!(
                    "Invalid default penalty {}. Give a positive float.",
                    options.default_penalty
                ),
            ));
        }
        // [spec:hfst:req:embed.no-panic]
        hfst::depth::on_compile_stack(|| {
            let analyzer: HfstTransducer<StdVectorFst> = match &self.inner {
                AnyTransducer::Tropical(t) => HfstTransducer::new_copy(t)?,
                other @ (AnyTransducer::OlW(_)
                | AnyTransducer::OlU(_)
                | AnyTransducer::Foma(_)
                | AnyTransducer::Thfst(_)) => other.copy()?.into_typed()?,
            };
            let mut guesser =
                hfst::guessify_fst::guessify_analyzer(analyzer, options.default_penalty)?;
            let (guesser, generator) = hfst::guessify_fst::guesser_transducers(
                &mut guesser,
                !options.do_not_compile_generator,
            )?;
            self.inner = AnyTransducer::OlW(guesser);
            Ok(generator.map(|g| Transducer::new(AnyTransducer::OlW(g))))
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn substitute(&mut self, options: &SubstituteOptions<'_>) -> Result<(), Error> {
        let from = options.from_label.as_deref().map(substitute_label);
        let to = options.to_label.as_deref().map(substitute_label);
        if matches!(from.as_deref(), Some("")) {
            return Err(Error::new(
                Code::InvalidArgument,
                "argument of source label option is empty",
            ));
        }
        if matches!(to.as_deref(), Some("")) {
            return Err(Error::new(
                Code::InvalidArgument,
                "argument of target label option is empty",
            ));
        }
        if from.is_none() && options.from_file.is_none() {
            return Err(Error::new(
                Code::InvalidArgument,
                "Must state name of labels to rewrite with -f or -F",
            ));
        }
        if to.is_none() && options.to_transducer.is_none() && options.from_file.is_none() {
            return Err(Error::new(
                Code::InvalidArgument,
                "Must give target labels with -t, -T or -F",
            ));
        }
        if let Some(file) = &options.from_file {
            let entries = relabel_entries(&file.text).map_err(|e| {
                Error::new(
                    Code::InvalidArgument,
                    format!("{}.{}: {}", file.name, e.line, e.error),
                )
            })?;
            return algebra!(self, "substitute", t => {
                let substitution = Substitution::Relabel {
                    entries: &entries,
                    in_order: options.in_order,
                    compose: options.compose,
                    file_name: &file.name,
                };
                Ok(hfst::tools::substitute::substitute(t, substitution, &Silent)?)
            });
        }
        let from = from.unwrap_or_default();
        if let Some(to) = to {
            return algebra!(self, "substitute", t => {
                let substitution: Substitution<'_, _> = Substitution::Label {
                    from: &from,
                    to: &to,
                    compose: options.compose,
                };
                Ok(hfst::tools::substitute::substitute(t, substitution, &Silent)?)
            });
        }
        let Some(to) = options.to_transducer else {
            return Err(Error::new(
                Code::InvalidArgument,
                "Must give target labels with -t, -T or -F",
            ));
        };
        // [spec:hfst:req:embed.no-panic]
        hfst::depth::on_compile_stack(|| {
            let mut converted = None;
            let mut itself = None;
            let replacement = binary_operand(
                &mut self.inner,
                Operand::Other(to.transducer),
                &mut itself,
                &mut converted,
                "substitute",
                "substitution",
                options.do_not_convert,
            )?;
            match (&mut self.inner, replacement) {
                (AnyTransducer::Tropical(t), AnyTransducer::Tropical(r)) => {
                    substitute_by_transducer(t, &from, r, to.name)
                }
                (AnyTransducer::Foma(t), AnyTransducer::Foma(r)) => {
                    substitute_by_transducer(t, &from, r, to.name)
                }
                _ => Err(Error::new(
                    Code::Internal,
                    "the operands did not convert to one type",
                )),
            }
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn compose(&mut self, other: Operand<'_>, options: &ComposeOptions) -> Result<(), Error> {
        let library = lib::ComposeOptions {
            binary: lib::BinaryOptions {
                harmonize: !options.do_not_harmonize,
                harmonize_flags: options.harmonize_flags,
            },
            xerox_composition: options.xerox_composition,
            flag_is_epsilon: options.flag_is_epsilon,
        };
        binary!(self, other, "compose", "composition", options.do_not_convert, first, mut second => {
            Ok(lib::compose(first, &mut second, &library, None, &mut binary_warning)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn concatenate(
        &mut self,
        other: Operand<'_>,
        options: &BinaryOptions,
    ) -> Result<(), Error> {
        let library = library_binary(options);
        binary!(self, other, "concatenate", "concatenation", options.do_not_convert, first, mut second => {
            Ok(lib::concatenate(first, &mut second, &library, &mut binary_warning)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn conjunct(&mut self, other: Operand<'_>, options: &BinaryOptions) -> Result<(), Error> {
        let library = library_binary(options);
        binary!(self, other, "conjunct", "conjunction", options.do_not_convert, first, mut second => {
            Ok(lib::conjunct(first, &mut second, &library, &mut binary_warning)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn subtract(&mut self, other: Operand<'_>, options: &BinaryOptions) -> Result<(), Error> {
        let library = library_binary(options);
        binary!(self, other, "subtract", "subtraction", options.do_not_convert, first, mut second => {
            Ok(lib::subtract(first, &mut second, &library, &mut binary_warning)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn disjunct(&mut self, other: Operand<'_>, options: &DisjunctOptions) -> Result<(), Error> {
        let library = lib::BinaryOptions {
            harmonize: !options.do_not_harmonize,
            harmonize_flags: false,
        };
        binary!(self, other, "disjunct", "disjunction", options.do_not_convert, first, second => {
            Ok(lib::disjunct(first, second, &library)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn priority_disjunct(
        &mut self,
        other: Operand<'_>,
        options: &ConvertOptions,
    ) -> Result<(), Error> {
        binary!(self, other, "priority-disjunct", "priority disjunction", options.do_not_convert, first, second => {
            Ok(lib::priority_disjunct(first, second)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    pub fn shuffle(&mut self, other: Operand<'_>, options: &ConvertOptions) -> Result<(), Error> {
        binary!(self, other, "shuffle", "shuffle", options.do_not_convert, first, second => {
            Ok(lib::shuffle(first, second)?)
        })
    }

    // [spec:hfst:req:embed.operations]
    /// `hfst compose-intersect`: this transducer, the lexicon, composed with
    /// the intersection of `rules`. Each rule is copied and minimized as the
    /// tool minimizes the rules it reads, and the rules convert to the
    /// lexicon's type, or the lexicon to theirs, as the first rule's type
    /// and the lexicon's say.
    pub fn compose_intersect(
        &mut self,
        rules: &[&Transducer],
        options: &ComposeIntersectOptions,
    ) -> Result<(), Error> {
        if is_optimized_lookup(&self.inner) || rules.iter().any(|r| is_optimized_lookup(&r.inner)) {
            return Err(refuses_optimized_lookup("compose-intersect"));
        }
        if let Some(first) = rules.first() {
            let (lexicon_type, rules_type) = (self.inner.get_type(), first.inner.get_type());
            if let Some(note) = conversion_needed(lexicon_type, rules_type) {
                if options.do_not_convert {
                    return Err(Error::new(
                        Code::TypeMismatch,
                        format!(
                            "formats {} and {} are not compatible for compose-intersect (do_not_convert was asked for)",
                            format_description(lexicon_type),
                            format_description(rules_type)
                        ),
                    ));
                }
                if note.target() != lexicon_type {
                    self.inner = self.inner.converted(note.target(), "")?;
                }
            }
        }
        let library = hfst::tools::compose_intersect::ComposeIntersectOptions {
            invert: options.invert,
            fast: options.fast,
            encode_weights: options.encode_weights,
            harmonize: options.harmonize,
        };
        let names = |lexicon: &str| -> String {
            if lexicon.is_empty() {
                options.lexicon_name.clone()
            } else {
                lexicon.to_string()
            }
        };
        algebra!(self, "compose-intersect", lexicon => {
            let mut typed = Vec::with_capacity(rules.len());
            for rule in rules {
                let mut rule = rule.inner.copy()?.into_typed()?;
                hfst::tools::compose_intersect::prepare_rule(&mut rule, &library)?;
                typed.push(rule);
            }
            let lexicon_name = names(&lexicon.get_name());
            Ok(hfst::tools::compose_intersect::compose_intersect(
                lexicon,
                &mut typed,
                &library,
                (&lexicon_name, &options.rules_name),
                None,
                &mut |w| match w {
                    ComposeIntersectWarning::FilteredOutputSymbol(symbol) => warn(&format!(
                        "Found output symbols (e.g. \"{}\") in the lexicon which will be filtered out because they are not found on the input tapes of the rules.",
                        symbol
                    )),
                    ComposeIntersectWarning::MultiCharOutputSymbol(symbol) => warn(&format!(
                        "Found output multi-char symbols (\"{}\") in the lexicon which are not found on the input tapes of the rules.",
                        symbol
                    )),
                },
            )?)
        })
    }
}

fn library_binary(options: &BinaryOptions) -> lib::BinaryOptions {
    lib::BinaryOptions {
        harmonize: !options.do_not_harmonize,
        harmonize_flags: options.harmonize_flags,
    }
}

fn substitute_by_transducer<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    from: &str,
    replacement: &HfstTransducer<B>,
    name: &str,
) -> Result<(), Error> {
    let substitution = Substitution::Transducer {
        from,
        to: HfstTransducer::new_copy(replacement)?,
        file_name: name,
    };
    Ok(hfst::tools::substitute::substitute(
        t,
        substitution,
        &Silent,
    )?)
}
