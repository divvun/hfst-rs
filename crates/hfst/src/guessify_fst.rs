//! Port of 'tools/src/guessify_fst.{h,cc}'.
//!
//! Functions for making an affix guesser from a morphological analyzer.
//!
//! @author Miikka Silfverberg (HFST Team)
//!
//! Faithful 1:1 port of the '#ifndef MAIN_TEST' production section. The
//! '#else // MAIN_TEST' 'main' is a stdin/stdout self-test harness; per the
//! porting convention its annotations are carried as a comment only (see the
//! 'main-fn' block at the end of this file).

use crate::backend::{AlgebraBackend, Backend};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::Symbol;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_lookup_flag_diacritics::FlagDiacriticTable;
use crate::hfst_output_stream::HfstOutputStream;
use crate::hfst_symbol_defs::{
    HfstSymbolPairSubstitutions, HfstSymbolSubstitutions, StringSet, internal_default,
    internal_epsilon, internal_identity, internal_unknown,
};
use crate::hfst_transducer::HfstTransducer;

// [spec:hfst:def:guessify-fst.hfst-basic-transitions]
pub use crate::hfst_basic_transducer::HfstBasicTransitions;

// Marker for removed symbols.
const REMOVED_SYMBOL: &str = "<removed_symbol>";

// Default penalty for skipping symbols in input strings.
pub const DEFAULT_PENALTY: f32 = 1.0;

// Prefix which all inflection category symbols in the morphological
// analyzer are expected to have.
pub const CATEGORY_SYMBOL_PREFIX: &str = "[GUESS_CATEGORY=";

// 'std::string my_default = "$_DEFAULT_SYMBOL_$";' — a file-global in the C++
// that is never reassigned, so it is a plain immutable static here.
static MY_DEFAULT: &str = "$_DEFAULT_SYMBOL_$";

fn my_default() -> &'static str {
    MY_DEFAULT
}

// [spec:hfst:def:guessify-fst.remove-flag-diacritics-fn]
// [spec:hfst:sem:guessify-fst.remove-flag-diacritics-fn]
pub fn remove_flag_diacritics<B: AlgebraBackend>(
    morphological_analyzer: &mut HfstTransducer<B>,
    alphabet: &StringSet,
) -> crate::error::Result<()> {
    let mut flag_diacritic_epsilon_pairs = HfstSymbolSubstitutions::new();

    for it in alphabet.iter() {
        if FlagDiacriticTable::is_diacritic(it) {
            flag_diacritic_epsilon_pairs.insert(it.clone(), Symbol::new_static(internal_epsilon));
        }
    }

    morphological_analyzer.substitute_symbol_substitutions(&flag_diacritic_epsilon_pairs)?;
    Ok(())
}

// [spec:hfst:def:guessify-fst.is-cathegory-symbol-fn]
// [spec:hfst:sem:guessify-fst.is-cathegory-symbol-fn]
pub fn is_cathegory_symbol(symbol: &str) -> bool {
    symbol.find(CATEGORY_SYMBOL_PREFIX) == Some(0)
}

// [spec:hfst:def:guessify-fst.get-cathegory-symbols-fn]
// [spec:hfst:sem:guessify-fst.get-cathegory-symbols-fn]
pub fn get_cathegory_symbols(alphabet: &StringSet) -> StringSet {
    let mut cathegory_symbols = StringSet::new();

    for it in alphabet.iter() {
        if is_cathegory_symbol(it) {
            cathegory_symbols.insert(it.clone());
        }
    }

    cathegory_symbols
}

// [spec:hfst:def:guessify-fst.get-prefix-remover-fn]
// [spec:hfst:sem:guessify-fst.get-prefix-remover-fn]
pub fn get_prefix_remover<B: AlgebraBackend>(
    alphabet: &StringSet,
) -> crate::error::Result<HfstTransducer<B>> {
    let cathegory_symbols = get_cathegory_symbols(alphabet);

    let mut cathegory_symbols_fst: HfstTransducer<B> = HfstTransducer::new();

    let mut identity_except_cathegory: HfstTransducer<B> =
        HfstTransducer::new_symbol(internal_identity)?;
    let mut basic_identity = HfstBasicTransducer::from_transducer(&identity_except_cathegory);

    // Add cathegory symbols as paths in cathegory_symbols_fst and add
    // them to the alphabet of basic_identity so that the identity
    // transitions won't cover cathegory symbols.
    for it in cathegory_symbols.iter() {
        let cathegory_symbol_fst = HfstTransducer::new_symbol(it)?;
        cathegory_symbols_fst.disjunct(&cathegory_symbol_fst, true)?;
        basic_identity.add_symbol_to_alphabet(it);
    }

    cathegory_symbols_fst.minimize()?;

    // Preserve one symbol after the cathegory marker.
    let identity = HfstTransducer::new_symbol(internal_identity)?;
    cathegory_symbols_fst
        .concatenate(&identity, true)?
        .minimize()?;
    identity_except_cathegory = HfstTransducer::new_from_basic_transducer(&basic_identity);
    identity_except_cathegory.repeat_star()?.minimize()?;

    let mut remove_symbol = HfstTransducer::new_symbol_pair(internal_unknown, REMOVED_SYMBOL)?;
    remove_symbol.repeat_star()?.minimize()?;

    let mut remove_suffix = HfstTransducer::new_copy(&cathegory_symbols_fst)?;
    remove_suffix.concatenate(&remove_symbol, true)?;
    remove_suffix.optionalize()?.minimize()?;

    identity_except_cathegory
        .concatenate(&remove_suffix, true)?
        .minimize()?;

    Ok(identity_except_cathegory)
}

