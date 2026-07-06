// Exercises the HfstTransducer facade dispatch over the tropical backend,
// including operations that round-trip through HfstBasicTransducer
// (determinize/minimize/compose) — these exercise the rustfst SymbolTable
// explicit-label support added to the fork.
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::hfst_transducer::HfstTransducer;
use hfst_openfst::StdVectorFst;

fn main() -> hfst::error::Result<()> {
    // construct a:b via the facade (monomorphized over the tropical backend)
    let mut ab = HfstTransducer::<StdVectorFst>::new_symbol_pair("a", "b")?;
    assert_eq!(ab.get_type(), TROPICAL_OPENFST_TYPE);
    assert!(ab.number_of_states() >= 1);

    // unary ops route through the apply() union dispatch + a basic round-trip
    ab.determinize()?.minimize()?;
    let states = ab.number_of_states();
    assert!(states >= 1);

    // Clone (the C++ copy constructor) + a binary op through apply_another
    let other = HfstTransducer::<StdVectorFst>::new_symbol_pair("b", "c")?;
    let mut comp = ab.clone();
    comp.compose(&other, true)?;

    // operator<<(ostream, HfstTransducer): write AT&T format to a buffer
    let mut buf: Vec<u8> = Vec::new();
    hfst::hfst_transducer::write_to(&mut buf, &ab);
    let att = String::from_utf8(buf).unwrap();
    assert!(
        att.contains('\t'),
        "AT&T output should be tab-separated, got: {att:?}"
    );

    println!(
        "facade OK: type={:?} ab_states={} compose_states={} att_lines={}",
        ab.get_type(),
        states,
        comp.number_of_states(),
        att.lines().count()
    );
    Ok(())
}
