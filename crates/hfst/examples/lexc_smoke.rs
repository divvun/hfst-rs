// Exercises the LEXC compiler: parse a small lexicon via nfst-lexc and assemble
// it into a transducer (the morphotax join over the facade + XRE).
use hfst::lexc::LexcCompiler;
use hfst_openfst::StdVectorFst;

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
    let mut c = LexcCompiler::<StdVectorFst>::new();
    let t = c.compile(SRC).expect("lexc compile returned null");

    let states = t.number_of_states();
    assert!(states >= 1, "expected a non-empty lexicon transducer");

    println!("lexc OK: Root/N lexicon -> {states} states");
}