// [spec:hfst:def:guessify-fst.get-invalid-form-filterer-fn]
// [spec:hfst:sem:guessify-fst.get-invalid-form-filterer-fn]
pub fn get_invalid_form_filterer<B: AlgebraBackend>(
    alphabet: &StringSet,
) -> crate::error::Result<HfstTransducer<B>> {
    let cathegory_symbols = get_cathegory_symbols(alphabet);

    let mut cathegory_symbols_fst: HfstTransducer<B> = HfstTransducer::new();
    for it in cathegory_symbols.iter() {
        let cathegory_symbol_fst = HfstTransducer::new_symbol(it)?;
        cathegory_symbols_fst.disjunct(&cathegory_symbol_fst, true)?;
    }

    cathegory_symbols_fst.minimize()?;

    let identity = HfstTransducer::new_symbol(internal_identity)?;

    let mut identity_star = HfstTransducer::new_copy(&identity)?;

    identity_star.repeat_star()?.minimize()?;

    let mut remover = HfstTransducer::new_copy(&identity_star)?;

    remover
        .concatenate(&cathegory_symbols_fst, true)?
        .concatenate(&identity, true)?
        .concatenate(&identity_star, true)?
        .minimize()?;

    Ok(remover)
}

// [spec:hfst:def:guessify-fst.rewrite-removed-symbols-fn]
// [spec:hfst:sem:guessify-fst.rewrite-removed-symbols-fn]
pub fn rewrite_removed_symbols<B: AlgebraBackend>(
    morphological_analyzer: &mut HfstTransducer<B>,
    alphabet: &StringSet,
) -> crate::error::Result<()> {
    let mut substitution_pairs = HfstSymbolPairSubstitutions::new();

    substitution_pairs.insert(
        (
            Symbol::new_static(internal_epsilon),
            Symbol::new_static(REMOVED_SYMBOL),
        ),
        (
            Symbol::new_static(internal_epsilon),
            Symbol::new_static(internal_epsilon),
        ),
    );

    for it in alphabet.iter() {
        if it.as_str() != internal_epsilon {
            substitution_pairs.insert(
                (it.clone(), Symbol::new_static(REMOVED_SYMBOL)),
                (it.clone(), it.clone()),
            );
        }
    }

    morphological_analyzer.substitute_symbol_pair_substitutions(&substitution_pairs)?;
    Ok(())
}

