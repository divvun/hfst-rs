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

use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_data_types::ImplementationType::{HFST_OLW_TYPE, TROPICAL_OPENFST_TYPE};
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
pub fn remove_flag_diacritics(morphological_analyzer: &mut HfstTransducer, alphabet: &StringSet) {
    let mut flag_diacritic_epsilon_pairs = HfstSymbolSubstitutions::new();

    for it in alphabet.iter() {
        if FlagDiacriticTable::is_diacritic(it) {
            flag_diacritic_epsilon_pairs.insert(it.clone(), internal_epsilon.to_string());
        }
    }

    morphological_analyzer.substitute_symbol_substitutions(&flag_diacritic_epsilon_pairs);
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
pub fn get_prefix_remover(alphabet: &StringSet) -> HfstTransducer {
    let cathegory_symbols = get_cathegory_symbols(alphabet);

    let mut cathegory_symbols_fst = HfstTransducer::new_type(TROPICAL_OPENFST_TYPE);

    let mut identity_except_cathegory =
        HfstTransducer::new_symbol(internal_identity, TROPICAL_OPENFST_TYPE);
    let mut basic_identity = HfstBasicTransducer::from_transducer(&identity_except_cathegory);

    // Add cathegory symbols as paths in cathegory_symbols_fst and add
    // them to the alphabet of basic_identity so that the identity
    // transitions won't cover cathegory symbols.
    for it in cathegory_symbols.iter() {
        let cathegory_symbol_fst = HfstTransducer::new_symbol(it, TROPICAL_OPENFST_TYPE);
        cathegory_symbols_fst.disjunct(&cathegory_symbol_fst, true);
        basic_identity.add_symbol_to_alphabet(it);
    }

    cathegory_symbols_fst.minimize();

    // Preserve one symbol after the cathegory marker.
    let identity = HfstTransducer::new_symbol(internal_identity, TROPICAL_OPENFST_TYPE);
    cathegory_symbols_fst
        .concatenate(&identity, true)
        .minimize();
    identity_except_cathegory =
        HfstTransducer::new_from_basic_transducer(&basic_identity, TROPICAL_OPENFST_TYPE);
    identity_except_cathegory.repeat_star().minimize();

    let mut remove_symbol =
        HfstTransducer::new_symbol_pair(internal_unknown, REMOVED_SYMBOL, TROPICAL_OPENFST_TYPE);
    remove_symbol.repeat_star().minimize();

    let mut remove_suffix = HfstTransducer::new_copy(&cathegory_symbols_fst);
    remove_suffix.concatenate(&remove_symbol, true);
    remove_suffix.optionalize().minimize();

    identity_except_cathegory
        .concatenate(&remove_suffix, true)
        .minimize();

    identity_except_cathegory
}

// [spec:hfst:def:guessify-fst.get-invalid-form-filterer-fn]
// [spec:hfst:sem:guessify-fst.get-invalid-form-filterer-fn]
pub fn get_invalid_form_filterer(alphabet: &StringSet) -> HfstTransducer {
    let cathegory_symbols = get_cathegory_symbols(alphabet);

    let mut cathegory_symbols_fst = HfstTransducer::new_type(TROPICAL_OPENFST_TYPE);
    for it in cathegory_symbols.iter() {
        let cathegory_symbol_fst = HfstTransducer::new_symbol(it, TROPICAL_OPENFST_TYPE);
        cathegory_symbols_fst.disjunct(&cathegory_symbol_fst, true);
    }

    cathegory_symbols_fst.minimize();

    let identity = HfstTransducer::new_symbol(internal_identity, TROPICAL_OPENFST_TYPE);

    let mut identity_star = HfstTransducer::new_copy(&identity);

    identity_star.repeat_star().minimize();

    let mut remover = HfstTransducer::new_copy(&identity_star);

    remover
        .concatenate(&cathegory_symbols_fst, true)
        .concatenate(&identity, true)
        .concatenate(&identity_star, true)
        .minimize();

    remover
}

// [spec:hfst:def:guessify-fst.rewrite-removed-symbols-fn]
// [spec:hfst:sem:guessify-fst.rewrite-removed-symbols-fn]
pub fn rewrite_removed_symbols(morphological_analyzer: &mut HfstTransducer, alphabet: &StringSet) {
    let mut substitution_pairs = HfstSymbolPairSubstitutions::new();

    substitution_pairs.insert(
        (internal_epsilon.to_string(), REMOVED_SYMBOL.to_string()),
        (internal_epsilon.to_string(), internal_epsilon.to_string()),
    );

    for it in alphabet.iter() {
        if it.as_str() != internal_epsilon {
            substitution_pairs.insert(
                (it.clone(), REMOVED_SYMBOL.to_string()),
                (it.clone(), it.clone()),
            );
        }
    }

    morphological_analyzer.substitute_symbol_pair_substitutions(&substitution_pairs);
}

// [spec:hfst:def:guessify-fst.guessify-analyzer-fn]
// [spec:hfst:sem:guessify-fst.guessify-analyzer-fn]
pub fn guessify_analyzer(
    mut morphological_analyzer: HfstTransducer,
    penalty: f32,
) -> HfstTransducer {
    // Convert to tropical openfst type so that all operations can be
    // performed.
    morphological_analyzer.convert(TROPICAL_OPENFST_TYPE, String::new());

    let morphological_analyzer_name = morphological_analyzer.get_name();

    // Start be reversing the morphological analyzer, since guessing is
    // based on suffixes of words.
    morphological_analyzer.reverse().minimize();

    // Get rid of flag diacritics. They're a nuissance and the
    // combinatorics would any way be screwed up because we modify the
    // behavior of the transducer using default-transitions.
    let alphabet = morphological_analyzer.get_alphabet();
    remove_flag_diacritics(&mut morphological_analyzer, &alphabet);

    morphological_analyzer.minimize();

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
        basic_guesser.add_transition(
            s,
            &HfstBasicTransition::new_symbols(
                sink_state,
                my_default().to_string(),
                my_default().to_string(),
                penalty,
            ),
            true,
        );
        s += 1;
    }

    // Add an a-transition to the sink state to all states where there
    // is only one transitions which is a default-transition, since some
    // version of hfst didn't suporrt default transitions in states
    // without any additional transitions.
    let mut s: HfstState = 0;
    while s <= basic_guesser.get_max_state() {
        if basic_guesser.index(s).len() == 1
            && basic_guesser.index(s)[0].get_input_symbol() == my_default()
        {
            basic_guesser.add_transition(
                s,
                &HfstBasicTransition::new_symbols(
                    sink_state,
                    "a".to_string(),
                    "a".to_string(),
                    penalty,
                ),
                true,
            );
        }
        s += 1;
    }

    let mut guesser =
        HfstTransducer::new_from_basic_transducer(&basic_guesser, TROPICAL_OPENFST_TYPE);

    let invalid_form_filterer = get_invalid_form_filterer(&alphabet);

    guesser.compose(&invalid_form_filterer, true).minimize();

    guesser.set_name(&format!("guessified({})", morphological_analyzer_name));

    guesser.set_property("reverse input", "true");

    guesser
}

