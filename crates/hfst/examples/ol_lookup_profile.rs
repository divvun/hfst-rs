//! OL-lookup profiling harness (not a test; a perf probe).
//!
//! Loads a real optimized-lookup analyzer and hammers it with a wordlist so a
//! sampling profiler (samply / `sample`) can show where the hfst-native lookup
//! hot path actually spends time — specifically whether the OL traversal in
//! `transducer.rs` (`get_analyses` / `try_epsilon_transitions` and its per-arc
//! `FlagDiacriticState` clones) is a measurable cost on a real lexicon.
//!
//! Usage:
//!   cargo run --release --example ol_lookup_profile -- <analyzer.hfstol> <wordlist.txt> [iterations]
//!
//! The wordlist is one surface form per line. Each iteration looks up every
//! word against every archive member (the union). `black_box` on the result
//! count keeps the optimizer from deleting the work.

use std::hint::black_box;
use std::time::Instant;

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use hfst::transducer::{Transducer, UnweightedTables, WeightedTables};

/// A loaded archive member reduced to the two OL table shapes the lookup engine
/// runs on (mirrors the CLI's `OlInner`).
enum Member {
    W(HfstTransducer<Transducer<WeightedTables>>),
    U(HfstTransducer<Transducer<UnweightedTables>>),
}

impl Member {
    fn lookup(&mut self, s: &str) -> usize {
        let r = match self {
            Member::W(t) => t.lookup_fd_string(s, -1, 0.0),
            Member::U(t) => t.lookup_fd_string(s, -1, 0.0),
        };
        r.map(|paths| paths.len()).unwrap_or(0)
    }

    /// Sorted, fully-rendered analyses for oracle diffing (weight + output).
    fn dump(&mut self, s: &str) -> Vec<String> {
        let r = match self {
            Member::W(t) => t.lookup_fd_string(s, -1, 0.0),
            Member::U(t) => t.lookup_fd_string(s, -1, 0.0),
        };
        let mut out: Vec<String> = match r {
            Ok(paths) => paths
                .into_iter()
                .map(|p| format!("{:.6}\t{}", p.first, p.second.join("")))
                .collect(),
            Err(e) => vec![format!("ERR: {e}")],
        };
        out.sort();
        out
    }
}

fn main() -> hfst::error::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: {} <analyzer.hfstol> <wordlist.txt> [iterations]",
            args[0]
        );
        std::process::exit(2);
    }
    let analyzer = &args[1];
    let wordlist = &args[2];
    let dump_mode = args.get(3).map(|s| s == "--dump").unwrap_or(false);
    let iterations: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20);

    // Load every member of the archive (the union), exactly like the CLI setup.
    let mut instream = HfstInputStream::new_filename(analyzer)?;
    let mut members: Vec<Member> = Vec::new();
    while instream.is_good() {
        let any = match instream.read() {
            Ok(v) => v,
            Err(_) => break,
        };
        let ty = any.get_type();
        // Tropical (and, with the feature, foma) members convert through the
        // tropical algebra into the weighted OL tables the lookup engine runs on.
        let into_weighted = |t: AnyTransducer| match t.into_typed() {
            Ok(t) => Member::W(t),
            Err(e) => {
                eprintln!("cannot convert member ({ty:?}) to OLW: {e}");
                std::process::exit(1);
            }
        };
        let member = match any {
            AnyTransducer::OlW(t) => Member::W(t),
            AnyTransducer::OlU(t) => Member::U(t),
            AnyTransducer::Thfst(t) => Member::W(t.into_olw()),
            other @ AnyTransducer::Tropical(_) => into_weighted(other),
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => into_weighted(other),
        };
        members.push(member);
    }
    if members.is_empty() {
        eprintln!("no transducers in {analyzer}");
        std::process::exit(1);
    }

    let text = match std::fs::read_to_string(wordlist) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("cannot read wordlist {wordlist}: {e}");
            std::process::exit(1);
        }
    };
    let words: Vec<String> = text
        .lines()
        .map(str::to_string)
        .filter(|w| !w.is_empty())
        .collect();
    eprintln!(
        "loaded {} member(s), {} words; running {} iteration(s)",
        members.len(),
        words.len(),
        iterations
    );

    // Oracle dump: emit every word's sorted analyses, deterministically, for a
    // before/after diff proving the container swap changes no output.
    if dump_mode {
        use std::io::Write;
        let mut out = std::io::BufWriter::new(std::io::stdout());
        for w in &words {
            for m in members.iter_mut() {
                let analyses = m.dump(w);
                let _ = writeln!(out, "{w}\t{}", analyses.join(" | "));
            }
        }
        return Ok(());
    }

    // Warmup: one full pass, and report the hit rate so we know the lookups are
    // actually landing in-vocabulary (OOV words short-circuit and would
    // under-exercise the traversal we care about).
    let mut hits = 0usize;
    let mut total_analyses = 0usize;
    for w in &words {
        for m in members.iter_mut() {
            let n = m.lookup(w);
            if n > 0 {
                hits += 1;
            }
            total_analyses += n;
        }
    }
    eprintln!(
        "warmup: {hits}/{} lookups produced analyses ({} total analyses)",
        words.len() * members.len(),
        total_analyses
    );

    let start = Instant::now();
    let mut sink = 0usize;
    for _ in 0..iterations {
        for w in &words {
            for m in members.iter_mut() {
                sink = sink.wrapping_add(black_box(m.lookup(w)));
            }
        }
    }
    let elapsed = start.elapsed();
    black_box(sink);

    let lookups = iterations * words.len() * members.len();
    let per = elapsed.as_secs_f64() / lookups as f64;
    eprintln!(
        "timed: {lookups} lookups in {:.3}s  =>  {:.0} lookups/s, {:.2} µs/lookup  (sink={sink})",
        elapsed.as_secs_f64(),
        lookups as f64 / elapsed.as_secs_f64(),
        per * 1e6,
    );
    Ok(())
}