// [spec:hfst:def:guessify-fst.guessify-analyzer-fn]
// [spec:hfst:sem:guessify-fst.guessify-analyzer-fn]
pub fn guessify_analyzer<B: AlgebraBackend>(
    mut morphological_analyzer: HfstTransducer<B>,
    penalty: f32,
) -> crate::error::Result<HfstTransducer<B>> {
    // The C++ "Convert to tropical openfst type so that all operations can be
    // performed" was pure capability gating; the AlgebraBackend bound
    // guarantees every operation now ([dec:hfst:monomorphic-backends]).

    let morphological_analyzer_name = morphological_analyzer.get_name();

    // Start be reversing the morphological analyzer, since guessing is
    // based on suffixes of words.
    morphological_analyzer.reverse()?.minimize()?;

    // Get rid of flag diacritics. They're a nuissance and the
    // combinatorics would any way be screwed up because we modify the
    // behavior of the transducer using default-transitions.
    let alphabet = morphological_analyzer.get_alphabet()?;
    remove_flag_diacritics(&mut morphological_analyzer, &alphabet)?;

    morphological_analyzer.minimize()?;

    // Remove the parts of analyses that precede the last cathegory
    // tag. After the last cathegory tag all input should be echoed as is to
    // the output.
    /*
    HfstTransducer analysis_prefix_remover = get_prefix_remover(alphabet);

    morphological_analyzer.compose(analysis_prefix_remover);
    rewrite_removed_symbols(morphological_analyzer, alphabet);

    morphological_analyzer.minimize();
    */

    // Add a sink state and default transitions from every state
    // (including the sink state) to the sink state. The default
    // transitions all have the same weight @a penalty.
    let mut basic_guesser = HfstBasicTransducer::from_transducer(&morphological_analyzer);

    let sink_state = basic_guesser.add_state_new();

    let mut s: HfstState = 0;
    while s <= basic_guesser.get_max_state() {
        basic_guesser.set_final_weight(s, &0.0);
        s += 1;
    }

    let mut s: HfstState = 0;
    while s <= basic_guesser.get_max_state() {
        let arc = HfstBasicTransition::new_symbols(
            sink_state,
            Symbol::new_static(my_default()),
            Symbol::new_static(my_default()),
            penalty,
            basic_guesser.coder_mut(),
        );
        basic_guesser.add_transition(s, &arc, true);
        s += 1;
    }

    // Add an a-transition to the sink state to all states where there
    // is only one transitions which is a default-transition, since some
    // version of hfst didn't suporrt default transitions in states
    // without any additional transitions.
    let mut s: HfstState = 0;
    while s <= basic_guesser.get_max_state() {
        if basic_guesser.index(s)?.len() == 1
            && basic_guesser.index(s)?[0].get_input_symbol(basic_guesser.coder()) == my_default()
        {
            let arc = HfstBasicTransition::new_symbols(
                sink_state,
                Symbol::new_static("a"),
                Symbol::new_static("a"),
                penalty,
                basic_guesser.coder_mut(),
            );
            basic_guesser.add_transition(s, &arc, true);
        }
        s += 1;
    }

    let mut guesser: HfstTransducer<B> = HfstTransducer::new_from_basic_transducer(&basic_guesser);

    let invalid_form_filterer = get_invalid_form_filterer(&alphabet)?;

    guesser.compose(&invalid_form_filterer, true)?.minimize()?;

    guesser.set_name(&format!("guessified({})", morphological_analyzer_name));

    guesser.set_property("reverse input", "true");

    Ok(guesser)
}

// [spec:hfst:def:guessify-fst.store-guesser-fn]
// [spec:hfst:sem:guessify-fst.store-guesser-fn]
pub fn store_guesser<B: AlgebraBackend>(
    guesser: &mut HfstTransducer<B>,
    out: &mut HfstOutputStream,
    compile_generator: bool,
) -> crate::error::Result<()> {
    let mut generator: HfstTransducer<B> = HfstTransducer::new();
    if compile_generator {
        generator = HfstTransducer::new_copy(guesser)?;
    }

    guesser.substitute(my_default(), internal_default, true, true)?;
    // The C++ 'convert(HFST_OLW_TYPE)' mutated the facade in place, keeping
    // its metadata (name + properties, e.g. the "reverse input" guesser
    // marker); the typed conversion pair pins the written result to the
    // weighted optimized-lookup backend and the metadata is carried across
    // explicitly ([dec:hfst:monomorphic-backends]).
    let mut guesser_olw: HfstTransducer<crate::transducer::Transducer> =
        crate::convert_transducer_format::ConversionFunctions::hfst_ol_to_hfst_transducer(
            &crate::convert_transducer_format::ConversionFunctions::hfst_transducer_to_hfst_ol(
                guesser,
            )?,
        )?;
    for (property, value) in guesser.get_properties() {
        guesser_olw.set_property(property, value);
    }
    out.operator_shl(&mut guesser_olw)?;

    if compile_generator {
        generator.invert()?;
        generator.set_name(&format!("inverted({})", guesser.get_name()));
        generator.substitute(my_default(), internal_default, true, true)?;
        let mut generator_olw: HfstTransducer<crate::transducer::Transducer> =
            crate::convert_transducer_format::ConversionFunctions::hfst_ol_to_hfst_transducer(
                &crate::convert_transducer_format::ConversionFunctions::hfst_transducer_to_hfst_ol(
                    &generator,
                )?,
            )?;
        for (property, value) in generator.get_properties() {
            generator_olw.set_property(property, value);
        }
        out.operator_shl(&mut generator_olw)?;
    }
    Ok(())
}

// Direction of guessing for `affix_guessify`. Lifted verbatim from
// hfst-affix-guessify's tool-local enum so the affix-guesser construction can
// live in the library.
// [spec:hfst:def:hfst-affix-guessify.guess-direction]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GuessDirection {
    GuessPrefix,
    GuessSuffix,
}

