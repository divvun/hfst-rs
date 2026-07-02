// Round-trip a constructed HfstTransducer through the real binary HFST
// writer (HfstOutputStream) and the newly-implemented binary reader
// (HfstInputStream + HfstTransducer::new_from_stream).

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;

fn roundtrip(ty: ImplementationType, label: &str) -> hfst::error::Result<()> {
    let path = std::env::temp_dir().join(format!("hfst_bin_roundtrip{label}.hfst"));
    let path = path
        .to_str()
        .expect("temp_dir path is valid UTF-8 on this platform")
        .to_string();

    // Build [a:b] of the requested type and write it to a binary HFST file.
    let mut t = HfstTransducer::new_symbol_pair("a", "b", ty)?;
    t.set_name("ab");
    {
        let mut out = HfstOutputStream::new_filename(&path, ty, true)?;
        out.redirect(&mut t)?;
        out.close();
    }

    // Read it back through the binary reader.
    let mut input = HfstInputStream::new_filename(&path)?;
    assert_eq!(input.get_type(), ty, "{label}: type survived the header");
    assert!(
        input.is_hfst_header_included(),
        "{label}: writer emits an HFST header"
    );
    assert!(!input.is_eof(), "{label}: not at eof before the first read");

    let t2 = HfstTransducer::new_from_stream(&mut input)?;
    assert_eq!(t2.get_type(), ty, "{label}: read type");
    assert_eq!(t2.get_name(), "ab", "{label}: name survived");

    // After one transducer the single-transducer stream is exhausted.
    assert!(input.is_eof(), "{label}: at eof after the only transducer");
    input.close();

    assert!(
        t.compare(&t2, false)?,
        "{label}: read transducer equals written"
    );

    let _ = std::fs::remove_file(&path);
    println!("{label} binary round-trip OK");
    Ok(())
}

// Full binary round-trip through the real HFST-OL output stream: build an
// acceptor, convert it to optimized-lookup form, write it with the now-wired
// HfstOlOutputStream behind HfstOutputStream, then read it back.
fn roundtrip_hfst_ol(weighted: bool, label: &str) -> hfst::error::Result<()> {
    let ty = if weighted {
        ImplementationType::HFST_OLW_TYPE
    } else {
        ImplementationType::HFST_OL_TYPE
    };

    let mut t = HfstTransducer::new_symbol("a", ImplementationType::TROPICAL_OPENFST_TYPE)?;
    t.convert(ty, String::new())?;
    t.set_name("ol_ab");

    let path = std::env::temp_dir().join(format!("hfst_bin_roundtrip{label}.hfst"));
    let path = path
        .to_str()
        .expect("temp_dir path is valid UTF-8 on this platform")
        .to_string();
    {
        let mut out = HfstOutputStream::new_filename(&path, ty, true)?;
        out.redirect(&mut t)?;
        out.close();
    }

    let mut input = HfstInputStream::new_filename(&path)?;
    assert_eq!(input.get_type(), ty, "{label}: OL type from header");
    assert!(
        input.is_hfst_header_included(),
        "{label}: OL header written"
    );
    let t2 = HfstTransducer::new_from_stream(&mut input)?;
    assert_eq!(t2.get_type(), ty, "{label}: read OL type");
    assert!(input.is_eof(), "{label}: at eof after the only transducer");
    input.close();

    let _ = std::fs::remove_file(&path);
    println!("{label} HFST-OL write+read round-trip OK");
    Ok(())
}

// Exercise the facade AT&T text reader HfstTransducer::read_in_att_format*.
fn read_att_facade() -> hfst::error::Result<()> {
    let path = std::env::temp_dir().join("hfst_bin_roundtrip_att.att");
    let path = path
        .to_str()
        .expect("temp_dir path is valid UTF-8 on this platform")
        .to_string();
    // [a:b] with a single final state.
    std::fs::write(&path, "0\t1\ta\tb\n1\n").unwrap();

    let t = HfstTransducer::read_in_att_format_filename(
        &path,
        ImplementationType::TROPICAL_OPENFST_TYPE,
        "@_EPSILON_SYMBOL_@",
        false,
    )
    .expect("written [a:b] AT&T file reads back as a valid transducer");
    assert_eq!(t.get_type(), ImplementationType::TROPICAL_OPENFST_TYPE);

    let expected =
        HfstTransducer::new_symbol_pair("a", "b", ImplementationType::TROPICAL_OPENFST_TYPE)?;
    assert!(t.compare(&expected, false)?, "att facade: [a:b] read back");

    // C++ returns a heap 'HfstTransducer&' the caller deletes; mirror with Box.
    drop(unsafe { Box::from_raw(t as *mut HfstTransducer) });
    let _ = std::fs::remove_file(&path);
    println!("att facade read OK");
    Ok(())
}

fn main() -> hfst::error::Result<()> {
    roundtrip(ImplementationType::TROPICAL_OPENFST_TYPE, "tropical")?;
    roundtrip(ImplementationType::LOG_OPENFST_TYPE, "log")?;
    roundtrip_hfst_ol(false, "hfst_ol")?;
    roundtrip_hfst_ol(true, "hfst_olw")?;
    read_att_facade()?;
    println!("all binary stream round-trips OK");
    Ok(())
}
