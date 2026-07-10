// Round-trip a constructed HfstTransducer through the real binary HFST
// writer (HfstOutputStream) and the binary reader (HfstInputStream::read,
// which yields the stream-boundary sum AnyTransducer).

use hfst::backend::AlgebraBackend;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{FromAnyTransducer, HfstTransducer};
use hfst_openfst::StdVectorFst;

fn roundtrip<B: AlgebraBackend + FromAnyTransducer>(label: &str) -> hfst::error::Result<()> {
    let ty = B::TYPE;
    let path = std::env::temp_dir().join(format!("hfst_bin_roundtrip{label}.hfst"));
    let path = path
        .to_str()
        .expect("temp_dir path is valid UTF-8 on this platform")
        .to_string();

    // Build [a:b] of the requested type and write it to a binary HFST file.
    let mut t = HfstTransducer::<B>::new_symbol_pair("a", "b")?;
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

    let any = input.read()?;
    assert_eq!(any.get_type(), ty, "{label}: read type");
    let t2: HfstTransducer<B> = any.into_typed()?;
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
// acceptor, convert it to optimized-lookup form (weighted-shaped tables either
// way; 'weighted' picks the header flag / stream type, as the old
// convert(HFST_OL[W]_TYPE) did), write it with the HfstOlOutputStream behind
// HfstOutputStream, then read it back.
fn roundtrip_hfst_ol(weighted: bool, label: &str) -> hfst::error::Result<()> {
    let ty = if weighted {
        ImplementationType::HFST_OLW_TYPE
    } else {
        ImplementationType::HFST_OL_TYPE
    };

    let tropical = HfstTransducer::<StdVectorFst>::new_symbol("a")?;
    let mut t = tropical.to_ol(weighted, "")?;
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
    let t2 = input.read()?;
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

    let t = HfstTransducer::<StdVectorFst>::read_in_att_format_filename(
        &path,
        "@_EPSILON_SYMBOL_@",
        false,
    )
    .expect("written [a:b] AT&T file reads back as a valid transducer");
    assert_eq!(t.get_type(), ImplementationType::TROPICAL_OPENFST_TYPE);

    let expected = HfstTransducer::<StdVectorFst>::new_symbol_pair("a", "b")?;
    assert!(t.compare(&expected, false)?, "att facade: [a:b] read back");

    let _ = std::fs::remove_file(&path);
    println!("att facade read OK");
    Ok(())
}

fn main() -> hfst::error::Result<()> {
    roundtrip::<StdVectorFst>("tropical")?;
    roundtrip_hfst_ol(false, "hfst_ol")?;
    roundtrip_hfst_ol(true, "hfst_olw")?;
    read_att_facade()?;
    println!("all binary stream round-trips OK");
    Ok(())
}
