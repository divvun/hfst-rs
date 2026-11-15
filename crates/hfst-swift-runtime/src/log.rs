//! The library's `tracing` events, sent to the sink the Swift layer sets,
//! which logs them to the unified log and to the caller's handler.
//!
//! The dispatcher is the global default. swift/build.sh prelinks the static
//! library so that only its C interface is global, so this `tracing` is the
//! library's own and no other Rust library in the app can set or share its
//! default. A dispatcher set for each call instead would make a handler that
//! calls back into the library panic: `tracing` refuses to set a scoped
//! default while it dispatches an event on the same thread.

use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Once, PoisonError, RwLock};

use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Dispatch, Level, Metadata, Subscriber};

// [spec:hfst:req:swift.logging+1]
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    fn of(level: Level) -> LogLevel {
        match level {
            Level::TRACE => LogLevel::Trace,
            Level::DEBUG => LogLevel::Debug,
            Level::INFO => LogLevel::Info,
            Level::WARN => LogLevel::Warning,
            Level::ERROR => LogLevel::Error,
        }
    }

    fn filter(self) -> LevelFilter {
        match self {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warning => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}

// [spec:hfst:req:swift.logging+1]
/// Where the events go: the Swift layer's sink, which logs each to the
/// unified log and hands it to the caller's handler.
#[uniffi::export(with_foreign)]
pub trait LogSink: Send + Sync {
    fn log(&self, level: LogLevel, target: String, message: String);
}

static SINK: RwLock<Option<Arc<dyn LogSink>>> = RwLock::new(None);

/// The least level logged as [`LevelFilter`]'s order counts it: 0 for none,
/// 1 for errors alone, up to 5 for every event. Warnings and errors unless
/// the caller says otherwise.
static LEVEL: AtomicU8 = AtomicU8::new(2);

fn filter() -> LevelFilter {
    match LEVEL.load(Ordering::Relaxed) {
        0 => LevelFilter::OFF,
        1 => LevelFilter::ERROR,
        2 => LevelFilter::WARN,
        3 => LevelFilter::INFO,
        4 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    }
}

// [spec:hfst:req:swift.logging+1]
/// Sets the sink every event goes to, and the first time, the library's
/// dispatcher as the global default.
#[uniffi::export]
pub fn set_log_sink(sink: Arc<dyn LogSink>) {
    *SINK.write().unwrap_or_else(PoisonError::into_inner) = Some(sink);
    INSTALLED.call_once(|| {
        // Nothing else in the library sets a global default, so this one is
        // the first.
        let _ = tracing::dispatcher::set_global_default(Dispatch::new(Forwarder));
    });
}

// [spec:hfst:req:swift.logging+1]
/// Logs events at `level` and the more severe levels, or none for `None`.
#[uniffi::export]
pub fn set_log_level(level: Option<LogLevel>) {
    let order = match level.map(LogLevel::filter) {
        None => 0,
        Some(LevelFilter::ERROR) => 1,
        Some(LevelFilter::WARN) => 2,
        Some(LevelFilter::INFO) => 3,
        Some(LevelFilter::DEBUG) => 4,
        Some(_) => 5,
    };
    LEVEL.store(order, Ordering::Relaxed);
    // Tracing caches each callsite's interest and the most verbose level any
    // dispatcher wants, so both are asked again.
    tracing::callsite::rebuild_interest_cache();
}

/// The least level logged, or `None` when nothing is.
#[uniffi::export]
pub fn log_level() -> Option<LogLevel> {
    filter().into_level().map(LogLevel::of)
}

static INSTALLED: Once = Once::new();

/// The subscriber the dispatcher holds. It enables events at the level
/// [`set_log_level`] set and never a span, and sends each event it enables
/// to the sink.
struct Forwarder;

impl Forwarder {
    fn keeps(metadata: &Metadata<'_>) -> bool {
        metadata.is_event() && *metadata.level() <= filter()
    }
}

impl Subscriber for Forwarder {
    fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        // The level changes while the program runs, so the answer is asked
        // of `enabled` each time.
        Interest::sometimes()
    }

    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        Forwarder::keeps(metadata)
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(filter())
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let metadata = event.metadata();
        if !Forwarder::keeps(metadata) {
            return;
        }
        let Some(sink) = SINK.read().unwrap_or_else(PoisonError::into_inner).clone() else {
            return;
        };
        let mut text = Text::default();
        event.record(&mut text);
        sink.log(
            LogLevel::of(*metadata.level()),
            metadata.target().to_owned(),
            text.rendered(),
        );
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

/// An event's message and its other fields.
#[derive(Default)]
struct Text {
    message: String,
    fields: String,
}

impl Text {
    /// The message followed by the other fields as `name=value`.
    fn rendered(self) -> String {
        if self.message.is_empty() {
            self.fields.trim_start().to_owned()
        } else {
            self.message + &self.fields
        }
    }
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
