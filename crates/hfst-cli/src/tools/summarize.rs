//! Faithful 1:1 port of tools/src/hfst-summarize.cc — the transducer
//! information / properties command-line tool. Option handling is clap 4
//! derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, parse_u64, verbose_print};
use hfst::hfst_basic_transducer::{HfstBasicTransducer, SummaryStats};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use std::io::Write;

/// hfst-summarize's command line.
// [spec:hfst:def:hfst-summarize.parse-options-fn]
// [spec:hfst:sem:hfst-summarize.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Calculate the properties of a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Print info about symbol pairs that occur at most N times (default is
    /// infinity)
    #[arg(
        short = 'S',
        long = "print-symbol-pair-statistics",
        value_name = "N",
        num_args = 0..=1,
        require_equals = true,
        allow_hyphen_values = true
    )]
    print_symbol_pair_statistics: Option<Option<String>>,
}

impl Args {
    /// Case 'S'. The C read an OPTIONAL_ARGUMENT, so the flag alone means "no
    /// threshold" (-1, which the unsigned comparison in the report turns into
    /// "every pair"); a supplied N is parsed with strtoul. The leading '=' the
    /// C's getopt left on an attached value is stripped the same way, so
    /// '-S=5' and '-S5' both read 5.
    fn symbol_pair_threshold(&self, common: &CommonOptions) -> i32 {
        let Some(Some(value)) = self.print_symbol_pair_statistics.as_ref() else {
            return -1;
        };
        let value = value.strip_prefix('=').unwrap_or(value);
        let threshold = parse_u64(common, value, 10) as i32;
        if threshold < 0 {
            error(
                common,
                1,
                0,
                &format!(
                    "{} is not a valid argument for option --print-symbol-pair-statistics\n",
                    threshold as u32
                ),
            );
        }
        if threshold == 0 {
            error(
                common,
                1,
                0,
                "0 is not a valid argument for option --print-symbol-pair-statistics\n",
            );
        }
        threshold
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The threshold rejections happened inside the C getopt loop, before
        // the parameter checks; run them here for the same ordering.
        self.symbol_pair_threshold(opts);
        Ok(())
    }
}

/// The two tool-local fields the report body reads, resolved once.
struct Options {
    print_symbol_pair_statistics: bool,
    symbol_pair_threshold: i32,
}

