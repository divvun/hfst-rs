//! Why `make_naive_tokenizer` harmonizes with a plain optimized-lookup
//! conversion of the dictionary and NOT with the `harmonizer_alphabet` option.
//!
//! `harmonizer_alphabet` (see `convert_ol_transducer::get_states_and_symbols`)
//! reclassifies every alphabet symbol that appears in no transition as an
//! *input* symbol instead of an output-only one. That does three things at
//! once: the reclassified symbols are numbered in the input block (so the real
//! input symbols are renumbered upwards), the header's `input_symbol_count`
//! grows to cover the whole alphabet, and the packer therefore pads the index
//! table with that many blank entries.
//!
//! `write_archive` needs it because its harmonizer is a *transition-less*
//! alphabet holder: without the option every symbol would be output-only, and
//! the numbering it imposes on every archive member would carry no input
//! structure at all. `make_naive_tokenizer` is the opposite case — its
//! harmonizer is the dictionary itself, a real transducer whose transitions
//! already say which symbols are inputs. The only symbols the option would
//! reclassify there are the tokenizer's own specials (which the dictionary
//! copies into its alphabet a few lines earlier) and output-only tags. Passing
//! it would renumber the dictionary's letters far up the alphabet and inflate
//! the index table (first test), and would buy nothing: the tables encode the
//! same automaton either way (second test).
//!
//! The under-padding is harmless because a probe past the end of the index
//! table answers exactly like the blank entry a longer padding would have put
//! there — `TransducerTable::at` / `PmatchTransducer::index_at` return `None`,
//! and `get_transitions_from_state` skips the slot. Slots that a state really
//! owns are always in range: the packer sizes the table from each state's own
//! symbol offsets. Nor does the smaller count cost the pmatch runtime anything:
//! `PmatchContainer` builds its `Encoder` over `orig_symbol_count` — the whole
//! symbol table — not over `input_symbol_count`. And it could not be avoided
//! anyway: every plain `.hfstol` ever written pads for its input symbols only,
//! and `hfst tokenize` has to accept those files as dictionaries.

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch_tokenize::{
    TokenizeInputSettings, TokenizeSettings, make_naive_tokenizer, process_input_stream,
};
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

