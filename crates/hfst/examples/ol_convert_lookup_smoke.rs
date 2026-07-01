use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;

fn main() -> hfst::error::Result<()> {
    // Build a:b / 0.5 with a final state weighted 0.3.
    let mut basic = HfstBasicTransducer::new();
    let tr = HfstBasicTransition::new_symbols(
        1,
        "a".to_string(),
        "b".to_string(),
        0.5,
        basic.coder_mut(),
    );
    basic.add_transition(0, &tr, true);
    basic.set_final_weight(1, &0.3);

    // Convert to the optimized-lookup format and look up "a".
    let mut ol = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(&basic, true, "", None)?;

    let results = ol.lookup_fd_str("a", -1, 0.0);
    assert_eq!(results.len(), 1, "expected exactly one analysis");
    let r = results.iter().next().unwrap();
    assert_eq!(r.second, vec!["b".to_string()], "output should be b");
    assert!(
        (r.first - 0.8).abs() < 1e-5,
        "weight was {} (expected 0.5 + 0.3)",
        r.first
    );
    println!("OL lookup OK (a -> b, weight {})", r.first);

    // Unknown input yields nothing.
    let none = ol.lookup_fd_str("x", -1, 0.0);
    assert!(none.is_empty(), "unknown input must give no analyses");
    println!("OL lookup of unknown input OK");

    // Round-trip the OL transducer back to a HfstBasicTransducer.
    let basic2 = ConversionFunctions::hfst_ol_to_hfst_basic_transducer(&ol);
    let t0 = basic2.transitions(0)?;
    assert_eq!(
        t0.len(),
        1,
        "round-tripped state 0 should have one transition"
    );
    assert_eq!(t0[0].get_input_symbol(basic2.coder()), "a");
    assert_eq!(t0[0].get_output_symbol(basic2.coder()), "b");
    assert!(basic2.is_final_state(t0[0].get_target_state()));
    println!("OL -> basic round-trip OK");
    Ok(())
}
