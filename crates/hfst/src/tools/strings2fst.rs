//! `hfst strings2fst`: compiling strings and pair strings, one per line with
//! an optional tab-separated weight, into transducers.

use std::cell::Cell;

use crate::backend::AlgebraBackend;
use crate::error::ErrorKind;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::Symbol;
use crate::hfst_strings2_fst_tokenizer::{HfstStrings2FstTokenizer, StringPairVector};
use crate::hfst_transducer::HfstTransducer;
use crate::tools::format::parse_weight;
use crate::tools::metadata::hfst_set_name;

/// The base of the negative logarithm taken of each weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogBase {
    E,
    Ten,
}

/// What `hfst strings2fst`'s options decide.
#[derive(Clone, Debug)]
pub struct StringsOptions {
    pub epsilon: String,
    /// The strings tokenized as one symbol.
    pub multichar_symbols: Vec<String>,
    /// Lines are pair strings (`a:b c`) rather than string pairs (`ac:bc`).
    pub pairstrings: bool,
    /// Symbols are separated by spaces.
    pub has_spaces: bool,
    /// One transducer for all the lines rather than one for each.
    pub disjunct: bool,
    /// Divide each weight by the sum of all weights; only with `disjunct`.
    pub normalize: bool,
    /// Take the negative logarithm of each weight.
    pub log: Option<LogBase>,
    /// A negative weight is reported.
    pub warn_negative_weights: bool,
    /// A reported negative weight fails the compile.
    pub warnings_are_errors: bool,
}

impl Default for StringsOptions {
    fn default() -> Self {
        Self {
            epsilon: "@0@".to_string(),
            multichar_symbols: Vec::new(),
            pairstrings: false,
            has_spaces: false,
            disjunct: false,
            normalize: false,
            log: None,
            warn_negative_weights: true,
            warnings_are_errors: false,
        }
    }
}

/// Why a line did not compile. Each `Display`s as the message the tool
/// prints for it.
#[derive(Clone, Debug)]
pub enum StringsError {
    /// The weight after the tab is not a number; carries the text read.
    NotAWeight(String),
    /// A negative weight, with warnings errors.
    NegativeWeight(f32),
    /// A `:` that is not a pair separator.
    UnescapedColons { string: String, pairstrings: bool },
    /// The string is not UTF-8.
    NotUtf8(String),
    /// The negative logarithm of a weight is not a number.
    Logarithm,
    /// Building the transducer failed.
    Build(crate::error::Error),
}

impl std::fmt::Display for StringsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StringsError::NotAWeight(text) => write!(f, "{} not a weight", text),
            StringsError::NegativeWeight(weight) => write!(f, "{}", negative_weight(*weight)),
            StringsError::UnescapedColons {
                string,
                pairstrings: true,
            } => write!(
                f,
                "String `{}' contains unescaped ':'-symbols,\nwhich are not pair separators. Use `\\:' for literal `:'.",
                string
            ),
            StringsError::UnescapedColons {
                string,
                pairstrings: false,
            } => write!(
                f,
                "String `{}' contains unescaped `:'-symbols,\nwhich are not pair separators. Use `\\:' for literal `:'.\nIf you are compiling pair strings, use option -p.",
                string
            ),
            StringsError::NotUtf8(string) => {
                write!(f, "Input string `{}' is not valid utf-8.", string)
            }
            StringsError::Logarithm => write!(f, "unable to take negative logarithm"),
            StringsError::Build(e) => write!(f, "{}", e),
        }
    }
}

impl From<crate::error::Error> for StringsError {
    fn from(e: crate::error::Error) -> Self {
        StringsError::Build(e)
    }
}

/// The report on a negative weight.
pub fn negative_weight(weight: f32) -> String {
    format!(
        "Found negative weight {:.6}; negative weights are supported but iffy, if you really need them use -Wno-negative-weights",
        weight as f64
    )
}

/// One line compiled.
pub struct CompiledLine<B: AlgebraBackend> {
    /// The line's own transducer, named `string()`; `None` when the lines
    /// are disjuncted.
    pub transducer: Option<HfstTransducer<B>>,
    /// The weight the line gave.
    pub weight: Option<f32>,
}

