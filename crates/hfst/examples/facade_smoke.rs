// Exercises the HfstTransducer facade construction + union dispatch over the
// tropical backend. Operations that round-trip through HfstBasicTransducer
// (determinize/minimize/compose) are NOT exercised here: they hit the known
// rustfst SymbolTable explicit-label gap at runtime (flagged for the fork).
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::hfst_transducer::HfstTransducer;

fn main() {
    // construct a:b via the facade (dispatches to TropicalWeightTransducer)
    let ab = HfstTransducer::new_symbol_pair("a", "b", TROPICAL_OPENFST_TYPE);
    assert_eq!(ab.get_type(), TROPICAL_OPENFST_TYPE);
    let states = ab.number_of_states();
    assert!(states >= 1);

    // empty + epsilon constructors dispatch through the union
    let empty = HfstTransducer::new_type(TROPICAL_OPENFST_TYPE);
    assert_eq!(empty.get_type(), TROPICAL_OPENFST_TYPE);

    // Clone == the C++ copy constructor (deep-copies the backend pointer)
    let copy = ab.clone();
    assert_eq!(copy.number_of_states(), states);
    assert_eq!(copy.get_type(), TROPICAL_OPENFST_TYPE);

    println!(
        "facade OK: type={:?} ab_states={} copy_states={}",
        ab.get_type(),
        states,
        copy.number_of_states()
    );
}
