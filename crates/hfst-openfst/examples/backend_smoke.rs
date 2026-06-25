// Proves the rustfst backend is wired: build a small tropical FST, run a couple
// of algorithms, check the results.
use hfst_openfst::prelude::*;
use hfst_openfst::{StdTransition, StdVectorFst, TropicalWeight};

fn main() {
    let mut fst = StdVectorFst::new();
    let s0 = fst.add_state();
    let s1 = fst.add_state();
    let s2 = fst.add_state();
    fst.set_start(s0).unwrap();
    fst.set_final(s2, TropicalWeight::new(0.3)).unwrap();
    // s0 -a:a/0.5-> s1 -b:b/0.2-> s2 ; s1 is dead-end-free, s_unreached is not.
    fst.add_tr(s0, StdTransition::new(1, 1, TropicalWeight::new(0.5), s1))
        .unwrap();
    fst.add_tr(s1, StdTransition::new(2, 2, TropicalWeight::new(0.2), s2))
        .unwrap();
    // An unreachable state to prove connect() prunes it.
    let dead = fst.add_state();
    fst.add_tr(dead, StdTransition::new(3, 3, TropicalWeight::new(1.0), s2))
        .unwrap();
    assert_eq!(fst.num_states(), 4);

    connect(&mut fst).unwrap();
    assert_eq!(
        fst.num_states(),
        3,
        "connect should drop the unreachable state"
    );

    // Shortest distance from the start: the only accepting path is a b, total
    // weight 0.5 + 0.2 + 0.3(final) = 1.0.
    let sp: StdVectorFst = shortest_path(&fst).unwrap();
    let mut total = TropicalWeight::zero();
    for path in sp.paths_iter() {
        total = path.weight;
    }
    println!(
        "backend OK: states after connect = {}, shortest-path weight = {:?}",
        fst.num_states(),
        total
    );
    assert!((total.value() - 1.0).abs() < 1e-5, "weight was {:?}", total);
    println!("rustfst backend wired ✓");
}
