//! `hfst compare`: whether two transducers are equivalent, and the line the
//! tool prints for each pair.

use std::io::Write;

use crate::backend::AlgebraBackend;
use crate::hfst_transducer::HfstTransducer;
use crate::tools::operations::Operand;

/// What `hfst compare`'s options decide.
#[derive(Clone, Copy, Debug)]
pub struct CompareOptions {
    /// Harmonize the alphabets before comparing; `--do-not-harmonize` turns
    /// it off.
    pub harmonize: bool,
    /// Eliminate the flag diacritics of both first: `--eliminate-flags`.
    pub eliminate_flags: bool,
}

impl Default for CompareOptions {
    fn default() -> Self {
        CompareOptions {
            harmonize: true,
            eliminate_flags: false,
        }
    }
}

// [spec:hfst:def:hfst-compare.compare-streams-fn]
// [spec:hfst:sem:hfst-compare.compare-streams-fn]
// [spec:hfst:req:embed.tools-in-library]
/// Whether `first` and `second` are equivalent, as `hfst compare` decides.
/// Eliminating flags changes both, and `second` is copied first when it is
/// borrowed; `eliminating` hears when that starts.
pub fn compare<B: AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &mut Operand<'_, B>,
    options: &CompareOptions,
    eliminating: &mut dyn FnMut(),
) -> crate::error::Result<bool> {
    if options.eliminate_flags {
        eliminating();
        first.eliminate_flags()?;
        second.to_mut()?.eliminate_flags()?;
    }
    first.compare(second.get(), options.harmonize)
}

// [spec:hfst:req:embed.tools-in-library]
/// The line `hfst compare` prints for a pair: `A == B` or `A != B`, with
/// each transducer's number in its stream after the first pair.
pub fn write_comparison(
    out: &mut dyn Write,
    (first_name, second_name): (&str, &str),
    (first_n, second_n): (usize, usize),
    equal: bool,
) -> std::io::Result<()> {
    let relation = if equal { "==" } else { "!=" };
    if first_n == 1 {
        writeln!(out, "{} {} {}", first_name, relation, second_name)
    } else {
        writeln!(
            out,
            "{}[{}] {} {}[{}]",
            first_name, first_n, relation, second_name, second_n
        )
    }
}
