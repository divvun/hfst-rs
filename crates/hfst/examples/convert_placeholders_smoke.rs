use hfst::convert::{FlagSymbolSet, IndexPlaceholders, StatePlaceholder, TransitionPlaceholder};

fn main() -> hfst::error::Result<()> {
    let flags = FlagSymbolSet::new();

    // State 0 is always nonsimple; a later state starts empty (=> simple).
    let s0 = StatePlaceholder::new(0, false, 0, 0.0);
    assert!(!s0.is_simple());
    let mut s = StatePlaceholder::new(1, false, 5, 0.0);
    assert!(s.is_simple());

    // One non-zero non-flag input => simple_nonzero_index (still simple).
    s.add_input(3, &flags);
    assert_eq!(s.inputs, 1);
    assert!(s.is_simple());
    assert!(s.input_present(3));
    assert!(!s.input_present(2));

    // Adding epsilon makes it nonsimple.
    s.add_input(0, &flags);
    assert!(!s.is_simple());

    // Transitions land in the right per-symbol slot.
    s.add_transition(TransitionPlaceholder::new(7, 3, 9, 0.5));
    assert_eq!(s.number_of_transitions(), 1);
    let tps = s.get_transition_placeholders(3);
    assert_eq!(tps.len(), 1);
    assert_eq!(tps[0].target, 7);
    println!("state-placeholder index-type + transitions OK");

    // symbol_offset: epsilons are written first, so a non-epsilon symbol's
    // offset equals the epsilon transition count before it.
    let mut s2 = StatePlaceholder::new(1, false, 0, 0.0);
    s2.add_input(0, &flags);
    s2.add_input(3, &flags);
    s2.add_transition(TransitionPlaceholder::new(1, 0, 0, 0.0)); // epsilon
    s2.add_transition(TransitionPlaceholder::new(2, 3, 3, 0.0)); // symbol 3
    assert_eq!(s2.symbol_offset(0, &flags)?, 0);
    assert_eq!(s2.symbol_offset(3, &flags)?, 1);
    println!("symbol_offset OK");

    // IndexPlaceholders: sparse assignment with NO_TABLE_INDEX gaps.
    let mut ip = IndexPlaceholders::new();
    assert!(!ip.used(0));
    ip.assign(2, 100, 5);
    assert!(ip.used(2));
    assert_eq!(ip.get_target(2), (100, 5));
    assert!(!ip.used(0));
    assert!(!ip.used(1));
    // an occupied position is unsuitable
    assert!(ip.unsuitable(2, 4, 1.0));
    println!("index-placeholders OK");
    Ok(())
}
