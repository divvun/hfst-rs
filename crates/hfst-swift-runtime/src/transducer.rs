//! Optimized-lookup transducers read from bytes, and lookup in them with the
//! hfst library's engine.

use std::collections::HashMap;
use std::sync::Arc;

use hfst_embed::transducer::{Transducer as Machine, read_transducers as read_all};
use hfst_embed::{Code, Error};

use crate::error::GlueError;
use crate::{PACKAGE, args};

// [spec:hfst:req:swift.lookup]
/// One path a lookup found: its output symbols, joined and one by one, and
/// its weight.
#[derive(uniffi::Record)]
pub struct LookupResult {
    pub output: String,
    pub symbols: Vec<String>,
    pub weight: f32,
}

impl From<hfst_embed::lookup::LookupResult> for LookupResult {
    fn from(found: hfst_embed::lookup::LookupResult) -> Self {
        LookupResult {
            output: found.output,
            symbols: found.symbols,
            weight: found.weight,
        }
    }
}

// [spec:hfst:req:swift.lookup]
/// An optimized-lookup transducer, looked up in with the hfst library's
/// engine. It never changes, so threads may look up in it at once: each
/// lookup makes its own run state over the shared tables.
#[derive(uniffi::Object)]
pub struct Transducer {
    machine: Machine,
}

#[uniffi::export]
impl Transducer {
    /// Its type's name, as `hfst fst2fst --format` names it.
    pub fn format(&self) -> String {
        self.machine.format().name().to_owned()
    }

    pub fn name(&self) -> String {
        self.machine.name().to_owned()
    }

    /// The properties of its HFST3 header but `version`, `type` and `name`.
    pub fn properties(&self) -> HashMap<String, String> {
        self.machine
            .properties()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect()
    }

    // [spec:hfst:req:swift.lookup]
    /// What `@divvun/hfst`'s `Transducer.lookup` returns for `input` and the
    /// same limits, `time_limit` in seconds.
    pub fn lookup(
        &self,
        input: String,
        limit: Option<i64>,
        time_limit: Option<f64>,
    ) -> Result<Vec<LookupResult>, GlueError> {
        let limit = args::limit(limit)?;
        let time_limit = args::seconds("a lookup time limit", time_limit)?;
        Ok(self
            .machine
            .lookup(&input, limit, time_limit)
            .into_iter()
            .map(LookupResult::from)
            .collect())
    }
}

// [spec:hfst:req:swift.load]
/// Every transducer in the bytes of a stream, as `@divvun/hfst`'s
/// `readTransducers` reads them.
#[uniffi::export]
pub fn read_transducers(bytes: Vec<u8>) -> Result<Vec<Arc<Transducer>>, GlueError> {
    let machines = read_all(&bytes, &PACKAGE)?;
    Ok(machines
        .into_iter()
        .map(|machine| Arc::new(Transducer { machine }))
        .collect())
}

// [spec:hfst:req:swift.load]
/// The one transducer in the bytes of a stream, or `transducer-count` when
/// there are more.
#[uniffi::export]
pub fn read_transducer(bytes: Vec<u8>) -> Result<Arc<Transducer>, GlueError> {
    let mut all = read_transducers(bytes)?;
    if all.len() == 1
        && let Some(transducer) = all.pop()
    {
        return Ok(transducer);
    }
    Err(Error::new(
        Code::TransducerCount,
        format!(
            "Transducer(data:) takes exactly one transducer, and the stream holds {}",
            all.len()
        ),
    )
    .into())
}
