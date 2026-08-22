//! Symbol-table helpers shared by the native Foma backend operations.

use crate::hfst_tropical_transducer_transition_data::SymbolType;

use foma::types::{Fsm, Sigma};
use std::collections::BTreeSet;

/// The HFST special-symbol strings for Foma's three reserved sigma numbers.
const EPSILON_SYMBOL: &str = "@_EPSILON_SYMBOL_@";
const UNKNOWN_SYMBOL: &str = "@_UNKNOWN_SYMBOL_@";
pub(crate) const IDENTITY_SYMBOL: &str = "@_IDENTITY_SYMBOL_@";

/// Whether `symbol` is one of the three HFST special strings Foma represents
/// by a reserved sigma number rather than an ordinary alphabet entry.
pub(crate) fn is_reserved_symbol(symbol: &str) -> bool {
    symbol == EPSILON_SYMBOL || symbol == UNKNOWN_SYMBOL || symbol == IDENTITY_SYMBOL
}

/// Map a Foma sigma number to its HFST symbol string.
pub(crate) fn sym(n: i32, sigma: &[Sigma]) -> SymbolType {
    match n {
        foma::types::EPSILON => SymbolType::from(EPSILON_SYMBOL),
        foma::types::UNKNOWN => SymbolType::from(UNKNOWN_SYMBOL),
        foma::types::IDENTITY => SymbolType::from(IDENTITY_SYMBOL),
        _ => SymbolType::from(
            foma::sigma::sigma_string(n, sigma).expect("arc symbol number resolves in sigma"),
        ),
    }
}

/// Declare every ordinary `symbol` in `net` and restore the sorted Foma sigma.
///
/// The sieve avoids a quadratic per-symbol scan over large alphabets. Sorting
/// is semantic: Foma merges sigmas in string order and rewrites arc numbers
/// when that order changes.
pub(crate) fn sigma_declare<'a>(
    net: &mut foma::types::Fsm,
    symbols: impl IntoIterator<Item = &'a str>,
) {
    let missing: std::collections::BTreeSet<&str> = {
        let present: std::collections::BTreeSet<&str> =
            net.sigma.iter().map(|s| s.symbol.as_str()).collect();
        symbols
            .into_iter()
            .filter(|s| !is_reserved_symbol(s) && !present.contains(s))
            .collect()
    };
    if missing.is_empty() {
        return;
    }
    for symbol in missing {
        foma::sigma::sigma_add(symbol, &mut net.sigma);
    }
    foma::sigma::sigma_sort(net);
}

/// The label pairs on `net`'s arcs whose two sides differ — the arcs that make
/// it a transducer rather than an automaton, named by their HFST symbol
/// strings.
pub(crate) fn transducing_pairs(net: &Fsm) -> BTreeSet<(SymbolType, SymbolType)> {
    let mut pairs = BTreeSet::new();
    for line in net.states.rows().iter() {
        if line.state_no == -1 {
            break;
        }
        let (input, output) = (line.r#in as i32, line.out as i32);
        // A state with no outgoing arcs still occupies a marker row.
        if line.target == -1 || input < 0 || output < 0 || input == output {
            continue;
        }
        pairs.insert((sym(input, &net.sigma), sym(output, &net.sigma)));
    }
    pairs
}
