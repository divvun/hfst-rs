//! `hfst check-alpha`: how the alphabets of two transducers differ, as data
//! and as the report the tool prints.

use std::io::Write;

use crate::hfst_symbol_defs::StringSet;
use crate::hfst_transducer::AnyTransducer;

/// The alphabets of two transducers: the symbols their arcs use, and the
/// alphabets they declare.
pub struct AlphabetComparison {
    pub first_name: String,
    pub second_name: String,
    pub first_used: StringSet,
    pub second_used: StringSet,
    pub first_declared: StringSet,
    pub second_declared: StringSet,
}

// [spec:hfst:def:hfst-check-alpha.fprint-stringset-fn]
// [spec:hfst:sem:hfst-check-alpha.fprint-stringset-fn]
/// `symbols` separated by commas.
fn write_set(out: &mut dyn Write, symbols: &StringSet) -> std::io::Result<()> {
    for (i, s) in symbols.iter().enumerate() {
        if i > 0 {
            write!(out, ", ")?;
        }
        write!(out, "{}", s)?;
    }
    Ok(())
}

impl AlphabetComparison {
    // [spec:hfst:req:embed.tools-in-library]
    pub fn of(first: &AnyTransducer, second: &AnyTransducer) -> crate::error::Result<Self> {
        Ok(Self {
            first_name: first.get_name(),
            second_name: second.get_name(),
            first_used: first.to_basic()?.symbols_used(),
            second_used: second.to_basic()?.symbols_used(),
            first_declared: first.get_alphabet()?,
            second_declared: second.get_alphabet()?,
        })
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// Whether either transducer has a symbol, used or declared, the other
    /// lacks.
    pub fn mismatch(&self) -> bool {
        self.first_used != self.second_used || self.first_declared != self.second_declared
    }

    // [spec:hfst:def:hfst-check-alpha.process-stream-fn]
    // [spec:hfst:sem:hfst-check-alpha.process-stream-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Write the report `hfst check-alpha` prints for one pair, the
    /// alphabets themselves included when `verbose`. The second line names
    /// the second transducer twice, as the C++ tool did.
    pub fn write_report(&self, out: &mut dyn Write, verbose: bool) -> std::io::Result<()> {
        let (first, second) = (&self.first_name, &self.second_name);
        writeln!(out, "Actual alphabet differences:")?;
        write_difference(
            out,
            (&self.first_used, &self.second_used),
            format!("In first {first} but not in second {second}:"),
            format!("First {first} alpha is superset of second {second}."),
        )?;
        write_difference(
            out,
            (&self.second_used, &self.first_used),
            format!("In second {second} but not in first {second}:"),
            format!("Second {second} alpha is superset of second {second}."),
        )?;
        if verbose {
            write!(out, "{first} alphabet:")?;
            write_set(out, &self.first_used)?;
            writeln!(out)?;
            write!(out, "{second} alphabet:")?;
            write_set(out, &self.second_used)?;
            writeln!(out)?;
        }
        self.write_sigma_report(out, verbose)
    }

    fn write_sigma_report(&self, out: &mut dyn Write, verbose: bool) -> std::io::Result<()> {
        let (first, second) = (&self.first_name, &self.second_name);
        writeln!(out, "sigma set difference:")?;
        write_difference(
            out,
            (&self.first_declared, &self.second_declared),
            format!("First {first} has but second {second} does not: "),
            format!("First {first} alpha is superset of second {second}."),
        )?;
        write_difference(
            out,
            (&self.second_declared, &self.first_declared),
            format!("Second {second} has but first {first} does not: "),
            format!("Second {second} alpha is superset of first {first}."),
        )?;
        if verbose {
            write!(out, "First ({first}):")?;
            write_set(out, &self.first_declared)?;
            writeln!(out)?;
            write!(out, "Second ({second}):")?;
            write_set(out, &self.second_declared)?;
            writeln!(out)?;
        }
        Ok(())
    }
}

/// The line on what `a` has that `b` lacks: `listed` and the symbols, or
/// `superset` when there are none.
fn write_difference(
    out: &mut dyn Write,
    (a, b): (&StringSet, &StringSet),
    listed: String,
    superset: String,
) -> std::io::Result<()> {
    let only: StringSet = a.difference(b).cloned().collect();
    if only.is_empty() {
        write!(out, "{superset}")?;
    } else {
        write!(out, "{listed}")?;
        write_set(out, &only)?;
    }
    writeln!(out)
}
