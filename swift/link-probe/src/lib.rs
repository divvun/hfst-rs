//! What the linking test calls in a second Rust library: code that uses std,
//! uniffi and tracing as any Rust library does, and sets tracing's global
//! default subscriber, as hfst's library does in its own copy of tracing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once, PoisonError};

use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

uniffi::setup_scaffolding!();

static EVENTS: AtomicU64 = AtomicU64::new(0);
static TARGETS: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// Counts every event this library's tracing dispatches, and keeps the
/// target of each.
struct Counter;

impl Subscriber for Counter {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        EVENTS.fetch_add(1, Ordering::Relaxed);
        let mut targets = TARGETS.lock().unwrap_or_else(PoisonError::into_inner);
        targets.insert(event.metadata().target().to_owned());
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

fn install() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let _ = tracing::subscriber::set_global_default(Counter);
    });
}

/// The words of `text` and how often each occurs, in order. It logs one
/// event.
#[uniffi::export]
pub fn word_counts(text: String) -> Vec<String> {
    install();
    let mut counts = BTreeMap::<&str, u32>::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_default() += 1;
    }
    tracing::info!(words = counts.len(), "counted");
    counts.iter().map(|(w, n)| format!("{w}={n}")).collect()
}

/// How many events this library's own subscriber has seen.
#[uniffi::export]
pub fn events_seen() -> u64 {
    EVENTS.load(Ordering::Relaxed)
}

/// The targets of the events this library's own subscriber has seen,
/// sorted: another library's events would show here under their own.
#[uniffi::export]
pub fn targets_seen() -> Vec<String> {
    let targets = TARGETS.lock().unwrap_or_else(PoisonError::into_inner);
    targets.iter().cloned().collect()
}

#[derive(Debug, uniffi::Error)]
pub enum ProbeError {
    Failed { message: String },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ProbeError::Failed { message } = self;
        f.write_str(message)
    }
}

impl std::error::Error for ProbeError {}

/// Panics with `message`; UniFFI catches the panic as it unwinds and throws
/// it in Swift.
#[uniffi::export]
pub fn panics(message: String) -> Result<u32, ProbeError> {
    panic!("{message}")
}

/// A tally kept across calls, behind a lock.
#[derive(uniffi::Object)]
pub struct Tally {
    seen: Mutex<Vec<String>>,
}

#[uniffi::export]
impl Tally {
    /// A tally that has seen `words` already.
    #[uniffi::constructor]
    pub fn new(words: Vec<String>) -> Arc<Tally> {
        Arc::new(Tally {
            seen: Mutex::new(words),
        })
    }

    pub fn add(&self, word: String) -> u32 {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        seen.push(word);
        u32::try_from(seen.len()).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_counted_in_order() {
        assert_eq!(word_counts("b a b".into()), ["a=1", "b=2"]);
        assert!(events_seen() >= 1);
        assert_eq!(targets_seen(), ["hfst_link_probe"]);
        let tally = Tally::new(vec!["x".into()]);
        assert_eq!(tally.add("y".into()), 2);
    }
}
