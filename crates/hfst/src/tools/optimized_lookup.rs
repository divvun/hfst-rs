//! `hfst optimized-lookup`: looking words up in an optimized-lookup stream,
//! the union of its transducers, with the analyses the tool prints for each.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use crate::hfst_data_types::{HfstOneLevelPaths, ImplementationType};
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_transducer::{AnyTransducer, HfstTransducer};
use crate::transducer::{Transducer, UnweightedTables, WeightedTables};

/// What `hfst optimized-lookup`'s options decide about the analyses.
#[derive(Clone, Copy, Debug)]
pub struct OptimizedLookupOptions {
    /// Each analysis once: `--unique`.
    pub unique: bool,
    /// At most this many analyses a word: `--analyses`.
    pub max_analyses: i32,
    /// Seconds of search a word, 0 for no limit: `--time-cutoff`.
    pub time_cutoff: f64,
    /// Only analyses within this much of the lightest, negative for all:
    /// `--beam`. Weighted transducers only.
    pub beam: f32,
    /// Every analysis as found, without limit or ordering: `--fast`.
    /// Unweighted, not unique transducers only.
    pub fast: bool,
}

impl Default for OptimizedLookupOptions {
    fn default() -> Self {
        OptimizedLookupOptions {
            unique: false,
            max_analyses: i32::MAX,
            time_cutoff: 0.0,
            beam: -1.0,
            fast: false,
        }
    }
}

/// What the tool finds for a word.
#[derive(Clone, Debug, PartialEq)]
pub enum OptimizedLookupOutcome {
    /// The word has a symbol no transducer knows, so it was not looked up.
    Untokenizable,
    /// The analyses the tool prints, each its output and weight, in the
    /// order it prints them.
    Analyses(Vec<(String, f32)>),
}

enum Member<'a> {
    W(Cow<'a, HfstTransducer<Transducer<WeightedTables>>>),
    U(Cow<'a, HfstTransducer<Transducer<UnweightedTables>>>),
}

impl Member<'_> {
    fn lookup(&self, s: &str, time_cutoff: f64) -> crate::error::Result<HfstOneLevelPaths> {
        match self {
            Member::W(t) => t.lookup_fd_string(s, -1, time_cutoff),
            Member::U(t) => t.lookup_fd_string(s, -1, time_cutoff),
        }
    }

    fn can_tokenize(&self, s: &str) -> bool {
        match self {
            Member::W(t) => t.can_tokenize(s),
            Member::U(t) => t.can_tokenize(s),
        }
    }
}

/// The transducers of a stream, looked up in as their union
/// [upstream hfst/hfst#395].
#[derive(Default)]
pub struct OptimizedLookup<'a> {
    members: Vec<Member<'a>>,
    /// Whether the first transducer is weighted, which decides how the tool
    /// orders and prints analyses.
    weighted: Option<bool>,
}

impl<'a> OptimizedLookup<'a> {
    fn note_type(&mut self, ty: ImplementationType) {
        self.weighted.get_or_insert(
            ty == ImplementationType::HFST_OLW_TYPE || ty == ImplementationType::THFST_TYPE,
        );
    }

    // [spec:hfst:def:hfst-optimized-lookup.setup-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.setup-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Add `t` to the union: an optimized-lookup table as it is, anything
    /// else converted to weighted optimized lookup.
    pub fn push(&mut self, t: AnyTransducer) -> crate::error::Result<()> {
        self.note_type(t.get_type());
        let member = match t {
            AnyTransducer::OlW(t) => Member::W(Cow::Owned(t)),
            AnyTransducer::OlU(t) => Member::U(Cow::Owned(t)),
            AnyTransducer::Thfst(t) => Member::W(Cow::Owned(t.into_olw())),
            other @ AnyTransducer::Tropical(_) => Member::W(Cow::Owned(other.into_typed()?)),
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => Member::W(Cow::Owned(other.into_typed()?)),
        };
        self.members.push(member);
        Ok(())
    }

