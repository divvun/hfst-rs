use hfst::transducer::{
    Encoder, HeaderFlag, IStream, SymbolTable, TransducerHeader, TransitionIndex,
};

fn main() -> hfst::error::Result<()> {
    // Header binary round-trip: write to a buffer, read it back, compare.
    let h = TransducerHeader::new_sizes(2, 5, 7, 11, true);
    let mut buf: Vec<u8> = Vec::new();
    h.write(&mut buf);
    let mut cursor = &buf[..];
    let mut is = IStream::new(&mut cursor);
    let h2 = TransducerHeader::new_istream(&mut is)?;
    assert_eq!(h2.input_symbol_count(), 2);
    assert_eq!(h2.symbol_count(), 5);
    assert_eq!(h2.index_table_size(), 7);
    assert_eq!(h2.target_table_size(), 11);
    assert!(h2.probe_flag(HeaderFlag::Weighted));
    println!("header round-trip OK");

    // TransitionIndex finality semantics.
    let fin = TransitionIndex::create_final();
    assert!(fin.is_final());
    assert_eq!(fin.final_weight(), 0.0);
    let nonfin = TransitionIndex::new_values(3, 100);
    assert!(!nonfin.is_final());
    assert!(nonfin.matches(3));
    assert!(!nonfin.matches(4));
    println!("transition-index OK");

    // Encoder: ascii single chars are tokenized one byte at a time; multi-byte
    // symbols go through the letter trie.
    let mut st = SymbolTable::new();
    st.push("@_EPSILON_SYMBOL_@".to_string()); // 0
    st.push("a".to_string()); // 1
    st.push("bc".to_string()); // 2
    let enc = Encoder::new(&st, 3);

    let input = b"abc\0";
    let mut p = 0usize;
    let s1 = enc.find_key(input, &mut p);
    let s2 = enc.find_key(input, &mut p);
    assert_eq!(s1, Some(1), "first token should be 'a' -> 1");
    assert_eq!(s2, Some(2), "second token should be 'bc' -> 2");
    println!("encoder OK (a=1, bc=2)");
    Ok(())
}