// The tropical transition-data symbol coding lives in process-global statics;
// cargo runs each #[test] as a parallel thread in one process, so construction
// is serialized through a shared lock, matching the house style elsewhere.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// The symbols `make_naive_tokenizer` copies into the dictionary's alphabet
/// before converting it: the tokenizer's specials and its latin-1 boundary
/// characters, none of which the dictionary has a transition on. Padded out
/// with output-only tags, the other population of symbols a real analyser
/// carries above its input alphabet.
fn alphabet_only_symbols() -> Vec<String> {
    let mut syms: Vec<String> = [
        " ",
        "\t",
        "\n",
        ".",
        ",",
        "!",
        "?",
        ";",
        ":",
        "@BOUNDARY@",
        "@PMATCH_ENTRY@",
        "@PMATCH_EXIT@",
        "@PMATCH_PASSTHROUGH@",
        "@X. _\t_\n_._,_!_?_;_:_@",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    syms.extend((0..200).map(|i| format!("+Tag{i}")));
    syms
}

/// cat / cats / dog / dogs as a trie, with `extra` in the alphabet but on no
/// transition — the shape `make_naive_tokenizer` hands to the conversion.
fn dictionary_net(extra: &[String]) -> HfstBasicTransducer {
    let mut basic = HfstBasicTransducer::new();
    let mut next: u32 = 1;
    for word in ["cat", "cats", "dog", "dogs"] {
        let mut src = 0u32;
        for ch in word.chars() {
            let label = ch.to_string();
            let dst = next;
            next += 1;
            let coder = basic.coder_mut();
            let tr = HfstBasicTransition::new_symbols(
                dst,
                label.as_str().into(),
                label.as_str().into(),
                0.0,
                coder,
            );
            basic.add_transition(src, &tr, true);
            src = dst;
        }
        basic.set_final_weight(src, &0.0);
    }
    for sym in extra {
        basic.add_symbol_to_alphabet(&sym.as_str().into());
    }
    basic
}

/// A tokenizer-shaped net: every arc is on a symbol the dictionary carries in
/// its alphabet only, so each one is numbered above the dictionary's
/// `input_symbol_count` and probes the index table past its padding.
fn tokenizer_net() -> HfstBasicTransducer {
    let mut basic = HfstBasicTransducer::new();
    let branches = [
        " ",
        ".",
        ",",
        "!",
        "@BOUNDARY@",
        "@PMATCH_ENTRY@",
        "@X. _\t_\n_._,_!_?_;_:_@",
        "+Tag199",
    ];
    for (i, sym) in branches.iter().enumerate() {
        let dst = 1 + i as u32;
        {
            let coder = basic.coder_mut();
            let tr =
                HfstBasicTransition::new_symbols(dst, (*sym).into(), (*sym).into(), 0.0, coder);
            basic.add_transition(0, &tr, true);
        }
        {
            let coder = basic.coder_mut();
            let tr =
                HfstBasicTransition::new_symbols(9, "+Tag0".into(), "+Tag1".into(), 0.0, coder);
            basic.add_transition(dst, &tr, true);
        }
    }
    basic.set_final_weight(9, &0.0);
    basic
}

fn to_ol(basic: &HfstBasicTransducer, options: &str) -> Transducer<WeightedTables> {
    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(basic, true, options, None)
        .expect("well within the optimized-lookup format limits")
}

/// The option is not free: it renumbers the dictionary's real input symbols
/// into the tail of the alphabet and pads for all of them, so the index table
/// grows by an order of magnitude for the same automaton.
#[test]
fn harmonizer_alphabet_renumbers_and_inflates_the_index_table() {
    let _guard = serialized();
    let basic = dictionary_net(&alphabet_only_symbols());
    let plain = to_ol(&basic, "");
    let padded = to_ol(&basic, "harmonizer_alphabet");

    // Same alphabet either way — only its partition into input / other differs.
    assert_eq!(
        plain.get_header().symbol_count(),
        padded.get_header().symbol_count(),
        "the option must not change which symbols exist"
    );
    // epsilon + the seven letters the transitions actually read.
    assert_eq!(
        plain.get_header().input_symbol_count(),
        8,
        "the dictionary's transitions define its input alphabet"
    );
    assert_eq!(
        padded.get_header().input_symbol_count(),
        padded.get_header().symbol_count(),
        "the option promotes every alphabet symbol to an input symbol"
    );
    assert!(
        padded.get_header().index_table_size() > 8 * plain.get_header().index_table_size(),
        "expected the promoted numbering to inflate the index table: {} vs {}",
        padded.get_header().index_table_size(),
        plain.get_header().index_table_size()
    );
}

/// And it buys nothing: harmonizing the tokenizer against either dictionary
/// backend yields tables that decode to the same automaton, even though the
/// plain one is padded for eight symbols while the tokenizer reads symbols
/// numbered in the hundreds.
#[test]
fn under_padded_harmonizer_keeps_every_transition() {
    let _guard = serialized();
    let dict = dictionary_net(&alphabet_only_symbols());
    let plain_dict = to_ol(&dict, "");
    let padded_dict = to_ol(&dict, "harmonizer_alphabet");

    let tokenizer = tokenizer_net();
    let harmonize = |harmonizer: &Transducer<WeightedTables>| {
        ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
            &tokenizer,
            true,
            "",
            Some(harmonizer),
        )
        .expect("well within the optimized-lookup format limits")
    };
    let against_plain = harmonize(&plain_dict);
    let against_padded = harmonize(&padded_dict);

    // The under-padding is real: the tokenizer inherits the dictionary's
    // input_symbol_count, so its index table is padded for the dictionary's
    // eight letters while its own arcs read symbols numbered in the hundreds.
    // Probes for those symbols run off the end of the table.
    let padding = against_plain.get_header().input_symbol_count();
    let highest_arc_symbol = against_plain
        .get_alphabet()
        .get_symbol_table()
        .iter()
        .position(|s| s == "+Tag199")
        .expect("the tokenizer's highest-numbered arc symbol");
    assert_eq!(padding, 8, "padding covers the dictionary's input symbols");
    assert!(
        highest_arc_symbol > padding as usize,
        "expected the tokenizer to read symbols past its padding: symbol {highest_arc_symbol} vs {padding} padded entries"
    );
    assert!(
        against_padded.get_header().index_table_size()
            > against_plain.get_header().index_table_size(),
        "expected the padded conversion to produce the longer table"
    );

    // Decoding walks the index table for every symbol in the alphabet, exactly
    // as the pmatch runtime does; if the short padding lost an arc, the two
    // reconstructions would differ.
    let decoded = |ol: &Transducer<WeightedTables>| {
        let basic = ConversionFunctions::hfst_ol_to_hfst_basic_transducer(ol);
        HfstTransducer::<StdVectorFst>::new_from_basic(&basic).expect("decodable")
    };
    let source = HfstTransducer::<StdVectorFst>::new_from_basic(&tokenizer).expect("decodable");
    assert!(
        decoded(&against_plain)
            .compare_default(&source)
            .expect("comparable"),
        "the under-padded table must decode back to every arc it was given"
    );
    assert!(
        decoded(&against_padded)
            .compare_default(&source)
            .expect("comparable"),
        "so must the padded one — the two paddings encode the same automaton"
    );
}

/// The end-to-end path this all serves: `hfst tokenize` handed a plain
/// optimized-lookup dictionary builds the naive tokenizer over exactly this
/// under-padded harmonizer and must tokenize with it, not crash on it.
#[test]
fn naive_tokenizer_tokenizes_with_a_plain_dictionary() {
    let _guard = serialized();
    let basic = dictionary_net(&[]);
    let mut dictionary = HfstTransducer::<StdVectorFst>::new_from_basic(&basic).expect("decodable");
    dictionary.set_name("dict");

    let mut container = make_naive_tokenizer(&mut dictionary).expect("naive tokenizer builds");
    container.set_single_codepoint_tokenization(!container.has_multichar_input_symbols());

    let settings = TokenizeSettings {
        verbose: false,
        ..TokenizeSettings::default()
    };
    let mut input = std::io::Cursor::new(b"dogs cats cat.\n".to_vec());
    let mut output: Vec<u8> = Vec::new();
    let mut msg: Vec<u8> = Vec::new();
    let rv = process_input_stream(
        &mut container,
        &mut input,
        &mut output,
        &mut msg,
        &settings,
        &TokenizeInputSettings::default(),
    );
    assert_eq!(rv, 0, "tokenization failed");
    assert_eq!(
        String::from_utf8(output).expect("utf-8 output"),
        "dogs\ncats\ncat\n\n",
        "expected each dictionary word to come out as its own token"
    );
}
