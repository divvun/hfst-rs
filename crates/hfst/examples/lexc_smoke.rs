// Exercises the LEXC compiler: parse a small lexicon via nfst-lexc and assemble
// it into a transducer (the morphotax join over the facade + XRE).
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::lexc::LexcCompiler;

const SRC: &str = "\
Multichar_Symbols +Pl

LEXICON Root
dog     N ;
cat     N ;

LEXICON N
+Pl:s   # ;
        # ;
";

fn main() {
    let mut c = LexcCompiler::new(TROPICAL_OPENFST_TYPE);
    let p = c.compile(SRC);
    assert!(!p.is_null(), "lexc compile returned null");
    let t = unsafe { *Box::from_raw(p) };

    let states = t.number_of_states();
    assert!(states >= 1, "expected a non-empty lexicon transducer");

    println!("lexc OK: Root/N lexicon -> {states} states");
}
