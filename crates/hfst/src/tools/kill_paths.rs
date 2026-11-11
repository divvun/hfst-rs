//! `hfst kill-paths`: removing every arc that carries a given symbol, for one
//! symbol or for each a TSV file lists.

use crate::backend::AlgebraBackend;
use crate::hfst_transducer::HfstTransducer;
use crate::tools::metadata::{hfst_set_formula_unary, hfst_set_name_unary};

// [spec:hfst:req:embed.tools-in-library]
/// The symbols a kill-paths TSV file lists: each line up to its newline,
/// skipping empty lines and `#` comments.
pub fn kill_symbols(tsv: &str) -> Vec<String> {
    tsv.split_inclusive('\n')
        .filter(|line| !line.starts_with('\n') && !line.starts_with('#'))
        .map(|line| line.split('\n').next().unwrap_or_default().to_string())
        .collect()
}

// [spec:hfst:def:hfst-kill-paths.process-stream-fn]
// [spec:hfst:sem:hfst-kill-paths.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
/// `t` with every arc that carries any of `symbols` removed, named
/// `pathkill(...)`, and its epsilons removed, as `hfst kill-paths` writes it.
/// `each` hears each symbol before its arcs go.
pub fn kill_paths<'s, B: AlgebraBackend>(
    mut t: HfstTransducer<B>,
    symbols: impl IntoIterator<Item = &'s str>,
    each: &mut dyn FnMut(&str),
) -> crate::error::Result<HfstTransducer<B>> {
    for symbol in symbols {
        each(symbol);
        t = t.kill_paths(symbol);
    }
    let unnamed = t.clone();
    hfst_set_name_unary(&mut t, &unnamed, "pathkill");
    hfst_set_formula_unary(&mut t, &unnamed, "PK");
    t.remove_epsilons()?;
    Ok(t)
}
