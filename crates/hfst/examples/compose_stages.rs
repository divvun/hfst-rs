//! Where does compose's peak memory go? Runs `HfstTransducer::compose`'s stages
//! up to a given cut point and then exits, so `/usr/bin/time -l` around each cut
//! attributes peak RSS to a stage. Reading RSS in-process would need libc, and
//! the workspace forbids unsafe.
//!
//!     cargo run --release --example compose_stages -- <stage> <a.hfst> <b.hfst>
//!
//! Stages: 1 load both · 2 + clone the second · 3 + harmonize_copy · 4 + product

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use hfst_openfst::StdVectorFst;

fn load(path: &str) -> HfstTransducer<StdVectorFst> {
    let mut s = HfstInputStream::new_filename(path).expect("open input");
    let any = s.read().expect("read transducer");
    let kind = any.get_type();
    match any {
        AnyTransducer::Tropical(t) => t,
        AnyTransducer::OlW(_)
        | AnyTransducer::OlU(_)
        | AnyTransducer::Foma(_)
        | AnyTransducer::Thfst(_) => {
            panic!("expected a tropical transducer, got {kind:?}")
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let stage: u8 = args[1].parse().expect("stage is 1..=4");
    let (pa, pb) = (args[2].as_str(), args[3].as_str());

    let mut a = load(pa);
    let b = load(pb);
    println!(
        "stage 1: loaded {} + {} states",
        a.number_of_states(),
        b.number_of_states()
    );
    if stage == 1 {
        return;
    }

    let mut copy = b.clone();
    println!(
        "stage 2: cloned second ({} states)",
        copy.number_of_states()
    );
    if stage == 2 {
        return;
    }

    copy = a
        .harmonize_copy(&copy)
        .expect("harmonize")
        .expect("tropical harmonization yields Some");
    println!("stage 3: harmonized ({} states)", copy.number_of_states());
    if stage == 3 {
        return;
    }

    a.compose(&copy, false).expect("compose");
    println!("stage 4: product {} states", a.number_of_states());
}
