// Round-trips a small transducer through AT&T write + add_att_line.
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;

fn main() -> hfst::error::Result<()> {
    let mut g = HfstBasicTransducer::new();
    let tr =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.5, g.coder_mut());
    g.add_transition(0, &tr, true);
    g.set_final_weight(1, &0.3);

    let mut buf: Vec<u8> = Vec::new();
    g.write_in_att_format_os(&mut buf, true);
    let text = String::from_utf8(buf).unwrap();
    println!("--- att ---\n{}", text);

    let mut g2 = HfstBasicTransducer::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        g2.add_att_line(line, "@_EPSILON_SYMBOL_@", false)?;
    }

    assert_eq!(g2.get_max_state(), 1);
    assert!(g2.is_final_state(1));
    assert!((g2.get_final_weight(1)? - 0.3).abs() < 1e-6);
    let trs = g2.transitions(0)?;
    assert_eq!(trs.len(), 1);
    assert_eq!(trs[0].get_input_symbol(g2.coder()), "a");
    assert_eq!(trs[0].get_output_symbol(g2.coder()), "b");
    assert!((trs[0].get_weight() - 0.5).abs() < 1e-6);
    assert_eq!(trs[0].get_target_state(), 1);
    println!("att round-trip (add_att_line) OK");

    // Also exercise the reader path (the getline read loop) via an in-memory
    // BufRead over the produced AT&T text.
    {
        let mut reader = std::io::Cursor::new(text.into_bytes());
        let mut lc: u32 = 0;
        let g3 = HfstBasicTransducer::read_in_att_format_file(
            &mut reader,
            "@_EPSILON_SYMBOL_@",
            &mut lc,
            false,
        )
        .expect("round-tripped AT&T text reads back as a valid transducer");

        assert_eq!(g3.get_max_state(), 1);
        assert!(g3.is_final_state(1));
        assert!((g3.get_final_weight(1)? - 0.3).abs() < 1e-6);
        let t = g3.transitions(0)?;
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].get_input_symbol(g3.coder()), "a");
        assert_eq!(t[0].get_output_symbol(g3.coder()), "b");
        assert!((t[0].get_weight() - 0.5).abs() < 1e-6);
        println!("att round-trip (read_in_att_format_file) OK");
    }
    Ok(())
}
