//! Faithful 1:1 port of tools/src/hfst-compose-intersect.cc — the
//! compose-intersect command-line tool (compose a lexicon with one or more
//! rule transducers). Drives the hfst-cli foundation (getopt, commandline,
//! program-options, tool-metadata, inc fragments). This is a BINARY tool: it
//! reads a first stream (the lexicon) and a second stream (the rule file).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-1/-2/…` fields) and a tool-local [`Options`] — both built
//! by `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::binary_ops::{open_output_stream, open_two_input_streams, resolve_output_type};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, is_input_stream_in_ol_format,
    verbose_print, warning,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_program_options,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary};
use crate::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use crate::memory_limit::{self, LimitSource, ResolvedMemoryLimit};
use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::internal_identity;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::EngineConfig;
use hfst::hfst_transducer::{HfstTransducer, HfstTransducerVector};
use std::io::Write;

const GETOPT_MEMORY_LIMIT: i32 = 0x100;

// static bool insert_missing_flags=false;

/// hfst-compose-intersect's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-I, --invert': if true, the intersection of the rules is composed with
    /// the lexicon; otherwise the lexicon is composed with the intersection of
    /// the rules.
    invert: bool,
    /// '-e, --encode-weights': encode weights when minimizing.
    encode_weights: bool,
    /// '-f, --fast': faster compose intersect using more memory.
    fast_ci: bool,
    /// '-a, --harmonize': harmonize symbols.
    harmonize: bool,
    /// '--memory-limit=SIZE': one-rule tropical product memory allowance.
    memory_limit_bytes: Option<u64>,
}

// [spec:hfst:req:cli.help]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = &common.program_name;
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\n\
         Compose a lexicon with one or more rule transducers.\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Composition options:\n\
         \x20 -I, --invert                 Compose the intersection of the\n\
         \x20                              rules with the lexicon instead\n\
         \x20                              of composing the lexicon with\n\
         \x20                              the intersection of the rules.\n\
         \x20 -f, --fast                   Faster compose instersect using\n\
         \x20                              more memory.\n\
         \x20 -e, --encode-weights         Encode weights when minimizing\n\
         \x20                              (default is false).\n\
         \x20 -a, --harmonize              Harmonize symbols.\n"
    );
    let _ = writeln!(
        msg,
        "      --memory-limit=SIZE         Working-memory allowance for one-rule, non-inverted OpenFst tropical compose-intersect (default: 50% of available RAM; excess spills)."
    );
    let _ = writeln!(
        msg,
        "SIZE is an integer byte count with an optional binary K/KB/KiB through T/TB/TiB suffix; 0 forces a nonempty product to spill. HFST_COMPOSE_MEMORY_LIMIT supplies SIZE when the option is absent."
    );
    // print_common_binary_program_parameter_instructions(message_out);
    let _ = write!(
        msg,
        "\nIf OUTFILE, or either INFILE1 or INFILE2 is missing or -, standard\n\
         streams will be used. INFILE1, INFILE2, or both, must be specified\n\
         The format of INFILE1 and INFILE2 must be the same; the result will\n\
         have the same format as these.\n\
         INFILE1 (the lexicon) must contain exactly one transducer.\n\
         INFILE2 (rule file) may contain several transducers.\n"
    );
    let _ = write!(
        msg,
        "\nExamples:\n\
         \x20 {} -o analyzer.hfst lexicon.hfst rules.hfst\n\
         compose rules with lexicon\n\n",
        program_name
    );
}

// [spec:hfst:req:cli.arg-parse]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_binary_long());
        long_options.push(getopt::GetOpt {
            name: "invert",
            has_arg: 0,
            val: b'I' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "encode-weights",
            has_arg: 0,
            val: b'e' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "fast",
            has_arg: 0,
            val: b'f' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "harmonize",
            has_arg: 0,
            val: b'a' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "memory-limit",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: GETOPT_MEMORY_LIMIT,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: binary
        // cases, common cases, the terminal error arm, then the tool's own
        // cases. The tool-specific cases must be tried before the error arm
        // falls through, so we test them ahead of handle_error_case.
        match handle_binary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        if c == b'I' as i32 {
            options.invert = true;
            continue;
        } else if c == b'e' as i32 {
            options.encode_weights = true;
            continue;
        } else if c == b'f' as i32 {
            options.fast_ci = true;
            continue;
        } else if c == b'a' as i32 {
            options.harmonize = true;
            continue;
        } else if c == GETOPT_MEMORY_LIMIT {
            let argument = opt.optarg();
            options.memory_limit_bytes = match memory_limit::parse_size(&argument) {
                Ok(bytes) => Some(bytes),
                Err(detail) => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: invalid value for --memory-limit: {detail}",
                        common.program_name
                    );
                    return Err(1);
                }
            };
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_binary_params(&mut common, &opt, args);
    check_common_params(&mut common);
    Ok((common, options))
}

