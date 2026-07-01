use hfst::transducer::{HeaderFlag, IStream, Transducer};

// Round-trip a constructed transducer through the real binary writer + reader.
fn roundtrip(weighted: bool) -> hfst::error::Result<()> {
    let t = Transducer::new_weighted(weighted);

    let mut buf: Vec<u8> = Vec::new();
    t.write(&mut buf);

    let mut cursor = &buf[..];
    let mut is = IStream::new(&mut cursor);
    let t2 = Transducer::new_istream(&mut is)?;

    let h1 = t.get_header();
    let h2 = t2.get_header();
    assert_eq!(h1.symbol_count(), h2.symbol_count());
    assert_eq!(h1.input_symbol_count(), h2.input_symbol_count());
    assert_eq!(h1.index_table_size(), h2.index_table_size());
    assert_eq!(h1.target_table_size(), h2.target_table_size());
    assert_eq!(
        h1.probe_flag(HeaderFlag::Weighted),
        h2.probe_flag(HeaderFlag::Weighted)
    );
    assert_eq!(h2.probe_flag(HeaderFlag::Weighted), weighted);

    // alphabet survived: epsilon symbol at index 0
    assert_eq!(t2.get_symbol_table()[0], "@_EPSILON_SYMBOL_@");
    assert_eq!(t.get_symbol_table().len(), t2.get_symbol_table().len());

    // the lone index-table entry is a final index in both
    assert!(t2.final_index(0));

    println!(
        "round-trip OK (weighted={}, {} bytes, index_size={})",
        weighted,
        buf.len(),
        h2.index_table_size()
    );
    Ok(())
}

fn main() -> hfst::error::Result<()> {
    roundtrip(false)?;
    roundtrip(true)?;
    Ok(())
}