// [spec:hfst:def:hfst-strings2fst.take-negative-logarithm-e-fn]
// [spec:hfst:sem:hfst-strings2fst.take-negative-logarithm-e-fn]
// [spec:hfst:def:hfst-strings2fst.take-negative-logarithm-10-fn]
// [spec:hfst:sem:hfst-strings2fst.take-negative-logarithm-10-fn]
/// The negative logarithm of `weight`, 0 for 0 (it should be infinity, which
/// transitions do not hold), `None` when it is not a number.
fn negative_logarithm(base: LogBase, weight: f32) -> Option<f32> {
    if weight == 0.0 {
        return Some(0.0);
    }
    let result = match base {
        LogBase::E => -weight.ln(),
        LogBase::Ten => -weight.log10(),
    };
    result.is_finite().then_some(result)
}

// [spec:hfst:def:hfst-strings2fst.divide-by-sum-of-weights-fn]
// [spec:hfst:sem:hfst-strings2fst.divide-by-sum-of-weights-fn]
fn divide_by_sum_of_weights(sum_of_weights: f32, weight: f32) -> f32 {
    if sum_of_weights == 0.0 {
        return 0.0;
    }
    weight / sum_of_weights
}

/// The text of `line` before its first newline or carriage return, and the
/// weight text after its first tab, if it has one.
fn split_line(line: &str) -> (&str, Option<&str>) {
    let end_of = |s: &str| s.find(['\n', '\r']).unwrap_or(s.len());
    match line.find('\t') {
        Some(tab) => {
            let rest = &line[tab + 1..];
            (&line[..tab], Some(&rest[..end_of(rest)]))
        }
        None => (&line[..end_of(line)], None),
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// The strings a multichar symbol file lists, one per non-empty line.
pub fn multichar_symbols(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Compiles lines as `hfst strings2fst` does, one at a time, so a caller can
/// write each line's transducer before reading the next.
pub struct StringsCompiler<B: AlgebraBackend> {
    options: StringsOptions,
    tokenizer: HfstStrings2FstTokenizer,
    disjunction: HfstBasicTransducer,
    sum_of_weights: f32,
    _backend: std::marker::PhantomData<B>,
}

impl<B: AlgebraBackend> StringsCompiler<B> {
    // [spec:hfst:req:embed.tools-in-library]
    pub fn new(options: StringsOptions) -> crate::error::Result<Self> {
        let symbols: Vec<Symbol> = options.multichar_symbols.iter().map(Symbol::new).collect();
        let tokenizer = HfstStrings2FstTokenizer::new(&symbols, &options.epsilon)?;
        Ok(Self {
            options,
            tokenizer,
            disjunction: HfstBasicTransducer::new(),
            sum_of_weights: 0.0,
            _backend: std::marker::PhantomData,
        })
    }

    /// The weight `weight_text` gives; a negative one goes to `warn`, or
    /// fails when warnings are errors.
    fn line_weight(
        &self,
        weight_text: &str,
        warn: &mut dyn FnMut(f32),
    ) -> Result<f32, StringsError> {
        let weight =
            parse_weight(weight_text).map_err(|text| StringsError::NotAWeight(text.to_string()))?;
        if weight < 0.0 && self.options.warn_negative_weights {
            if self.options.warnings_are_errors {
                return Err(StringsError::NegativeWeight(weight));
            }
            warn(weight);
        }
        Ok(weight)
    }

    fn tokenize(&self, string: &str) -> Result<StringPairVector, StringsError> {
        let tokenized = if self.options.pairstrings {
            self.tokenizer
                .tokenize_pair_string(string, self.options.has_spaces)
        } else {
            self.tokenizer
                .tokenize_string_pair(string, self.options.has_spaces)
        };
        tokenized.map_err(|e| {
            if e.kind == ErrorKind::UnescapedColsFound {
                StringsError::UnescapedColons {
                    string: string.to_string(),
                    pairstrings: self.options.pairstrings,
                }
            } else {
                StringsError::NotUtf8(string.to_string())
            }
        })
    }

    // [spec:hfst:def:hfst-strings2fst.process-stream-fn]
    // [spec:hfst:sem:hfst-strings2fst.process-stream-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Compile `line`, newline and all. A negative weight it gives goes to
    /// `warn` as soon as it is read, before anything else on the line can
    /// fail.
    pub fn compile_line(
        &mut self,
        line: &str,
        warn: &mut dyn FnMut(f32),
    ) -> Result<CompiledLine<B>, StringsError> {
        let (string, weight_text) = split_line(line);
        let weight = weight_text
            .map(|text| self.line_weight(text, warn))
            .transpose()?;
        let pairs = self.tokenize(string)?;
        let mut path_weight = weight.unwrap_or(0.0);
        self.sum_of_weights += path_weight;
        if self.options.disjunct {
            self.disjunction.disjunct_path(&pairs, path_weight);
            return Ok(CompiledLine {
                transducer: None,
                weight,
            });
        }
        if let Some(base) = self.options.log {
            path_weight =
                negative_logarithm(base, weight.unwrap_or(0.0)).ok_or(StringsError::Logarithm)?;
        }
        let mut path = HfstBasicTransducer::new();
        path.disjunct_path(&pairs, path_weight);
        let mut transducer = HfstTransducer::new_from_basic(&path)?;
        hfst_set_name(&mut transducer, "", "string");
        Ok(CompiledLine {
            transducer: Some(transducer),
            weight,
        })
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// The disjunction of every line compiled, named `strings(?)`, with
    /// weights normalized and taken the logarithm of as the options say; or
    /// `None` when the lines are not disjuncted.
    pub fn finish(self) -> Result<Option<HfstTransducer<B>>, StringsError> {
        if !self.options.disjunct {
            return Ok(None);
        }
        let mut disjunction = self.disjunction;
        if self.options.normalize {
            let sum = self.sum_of_weights;
            disjunction = disjunction.transform_weights(|w, _, _| divide_by_sum_of_weights(sum, w));
        }
        if let Some(base) = self.options.log {
            let failed = Cell::new(false);
            disjunction = disjunction.transform_weights(|w, _, _| {
                negative_logarithm(base, w).unwrap_or_else(|| {
                    failed.set(true);
                    w
                })
            });
            if failed.get() {
                return Err(StringsError::Logarithm);
            }
        }
        let mut transducer = HfstTransducer::new_from_basic(&disjunction)?;
        hfst_set_name(&mut transducer, "?", "strings");
        Ok(Some(transducer))
    }
}

/// What [`compile_strings`] compiles: the transducers, and each negative
/// weight reported with its line number.
pub struct CompiledStrings<B: AlgebraBackend> {
    pub transducers: Vec<HfstTransducer<B>>,
    pub negative_weights: Vec<(usize, f32)>,
}

/// A line that did not compile, and its number; 0 for a failure before the
/// first line.
#[derive(Clone, Debug)]
pub struct LineError {
    pub line: usize,
    pub error: StringsError,
}

// [spec:hfst:req:embed.tools-in-library]
/// Every transducer `text` compiles to, its lines compiled as by
/// [`StringsCompiler`]: one per line, or their disjunction.
pub fn compile_strings<B: AlgebraBackend>(
    text: &str,
    options: StringsOptions,
) -> Result<CompiledStrings<B>, LineError> {
    let at = |line: usize| move |error| LineError { line, error };
    let mut compiler = StringsCompiler::<B>::new(options).map_err(|e| at(0)(e.into()))?;
    let mut compiled = CompiledStrings {
        transducers: Vec::new(),
        negative_weights: Vec::new(),
    };
    let mut lines = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        lines = index + 1;
        let negative = &mut compiled.negative_weights;
        let one = compiler
            .compile_line(line, &mut |w| negative.push((lines, w)))
            .map_err(at(lines))?;
        compiled.transducers.extend(one.transducer);
    }
    compiled
        .transducers
        .extend(compiler.finish().map_err(at(lines))?);
    Ok(compiled)
}