// [spec:hfst:def:hfst-compose-intersect.string-set]
// (typedef std::set<std::string> StringSet → std::collections::BTreeSet<String>)

// [spec:hfst:def:hfst-compose-intersect.is-special-symbol-fn]
// [spec:hfst:sem:hfst-compose-intersect.is-special-symbol-fn]
fn is_special_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    symbol.len() > 2 && bytes[0] == b'@' && bytes[symbol.len() - 1] == b'@'
}

// [spec:hfst:def:hfst-compose-intersect.check-all-symbols-fn]
// [spec:hfst:sem:hfst-compose-intersect.check-all-symbols-fn]
fn check_all_symbols<B: hfst::backend::AlgebraBackend>(
    lexicon: &HfstTransducer<B>,
    rule: &HfstTransducer<B>,
) -> hfst::error::Result<String> {
    let rule_b = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(rule)?;

    let rule_input_symbols = rule_b.input_symbols_used();

    if rule_input_symbols.contains(internal_identity) {
        return Ok(String::new());
    }

    let lexicon_b = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(lexicon)?;

    for s in 0..=lexicon_b.get_max_state() {
        for it in lexicon_b.transitions(s)?.iter() {
            let output_symbol = it.get_output_symbol(lexicon_b.coder());

            if !rule_input_symbols.contains(&output_symbol) {
                return Ok(output_symbol.to_string());
            }
        }
    }

    Ok(String::new())
}

// [spec:hfst:def:hfst-compose-intersect.check-multi-char-symbols-fn]
// [spec:hfst:sem:hfst-compose-intersect.check-multi-char-symbols-fn]
fn check_multi_char_symbols<B: hfst::backend::AlgebraBackend>(
    lexicon: &HfstTransducer<B>,
    rule: &HfstTransducer<B>,
) -> hfst::error::Result<String> {
    let lexicon_b = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(lexicon)?;
    let rule_b = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(rule)?;

    let tokenizer = HfstTokenizer::new();

    let rule_input_symbols = rule_b.input_symbols_used();

    for s in 0..=lexicon_b.get_max_state() {
        for it in lexicon_b.transitions(s)?.iter() {
            let output_symbol = it.get_output_symbol(lexicon_b.coder());

            if !rule_input_symbols.contains(&output_symbol) {
                if is_special_symbol(&output_symbol) {
                    continue;
                }

                if tokenizer.tokenize_one_level(&output_symbol, false).len() > 1 {
                    return Ok(output_symbol.to_string());
                }
            }
        }
    }

    Ok(String::new())
}

// [spec:hfst:def:hfst-compose-intersect.harmonize-rules-fn]
// [spec:hfst:sem:hfst-compose-intersect.harmonize-rules-fn]
fn harmonize_rules<B: hfst::backend::AlgebraBackend>(
    lexicon: &mut HfstTransducer<B>,
    rules: &mut [HfstTransducer<B>],
) -> hfst::error::Result<()> {
    for it in rules.iter_mut() {
        it.harmonize(lexicon, false)?;
    }
    Ok(())
}

fn explicit_memory_limit_name(source: LimitSource) -> Option<&'static str> {
    match source {
        LimitSource::Cli => Some("--memory-limit"),
        LimitSource::Environment => Some("HFST_COMPOSE_MEMORY_LIMIT"),
        LimitSource::Automatic | LimitSource::ProbeFallback => None,
    }
}

