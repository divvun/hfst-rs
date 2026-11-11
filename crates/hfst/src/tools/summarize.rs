//! `hfst summarize`: the properties of a transducer, as data and as the
//! report the tool prints.

use std::io::Write;

use crate::hfst_basic_transducer::SummaryStats;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_transducer::AnyTransducer;

/// What `hfst summarize` reports on one transducer.
pub struct Summary {
    pub name: String,
    pub implementation: ImplementationType,
    pub stats: SummaryStats,
    pub alphabet: StringSet,
    /// Measured only for a detailed summary.
    pub details: Option<SummaryDetails>,
}

/// What a detailed summary measures besides.
pub struct SummaryDetails {
    pub infinitely_ambiguous: bool,
    /// The symbols a path can start with; `None` for the optimized-lookup
    /// types, which have no graph to walk.
    pub first_input_symbols: Option<StringSet>,
}

/// Which parts of the report to write.
#[derive(Clone, Copy, Debug, Default)]
pub struct SummaryOptions {
    /// Write the detailed statistics; the summary has to be detailed.
    pub verbose: bool,
    /// Write the symbol pairs occurring at most this many times; -1 writes
    /// them all.
    pub symbol_pairs: Option<i32>,
}

fn first_input_symbols(t: &AnyTransducer) -> crate::error::Result<Option<StringSet>> {
    match t {
        AnyTransducer::Tropical(t) => t.get_first_input_symbols().map(Some),
        #[cfg(feature = "foma")]
        AnyTransducer::Foma(t) => t.get_first_input_symbols().map(Some),
        AnyTransducer::OlW(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_) => Ok(None),
    }
}

impl Summary {
    // [spec:hfst:def:hfst-summarize.process-stream-fn]
    // [spec:hfst:sem:hfst-summarize.process-stream-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Summarize `t`, in detail when `detailed`.
    pub fn of(t: &AnyTransducer, detailed: bool) -> crate::error::Result<Summary> {
        let first_input_symbols = if detailed {
            Some(first_input_symbols(t)?)
        } else {
            None
        };
        let basic = t.to_basic()?;
        let alphabet = t.get_alphabet()?;
        Ok(Summary {
            name: t.get_name(),
            implementation: t.get_type(),
            stats: basic.summarize(),
            alphabet,
            details: first_input_symbols.map(|first_input_symbols| SummaryDetails {
                infinitely_ambiguous: basic.is_infinitely_ambiguous(),
                first_input_symbols,
            }),
        })
    }

    /// Whether the type can be changed in place, and whether it is weighted.
    fn mutable_and_weighted(&self) -> (bool, bool) {
        match self.implementation {
            ImplementationType::SFST_TYPE | ImplementationType::FOMA_TYPE => (true, false),
            ImplementationType::TROPICAL_OPENFST_TYPE => (true, true),
            ImplementationType::HFST_OL_TYPE => (false, false),
            ImplementationType::HFST_OLW_TYPE | ImplementationType::THFST_TYPE => (false, true),
            ImplementationType::XFSM_TYPE
            | ImplementationType::HFST2_TYPE
            | ImplementationType::UNSPECIFIED_TYPE
            | ImplementationType::ERROR_TYPE => (false, true),
        }
    }

    /// The `fst type`/`arc type` lines, printed as OpenFst's fstinfo prints
    /// them.
    fn type_lines(&self) -> &'static str {
        match self.implementation {
            ImplementationType::SFST_TYPE => "fst type: SFST\narc type: SFST\n",
            ImplementationType::TROPICAL_OPENFST_TYPE => "fst type: OpenFST\narc type: tropical\n",
            ImplementationType::FOMA_TYPE => "fst type: foma\narc type: foma\n",
            ImplementationType::HFST_OL_TYPE => {
                "fst type: HFST optimized lookup\narc type: unweighted\n"
            }
            ImplementationType::HFST_OLW_TYPE | ImplementationType::THFST_TYPE => {
                "fst type: HFST optimized lookup\narc type: weighted\n"
            }
            ImplementationType::XFSM_TYPE
            | ImplementationType::HFST2_TYPE
            | ImplementationType::UNSPECIFIED_TYPE
            | ImplementationType::ERROR_TYPE => "fst type: ???\narc type: ???\n",
        }
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// Write the report `hfst summarize` prints for the `number`th transducer
    /// of a stream.
    pub fn write_report(
        &self,
        out: &mut dyn Write,
        number: usize,
        options: &SummaryOptions,
    ) -> std::io::Result<()> {
        if number > 1 {
            write!(out, "-- \nTransducer #{}:\n", number)?;
        }
        writeln!(out, "name: \"{}\"", self.name)?;
        write!(out, "{}", self.type_lines())?;
        self.write_counts(out)?;
        self.write_properties(out)?;
        if options.verbose
            && let Some(details) = &self.details
        {
            self.write_details(out, details)?;
        }
        if let Some(threshold) = options.symbol_pairs {
            self.write_symbol_pairs(out, threshold)?;
        }
        Ok(())
    }

