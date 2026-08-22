//! The root optimization a compiled XRE expression owes the grammar's
//! `REGEXP2` reduction.
//!
//! A bracketed root reaches the compiler boundary already carrying the
//! bracket's own `optimize()`, which is tempting to read as "already
//! optimized". It is not the same optimization: `[ ... ]` is a `REGEXP11`
//! production and the `REGEXP2` one still follows it. Under `encode_weights`
//! optimization is not idempotent — `Minimize` partitions a weight-encoded
//! machine against the weight distribution it pushed toward the initial state,
//! not the machine's own — so dropping the second one leaves surplus states
//! behind. Upstream C++ HFST 3.17.1 is the oracle for the shape asserted here.

use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

// Two weighted alternations sharing the prefix "a" and the branch "ab". After
// one weight-encoded optimization the "b", "c" and "d" branches still sit on
// two distinct final states that carry the same weight and no outgoing arcs;
// the second one merges them. Chosen because the surplus shows up as a state
// count rather than an arc count, so it survives any arc ordering.
const EXPRESSION: &str = "[ [{ab}|{ac}|{ad}]::1 | [{ab}|{ae}]::2 ]";

// The same alternations without the brackets. Denotes the identical relation
// and reaches the boundary through the plain union path, so it exercises the
// arm that was never in question.
const UNBRACKETED: &str = "[{ab}|{ac}|{ad}]::1 | [{ab}|{ae}]::2";

fn compile_encoded(expression: &str) -> HfstTransducer<StdVectorFst> {
    let mut compiler = XreCompiler::<StdVectorFst>::new();
    compiler.set_encode_weights(true);
    compiler
        .compile(expression)
        .unwrap_or_else(|| panic!("XRE compilation of {expression:?} failed"))
}

// [spec:hfst:req:xre-finalization.root-optimize/test]
#[test]
fn bracketed_root_is_optimized_at_the_compiler_boundary() {
    let bracketed = compile_encoded(EXPRESSION);

    // The C++ oracle's shape: start, the shared "a" state, and one final state
    // per surviving weight. Skipping the boundary optimization yields five.
    assert_eq!(
        bracketed.number_of_states(),
        4,
        "bracketed root kept a surplus state, so the REGEXP2 optimization was skipped"
    );
}

// [spec:hfst:req:xre-finalization.root-optimize/test]
#[test]
fn boundary_optimization_reaches_the_fixed_point() {
    let mut bracketed = compile_encoded(EXPRESSION);
    let states = bracketed.number_of_states();
    let arcs = bracketed.number_of_arcs();

    let config = hfst::hfst_transducer::EngineConfig {
        encode_weights: true,
        ..hfst::hfst_transducer::EngineConfig::default()
    };
    bracketed
        .optimize_with_config(&config)
        .expect("re-optimize the compiled root");

    // A third pass must find nothing left to merge; if it does, the boundary
    // optimization is still one short rather than merely differently ordered.
    assert_eq!(
        (bracketed.number_of_states(), bracketed.number_of_arcs()),
        (states, arcs)
    );
}

// [spec:hfst:req:xre-finalization.root-optimize/test]
#[test]
fn the_extra_optimization_preserves_the_relation() {
    let bracketed = compile_encoded(EXPRESSION);
    let unbracketed = compile_encoded(UNBRACKETED);

    // Optimization is a representation choice: the surplus states the bracketed
    // root used to keep never changed which strings mapped to which weights.
    assert!(
        bracketed
            .compare(&unbracketed, true)
            .expect("compare bracketed against unbracketed"),
        "the boundary optimization changed the relation, not just its representation"
    );
}