fn report_memory_policy(common: &CommonOptions, memory_limit: ResolvedMemoryLimit) {
    if common.silent {
        return;
    }
    if memory_limit.source == LimitSource::ProbeFallback {
        warning(
            common,
            0,
            0,
            "Could not determine available RAM; using a 0-byte compose-intersect memory allowance and spilling immediately. Use --memory-limit to override.",
        );
    }
    if memory_limit.cgroup_clamped
        && let Some(requested) = memory_limit.requested_bytes
    {
        warning(
            common,
            0,
            0,
            &format!(
                "Requested compose-intersect memory allowance of {requested} bytes exceeds current cgroup headroom; using {} bytes.",
                memory_limit.allowance_bytes
            ),
        );
    }
}

// [spec:hfst:def:hfst-compose-intersect.compose-streams-fn]
// [spec:hfst:sem:hfst-compose-intersect.compose-streams-fn]
fn compose_streams(
    common: &CommonOptions,
    options: &Options,
    memory_limit: ResolvedMemoryLimit,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
) -> i32 {
    // there must be at least one transducer in both input streams
    let type1 = firststream.get_type();
    let type2 = secondstream.get_type();
    let output_type = resolve_output_type(
        common,
        "hfst-compose-intersect",
        "compose-intersect",
        type1,
        type2,
    );

    let tropical = output_type == hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
    if !tropical {
        if let Some(name) = explicit_memory_limit_name(memory_limit.source) {
            error(
                common,
                1,
                0,
                &format!("{name} is supported only for OpenFst tropical compose-intersect"),
            );
            return 1;
        }
    } else {
        report_memory_policy(common, memory_limit);
    }

    let mut outstream = match open_output_stream(common, output_type) {
        Ok(s) => s,
        Err(code) => return code,
    };

    let _both_inputs = firststream.is_good() && secondstream.is_good();

    if is_input_stream_in_ol_format(firststream, "hfst-compose-intersect")
        || is_input_stream_in_ol_format(secondstream, "hfst-compose-intersect")
    {
        return 1;
    }

    // The resolved output type is matched ONCE into the backend type
    // parameter ([dec:hfst:monomorphic-backends]); every read converts to
    // it, exactly as the C++ convert(output_type) calls did.
    match output_type {
        #[cfg(feature = "foma")]
        hfst::hfst_data_types::ImplementationType::FOMA_TYPE => {
            compose_streams_typed::<hfst::backend_foma::FomaTransducer>(
                common,
                options,
                memory_limit,
                firststream,
                secondstream,
                &mut outstream,
            )
        }
        _ => compose_streams_typed::<hfst_openfst::StdVectorFst>(
            common,
            options,
            memory_limit,
            firststream,
            secondstream,
            &mut outstream,
        ),
    }
}

fn compose_streams_typed<
    B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer,
