//! Product-pruned compose-intersect preparation.

use super::*;

pub(super) fn try_lookahead<B: AlgebraBackend>(
    lexicon: &HfstTransducer<B>,
    rule: &HfstTransducer<B>,
    rule_count: usize,
    invert: bool,
    config: &EngineConfig,
) -> crate::error::Result<Option<HfstTransducer<B>>> {
    if rule_count != 1
        || invert
        || !B::SUPPORTS_COMPOSE_LOOKAHEAD
        || config.flag_is_epsilon_in_composition
        || config.xerox_composition
    {
        return Ok(None);
    }

    let lexicon_alphabet = lexicon.get_alphabet()?;
    let rule_alphabet = rule.get_alphabet()?;
    let right_self_loops = lexicon_alphabet
        .iter()
        .filter(|symbol| FdOperation::is_diacritic(symbol) && !rule_alphabet.contains(*symbol))
        .cloned()
        .collect();
    let overlay = FlagDiacriticComposeOverlay {
        left_self_loops: StringSet::new(),
        right_self_loops,
        enforce_left_before_right: false,
    };

    // Preserve compose-intersect's special-symbol semantics while ordinary
    // harmonization expands only the rule's identity wildcard. The rule's
    // literal unknown arc is not a wildcard in this operation, and the
    // lexicon placeholders must be visible before identity expansion so the
    // resulting labels still match.
    let mut prepared_lexicon = lexicon.clone();
    prepared_lexicon.substitute_symbol(internal_identity, "||_IDENTITY_SYMBOL_||", true, true)?;
    prepared_lexicon.substitute_symbol(internal_unknown, "||_UNKNOWN_SYMBOL_||", true, true)?;
    let mut prepared_rule = rule.clone();
    prepared_rule.substitute_symbol(internal_unknown, "||_RULE_UNKNOWN_SYMBOL_||", true, true)?;
    prepared_lexicon = prepared_rule
        .harmonize_copy_owned(prepared_lexicon)?
        .expect("lookahead-capable backends harmonize through the interchange graph");

    prepared_lexicon.is_trie = false;
    let left = std::mem::replace(&mut prepared_lexicon.fst, B::empty());
    prepared_lexicon.fst = left.try_compose_lookahead_owned(
        prepared_rule.fst,
        Some(&overlay),
        config.compose_memory_limit_bytes,
    )?;
    prepared_lexicon.substitute_symbol("||_IDENTITY_SYMBOL_||", internal_identity, true, true)?;
    prepared_lexicon.substitute_symbol("||_UNKNOWN_SYMBOL_||", internal_unknown, true, true)?;
    prepared_lexicon.substitute_symbol(
        "||_RULE_UNKNOWN_SYMBOL_||",
        internal_unknown,
        true,
        true,
    )?;
    prepared_lexicon.prune_alphabet(true)?;
    Ok(Some(prepared_lexicon))
}
