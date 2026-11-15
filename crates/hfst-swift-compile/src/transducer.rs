//! Transducers of every type the hfst library reads, each behind a lock that
//! an operation takes to change it and every other call to read it.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use hfst_embed::compile::{self as embed, Operand, Transducer as Machine};
use hfst_embed::format::Format;
use hfst_embed::{Code, Error};
use hfst_swift_runtime::GlueError;
use hfst_swift_runtime::args;
use hfst_swift_runtime::error::invalid;
use hfst_swift_runtime::transducer::LookupResult;

// [spec:hfst:req:swift.transducer]
// [spec:hfst:req:swift.threads]
/// A transducer of any type the library reads. An operation holds its lock
/// for its length, so a call on a transducer another thread is changing
/// waits until the change is done, and never sees it half made.
#[derive(uniffi::Object)]
pub struct Transducer {
    machine: RwLock<Machine>,
}

impl Transducer {
    pub fn new(machine: Machine) -> Arc<Transducer> {
        Arc::new(Transducer {
            machine: RwLock::new(machine),
        })
    }

    /// The transducer to read, once no operation is changing it. An
    /// operation that failed partway leaves it as the library left it, which
    /// is still a transducer, so a poisoned lock is read through.
    fn reading(&self) -> RwLockReadGuard<'_, Machine> {
        self.machine.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn changing(&self) -> RwLockWriteGuard<'_, Machine> {
        self.machine.write().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn read<T>(&self, f: impl FnOnce(&Machine) -> Result<T, Error>) -> Result<T, GlueError> {
        Ok(f(&self.reading())?)
    }

    pub fn change<T>(
        &self,
        f: impl FnOnce(&mut Machine) -> Result<T, Error>,
    ) -> Result<T, GlueError> {
        Ok(f(&mut self.changing())?)
    }

    // [spec:hfst:req:swift.threads]
    /// `f` changing the transducer with `other` as its operand. When `other`
    /// is this transducer, the operation reads it as it was before the
    /// change.
    pub fn change_with<T>(
        &self,
        other: &Transducer,
        f: impl FnOnce(&mut Machine, Operand<'_>) -> Result<T, Error>,
    ) -> Result<T, GlueError> {
        let (changed, read) = locked(Some(self), &[other]);
        let mut changed = changed.ok_or_else(unlocked)?;
        Ok(match read.get(other) {
            Some(other) => f(&mut changed, Operand::Other(other)),
            None => f(&mut changed, Operand::Itself),
        }?)
    }
}

/// The error for a lock [`locked`] was asked for and did not take, which
/// cannot happen.
pub fn unlocked() -> Error {
    Error::new(Code::Internal, "the transducer to change was not locked")
}

fn address(t: &Transducer) -> usize {
    std::ptr::from_ref(t).addr()
}

/// The transducers a call reads, each locked once.
pub struct Reading<'a> {
    guards: Vec<(usize, RwLockReadGuard<'a, Machine>)>,
}

impl Reading<'_> {
    /// What `t` holds, unless `t` is the transducer the call changes.
    pub fn get(&self, t: &Transducer) -> Option<&Machine> {
        let at = address(t);
        self.guards
            .iter()
            .find(|(held, _)| *held == at)
            .map(|(_, guard)| &**guard)
    }
}

// [spec:hfst:req:swift.threads]
/// Locks `changed` to change and every transducer of `read` to read. Each is
/// locked once, however often it is named, and all of them in the order of
/// their addresses, so two calls that lock the same transducers take them in
/// the same order, and neither can hold one the other waits for while
/// waiting for one the other holds: `a.compose(b)` and `b.compose(a)` on two
/// threads take turns.
pub fn locked<'a>(
    changed: Option<&'a Transducer>,
    read: &[&'a Transducer],
) -> (Option<RwLockWriteGuard<'a, Machine>>, Reading<'a>) {
    let mut all: Vec<&'a Transducer> = read.iter().copied().chain(changed).collect();
    all.sort_by_key(|t| address(t));
    all.dedup_by_key(|t| address(t));
    let changed_at = changed.map(address);
    let mut guard = None;
    let mut reading = Reading { guards: Vec::new() };
    for t in all {
        if Some(address(t)) == changed_at {
            guard = Some(t.changing());
        } else {
            reading.guards.push((address(t), t.reading()));
        }
    }
    (guard, reading)
}