>(
    common: &CommonOptions,
    options: &Options,
    memory_limit: ResolvedMemoryLimit,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
    outstream: &mut hfst::hfst_output_stream::HfstOutputStream,
) -> i32 {
    let mut rules: HfstTransducerVector<B> = Vec::new();
    let mut rule_n: usize = 1;

    while secondstream.is_good() {
        let mut rule: HfstTransducer<B> = match secondstream.read().and_then(|any| any.into_typed())
        {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let rulename = rule.get_name();
        if !rulename.is_empty() {
            verbose_print(
                common,
                &format!("Reading and minimizing rule {}...\n", rulename),
            );
        } else {
            verbose_print(
                common,
                &format!("Reading and minimizing rule {}...\n", rule_n),
            );
        }
        if let Err(e) = rule.minimize_with_config(&EngineConfig {
            encode_weights: options.encode_weights,
            ..EngineConfig::default()
        }) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }

        rules.push(rule);
        rule_n += 1;
    }

    if explicit_memory_limit_name(memory_limit.source).is_some()
        && (rules.len() != 1 || options.invert || options.fast_ci || !B::SUPPORTS_COMPOSE_LOOKAHEAD)
    {
        error(
            common,
            1,
            0,
            "an explicit compose-intersect memory limit requires exactly one rule, non-inverted composition without --fast, and the OpenFst tropical backend",
        );
        return 1;
    }

    let engine_config = EngineConfig {
        encode_weights: options.encode_weights,
        compose_memory_limit_bytes: B::SUPPORTS_COMPOSE_LOOKAHEAD
            .then_some(memory_limit.allowance_bytes),
        ..EngineConfig::default()
    };

    while firststream.is_good() {
        verbose_print(common, "Reading lexicon...");
        let mut lexicon: HfstTransducer<B> =
            match firststream.read().and_then(|any| any.into_typed()) {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
        let lexiconname = hfst_get_name(&lexicon, &common.first_filename);
        verbose_print(common, &format!(" {} read\n", lexiconname));

        verbose_print(common, "Computing intersecting composition...\n");

        if !rules.is_empty() {
            let symbol = match check_all_symbols(&lexicon, &rules[0]) {
                Ok(s) => s,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if !symbol.is_empty() {
                warning(
                    common,
                    0,
                    0,
                    &format!(
                        "\nFound output symbols (e.g. \"{}\") in transducer in\n\
                         file {} which will be filtered out because they are\n\
                         not found on the input tapes of transducers in file\n\
                         {}.",
                        symbol, common.first_filename, common.second_filename
                    ),
                );
            } else {
                let symbol = match check_multi_char_symbols(&lexicon, &rules[0]) {
                    Ok(s) => s,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if !symbol.is_empty() {
                    warning(
                        common,
                        0,
                        0,
                        &format!(
                            "\nFound output multi-char symbols (\"{}\") in \n\
                             transducer in file {} which are not found on the\n\
                             input tapes of transducers in file {}.",
                            symbol, common.first_filename, common.second_filename
                        ),
                    );
                }
            }
        }

        if options.harmonize
            && let Err(e) = harmonize_rules(&mut lexicon, &mut rules)
        {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }

        if options.fast_ci {
            // To hopefully speed up stuff: Compose intersect the output
            // of the lexicon with the rules and then compose the original
            // lexicon with the result.

            if options.invert {
                let mut lexicon_input = lexicon.clone();
                if let Err(e) = lexicon_input.input_project() {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                if let Err(e) = lexicon_input.minimize() {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                if let Err(e) =
                    lexicon_input.compose_intersect_with_config(&rules, true, true, &engine_config)
                {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }

                if let Err(e) = lexicon_input.compose_with_config(&lexicon, true, &engine_config) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                lexicon = lexicon_input;
            } else {
                let mut lexicon_output = lexicon.clone();
                if let Err(e) = lexicon_output.output_project() {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                if let Err(e) = lexicon_output.minimize() {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                if let Err(e) = lexicon_output.compose_intersect_with_config(
                    &rules,
                    false,
                    true,
                    &engine_config,
                ) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                if let Err(e) = lexicon.compose_with_config(&lexicon_output, true, &engine_config) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }
        } else {
            if let Err(e) =
                lexicon.compose_intersect_with_config(&rules, options.invert, true, &engine_config)
            {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }

        let composed_name = format!(
            "compose({}, intersect({}))",
            lexiconname, common.second_filename
        );
        lexicon.set_name(&composed_name);
        let src = lexicon.clone();
        hfst_set_formula_unary(&mut lexicon, &src, " \u{2218} \u{22c2}R");

        verbose_print(
            common,
            &format!("Storing result in {}...\n", common.output_filename),
        );
        if let Err(e) = outstream.redirect(&mut lexicon) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }

    firststream.close();
    secondstream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-compose-intersect.main-fn]
// [spec:hfst:sem:hfst-compose-intersect.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstComposeIntersect");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let memory_limit = match memory_limit::resolve(options.memory_limit_bytes) {
        Ok(limit) => limit,
        Err(detail) => {
            let _ = writeln!(std::io::stderr(), "{}: {detail}", common.program_name);
            return 1;
        }
    };
    // close buffers, we use streams
    verbose_print(
        &common,
        &format!(
            "Reading from {} and {}, writing to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    let (mut firststream, mut secondstream) = match open_two_input_streams(&common) {
        Ok(v) => v,
        Err(code) => return code,
    };

    compose_streams(
        &common,
        &options,
        memory_limit,
        &mut firststream,
        &mut secondstream,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_limit_sources_are_named() {
        assert_eq!(
            explicit_memory_limit_name(LimitSource::Cli),
            Some("--memory-limit")
        );
        assert_eq!(
            explicit_memory_limit_name(LimitSource::Environment),
            Some("HFST_COMPOSE_MEMORY_LIMIT")
        );
    }

    #[test]
    fn automatic_limits_are_implicit() {
        assert_eq!(explicit_memory_limit_name(LimitSource::Automatic), None);
        assert_eq!(explicit_memory_limit_name(LimitSource::ProbeFallback), None);
    }
}
