use super::*;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_input_stream::HfstInputStream;
use crate::hfst_output_stream::HfstOutputStream;
use crate::hfst_transducer::{AnyTransducer, HfstTransducer};
use std::collections::BTreeSet;

// Write a foma transducer through the REAL HfstOutputStream facade (the
// FOMA_TYPE arm: operator<< writes the "FOMA" header + Backend::write
// payload), then read it back via HfstInputStream — a true facade
// round-trip, no hand-assembled framing.
// [spec:hfst:sem:foma-backend.stream-io/test]
#[test]
fn write_through_hfst_output_stream_round_trip() -> crate::error::Result<()> {
    let ab = FomaTransducer::define_transducer_symbol_pair("a", "b");
    let cd = FomaTransducer::define_transducer_symbol_pair("c", "d");
    let original = ab.disjunct(&cd)?;
    let states1 = original.to_basic().unwrap().states().len();

    let mut tr = HfstTransducer::wrap(original);
    let path = std::env::temp_dir().join(format!(
        "hfst_foma_facade_{}_{}.hfst",
        std::process::id(),
        line!()
    ));
    {
        let mut out = HfstOutputStream::new_filename(
            path.to_str().unwrap(),
            ImplementationType::FOMA_TYPE,
            true,
        )
        .expect("HfstOutputStream(FOMA_TYPE)");
        out.write(&mut tr).expect("write foma transducer <<");
        out.close();
    }

    let mut instream = HfstInputStream::new_filename(path.to_str().unwrap())
        .expect("HfstInputStream over facade-written foma");
    let any = instream.read().expect("read foma transducer back");
    instream.close();
    let _ = std::fs::remove_file(&path);

    match any {
        AnyTransducer::Foma(t) => {
            let states2 = t.to_basic().unwrap().states().len();
            assert_eq!(states1, states2, "state count survives facade round-trip");
        }
        other @ (AnyTransducer::Tropical(_)
        | AnyTransducer::OlW(_)
        | AnyTransducer::OlU(_)
        | AnyTransducer::Thfst(_)) => {
            panic!("expected AnyTransducer::Foma, got {:?}", other.get_type())
        }
    }
    Ok(())
}

/// Reduce a basic transducer to a value that captures its recognized
/// relation and alphabet: (state count, final states, alphabet, arcs).
fn snapshot(net: &HfstBasicTransducer) -> FomaSnapshot {
    let coder = net.coder();
    let n_states = (net.get_max_state() + 1) as usize;
    let mut finals = BTreeSet::new();
    let mut arcs = BTreeSet::new();
    for (s, transitions) in net.states_and_transitions().iter().enumerate() {
        let s = s as u32;
        if net.is_final_state(s) {
            finals.insert(s);
        }
        for tr in transitions.iter() {
            arcs.insert((
                s,
                tr.get_input_symbol(coder).to_string(),
                tr.get_output_symbol(coder).to_string(),
                tr.get_target_state(),
            ));
        }
    }
    let alphabet = net
        .get_alphabet()
        .iter()
        .map(|s| s.to_string())
        .collect::<BTreeSet<String>>();
    (n_states, finals, alphabet, arcs)
}

/// Build the foma net for the relation a:b (state 0 -a:b-> state 1 final)
/// via foma's construction API.
fn build_ab() -> FomaTransducer {
    let mut handle = foma::dynarray::fsm_construct_init("ab");
    foma::dynarray::fsm_construct_set_initial(&mut handle, 0);
    foma::dynarray::fsm_construct_add_arc(&mut handle, 0, 1, "a", "b");
    foma::dynarray::fsm_construct_set_final(&mut handle, 1);
    FomaTransducer::wrap(foma::dynarray::fsm_construct_done(handle))
}

// [spec:hfst:sem:foma-backend.to-basic-fn/test]
// [spec:hfst:sem:foma-backend.from-basic-fn/test]
#[test]
fn round_trip_preserves_relation_and_alphabet() {
    let foma = build_ab();

    // foma -> basic
    let basic1 = foma.to_basic().expect("to_basic");
    // basic -> foma -> basic
    let foma2 = FomaTransducer::from_basic(&basic1).expect("from_basic");
    let basic2 = foma2.to_basic().expect("to_basic (round-trip)");

    let s1 = snapshot(&basic1);
    let s2 = snapshot(&basic2);

    // The round trip is stable across states, finals, alphabet and arcs.
    assert_eq!(s1, s2, "round-trip to_basic∘from_basic must be stable");

    // And the concrete shape is what we built.
    assert_eq!(s1.0, 2, "two states");
    assert_eq!(s1.1, BTreeSet::from([1u32]), "state 1 is final");
    assert!(s1.2.contains("a"), "alphabet has a");
    assert!(s1.2.contains("b"), "alphabet has b");
    assert!(
        s1.3.contains(&(0u32, "a".to_string(), "b".to_string(), 1u32)),
        "arc 0 -a:b-> 1 recognized"
    );
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn algebra_union_determinize_minimize_equivalence() -> crate::error::Result<()> {
    // Two symbol acceptors and their union {a, b}.
    let a = FomaTransducer::define_transducer_symbol("a");
    let b = FomaTransducer::define_transducer_symbol("b");
    let u = a.disjunct(&b)?;

    // Determinize + minimize the union (the very ops foma exists to run
    // unweighted); the recognized alphabet still carries a and b.
    let d = u.determinize(false)?.minimize(false)?;
    let basic = d.to_basic().expect("to_basic after determinize/minimize");
    let alphabet = snapshot(&basic).2;
    assert!(alphabet.contains("a"), "alphabet retains a");
    assert!(alphabet.contains("b"), "alphabet retains b");

    // Union is commutative up to equivalence, and {a,b} != {a}.
    let u_rev = b.disjunct(&a)?;
    assert!(
        d.are_equivalent(&u_rev, false)?,
        "det/min union equivalent to reverse-order union"
    );
    assert!(
        !d.are_equivalent(&a, false)?,
        "union {{a,b}} is not equivalent to {{a}}"
    );

    // Lookup confirms the recognized language: a -> a, b -> b, c -> nothing.
    let mut d = d;
    let out_a: Vec<String> = d
        .lookup_fd_str("a", -1, 0.0)
        .iter()
        .map(|p| p.second.iter().map(|s| s.as_str()).collect::<String>())
        .collect();
    assert_eq!(out_a, vec!["a".to_string()]);
    assert!(d.lookup_fd_str("c", -1, 0.0).is_empty(), "c not accepted");
    Ok(())
}

// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn lookup_ab_yields_b() {
    let mut t = build_ab();

    // Applying the input "a" through a:b yields exactly the output "b".
    let paths = t.lookup_fd_str("a", -1, 0.0);
    let outputs: Vec<String> = paths
        .iter()
        .map(|p| p.second.iter().map(|s| s.as_str()).collect::<String>())
        .collect();
    assert_eq!(outputs, vec!["b".to_string()], "a -> b");

    // "b" is not in the input language of a:b.
    assert!(t.lookup_fd_str("b", -1, 0.0).is_empty(), "b not an input");

    // The two-level view pairs the whole input with the whole output.
    let pairs = t.lookup_fd_pairs_str("a", -1, 0.0);
    assert_eq!(pairs.len(), 1);
    let p = pairs.iter().next().unwrap();
    assert_eq!(p.second.len(), 1);
    assert_eq!(p.second[0].0.as_str(), "a");
    assert_eq!(p.second[0].1.as_str(), "b");
}
