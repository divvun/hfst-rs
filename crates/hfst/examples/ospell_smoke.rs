use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::ospell::Speller;

fn main() {
    // A tiny "lexicon" accepting exactly the identity path c:c a:a t:t -> "cat".
    let mut basic = HfstBasicTransducer::new();
    let tr = HfstBasicTransition::new_symbols(
        1,
        "c".to_string(),
        "c".to_string(),
        0.0,
        basic.coder_mut(),
    );
    basic.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        2,
        "a".to_string(),
        "a".to_string(),
        0.0,
        basic.coder_mut(),
    );
    basic.add_transition(1, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        3,
        "t".to_string(),
        "t".to_string(),
        0.0,
        basic.coder_mut(),
    );
    basic.add_transition(2, &tr, true);
    basic.set_final_weight(3, &0.0);

    let ol = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(&basic, true, "", None);

    // Use the identity transducer as both mutator and lexicon.
    let mut speller = Speller::new(&ol, &ol);

    // check(): is the word in the lexicon?
    assert!(speller.check("cat"), "cat should be in the lexicon");
    assert!(!speller.check("dog"), "dog should not be in the lexicon");
    assert!(!speller.check("ca"), "ca is a prefix, not final");
    println!("Speller::check OK");

    // correct(): with an identity mutator the only correction of "cat" is "cat".
    let cq = speller.correct("cat");
    assert_eq!(cq.size(), 1, "expected exactly one correction");
    let top = cq.top();
    assert_eq!(top.0, "cat");
    assert!((top.1 - 0.0).abs() < 1e-6, "weight was {}", top.1);
    println!("Speller::correct OK (cat -> {} @ {})", top.0, top.1);
}
