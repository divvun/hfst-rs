// Exercises XreCompiler @-load file evaluation (eval_read_file): @txt, @bin, @pl.

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;

fn compile(c: &mut XreCompiler, src: &str) -> HfstTransducer {
    c.compile(src)
        .unwrap_or_else(|| panic!("compile returned null for {src:?}"))
}

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

fn main() -> hfst::error::Result<()> {
    let mut c = XreCompiler::new(TROPICAL_OPENFST_TYPE);

    // @txt: one word per line, each tokenized char by char -> {cat} | {dog}.
    let txt = tmp("xre_readfile.txt");
    std::fs::write(&txt, "cat\ndog\n").unwrap();
    let from_txt = compile(&mut c, &format!("@txt\"{txt}\""));
    let expected_txt = compile(&mut c, "{cat} | {dog}");
    assert!(
        from_txt.compare(&expected_txt, false)?,
        "@txt load equals {{cat}} | {{dog}}"
    );

    // @bin: write a binary a:b transducer, then read it back through @bin.
    let bin = tmp("xre_readfile.hfst");
    {
        let mut t = HfstTransducer::new_symbol_pair("a", "b", TROPICAL_OPENFST_TYPE)?;
        let mut out = HfstOutputStream::new_filename(&bin, TROPICAL_OPENFST_TYPE, true)?;
        out.redirect(&mut t)?;
        out.close();
    }
    let from_bin = compile(&mut c, &format!("@bin\"{bin}\""));
    let expected_bin = compile(&mut c, "a:b");
    assert!(
        from_bin.compare(&expected_bin, false)?,
        "@bin load equals a:b"
    );

    // @pl: emit a:b via the ported prolog writer, then read it back through @pl.
    let pl = tmp("xre_readfile.pl");
    {
        let mut g = HfstBasicTransducer::new();
        let tr = HfstBasicTransition::new_symbols(
            1,
            "a".to_string(),
            "b".to_string(),
            0.0,
            g.coder_mut(),
        );
        g.add_transition(0, &tr, true);
        g.set_final_weight(1, &0.0);
        let mut buf: Vec<u8> = Vec::new();
        g.write_in_prolog_format_os(&mut buf, "ab", true);
        std::fs::write(&pl, buf).unwrap();
    }
    let from_pl = compile(&mut c, &format!("@pl\"{pl}\""));
    assert!(
        from_pl.compare(&expected_bin, false)?,
        "@pl load equals a:b"
    );

    for f in [&txt, &bin, &pl] {
        let _ = std::fs::remove_file(f);
    }
    println!("xre @-load OK: @txt={{cat|dog}}, @bin=a:b, @pl=a:b");
    Ok(())
}