    fn write_counts(&self, out: &mut dyn Write) -> std::io::Result<()> {
        let s = &self.stats;
        write!(
            out,
            "input symbol table: yes\n\
             output symbol table: yes\n\
             # of states: {}\n\
             # of arcs: {}\n\
             initial state: {}\n\
             # of final states: {}\n\
             # of input/output epsilons: {}\n\
             # of input epsilons: {}\n\
             # of output epsilons: {}\n\
             # of ... accessible states: ???\n\
             # of ... coaccessible states: ???\n\
             # of ... connected states: ???\n\
             # of ... strongly conn components: ???\n",
            s.states, s.arcs, 0, s.final_states, s.io_epsilons, s.input_epsilons, s.output_epsilons
        )
    }

    fn write_properties(&self, out: &mut dyn Write) -> std::io::Result<()> {
        let yes = |b: bool| if b { "yes" } else { "no" };
        let (mutable, weighted) = self.mutable_and_weighted();
        let s = &self.stats;
        write!(
            out,
            "expanded: ???\n\
             mutable: {}\n\
             acceptor: {}\n\
             input deterministic: {}\n\
             output deterministic: {}\n\
             input label sorted: ???\n\
             output label sorted: ???\n\
             weighted: {}\n\
             cyclic: {}\n\
             cyclic at initial state: {}\n\
             topologically sorted: ???\n\
             accessible: ???\n\
             coaccessible: ???\n\
             string: ???\n\
             minimised: ???\n",
            yes(mutable),
            yes(s.acceptor),
            yes(s.input_deterministic),
            yes(s.output_deterministic),
            yes(weighted),
            yes(s.cyclic),
            yes(s.cyclic_at_initial_state)
        )
    }

    fn write_details(&self, out: &mut dyn Write, details: &SummaryDetails) -> std::io::Result<()> {
        let s = &self.stats;
        let average_arcs_per_state = (s.arcs as f64) / (s.states as f32) as f64;
        write!(
            out,
            "number of arcs in sparsest state: {}\n\
             number of arcs in densest state: {}\n\
             average arcs per state: {:.6}\n\
             average input epsilons per state: {:.6}\n\
             most ambiguous input: {} {}\n\
             most ambiguous output: {} {}\n\
             average input ambiguity: {:.6}\n\
             average output ambiguity: {:.6}\n\
             expected arcs per symbol: {:.6}\n\
             infinitely ambiguous: {}\n",
            s.sparsest_arcs,
            s.densest_arcs,
            average_arcs_per_state,
            (s.input_epsilons as f64) / (s.states as f64),
            s.most_ambiguous_input.0,
            s.most_ambiguous_input.1,
            s.most_ambiguous_output.0,
            s.most_ambiguous_output.1,
            (s.arcs as f64) / (s.uniq_input_arcs as f64),
            (s.arcs as f64) / (s.uniq_output_arcs as f64),
            average_arcs_per_state / (s.found_alphabet.len() as f32) as f64,
            if details.infinitely_ambiguous {
                "yes"
            } else {
                "no"
            }
        )?;
        writeln!(out, "sigma set:")?;
        write_list(out, self.alphabet.iter())?;
        writeln!(out, "arc symbols actually seen in transducer:")?;
        write_list(out, s.found_alphabet.iter())?;
        writeln!(out, "sigma symbols missing from transducer:")?;
        write_list(out, self.alphabet.difference(&s.found_alphabet))?;
        if let Some(first) = &details.first_input_symbols {
            writeln!(out, "first input symbols:")?;
            write_list(out, first.iter())?;
        }
        Ok(())
    }

    fn write_symbol_pairs(&self, out: &mut dyn Write, threshold: i32) -> std::io::Result<()> {
        if threshold > -1 {
            writeln!(
                out,
                "symbol pairs that occur at most {} times:",
                threshold as u32
            )?;
        } else {
            writeln!(out, "symbol pairs:")?;
        }
        // The C++ compared the count against the threshold as unsigned, so
        // -1 lets every pair through.
        for ((input, output), count) in &self.stats.symbol_pairs {
            if *count <= threshold as u32 {
                writeln!(out, "{}:{}\t{}", input, output, count)?;
            }
        }
        writeln!(out)
    }
}

/// `symbols` separated by commas, then a newline.
fn write_list<'s>(
    out: &mut dyn Write,
    symbols: impl Iterator<Item = &'s crate::hfst_data_types::Symbol>,
) -> std::io::Result<()> {
    for (i, s) in symbols.enumerate() {
        if i > 0 {
            write!(out, ", ")?;
        }
        write!(out, "{}", s)?;
    }
    writeln!(out)
}

// [spec:hfst:req:embed.tools-in-library]
/// The line `hfst summarize` and `hfst check-alpha` end with, after `count`
/// transducers.
pub fn write_total(out: &mut dyn Write, count: usize) -> std::io::Result<()> {
    write!(out, "\nRead {} transducers in total.\n", count)
}