/// `f` reading every transducer of `transducers`, each locked once, in the
/// order they were given.
pub fn reading_all<T>(
    transducers: &[Arc<Transducer>],
    f: impl FnOnce(&[&Machine]) -> Result<T, Error>,
) -> Result<T, GlueError> {
    let refs: Vec<&Transducer> = transducers.iter().map(Arc::as_ref).collect();
    let (_, read) = locked(None, &refs);
    let machines: Vec<&Machine> = refs.iter().filter_map(|t| read.get(t)).collect();
    Ok(f(&machines)?)
}

/// The type `name` names, as `hfst fst2fst --format` names it.
pub fn format_named(name: &str) -> Result<Format, GlueError> {
    Format::ALL
        .iter()
        .find(|format| format.name() == name)
        .copied()
        .ok_or_else(|| invalid(format!("there is no transducer type called {name}")))
}

#[uniffi::export]
impl Transducer {
    pub fn format(&self) -> String {
        self.reading().format().name().to_owned()
    }

    pub fn name(&self) -> String {
        self.reading().name()
    }

    pub fn properties(&self) -> HashMap<String, String> {
        self.reading().properties().into_iter().collect()
    }

    // [spec:hfst:req:swift.transducer]
    /// What `divvun-hfst`'s `Transducer.lookup` returns for `input`.
    pub fn lookup(
        &self,
        input: String,
        limit: Option<i64>,
        time_limit: Option<f64>,
    ) -> Result<Vec<LookupResult>, GlueError> {
        let limit = args::limit(limit)?;
        let time_limit = args::seconds("a lookup time limit", time_limit)?;
        let found = self.read(|t| t.lookup(&input, limit, time_limit))?;
        Ok(found.into_iter().map(LookupResult::from).collect())
    }

    // [spec:hfst:req:swift.transducer]
    pub fn copy(&self) -> Result<Arc<Transducer>, GlueError> {
        self.read(Machine::copy).map(Transducer::new)
    }

    // [spec:hfst:req:swift.operations]
    /// Converts the transducer to the type `format` names, as `hfst fst2fst`
    /// converts it.
    pub fn convert(&self, format: String, quick: bool) -> Result<(), GlueError> {
        let format = format_named(&format)?;
        self.change(|t| t.convert(format, quick))
    }
}

// [spec:hfst:req:swift.transducer]
/// Every transducer in the bytes of a stream, of any type the library reads.
#[uniffi::export]
pub fn read_transducers(bytes: Vec<u8>) -> Result<Vec<Arc<Transducer>>, GlueError> {
    let machines = embed::read_transducers(&bytes)?;
    Ok(machines.into_iter().map(Transducer::new).collect())
}

/// Refuses a stream that does not hold exactly one, as `transducer-count`,
/// in an error that names `call`.
pub fn single(call: &str, mut all: Vec<Arc<Transducer>>) -> Result<Arc<Transducer>, GlueError> {
    if all.len() == 1
        && let Some(transducer) = all.pop()
    {
        return Ok(transducer);
    }
    Err(Error::new(
        Code::TransducerCount,
        format!(
            "{call} takes exactly one transducer, and the stream holds {}",
            all.len()
        ),
    )
    .into())
}

// [spec:hfst:req:swift.transducer]
#[uniffi::export]
pub fn read_transducer(bytes: Vec<u8>) -> Result<Arc<Transducer>, GlueError> {
    single("Transducer(data:)", read_transducers(bytes)?)
}

// [spec:hfst:req:swift.transducer]
/// The transducer of the THFST directory at `path`, as `hfst fst2fst` reads
/// one.
#[uniffi::export]
pub fn read_thfst(path: String) -> Result<Arc<Transducer>, GlueError> {
    Ok(Transducer::new(embed::read_thfst(std::path::Path::new(
        &path,
    ))?))
}
