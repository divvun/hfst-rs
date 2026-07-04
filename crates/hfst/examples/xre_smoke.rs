// Exercises the XRE compiler: parse an XRE string via nfst-xre and evaluate it
// onto a real HfstTransducer over the tropical backend.
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

fn compile(c: &mut XreCompiler<StdVectorFst>, src: &str) -> HfstTransducer<StdVectorFst> {
    c.compile(src)
        .unwrap_or_else(|| panic!("compile returned null for {src:?}"))
}

fn main() {
    let mut c = XreCompiler::<StdVectorFst>::new();

    // a single symbol pair
    let ab = compile(&mut c, "a:b");
    assert_eq!(ab.get_type(), TROPICAL_OPENFST_TYPE);
    assert!(ab.number_of_states() >= 1);

    // union of two symbols  a | b
    let union = compile(&mut c, "a | b");
    assert!(union.number_of_states() >= 1);

    // concatenation + star  [a b]*
    let star = compile(&mut c, "[a b]*");
    assert!(star.number_of_states() >= 1);

    // a definition, then reference it
    assert!(c.define("V", "a | e | i | o | u"));
    assert!(c.is_definition("V"));

    println!(
        "xre OK: a:b={} union={} star={} defined(V)={}",
        ab.number_of_states(),
        union.number_of_states(),
        star.number_of_states(),
        c.is_definition("V")
    );
}
