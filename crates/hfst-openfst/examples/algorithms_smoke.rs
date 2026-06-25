// Exercises the OpenFST-shaped algorithm wrappers over rustfst.
use hfst_openfst::algorithms::*;
use hfst_openfst::prelude::*;
use hfst_openfst::{StdTransition, StdVectorFst, TropicalWeight};

// A one-arc acceptor for `label`.
fn acc(label: u32) -> StdVectorFst {
    let mut f = StdVectorFst::new();
    let s0 = f.add_state();
    let s1 = f.add_state();
    f.set_start(s0).unwrap();
    f.set_final(s1, TropicalWeight::one()).unwrap();
    f.add_tr(
        s0,
        StdTransition::new(label, label, TropicalWeight::new(0.0), s1),
    )
    .unwrap();
    f
}

fn main() {
    let mut a = acc(1);
    // in-place algorithms run without panicking
    ArcSortInput(&mut a);
    ArcSortOutput(&mut a);
    RmEpsilon(&mut a);
    Connect(&mut a);
    TopSort(&mut a);

    let mut inv = a.clone();
    Invert(&mut inv);

    // determinize then minimize
    let mut det = StdVectorFst::new();
    Determinize(&a, &mut det);
    Minimize(&mut det);
    assert!(det.num_states() > 0);

    // reverse
    let mut rev = StdVectorFst::new();
    Reverse(&a, &mut rev);

    // compose acc(1) ∘ acc(1) -> accepts 1:1
    let b = acc(1);
    let mut comp = StdVectorFst::new();
    Compose(&a, &b, &mut comp);
    assert!(comp.num_states() > 0, "compose produced an empty fst");

    // concat / union are in-place on the left operand
    let mut cat = acc(1);
    Concat(&mut cat, &acc(2));
    let mut uni = acc(1);
    Union(&mut uni, &acc(2));

    // encode weights+labels, then decode round-trips
    let mut enc = a.clone();
    let table = Encode(&mut enc, EncodeType::EncodeWeightsAndLabels);
    Decode(&mut enc, table);

    println!(
        "algorithms OK: det={}, rev={}, compose={}, concat={}, union={}",
        det.num_states(),
        rev.num_states(),
        comp.num_states(),
        cat.num_states(),
        uni.num_states()
    );
}
