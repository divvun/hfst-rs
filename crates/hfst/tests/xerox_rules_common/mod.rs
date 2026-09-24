//! Helpers shared by the test_xerox_rules*.rs integration tests.

use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;

// The tropical transition-data symbol coding lives in process-global statics
// behind Mutexes. cargo runs every #[test] as a parallel thread in ONE process,
// but each C++ test was its own process. Serializing the tests through this lock
// restores the one-at-a-time-per-process model without touching the library or
// weakening any assertion. into_inner() recovers from a poisoned lock so one
// failing test does not cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(super) fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// C++ pattern: tmp = left; tmp.compose(right).minimize();
pub(super) fn compose_minimize<B: AlgebraBackend>(
    left: &HfstTransducer<B>,
    right: &HfstTransducer<B>,
) -> Result<HfstTransducer<B>, hfst::error::Error> {
    let mut t = left.clone();
    t.compose(right, true)?.minimize()?;
    Ok(t)
}