// [spec:hfst:def:hfst-summarize.process-stream-fn]
// [spec:hfst:sem:hfst-summarize.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-summarize: cannot open output: {e}");
            return 1;
        }
    };
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;

        if transducer_n < 2 {
            verbose_print(common, "Summarizing...\n");
        } else {
            verbose_print(common, &format!("Summarizing... {}\n", transducer_n));
        }
        let any = match instream.read() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        // The first-input-symbols query of the verbose branch, answered at the
        // boundary. It belongs to the algebra backends; the lookup-only runtime
        // formats have no graph to walk, so for them it is a compile-time
        // absence rather than the C++'s runtime type gate.
        let first_input_symbols = if common.verbose {
            let symbols = match &any {
                hfst::hfst_transducer::AnyTransducer::Tropical(t) => {
                    Some(t.get_first_input_symbols())
                }
                #[cfg(feature = "foma")]
                hfst::hfst_transducer::AnyTransducer::Foma(t) => Some(t.get_first_input_symbols()),
                hfst::hfst_transducer::AnyTransducer::OlW(_)
                | hfst::hfst_transducer::AnyTransducer::OlU(_)
                | hfst::hfst_transducer::AnyTransducer::Thfst(_) => None,
            };
            match symbols.transpose() {
                Ok(ss) => ss,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }
        } else {
            None
        };
        crate::for_any!(any, trans => {
            let mutt = HfstBasicTransducer::new_from_transducer(&trans);
            let initial_state: u32 = 0; // mutt.get_initial_state();
            let transducer_alphabet: StringSet = match trans.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let transducer_knows_alphabet = true;
            //let expanded = true;
            #[allow(unused_assignments)]
            let mut is_mutable = true;
            //let input_label_sorted = false;
            //let output_label_sorted = false;
            #[allow(unused_assignments)]
            let mut weighted = true;
            //let topologically_sorted = false;
            //let accessible = true;
            //let coaccessible = true;
            //let is_string = true;
            //let minimised = false;
            // assign data from knowledge about source type
            match trans.get_type() {
                ImplementationType::SFST_TYPE => {
                    is_mutable = true;
                    weighted = false;
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    is_mutable = true;
                    weighted = true;
                }
                ImplementationType::FOMA_TYPE => {
                    is_mutable = true;
                    weighted = false;
                }
                ImplementationType::HFST_OL_TYPE => {
                    is_mutable = false;
                    weighted = false;
                }
                // THFST is the weighted optimized-lookup family (directory format);
                // same immutable, weighted answer as HFST_OLW.
                ImplementationType::HFST_OLW_TYPE | ImplementationType::THFST_TYPE => {
                    is_mutable = false;
                    weighted = true;
                }
                ImplementationType::XFSM_TYPE | ImplementationType::HFST2_TYPE | ImplementationType::UNSPECIFIED_TYPE | ImplementationType::ERROR_TYPE => {
                    is_mutable = false;
                }
            }

            let SummaryStats {
                states,
                final_states,
                arcs,
                io_epsilons,
                input_epsilons,
                output_epsilons,
                densest_arcs,
                sparsest_arcs,
                uniq_input_arcs,
                uniq_output_arcs,
                most_ambiguous_input,
                most_ambiguous_output,
                found_alphabet,
                symbol_pairs,
                acceptor,
                input_deterministic,
                output_deterministic,
                cyclic,
                cyclic_at_initial_state,
            } = mutt.summarize();
            // traverse

            // count physical size

            // average calculations
            let average_arcs_per_state = (arcs as f64) / (states as f32) as f64;
            let average_input_epsilons = (input_epsilons as f64) / (states as f64);
            let average_input_ambiguity = (arcs as f64) / (uniq_input_arcs as f64);
            let average_output_ambiguity = (arcs as f64) / (uniq_output_arcs as f64);
            let expected_arcs_per_symbol =
                (average_arcs_per_state) / (found_alphabet.len() as f32) as f64;

            if transducer_n > 1 {
                let _ = write!(out, "-- \nTransducer #{}:\n", transducer_n);
            }
            let _ = writeln!(out, "name: \"{}\"", trans.get_name());
            // next is printed as in OpenFST's fstinfo
            // do not modify for compatibility
            match trans.get_type() {
                ImplementationType::SFST_TYPE => {
                    let _ = write!(out, "fst type: SFST\narc type: SFST\n");
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    let _ = write!(out, "fst type: OpenFST\narc type: tropical\n");
                }
                ImplementationType::FOMA_TYPE => {
                    let _ = write!(out, "fst type: foma\narc type: foma\n");
                }
                ImplementationType::HFST_OL_TYPE => {
                    let _ = write!(
                        out,
                        "fst type: HFST optimized lookup\narc type: unweighted\n"
                    );
                }
                ImplementationType::HFST_OLW_TYPE | ImplementationType::THFST_TYPE => {
                    let _ = write!(out, "fst type: HFST optimized lookup\narc type: weighted\n");
                }
                ImplementationType::XFSM_TYPE | ImplementationType::HFST2_TYPE | ImplementationType::UNSPECIFIED_TYPE | ImplementationType::ERROR_TYPE => {
                    let _ = write!(out, "fst type: ???\narc type: ???\n");
                }
            }
            let _ = write!(
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
                states,
                arcs,
                initial_state as i64,
                final_states,
                io_epsilons,
                input_epsilons,
                output_epsilons
            );
            // other names from properties...
            let _ = write!(
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
                if is_mutable { "yes" } else { "no" },
                if acceptor { "yes" } else { "no" },
                if input_deterministic { "yes" } else { "no" },
                if output_deterministic { "yes" } else { "no" },
                if weighted { "yes" } else { "no" },
                if cyclic { "yes" } else { "no" },
                if cyclic_at_initial_state { "yes" } else { "no" }
            );
            if common.verbose {
                // our extensions for nice statistics maybe
                let _ = write!(
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
                    sparsest_arcs,
                    densest_arcs,
                    average_arcs_per_state,
                    average_input_epsilons,
                    most_ambiguous_input.0,
                    most_ambiguous_input.1,
                    most_ambiguous_output.0,
                    most_ambiguous_output.1,
                    average_input_ambiguity,
                    average_output_ambiguity,
                    expected_arcs_per_symbol,
                    if mutt.is_infinitely_ambiguous() {
                        "yes"
                    } else {
                        "no"
                    }
                );
                // alphabets
                let _ = writeln!(out, "sigma set:");
                if transducer_knows_alphabet {
                    let mut first = true;
                    for s in transducer_alphabet.iter() {
                        if !first {
                            let _ = write!(out, ", ");
                        }
                        let _ = write!(out, "{}", s);
                        first = false;
                    }
                    let _ = writeln!(out);
                } else {
                    let _ = writeln!(out, "<Unknown in used transducer format>");
                }
                let _ = writeln!(out, "arc symbols actually seen in transducer:");
                let mut first = true;
                for s in found_alphabet.iter() {
                    if !first {
                        let _ = write!(out, ", ");
                    }
                    let _ = write!(out, "{}", s);
                    first = false;
                }
                let _ = writeln!(out);
                let _ = writeln!(out, "sigma symbols missing from transducer:");
                if transducer_knows_alphabet {
                    let transducer_minus_set: StringSet = transducer_alphabet
                        .difference(&found_alphabet)
                        .cloned()
                        .collect();

                    first = true;
                    for s in transducer_minus_set.iter() {
                        if !first {
                            let _ = write!(out, ", ");
                        }
                        let _ = write!(out, "{}", s);
                        first = false;
                    }
                    let _ = writeln!(out);
                } else {
                    let _ = writeln!(out, "<Unknown in used transducer format>");
                }
                // ADDED
                if let Some(ss) = &first_input_symbols {
                    let _ = writeln!(out, "first input symbols:");
                    first = true;
                    for s in ss.iter() {
                        if !first {
                            let _ = write!(out, ", ");
                        }
                        let _ = write!(out, "{}", s);
                        first = false;
                    }
                    let _ = writeln!(out);
                }
            } // if verbose

            // ADDED
            if options.print_symbol_pair_statistics {
                if options.symbol_pair_threshold > -1 {
                    let _ = writeln!(
                        out,
                        "symbol pairs that occur at most {} times:",
                        options.symbol_pair_threshold as u32
                    );
                } else {
                    let _ = writeln!(out, "symbol pairs:");
                }
                for (key, value) in symbol_pairs.iter() {
                    // C: 'it->second <= symbol_pair_threshold' compares unsigned
                    // against int, promoting the int to unsigned; a -1 threshold
                    // wraps to UINT_MAX so every pair passes. Mirror with the same
                    // unsigned comparison.
                    if *value <= (options.symbol_pair_threshold as u32) {
                        let _ = writeln!(out, "{}:{}\t{}", key.0, key.1, value);
                    }
                }
                let _ = writeln!(out);
            }
        });
    }

    let _ = write!(out, "\nRead {} transducers in total.\n", transducer_n);

    0
}

// [spec:hfst:def:hfst-summarize.main-fn]
// [spec:hfst:sem:hfst-summarize.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    cli::exit_code(execute(args))
}

fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options {
        print_symbol_pair_statistics: args.print_symbol_pair_statistics.is_some(),
        symbol_pair_threshold: args.symbol_pair_threshold(&common),
    };
    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; on a bad file the
    // Rust ctor currently panics rather than throwing, so the catch arm that
    // reports "%s is not a valid transducer file" is not reproduced here.)
    let instream_result = if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    };
    let mut instream = match instream_result {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    let retval = process_stream(&common, &options, &mut instream);
    let _ = retval;
    Ok(())
}
