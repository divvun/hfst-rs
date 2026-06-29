// Exercises the PMATCH compiler: parse a small pmatch grammar via nfst-pmatch
// and assemble it into transducers (the PmatchObject evaluate() hierarchy over
// the facade + XRE). The compiler returns a map keyed by definition name; the
// main result is "TOP".
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::pmatch_compiler::PmatchCompiler;

const SRC: &str = "\
Define TOP [{cat} | {dog}] ;
";

fn main() {
    let mut c = PmatchCompiler::new(TROPICAL_OPENFST_TYPE);
    let defs = c.compile(SRC);
    assert!(!defs.is_empty(), "pmatch compile produced no transducers");

    let top = defs.get("TOP").expect("no TOP in pmatch result");
    let states = top.number_of_states();
    assert!(states >= 1, "expected a non-empty TOP transducer");

    println!("pmatch OK: Define TOP [cat|dog] -> {states} states");
}
