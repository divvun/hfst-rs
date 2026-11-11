//! `hfst compose-intersect`: a lexicon composed with the intersection of rule
//! transducers, without building the intersection.

use crate::backend::AlgebraBackend;
use crate::hfst_symbol_defs::internal_identity;
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::{EngineConfig, HfstTransducer};
use crate::tools::metadata::hfst_set_formula_unary;

type Result<T> = crate::error::Result<T>;

/// What `hfst compose-intersect`'s options decide.
#[derive(Clone, Copy, Debug, Default)]
pub struct ComposeIntersectOptions {
    /// Compose the intersection of the rules with the lexicon, rather than
    /// the lexicon with the intersection: `--invert`.
    pub invert: bool,
    /// Compose-intersect the lexicon's output side first and compose the
    /// lexicon with that: `--fast`.
    pub fast: bool,
    pub encode_weights: bool,
    /// Harmonize each rule with the lexicon: `--harmonize`.
    pub harmonize: bool,
}

/// What the tool warns of about a lexicon and its first rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeIntersectWarning {
    /// An output symbol of the lexicon that no rule reads, so its paths are
    /// filtered out.
    FilteredOutputSymbol(String),
    /// A multicharacter output symbol of the lexicon that no rule reads.
    MultiCharOutputSymbol(String),
}

// [spec:hfst:def:hfst-compose-intersect.is-special-symbol-fn]
// [spec:hfst:sem:hfst-compose-intersect.is-special-symbol-fn]
fn is_special_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    symbol.len() > 2 && bytes[0] == b'@' && bytes[symbol.len() - 1] == b'@'
}

// [spec:hfst:def:hfst-compose-intersect.check-all-symbols-fn]
// [spec:hfst:sem:hfst-compose-intersect.check-all-symbols-fn]
/// The first output symbol of `lexicon` that `rule` does not read, unless
/// the rule reads the identity symbol.
fn check_all_symbols<B: AlgebraBackend>(
    lexicon: &HfstTransducer<B>,
    rule: &HfstTransducer<B>,
) -> Result<Option<String>> {
    let rule_input_symbols = rule.to_basic()?.input_symbols_used();
    if rule_input_symbols.contains(internal_identity) {
        return Ok(None);
    }
    let lexicon_b = lexicon.to_basic()?;
    for s in 0..=lexicon_b.get_max_state() {
        for it in lexicon_b.index(s)?.iter() {
            let output_symbol = it.get_output_symbol(lexicon_b.coder());
            if !rule_input_symbols.contains(&output_symbol) {
                return Ok(Some(output_symbol.to_string()));
            }
        }
    }
    Ok(None)
}

// [spec:hfst:def:hfst-compose-intersect.check-multi-char-symbols-fn]
// [spec:hfst:sem:hfst-compose-intersect.check-multi-char-symbols-fn]
/// The first multicharacter output symbol of `lexicon`, special symbols
/// aside, that `rule` does not read.
fn check_multi_char_symbols<B: AlgebraBackend>(
    lexicon: &HfstTransducer<B>,
    rule: &HfstTransducer<B>,
) -> Result<Option<String>> {
    let lexicon_b = lexicon.to_basic()?;
    let rule_input_symbols = rule.to_basic()?.input_symbols_used();
    let tokenizer = HfstTokenizer::new();
    for s in 0..=lexicon_b.get_max_state() {
        for it in lexicon_b.index(s)?.iter() {
            let output_symbol = it.get_output_symbol(lexicon_b.coder());
            if !rule_input_symbols.contains(&output_symbol)
                && !is_special_symbol(&output_symbol)
                && tokenizer.tokenize_one_level(&output_symbol, false).len() > 1
            {
                return Ok(Some(output_symbol.to_string()));
            }
        }
    }
    Ok(None)
}

// [spec:hfst:req:embed.tools-in-library]
/// A rule as the tool reads it: minimized.
pub fn prepare_rule<B: AlgebraBackend>(
    rule: &mut HfstTransducer<B>,
    options: &ComposeIntersectOptions,
) -> Result<()> {
    rule.minimize_with_config(&EngineConfig {
        encode_weights: options.encode_weights,
        ..EngineConfig::default()
    })?;
    Ok(())
}

// [spec:hfst:def:hfst-compose-intersect.compose-streams-fn]
// [spec:hfst:sem:hfst-compose-intersect.compose-streams-fn]
// [spec:hfst:req:embed.tools-in-library]
/// Compose `lexicon` with the intersection of `rules`, each prepared by
/// [`prepare_rule`], and name the result after `lexicon_name` and
/// `rules_name`, as the tool names it after the lexicon (or its file) and the
/// rules' file. `warn` hears what the lexicon's output symbols suggest about
/// the first rule. With `harmonize`, the rules are harmonized with the
/// lexicon, which changes them. `memory_limit` bounds the working memory of
/// the backends that can spill; it never changes the result.
pub fn compose_intersect<B: AlgebraBackend>(
    lexicon: &mut HfstTransducer<B>,
    rules: &mut [HfstTransducer<B>],
    options: &ComposeIntersectOptions,
    (lexicon_name, rules_name): (&str, &str),
    memory_limit: Option<u64>,
    warn: &mut dyn FnMut(ComposeIntersectWarning),
) -> Result<()> {
    let config = EngineConfig {
        encode_weights: options.encode_weights,
        compose_memory_limit_bytes: memory_limit,
        ..EngineConfig::default()
    };
    if let Some(rule) = rules.first() {
        match check_all_symbols(lexicon, rule)? {
            Some(symbol) => warn(ComposeIntersectWarning::FilteredOutputSymbol(symbol)),
            None => {
                if let Some(symbol) = check_multi_char_symbols(lexicon, rule)? {
                    warn(ComposeIntersectWarning::MultiCharOutputSymbol(symbol));
                }
            }
        }
    }
    if options.harmonize {
        for rule in rules.iter_mut() {
            rule.harmonize(lexicon, false)?;
        }
    }
    let rules: &[HfstTransducer<B>] = rules;
    if options.fast {
        // Compose-intersect one side of the lexicon with the rules, then
        // compose the lexicon with that.
        if options.invert {
            let mut lexicon_input = HfstTransducer::new_copy(lexicon)?;
            lexicon_input.input_project()?;
            lexicon_input.minimize()?;
            lexicon_input.compose_intersect_with_config(rules, true, true, &config)?;
            lexicon_input.compose_with_config(lexicon, true, &config)?;
            *lexicon = lexicon_input;
        } else {
            let mut lexicon_output = HfstTransducer::new_copy(lexicon)?;
            lexicon_output.output_project()?;
            lexicon_output.minimize()?;
            lexicon_output.compose_intersect_with_config(rules, false, true, &config)?;
            lexicon.compose_with_config(&lexicon_output, true, &config)?;
        }
    } else {
        lexicon.compose_intersect_with_config(rules, options.invert, true, &config)?;
    }
    lexicon.set_name(&format!(
        "compose({}, intersect({}))",
        lexicon_name, rules_name
    ));
    hfst_set_formula_unary(lexicon, " \u{2218} \u{22c2}R");
    Ok(())
}
