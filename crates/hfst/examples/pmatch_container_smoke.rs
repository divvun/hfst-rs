// Exercises PmatchContainer::new_from_hfst_transducers (single-transducer case):
// compile a pmatch grammar to its TOP transducer, build a runtime container from
// it in memory, and run a match.
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst_openfst::StdVectorFst;

const SRC: &str = "Define TOP [{cat} | {dog}] EndTag(animal) ;\n";

fn main() -> hfst::error::Result<()> {
    // Compile to the TOP transducer.
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let defs = compiler.compile(SRC)?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    // The pmatch runtime pins the weighted optimized-lookup instantiation;
    // the typed conversion replaces the old implicit convert(HFST_OLW_TYPE).
    let top_ol = top.to_ol(true, "")?;

    // Build a runtime container straight from the in-memory transducer.
    let mut container = PmatchContainer::new_from_hfst_transducers(vec![top_ol])?;

    // Match: the runtime tags recognised input. We assert it runs and that the
    // recognised word survives in the output.
    let out = container.do_match("cat", 0.0, 0.0);
    println!("pmatch container match(\"cat\") = {out:?}");
    assert!(
        out.contains("cat"),
        "expected 'cat' in match output, got {out:?}"
    );

    let out2 = container.do_match("dog", 0.0, 0.0);
    assert!(
        out2.contains("dog"),
        "expected 'dog' in match output, got {out2:?}"
    );

    println!("pmatch container (from in-memory transducer) OK");
    Ok(())
}