    /// [`Self::push`] for a transducer the caller keeps: an optimized-lookup
    /// table is looked up in where it is.
    pub fn push_borrowed(&mut self, t: &'a AnyTransducer) -> crate::error::Result<()> {
        self.note_type(t.get_type());
        let member = match t {
            AnyTransducer::OlW(t) => Member::W(Cow::Borrowed(t)),
            AnyTransducer::OlU(t) => Member::U(Cow::Borrowed(t)),
            other @ AnyTransducer::Thfst(_) | other @ AnyTransducer::Tropical(_) => {
                Member::W(Cow::Owned(other.copy()?.into_typed()?))
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => Member::W(Cow::Owned(other.copy()?.into_typed()?)),
        };
        self.members.push(member);
        Ok(())
    }

    pub fn weighted(&self) -> bool {
        self.weighted.unwrap_or(false)
    }

    // [spec:hfst:def:hfst-optimized-lookup.run-transducer-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.run-transducer-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// What the tool finds for `input` and prints. A transducer whose lookup
    /// fails is passed over, and `errors` hears why.
    pub fn look_up(
        &self,
        input: &str,
        options: &OptimizedLookupOptions,
        errors: &mut dyn FnMut(&crate::error::Error),
    ) -> OptimizedLookupOutcome {
        if !self.members.iter().any(|m| m.can_tokenize(input)) {
            return OptimizedLookupOutcome::Untokenizable;
        }
        let mut found: Vec<(String, f32)> = Vec::new();
        for member in &self.members {
            match member.lookup(input, options.time_cutoff) {
                Ok(paths) => {
                    for path in &paths {
                        // The tool's alphabet reader blanked flag symbols, so
                        // they never reach its output.
                        let output: String = path
                            .second
                            .iter()
                            .filter(|s| !FdOperation::is_diacritic(s))
                            .map(|s| s.as_str())
                            .collect();
                        found.push((output, path.first));
                    }
                }
                Err(e) => errors(&e),
            }
        }
        OptimizedLookupOutcome::Analyses(self.select(found, options))
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
    /// The analyses found, in the order the tool prints them, as many as it
    /// prints, for the variant the weightedness and `unique` select. The
    /// C++ tool's display containers are what each arm builds: a vector
    /// (plain), a set (unique), a multimap by weight (weighted) and a map by
    /// output (weighted unique), weights being `f32`.
    // [spec:hfst:def:hfst-optimized-lookup.weight]
    // [spec:hfst:def:hfst-optimized-lookup.display-vector]
    // [spec:hfst:def:hfst-optimized-lookup.display-set]
    // [spec:hfst:def:hfst-optimized-lookup.display-multi-map]
    // [spec:hfst:def:hfst-optimized-lookup.display-map]
    fn select(
        &self,
        found: Vec<(String, f32)>,
        options: &OptimizedLookupOptions,
    ) -> Vec<(String, f32)> {
        let max = options.max_analyses.max(0) as usize;
        let beam = options.beam;
        match (self.weighted(), options.unique) {
            (false, false) if options.fast => found,
            (false, false) => found.into_iter().take(max).collect(),
            (false, true) => {
                let unique: BTreeSet<String> =
                    found.into_iter().map(|(output, _)| output).collect();
                unique
                    .into_iter()
                    .take(max)
                    .map(|output| (output, 0.0))
                    .collect()
            }
            (true, false) => {
                let mut sorted = found;
                sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                let lowest = sorted.first().map_or(-1.0, |first| first.1);
                sorted
                    .into_iter()
                    .take(max)
                    .filter(|(_, weight)| beam < 0.0 || *weight <= lowest + beam)
                    .collect()
            }
            (true, true) => {
                // The first weight an output is found with is kept, as the C++
                // map's insert keeps it.
                let mut by_output: BTreeMap<String, f32> = BTreeMap::new();
                for (output, weight) in found {
                    by_output.entry(output).or_insert(weight);
                }
                // The beam is measured from the first output in key order.
                let lowest = by_output.values().next().copied().unwrap_or(-1.0);
                let mut sorted: Vec<(String, f32)> = by_output
                    .into_iter()
                    .filter(|(_, weight)| beam < 0.0 || *weight <= lowest + beam)
                    .collect();
                sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                sorted.into_iter().take(max).collect()
            }
        }
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// What the tool prints for `input` and its `outcome`: Xerox lines, with
    /// weights when `display_weights` and the transducers are weighted.
    pub fn write(
        &self,
        out: &mut dyn Write,
        input: &str,
        outcome: &OptimizedLookupOutcome,
        options: &OptimizedLookupOptions,
        display_weights: bool,
    ) -> std::io::Result<()> {
        // What follows a word with no analysis: an unweighted transducer's
        // record ends in three blank lines and a weighted one's in one, as
        // the C++ tool's preprocessor brackets leave them on POSIX.
        let none = |out: &mut dyn Write, weighted: bool| {
            if weighted {
                write!(out, "{}\t{}\t+?\n\n", input, input)
            } else {
                write!(out, "{}\t{}\t+?\n\n\n\n", input, input)
            }
        };
        let analyses = match outcome {
            OptimizedLookupOutcome::Untokenizable => return none(out, false),
            OptimizedLookupOutcome::Analyses(analyses) => analyses,
        };
        let weighted = self.weighted();
        if !weighted && !options.unique && options.fast {
            for (output, _) in analyses {
                writeln!(out, "{}", output)?;
            }
            return Ok(());
        }
        if analyses.is_empty() {
            return none(out, weighted);
        }
        for (output, weight) in analyses {
            write!(out, "{}\t{}", input, output)?;
            if weighted && display_weights {
                write!(out, "\t{}", format_weight(*weight))?;
            }
            out.write_all(b"\n")?;
        }
        out.write_all(b"\n")
    }
}

/// A weight as C++ streams a float: `%g` at six significant digits.
fn format_weight(w: f32) -> String {
    if w.is_nan() {
        return "nan".to_string();
    }
    if w.is_infinite() {
        return if w < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    // The exponent after rounding to six digits picks the notation, so
    // 999999.9 shows as 1e+06.
    let sci = format!("{:.5e}", w);
    let (mantissa, exp) = match sci.split_once('e') {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (sci.as_str(), 0),
    };
    if (-4..6).contains(&exp) {
        strip_trailing_zeros(&format!("{:.*}", (5 - exp) as usize, w))
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        format!(
            "{}e{}{:02}",
            strip_trailing_zeros(mantissa),
            sign,
            exp.abs()
        )
    }
}

fn strip_trailing_zeros(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    let trimmed = s.trim_end_matches('0');
    trimmed.strip_suffix('.').unwrap_or(trimmed).to_string()
}

#[cfg(test)]
mod tests {
    use super::format_weight as fmt_weight;

    /// Expectations taken from the C++ tool's own output, not from printf docs:
    /// `hfst-optimized-lookup -w` on a 0.5-weighted arc prints `0.5`.
    #[test]
    fn weights_render_as_ostream_g() {
        assert_eq!(fmt_weight(0.5), "0.5");
        assert_eq!(fmt_weight(2.5), "2.5");
        assert_eq!(fmt_weight(0.0), "0");
        assert_eq!(fmt_weight(-0.5), "-0.5");
        assert_eq!(fmt_weight(1.0), "1");
    }

    #[test]
    fn six_significant_digits_then_strip() {
        assert_eq!(fmt_weight(1.2345678), "1.23457");
        assert_eq!(fmt_weight(100000.0), "100000");
        assert_eq!(fmt_weight(0.0001), "0.0001");
    }

    #[test]
    fn out_of_range_exponents_go_scientific() {
        assert_eq!(fmt_weight(1000000.0), "1e+06");
        assert_eq!(fmt_weight(1234567.0), "1.23457e+06");
        assert_eq!(fmt_weight(0.00001), "1e-05");
    }
}
