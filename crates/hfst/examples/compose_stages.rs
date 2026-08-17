//! Where does `hfst compose -F` peak? Runs the production virtual flag-overlay
//! path up to a given cut point and then exits, so `/usr/bin/time -l` around
//! each cut attributes peak RSS to a stage. Reading RSS in-process would need
//! libc, and the workspace forbids unsafe.
//!
//!     cargo run --release --example compose_stages -- \
//!         <stage> <a.hfst> <b.hfst> [memory-limit-bytes]
//!
//! Stages: 1 load both · 2 + prepare the alphabet-only flag overlay ·
//! 3 + harmonize and materialize the product. Omitting `memory-limit-bytes`
//! selects the library's unbounded path; the CLI normally supplies its resolved
//! allowance (automatic, environment, or `--memory-limit`) explicitly.

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::{AnyTransducer, EngineConfig, HfstTransducer};
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

fn shape(label: &str, t: &HfstTransducer<StdVectorFst>) {
    println!(
        "{label}: {} states / {} arcs",
        t.number_of_states(),
        t.number_of_arcs()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let stage: u8 = args[1].parse().expect("stage is 1..=3");
    let (pa, pb) = (args[2].as_str(), args[3].as_str());
    let memory_limit_bytes = args
        .get(4)
        .map(|value| value.parse::<u64>().expect("memory limit is a byte count"));

    let mut a = load(pa);
    let mut b = load(pb);
    shape("stage 1 first", &a);
    shape("stage 1 second", &b);
    if stage == 1 {
        return;
    }

    // `hfst-compose -F` on OpenFst tropical operands: rename both-sided flags
    // and add only the missing symbols to the alphabets. No state×flag arcs are
    // materialized here.
    let overlay = a
        .prepare_flag_diacritics_for_compose(&mut b)
        .expect("prepare virtual flag overlay");
    shape("stage 2 first", &a);
    shape("stage 2 second", &b);
    println!(
        "stage 2 overlay: {} left loops / {} right loops",
        overlay.left_self_loops.len(),
        overlay.right_self_loops.len()
    );
    if stage == 2 {
        return;
    }

    let config = EngineConfig {
        compose_memory_limit_bytes: memory_limit_bytes,
        ..EngineConfig::default()
    };
    a.compose_with_config_and_flag_overlay(&b, true, &config, Some(&overlay))
        .expect("virtual flag-overlay compose");
    shape("stage 3 product", &a);
}
