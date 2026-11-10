//! The library's tracing events, kept while a call goes on without the GIL
//! and handed to Python's `logging` once the call has it back.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Dispatch, Level, Metadata, Subscriber};

// [spec:hfst:req:python.logging]
/// The loggers the library's events go under, named after the crates whose
/// targets they carry: the hfst library, and divvunspell's engine behind the
/// optimised lookup. An event's root is the first `::` segment of its target.
const ROOTS: [&str; 2] = ["hfst", "divvun_fst"];

/// The loggers of [`ROOTS`]. `logging.getLogger` gives the one logger of a
/// name for the life of the process, so asking once is enough.
static LOGGERS: PyOnceLock<[Py<PyAny>; 2]> = PyOnceLock::new();

/// One event, as it reaches `logging`.
struct Event {
    level: Level,
    target: &'static str,
    message: String,
    fields: String,
}

impl Event {
    /// The message followed by the event's other fields as `name=value`.
    fn rendered(&self) -> String {
        if self.message.is_empty() {
            self.fields.trim_start().to_owned()
        } else {
            format!("{}{}", self.message, self.fields)
        }
    }
}

/// The least level each root logger handles at the start of a call.
#[derive(Clone, Copy)]
pub struct Thresholds([LevelFilter; 2]);

impl Thresholds {
    /// The least level any root handles, the least an event must have to be
    /// kept at all.
    fn any(&self) -> LevelFilter {
        self.0[0].max(self.0[1])
    }

    /// The least level an event with `target` must have to reach `logging`.
    /// A target under neither root goes by the more verbose of the two.
    fn of(&self, target: &str) -> LevelFilter {
        let root = target.split("::").next().unwrap_or(target);
        ROOTS
            .iter()
            .position(|known| *known == root)
            .map_or_else(|| self.any(), |at| self.0[at])
    }
}

// [spec:hfst:req:python.logging]
/// The least level each root logger handles at the start of a call, as
/// `logging.disable` and the logger's effective level decide. Nothing below
/// it is kept.
pub fn thresholds(py: Python<'_>) -> PyResult<Thresholds> {
    let loggers = LOGGERS.get_or_try_init(py, || -> PyResult<_> {
        let get_logger = py.import("logging")?.getattr("getLogger")?;
        Ok([
            get_logger.call1((ROOTS[0],))?.unbind(),
            get_logger.call1((ROOTS[1],))?.unbind(),
        ])
    })?;
    let mut levels = [LevelFilter::OFF; 2];
    for (logger, level) in loggers.iter().zip(&mut levels) {
        *level = threshold(logger.bind(py))?;
    }
    Ok(Thresholds(levels))
}

fn threshold(logger: &Bound<'_, PyAny>) -> PyResult<LevelFilter> {
    for level in [
        Level::TRACE,
        Level::DEBUG,
        Level::INFO,
        Level::WARN,
        Level::ERROR,
    ] {
        if logger
            .call_method1("isEnabledFor", (python_level(level),))?
            .is_truthy()?
        {
            return Ok(LevelFilter::from_level(level));
        }
    }
    Ok(LevelFilter::OFF)
}

/// `logging`'s number for a level. Python has no trace level, so trace is 5,
/// below `logging.DEBUG`.
fn python_level(level: Level) -> u8 {
    match level {
        Level::ERROR => 40,
        Level::WARN => 30,
        Level::INFO => 20,
        Level::DEBUG => 10,
        Level::TRACE => 5,
    }
}

/// The events a call kept.
pub struct Kept {
    thresholds: Thresholds,
    events: Vec<Event>,
}

// [spec:hfst:req:python.logging]
/// Runs `f`, keeping every event a root logger would handle. With every
/// level turned off it runs `f` alone, and makes no subscriber.
pub fn collecting<T>(thresholds: Thresholds, f: impl FnOnce() -> T) -> (T, Kept) {
    let keep = thresholds.any();
    if keep == LevelFilter::OFF {
        let kept = Kept {
            thresholds,
            events: Vec::new(),
        };
        return (f(), kept);
    }
    let events = Arc::new(Mutex::new(Vec::new()));
    let dispatch = Dispatch::new(Collector {
        keep,
        events: Arc::clone(&events),
    });
    let value = tracing::dispatcher::with_default(&dispatch, f);
    drop(dispatch);
    let events = std::mem::take(&mut *events.lock().unwrap_or_else(PoisonError::into_inner));
    (value, Kept { thresholds, events })
}

impl Kept {
    // [spec:hfst:req:python.logging]
    /// Logs each kept event its root logger handles, in order, on the logger
    /// named after its target with `::` read as `.`.
    pub fn hand_over(&self, py: Python<'_>) -> PyResult<()> {
        let mut events = self
            .events
            .iter()
            .filter(|e| e.level <= self.thresholds.of(e.target))
            .peekable();
        if events.peek().is_none() {
            return Ok(());
        }
        let get_logger = py.import("logging")?.getattr("getLogger")?;
        for event in events {
            get_logger
                .call1((event.target.replace("::", "."),))?
                .call_method1("log", (python_level(event.level), event.rendered()))?;
        }
        Ok(())
    }
}

/// The subscriber for one call, set as its thread's default for the call
/// alone. Spans are never enabled, so none is ever made.
///
/// Tracing caches one interest per callsite for every live subscriber
/// together, and rebuilds it whenever a subscriber is made, so a call made with
/// a lower threshold than another thread's still sees its events. `event`
/// checks again what it receives, because the cache is shared.
struct Collector {
    keep: LevelFilter,
    events: Arc<Mutex<Vec<Event>>>,
}

impl Collector {
    fn keeps(&self, metadata: &Metadata<'_>) -> bool {
        metadata.is_event() && *metadata.level() <= self.keep
    }
}

impl Subscriber for Collector {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.keeps(metadata) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.keeps(metadata)
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(self.keep)
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let metadata = event.metadata();
        if !self.keeps(metadata) {
            return;
        }
        let mut text = Text::default();
        event.record(&mut text);
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Event {
                level: *metadata.level(),
                target: metadata.target(),
                message: text.message,
                fields: text.fields,
            });
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

#[derive(Default)]
struct Text {
    message: String,
    fields: String,
}

impl Visit for Text {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.fields
                .push_str(&format!(" {}={value:?}", field.name()));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message.push_str(&format!("{value:?}"));
        } else {
            self.fields
                .push_str(&format!(" {}={value:?}", field.name()));
        }
    }
}
