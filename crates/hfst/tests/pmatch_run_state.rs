// The core/run split of the pmatch runtime.
//
// The loaded archive used to carry the tapes, the entry and RTN stacks, the
// capture bookkeeping and the line counter, and each net carried the
// local-variable stack of whichever walk was running it, so matching a text took
// the whole archive exclusively and an out-of-alphabet character rewrote the
// alphabet it was being matched against. These tests pin what the split has to
// preserve: a run sees only its own scratch, an admitted symbol never reaches
// the core, and one loaded core serves several threads at once.
//
// The tropical transition-data symbol coding used by `HfstBasicTransducer` lives
// in process-global statics; cargo runs each #[test] as a parallel thread in one
// process, so compiling and loading are serialized here as they are in the
// sibling optimized-lookup test file. Nothing inside `std::thread::scope` builds
// a container — that is the point.

use std::sync::Arc;

use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::pmatch_core::PmatchCore;
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

type Weighted = HfstTransducer<Transducer<WeightedTables>>;

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Characters outside the fixture's alphabet, so each only reaches the output
/// after some run has admitted it.
const UNKNOWNS: [&str; 6] = ["ä", "ö", "ü", "ø", "å", "ñ"];

/// Matches two animals and echoes everything else, which is what puts an
/// admitted symbol on the output tape.
const GRAMMAR: &str = "Define TOP [{cat} | {dog}] EndTag(animal) ;\n";

fn compiled_top() -> Result<Weighted, hfst::error::Error> {
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let defs = compiler.compile(GRAMMAR)?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    HfstTransducer::new_from_basic(&top.to_basic()?)
}

fn load(top: &Weighted) -> Result<PmatchContainer, hfst::error::Error> {
    PmatchContainer::new_from_hfst_transducers(vec![top.clone()])
}

/// Lines whose unmatched text is unique to one run, so a run state that leaked
/// would be handing another run a symbol it never saw.
fn lines_for(marker: &str) -> Vec<String> {
    vec![
        format!("cat {marker} dog"),
        format!("{marker}{marker} cat"),
        format!("dog{marker}"),
        format!("{marker} the cat and the dog {marker}"),
    ]
}

fn match_all(container: &mut PmatchContainer, lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| container.do_match(line, 0.0, 0.0))
        .collect()
}

fn assert_send_sync<T: Send + Sync>() {}

/// The loaded archive has to cross a thread boundary for any of this to be
/// worth doing.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core/test]
#[test]
fn the_loaded_core_is_send_and_sync() {
    assert_send_sync::<PmatchCore>();
    assert_send_sync::<Arc<PmatchCore>>();
}

/// One core serving two interleaved run states must answer exactly as two
/// privately loaded containers would.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core/test]
#[test]
fn two_run_states_over_one_core_stay_isolated() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    let top = compiled_top()?;
    let core = load(&top)?.core();
    let mut shared_first = PmatchContainer::from_core(Arc::clone(&core));
    let mut shared_second = PmatchContainer::from_core(Arc::clone(&core));
    let mut private_first = load(&top)?;
    let mut private_second = load(&top)?;

    let first = lines_for(UNKNOWNS[0]);
    let second = lines_for(UNKNOWNS[1]);
    for (a, b) in first.iter().zip(second.iter()) {
        let shared_a = shared_first.do_match(a, 0.0, 0.0);
        let shared_b = shared_second.do_match(b, 0.0, 0.0);
        assert_eq!(
            shared_a,
            private_first.do_match(a, 0.0, 0.0),
            "sharing a core changed the match of {a:?}"
        );
        assert_eq!(
            shared_b,
            private_second.do_match(b, 0.0, 0.0),
            "sharing a core changed the match of {b:?}"
        );
    }
    Ok(())
}

/// A run that admits an out-of-alphabet symbol must echo it back as the old
/// alphabet-growing implementation did, and leave the core as it found it — the
/// alphabet it matched in is the alphabet the next run gets.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core/test]
#[test]
fn admitting_a_symbol_leaves_the_core_unchanged() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    let core = load(&compiled_top()?)?.core();
    let symbols_before = core.symbol_count();

    let mut run = PmatchContainer::from_core(Arc::clone(&core));
    for marker in UNKNOWNS {
        for line in lines_for(marker) {
            let matched = run.do_match(&line, 0.0, 0.0);
            assert!(
                matched.contains(marker),
                "{marker:?} was dropped instead of echoed: {matched:?}"
            );
        }
    }

    assert_eq!(
        core.symbol_count(),
        symbols_before,
        "matching grew the core's alphabet"
    );
    // A fresh run over the same core must still have to admit the symbol
    // itself, which it can only do if the first run's overlay stayed private.
    let mut fresh = PmatchContainer::from_core(core);
    let line = lines_for(UNKNOWNS[0]).remove(0);
    assert_eq!(
        fresh.do_match(&line, 0.0, 0.0),
        run.do_match(&line, 0.0, 0.0),
        "a run that had already admitted the symbol answered differently"
    );
    Ok(())
}

/// Several runs over one shared core, with no lock around the traversal, answer
/// as the same lines do run one after another against a private copy.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core/test]
#[test]
fn concurrent_matches_share_one_loaded_core() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    let top = compiled_top()?;
    let core = load(&top)?.core();

    let work: Vec<Vec<String>> = UNKNOWNS.iter().map(|m| lines_for(m)).collect();
    let mut expected: Vec<Vec<String>> = Vec::new();
    for lines in work.iter() {
        expected.push(match_all(&mut load(&top)?, lines));
    }

    let core = &core;
    let found: Vec<Vec<String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = work
            .iter()
            .map(|lines| {
                scope.spawn(move || {
                    let mut run = PmatchContainer::from_core(Arc::clone(core));
                    match_all(&mut run, lines)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("match thread did not panic"))
            .collect()
    });

    assert_eq!(found, expected, "a concurrent match answered differently");
    Ok(())
}

/// Every fixture unknown above is two bytes of UTF-8; a four-byte sequence
/// must ride the echo path identically and stay out of the core.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core/test]
#[test]
fn a_four_byte_symbol_echoes_through_pmatch() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    let core = load(&compiled_top()?)?.core();
    let symbols_before = core.symbol_count();

    let mut run = PmatchContainer::from_core(Arc::clone(&core));
    for line in lines_for("😀") {
        let matched = run.do_match(&line, 0.0, 0.0);
        assert!(
            matched.contains("😀"),
            "a four-byte symbol was dropped instead of echoed: {matched:?}"
        );
    }

    assert_eq!(
        core.symbol_count(),
        symbols_before,
        "matching a four-byte symbol grew the core's alphabet"
    );
    let mut fresh = PmatchContainer::from_core(core);
    let line = lines_for("😀").remove(0);
    assert_eq!(
        fresh.do_match(&line, 0.0, 0.0),
        run.do_match(&line, 0.0, 0.0),
        "a run that had admitted the four-byte symbol answered differently"
    );
    Ok(())
}
