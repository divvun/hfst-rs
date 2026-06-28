// Exercises the TWOLC two-level rule compiler: parse a small grammar via
// nfst-twolc and assemble the rule into a transducer (the rule hierarchy +
// conflict resolution over the facade).
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::twolc::TwolcCompiler;

const SRC: &str = "\
Alphabet
a:b c ;

Rules

\"a to b after c\"
a:b => c _ ;
";

fn main() {
    let mut c = TwolcCompiler::new(TROPICAL_OPENFST_TYPE);
    let t = c.compile(SRC).expect("twolc compile returned null");

    let states = t.number_of_states();
    assert!(states >= 1, "expected a non-empty rule transducer");

    println!("twolc OK: a:b => _ c: -> {states} states");
}
