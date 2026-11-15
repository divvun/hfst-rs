//! The optimised lookup: an optimized-lookup transducer read by divvunspell's
//! `divvun-fst`, and words looked up in it by that crate's lookup.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use hfst_embed::optimised::{Lookup, OptimisedTransducer as Machine};

use crate::error::GlueError;
use crate::log::logged;
use crate::transducer::LookupResult;
use crate::{PACKAGE, args};

// [spec:hfst:req:swift.lookup]
// [spec:hfst:req:swift.threads]
/// An optimized-lookup transducer read for the optimised lookup. The
/// transducer never changes; the lookup's working tables are kept in a pool,
/// so concurrent lookups each take a set of their own and the sets keep
/// their capacity between lookups.
#[derive(uniffi::Object)]
pub struct OptimisedTransducer {
    machine: Machine,
    tables: Mutex<Vec<Lookup>>,
}

#[uniffi::export]
impl OptimisedTransducer {
    pub fn format(&self) -> String {
        self.machine.format().name().to_owned()
    }

    pub fn name(&self) -> String {
        self.machine.name().to_owned()
    }

    pub fn properties(&self) -> HashMap<String, String> {
        self.machine
            .properties()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect()
    }

    // [spec:hfst:req:swift.lookup]
    /// What `@divvun/hfst`'s optimised lookup returns for `input`.
    pub fn lookup(
        &self,
        input: String,
        limit: Option<i64>,
    ) -> Result<Vec<LookupResult>, GlueError> {
        let limit = args::limit(limit)?;
        let mut tables = self
            .tables
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop()
            .unwrap_or_default();
        let found = logged(|| self.machine.lookup(&mut tables, &input, limit));
        self.tables
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tables);
        Ok(found
            .into_iter()
            .map(|found| LookupResult {
                output: found.output.into(),
                symbols: found.symbols.into_iter().map(String::from).collect(),
                weight: found.weight.0,
            })
            .collect())
    }
}

// [spec:hfst:req:swift.load]
/// The one optimized-lookup transducer in `bytes`, as `@divvun/hfst`'s
/// `readOptimisedTransducer` reads it.
#[uniffi::export]
pub fn read_optimised_transducer(bytes: Vec<u8>) -> Result<Arc<OptimisedTransducer>, GlueError> {
    let machine = logged(|| Machine::read(bytes, &PACKAGE))?;
    Ok(Arc::new(OptimisedTransducer {
        machine,
        tables: Mutex::new(Vec::new()),
    }))
}
