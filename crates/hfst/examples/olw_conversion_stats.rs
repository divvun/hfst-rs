//! Sizing probe for the tropical→OLW conversion structures: loads a
//! transducer, converts to the basic form, and reports the counts that
//! determine the OL packer's memory footprint (states, arcs, distinct
//! (state, input) groups, alphabet spread).
//! Usage: cargo run --release --example olw_conversion_stats <file.hfst>

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: olw_conversion_stats <file.hfst>")?;
    let mut stream = HfstInputStream::new_filename(&path)?;
    let any = stream.read()?;
    let t: HfstTransducer<hfst_openfst::StdVectorFst> = any.into_typed()?;
    let net = t.get_basic_transducer()?;

    let mut arcs: u64 = 0;
    let mut groups: u64 = 0;
    let mut single_arc_groups: u64 = 0;
    let states = net.state_vector.len() as u64;
    for trs in net.state_vector.iter() {
        let mut seen: Vec<u32> = Vec::new();
        let mut counts: Vec<u64> = Vec::new();
        for tr in trs.iter() {
            arcs += 1;
            let n = tr.get_input_number();
            match seen.iter().position(|&x| x == n) {
                Some(i) => counts[i] += 1,
                None => {
                    seen.push(n);
                    counts.push(1);
                }
            }
        }
        groups += seen.len() as u64;
        single_arc_groups += counts.iter().filter(|&&c| c == 1).count() as u64;
    }
    let sigma = net.get_alphabet().len();
    println!("states            {states}");
    println!("arcs              {arcs}");
    println!("distinct (state,input) groups {groups}");
    println!("  of which single-arc groups  {single_arc_groups}");
    println!("avg groups/state  {:.1}", groups as f64 / states as f64);
    println!("avg arcs/group    {:.1}", arcs as f64 / groups as f64);
    println!("alphabet size     {sigma}");
    // Footprint model of the current packer, in bytes:
    let tp = arcs * 16; // TransitionPlaceholder payloads
    let inner_vecs = groups * (24 + 32); // Vec header in outer vec + min heap alloc
    let sparse_map = groups * 8; // (SymbolNumber, u32) pairs
    let wtransition = (arcs + states) * 12; // final in-memory transition table
    println!(
        "model: TP {} MB, inner-vec overhead {} MB, sparse maps {} MB, wtransition {} MB",
        tp / 1_048_576,
        inner_vecs / 1_048_576,
        sparse_map / 1_048_576,
        wtransition / 1_048_576
    );
    Ok(())
}
