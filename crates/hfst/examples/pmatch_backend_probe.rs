//! Phase-1 gate probe for the "foma (unweighted) pmatch backend shrinks the
//! tokeniser archive" hypothesis (WBS node `foma-tokeniser-build`).
//!
//! Compiles a real pmscript with the pmatch compiler pinned to either the
//! tropical (weighted) backend or the foma (unweighted) backend and reports the
//! per-definition state/arc counts — TOP is the multi-million-arc archive
//! payload the 541 MB `.pmhfst` inflation is attributed to. Optionally runs the
//! archive writer (`write_archive`) to an output file so the archive-relevant
//! size (foma members convert to weighted OL via the basic route — OLW output
//! format is preserved) can be compared byte-for-byte.
//!
//! Usage:
//!   cargo run --release --example pmatch_backend_probe -- \
//!     <tropical|foma> <pmscript> [output.pmhfst]
//!
//! The pmscript's `@bin` reads resolve relative to the pmscript's own
//! directory (mirroring hfst-pmatch2fst's includedir handling), so run it with
//! that directory populated with the referenced `.hfst` inputs.

use std::collections::HashMap;

use hfst::backend::AlgebraBackend;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{FromAnyTransducer, HfstTransducer};
use hfst::pmatch_compiler::{PmatchCompiler, write_archive};

fn includedir_for(pmscript: &str) -> String {
    // Mirror hfst-pmatch2fst: derive the include dir from the input filename.
    let abs = if pmscript.starts_with('/') {
        pmscript.to_string()
    } else {
        let pwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        format!("{pwd}/{pmscript}")
    };
    match abs.rfind('/') {
        None => String::new(),
        Some(slashpos) => abs[..slashpos + 1].to_string(),
    }
}

fn run<B>(label: &str, pmscript: &str, out: Option<&str>) -> Result<(), Box<dyn std::error::Error>>
where
    B: AlgebraBackend + FromAnyTransducer + 'static,
{
    let src = std::fs::read_to_string(pmscript)?;
    let mut comp = PmatchCompiler::<B>::new();
    comp.set_verbose(true);
    comp.set_include_path(includedir_for(pmscript));

    let t0 = std::time::Instant::now();
    let mut defs: HashMap<String, HfstTransducer<B>> = comp.compile(&src)?;
    let compile_secs = t0.elapsed().as_secs_f64();

    // Report every definition, TOP first, with state/arc counts.
    let mut keys: Vec<String> = defs.keys().cloned().collect();
    keys.sort_by_key(|k| (k != "TOP", k.clone()));
    println!("=== backend={label} compile={compile_secs:.1}s ===");
    for k in &keys {
        let t = &defs[k];
        println!(
            "  {:<32} states={:>10} arcs={:>12}",
            k,
            t.number_of_states(),
            t.number_of_arcs()
        );
    }

    if let Some(out) = out {
        let t1 = std::time::Instant::now();
        let mut outstream =
            HfstOutputStream::new_filename(out, ImplementationType::HFST_OLW_TYPE, true)?;
        let wrote = write_archive(&mut defs, &mut outstream, true, &mut std::io::stderr())?;
        outstream.close();
        let write_secs = t1.elapsed().as_secs_f64();
        let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
        println!("  write_archive wrote={wrote} secs={write_secs:.1} bytes={size} -> {out}");
    }
    Ok(())
}

/// Compile TOP on the tropical backend, then test whether zeroing every weight
/// and re-minimizing collapses the arc count. This is the direct, runnable
/// probe of the "weight-diversity inflates TOP" premise the whole foma
/// hypothesis rests on: if an unweighted TOP does not have fewer arcs, an
/// unweighted (foma) compile cannot shrink the archive structurally.
fn weight_test(pmscript: &str) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(pmscript)?;
    let mut comp = PmatchCompiler::<hfst_openfst::StdVectorFst>::new();
    comp.set_include_path(includedir_for(pmscript));
    let mut defs = comp.compile(&src)?;
    let mut top = defs.remove("TOP").ok_or("no TOP in compiled definitions")?;
    println!(
        "weighted TOP:            states={:>10} arcs={:>12}",
        top.number_of_states(),
        top.number_of_arcs()
    );

    // Zero every arc weight and every final weight, then minimize. If the
    // machine was inflated by weight diversity, states/arcs drop here.
    top.transform_weights(|_| 0.0)?;
    top.set_final_weights(0.0, false)?;
    println!(
        "weights zeroed (pre-min): states={:>10} arcs={:>12}",
        top.number_of_states(),
        top.number_of_arcs()
    );
    let t0 = std::time::Instant::now();
    top.minimize()?;
    println!(
        "weights zeroed + minimize: states={:>10} arcs={:>12}  ({:.1}s)",
        top.number_of_states(),
        top.number_of_arcs(),
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let backend = args.get(1).map(String::as_str).unwrap_or("");
    let pmscript = match args.get(2) {
        Some(p) => p.as_str(),
        None => {
            eprintln!(
                "usage: pmatch_backend_probe <tropical|foma|weighttest> <pmscript> [output.pmhfst]"
            );
            std::process::exit(2);
        }
    };
    let out = args.get(3).map(String::as_str);

    match backend {
        "tropical" => run::<hfst_openfst::StdVectorFst>("tropical", pmscript, out)?,
        #[cfg(feature = "foma")]
        "foma" => run::<hfst::backend_foma::FomaTransducer>("foma", pmscript, out)?,
        "weighttest" => weight_test(pmscript)?,
        other => {
            eprintln!("unknown backend {other:?} (expected tropical|foma|weighttest)");
            std::process::exit(2);
        }
    }
    Ok(())
}