// [spec:hfst:def:guessify-fst.store-guesser-fn]
// [spec:hfst:sem:guessify-fst.store-guesser-fn]
pub fn store_guesser(
    guesser: &mut HfstTransducer,
    out: &mut HfstOutputStream,
    compile_generator: bool,
) {
    let mut generator = HfstTransducer::new_type(TROPICAL_OPENFST_TYPE);
    if compile_generator {
        generator = HfstTransducer::new_copy(guesser);
    }

    guesser.substitute(my_default(), internal_default, true, true);
    guesser.convert(HFST_OLW_TYPE, String::new());
    out.operator_shl(guesser);

    if compile_generator {
        generator.invert();
        generator.set_name(&format!("inverted({})", guesser.get_name()));
        generator.substitute(my_default(), internal_default, true, true);
        generator.convert(HFST_OLW_TYPE, String::new());
        out.operator_shl(&mut generator);
    }
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
// that every state can reach by an identity arc. The result is converted to
// `format`. (The tool's mid-algorithm "-v" traces — "Creating guesser
// prefix...", "Rebuilding suffix...", "converting and saving..." — were
// diagnostic and are not reproduced here; the constructed transducer is
// unchanged. The owning tool keeps the [spec:hfst:*:hfst-affix-guessify
// .process-stream-fn] annotation on its stream-driver loop.)
pub fn affix_guessify(
    trans: &HfstTransducer,
    direction: GuessDirection,
    weight: f32,
    format: ImplementationType,
) -> HfstTransducer {
    let alpha = trans.get_alphabet();
    match direction {
        GuessDirection::GuessSuffix => {
            let mutt = HfstBasicTransducer::from_transducer(trans);
            let mut repl = HfstBasicTransducer::new();
            let guess_state = repl.add_state(0);
            let guess_arc = HfstBasicTransition::new_symbols(
                guess_state,
                internal_identity.to_string(),
                internal_identity.to_string(),
                weight,
            );
            repl.add_transition(guess_state, &guess_arc, true);
            for x in alpha.iter() {
                let x_arc =
                    HfstBasicTransition::new_symbols(guess_state, x.clone(), x.clone(), weight);
                repl.add_transition(guess_state, &x_arc, true);
            }
            for s in 0..=mutt.get_max_state() {
                let d = repl.add_state(s + 1);
                if mutt.is_final_state(s) {
                    let fw = mutt.get_final_weight(s);
                    repl.set_final_weight(d, &fw);
                }
                let guess_arc = HfstBasicTransition::new_symbols(
                    d,
                    internal_identity.to_string(),
                    internal_identity.to_string(),
                    weight,
                );
                repl.add_transition(guess_state, &guess_arc, true);
                for x in alpha.iter() {
                    let x_arc = HfstBasicTransition::new_symbols(d, x.clone(), x.clone(), weight);
                    repl.add_transition(guess_state, &x_arc, true);
                }
                for arc in mutt.transitions(s).iter() {
                    let newarc = HfstBasicTransition::new_symbols(
                        arc.get_target_state() + 1,
                        arc.get_input_symbol(),
                        arc.get_output_symbol(),
                        arc.get_weight(),
                    );
                    repl.add_transition(d, &newarc, true);
                }
            }
            HfstTransducer::new_from_basic(&repl, format)
        }
        GuessDirection::GuessPrefix => {
            let mut repl = HfstBasicTransducer::from_transducer(trans);
            let guess_state = repl.add_state_new();
            repl.set_final_weight(guess_state, &0.0f32);
            let guess_arc = HfstBasicTransition::new_symbols(
                guess_state,
                internal_identity.to_string(),
                internal_identity.to_string(),
                weight,
            );
            repl.add_transition(guess_state, &guess_arc, true);
            let max_state = repl.get_max_state();
            for s in 0..=max_state {
                let newarc = HfstBasicTransition::new_symbols(
                    guess_state,
                    internal_identity.to_string(),
                    internal_identity.to_string(),
                    weight,
                );
                repl.add_transition(s, &newarc, true);
            }
            HfstTransducer::new_from_basic(&repl, format)
        }
    }
}

// [spec:hfst:def:guessify-fst.main-fn]
// [spec:hfst:sem:guessify-fst.main-fn]
//
// The MAIN_TEST 'main' (the '#else // MAIN_TEST' branch) is a stdin/stdout
// self-test harness, not production code. Per the porting convention,
// MAIN_TEST sections are not emitted; the annotation is carried here as a
// comment only.
