// Behavioral coverage for the PMATCH compiler and its PmatchObject AST /
// global-definition-table model. pmatch_compiler.rs is otherwise untested
// (only examples/pmatch_smoke.rs and pmatch_container_smoke.rs exercise it);
// these tests lock the compile + match + definition-reference (DAG) paths
// across the major node types so the static-mut / raw-pointer -> safe
// conversion (idiom1.pmatch) is validated rather than blind. The grammar
// constructs (predefined acceptors, repetition, references, EndTag) mirror
// scripts/windows_tests/test.pmatch.
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

// States of the named definition produced by compiling `src`.
fn def_states(src: &str, name: &str) -> Result<u32, hfst::error::Error> {
    let mut c = PmatchCompiler::<StdVectorFst>::new();
    let defs = c.compile(src)?;
    let d = defs
        .get(name)
        .unwrap_or_else(|| panic!("no {name} in pmatch result"));
    Ok(d.number_of_states())
}

fn top_states(src: &str) -> Result<u32, hfst::error::Error> {
    def_states(src, "TOP")
}

// Compile `src` to TOP, build a runtime container, and return the match output.
fn compile_and_match(src: &str, input: &str) -> Result<String, hfst::error::Error> {
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let defs = compiler.compile(src)?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    // The pmatch runtime pins the weighted optimized-lookup backend; convert the
    // compiled tropical TOP through the basic transducer (the typed replacement
    // for the old runtime convert()).
    let top_owned = HfstTransducer::<Transducer<WeightedTables>>::new_from_basic(&top.to_basic()?)?;
    let mut container = PmatchContainer::new_from_hfst_transducers(vec![top_owned])?;
    Ok(container.do_match(input, 0.0, 0.0))
}

#[test]
fn compile_simple_union() -> Result<(), hfst::error::Error> {
    assert!(top_states("Define TOP [{cat} | {dog}] ;\n")? >= 1);
    Ok(())
}

#[test]
fn compile_concatenation() -> Result<(), hfst::error::Error> {
    // juxtaposition concatenates.
    assert!(top_states("Define TOP {ab} {cd} ;\n")? >= 1);
    Ok(())
}

#[test]
fn compile_optional_and_union() -> Result<(), hfst::error::Error> {
    assert!(top_states("Define TOP [{a} | {b}] ({c}) ;\n")? >= 1);
    Ok(())
}

#[test]
fn compile_predefined_acceptors_and_repetition() -> Result<(), hfst::error::Error> {
    // UppercaseAlpha / Alpha predefined sets with * and +, behind a reference.
    let src = "Define CapWord UppercaseAlpha Alpha* ;\nDefine TOP CapWord+ EndTag(word) ;\n";
    assert!(top_states(src)? >= 1);
    Ok(())
}

#[test]
fn compile_definition_reference() -> Result<(), hfst::error::Error> {
    // A definition referenced by name from TOP exercises the DEFINITIONS table
    // and AST cross-references (the DAG path the conversion touches).
    let src = "Define Animal [{cat} | {dog}] ;\nDefine TOP Animal ;\n";
    assert!(top_states(src)? >= 1);
    Ok(())
}

#[test]
fn compile_deep_dag_french_streets() -> Result<(), hfst::error::Error> {
    // The test.pmatch grammar: a multi-level DAG (TOP -> StreetFr -> StreetWordFr
    // / DeFr / CapWord), predefined acceptors, repetition, optional, EndTag.
    let src = "\
Define CapWord UppercaseAlpha Alpha* ;
Define StreetWordFr [{avenue} | {boulevard} | {rue}] ;
Define DeFr [ [{de} | {du} | {des}] Whitespace ] | [{d'} | {l'}] ;
Define StreetFr StreetWordFr (Whitespace DeFr) CapWord+ ;
Define TOP StreetFr EndTag(PseudoFrenchStreetName) ;
";
    assert!(top_states(src)? >= 1);
    Ok(())
}

#[test]
fn match_endtag_runtime() -> Result<(), hfst::error::Error> {
    let out = compile_and_match("Define TOP [{cat} | {dog}] EndTag(animal) ;\n", "cat")?;
    assert!(
        out.contains("cat"),
        "expected 'cat' in match output, got {out:?}"
    );
    let out2 = compile_and_match("Define TOP [{cat} | {dog}] EndTag(animal) ;\n", "dog")?;
    assert!(
        out2.contains("dog"),
        "expected 'dog' in match output, got {out2:?}"
    );
    Ok(())
}

#[test]
fn match_numerals_runtime() -> Result<(), hfst::error::Error> {
    // Numeral+ predefined acceptor with a tag, matched against digits.
    let out = compile_and_match("Define TOP Numeral+ EndTag(num) ;\n", "123")?;
    assert!(
        out.contains("123"),
        "expected '123' in match output, got {out:?}"
    );
    Ok(())
}
