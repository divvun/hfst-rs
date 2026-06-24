// Round-trips a small transducer through prolog write + parse to exercise the
// sscanf-replicating parsers.
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;

fn main() {
    // state0 --a:b/0.5--> state1 (final, weight 0.3)
    let mut g = HfstBasicTransducer::new();
    g.name = "foo".to_string();
    g.add_transition(
        0,
        &HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.5),
        true,
    );
    g.set_final_weight(1, &0.3);

    let mut buf: Vec<u8> = Vec::new();
    g.write_in_prolog_format_os(&mut buf, "foo", true);
    let text = String::from_utf8(buf).unwrap();
    println!("--- prolog ---\n{}", text);

    let mut g2 = HfstBasicTransducer::new();
    let mut lines = text.lines();
    let net = lines.next().unwrap();
    assert!(
        HfstBasicTransducer::parse_prolog_network_line(net, &mut g2),
        "network line failed: {net}"
    );
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let ok = HfstBasicTransducer::parse_prolog_arc_line(line, &mut g2)
            || HfstBasicTransducer::parse_prolog_final_line(line, &mut g2)
            || HfstBasicTransducer::parse_prolog_symbol_line(line, &mut g2);
        assert!(ok, "line not parsed: {line}");
    }

    assert_eq!(g2.name, "foo");
    assert_eq!(g2.get_max_state(), 1);
    assert!(g2.is_final_state(1));
    assert!((g2.get_final_weight(1) - 0.3).abs() < 1e-6);
    let trs = g2.transitions(0);
    assert_eq!(trs.len(), 1);
    assert_eq!(trs[0].get_input_symbol(), "a");
    assert_eq!(trs[0].get_output_symbol(), "b");
    assert!((trs[0].get_weight() - 0.5).abs() < 1e-6);
    assert_eq!(trs[0].get_target_state(), 1);
    println!("prolog round-trip OK");
}
