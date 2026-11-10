//! The results of a lookup, as both packages return them.

use hfst::hfst_data_types::HfstOneLevelPaths;
use hfst::hfst_flag_diacritics::FdOperation;
use hfst::hfst_symbol_defs::is_epsilon;

// [spec:hfst:req:javascript.lookup]
// [spec:hfst:req:python.lookup]
/// One path a lookup found.
#[derive(Clone, Debug, PartialEq)]
pub struct LookupResult {
    /// The symbols, concatenated.
    pub output: String,
    /// The output symbols of the path, without its flag diacritics and
    /// epsilons, which `hfst lookup` does not print either.
    pub symbols: Vec<String>,
    pub weight: f32,
}

// [spec:hfst:req:javascript.lookup]
// [spec:hfst:req:python.lookup]
/// The results for the paths a lookup found: one for each path, ordered by
/// weight, then by output, then by symbols. Two paths that differ only in
/// their flags or epsilons stay two results, as `hfst lookup` prints both.
pub(crate) fn lookup_results(paths: HfstOneLevelPaths) -> Vec<LookupResult> {
    let mut results: Vec<LookupResult> = paths
        .into_iter()
        .map(|path| {
            let symbols: Vec<String> = path
                .second
                .iter()
                .filter(|symbol| {
                    !symbol.is_empty() && !is_epsilon(symbol) && !FdOperation::is_diacritic(symbol)
                })
                .map(|symbol| symbol.to_string())
                .collect();
            LookupResult {
                output: symbols.concat(),
                symbols,
                weight: path.first,
            }
        })
        .collect();
    results.sort_by(|a, b| {
        a.weight
            .total_cmp(&b.weight)
            .then_with(|| a.output.cmp(&b.output))
            .then_with(|| a.symbols.cmp(&b.symbols))
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::hfst_data_types::{HfstOneLevelPath, Symbol};

    fn path(weight: f32, symbols: &[&str]) -> HfstOneLevelPath {
        HfstOneLevelPath {
            first: weight,
            second: symbols.iter().map(|s| Symbol::from(*s)).collect(),
        }
    }

    fn summary(results: &[LookupResult]) -> Vec<String> {
        results
            .iter()
            .map(|r| format!("{} {} {}", r.weight, r.output, r.symbols.join(" ")))
            .collect()
    }

    #[test]
    fn flags_and_epsilons_leave_the_symbols() {
        let paths = [path(
            0.5,
            &[
                "@P.CASE.NOM@",
                "t",
                "",
                "@_EPSILON_SYMBOL_@",
                "o",
                "@R.CASE.NOM@",
                "+N",
            ],
        )]
        .into_iter()
        .collect();
        let results = lookup_results(paths);
        assert_eq!(summary(&results), ["0.5 to+N t o +N"]);
    }

    #[test]
    fn results_order_by_weight_output_then_symbols() {
        let paths = [
            path(1.0, &["xy"]),
            path(1.0, &["x", "y"]),
            path(1.0, &["b"]),
            path(0.25, &["c"]),
            path(0.25, &["a", "b"]),
            path(2.0, &["a"]),
        ]
        .into_iter()
        .collect();
        let results = lookup_results(paths);
        assert_eq!(
            summary(&results),
            [
                "0.25 ab a b",
                "0.25 c c",
                "1 b b",
                "1 xy x y",
                "1 xy xy",
                "2 a a"
            ]
        );
    }

    #[test]
    fn flag_only_differences_stay_two_results() {
        let paths = [path(0.0, &["@U.X.A@", "a"]), path(0.0, &["@U.X.B@", "a"])]
            .into_iter()
            .collect();
        assert_eq!(summary(&lookup_results(paths)), ["0 a a", "0 a a"]);
    }
}