// Build a weighted affix guesser from a single automaton — the per-transducer
// body of hfst-affix-guessify's process_stream, lifted into the library. The
// `GuessSuffix` branch rebuilds the automaton with a fresh "guess" prefix state
// (state 0) whose identity/alphabet self-loops carry `weight`, shifting every
// original state by +1; the `GuessPrefix` branch appends a single guess state
// that every state can reach by an identity arc. The result stays in the
// caller's backend type (the tool's runtime 'format' parameter is the type
// parameter now). (The tool's mid-algorithm "-v" traces — "Creating guesser
// prefix...", "Rebuilding suffix...", "converting and saving..." — were
// diagnostic and are not reproduced here; the constructed transducer is
// unchanged. The owning tool keeps the [spec:hfst:*:hfst-affix-guessify
// .process-stream-fn] annotation on its stream-driver loop.)
pub fn affix_guessify<B: Backend>(
    trans: &HfstTransducer<B>,
    direction: GuessDirection,
    weight: f32,
) -> crate::error::Result<HfstTransducer<B>> {
    let alpha = trans.get_alphabet()?;
    Ok(match direction {
        GuessDirection::GuessSuffix => {
            let mutt = HfstBasicTransducer::from_transducer(trans);
            let mut repl = HfstBasicTransducer::new();
            let guess_state = repl.add_state(0);
            let guess_arc = HfstBasicTransition::new_symbols(
                guess_state,
                Symbol::new_static(internal_identity),
                Symbol::new_static(internal_identity),
                weight,
                repl.coder_mut(),
            );
            repl.add_transition(guess_state, &guess_arc, true);
            for x in alpha.iter() {
                let x_arc = HfstBasicTransition::new_symbols(
                    guess_state,
                    x.clone(),
                    x.clone(),
                    weight,
                    repl.coder_mut(),
                );
                repl.add_transition(guess_state, &x_arc, true);
            }
            for s in 0..=mutt.get_max_state() {
                let d = repl.add_state(s + 1);
                if mutt.is_final_state(s) {
                    let fw = mutt.get_final_weight(s)?;
                    repl.set_final_weight(d, &fw);
                }
                let guess_arc = HfstBasicTransition::new_symbols(
                    d,
                    Symbol::new_static(internal_identity),
                    Symbol::new_static(internal_identity),
                    weight,
                    repl.coder_mut(),
                );
                repl.add_transition(guess_state, &guess_arc, true);
                for x in alpha.iter() {
                    let x_arc = HfstBasicTransition::new_symbols(
                        d,
                        x.clone(),
                        x.clone(),
                        weight,
                        repl.coder_mut(),
                    );
                    repl.add_transition(guess_state, &x_arc, true);
                }
                for arc in mutt.transitions(s)?.iter() {
                    // Cross-graph copy: read arc symbols via 'mutt's coder, then
                    // build the new transition into 'repl' via 'repl's coder.
                    let target = arc.get_target_state() + 1;
                    let isym = arc.get_input_symbol(mutt.coder());
                    let osym = arc.get_output_symbol(mutt.coder());
                    let w = arc.get_weight();
                    let newarc =
                        HfstBasicTransition::new_symbols(target, isym, osym, w, repl.coder_mut());
                    repl.add_transition(d, &newarc, true);
                }
            }
            HfstTransducer::new_from_basic(&repl)?
        }
        GuessDirection::GuessPrefix => {
            let mut repl = HfstBasicTransducer::from_transducer(trans);
            let guess_state = repl.add_state_new();
            repl.set_final_weight(guess_state, &0.0f32);
            let guess_arc = HfstBasicTransition::new_symbols(
                guess_state,
                Symbol::new_static(internal_identity),
                Symbol::new_static(internal_identity),
                weight,
                repl.coder_mut(),
            );
            repl.add_transition(guess_state, &guess_arc, true);
            let max_state = repl.get_max_state();
            for s in 0..=max_state {
                let newarc = HfstBasicTransition::new_symbols(
                    guess_state,
                    Symbol::new_static(internal_identity),
                    Symbol::new_static(internal_identity),
                    weight,
                    repl.coder_mut(),
                );
                repl.add_transition(s, &newarc, true);
            }
            HfstTransducer::new_from_basic(&repl)?
        }
    })
}

// [spec:hfst:def:guessify-fst.main-fn]
// [spec:hfst:sem:guessify-fst.main-fn]
//
// The MAIN_TEST 'main' (the '#else // MAIN_TEST' branch) is a stdin/stdout
// self-test harness, not production code. Per the porting convention,
// MAIN_TEST sections are not emitted; the annotation is carried here as a
// comment only.
