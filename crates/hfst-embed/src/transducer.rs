//! Optimized-lookup transducers read from bytes, and lookup in them with the
//! hfst library's engine.

use hfst::lookup_state::LookupState;
use hfst::transducer::{
    ContainerAlphabet, OlMember, Transducer as OlTransducer, UnweightedTables, WeightedTables,
};

use crate::Package;
use crate::error::{Code, Error};
use crate::format::Format;
use crate::lookup::{LookupResult, lookup_results};
use crate::stream::{self, Properties};

enum Machine {
    Weighted(OlTransducer<WeightedTables>),
    Unweighted(OlTransducer<UnweightedTables>),
}

/// A transducer read from a stream, with the properties of its HFST3 header.
pub struct Transducer {
    machine: Machine,
    properties: Properties,
}

impl Transducer {
    pub fn format(&self) -> Format {
        match self.machine {
            Machine::Weighted(_) => Format::OptimizedLookupWeighted,
            Machine::Unweighted(_) => Format::OptimizedLookupUnweighted,
        }
    }

    /// The `name` property of its header, or `""` without one.
    pub fn name(&self) -> &str {
        self.properties.get("name").map_or("", String::as_str)
    }

    /// The properties of its header but `version`, `type` and `name`, in key
    /// order.
    pub fn properties(&self) -> impl Iterator<Item = (&str, &str)> {
        stream::shown(&self.properties)
    }

    // [spec:hfst:req:javascript.lookup]
    // [spec:hfst:req:python.lookup]
    /// Every result the library's flag-aware lookup finds for `input`, as
    /// [`lookup_results`] orders them. The search stops once it has found
    /// `limit` paths, or once `time_limit` seconds have passed, keeping what
    /// it found; `None` sets no limit, and so does a time limit of 0. The time
    /// limit is a finite number of seconds, 0 or more, which each package
    /// checks in its own terms.
    ///
    /// Each lookup makes its own run state: the library's run state borrows
    /// the machine it runs on, so one kept beside that machine would make this
    /// object refer to itself, and one run state per lookup lets threads look
    /// up in one transducer at once. Making one costs a reference count and
    /// empty tapes; the tables are borrowed, never copied.
    pub fn lookup(
        &self,
        input: &str,
        limit: Option<usize>,
        time_limit: Option<f64>,
    ) -> Vec<LookupResult> {
        // A cap the library's count cannot hold is no cap, since nothing finds
        // that many results.
        let cap = limit.map_or(-1, |limit| isize::try_from(limit).unwrap_or(isize::MAX));
        let seconds = time_limit.unwrap_or(0.0);
        let paths = match &self.machine {
            Machine::Weighted(machine) => LookupState::new(machine).lookup_fd(input, cap, seconds),
            Machine::Unweighted(machine) => {
                LookupState::new(machine).lookup_fd(input, cap, seconds)
            }
        };
        lookup_results(paths)
    }
}

// [spec:hfst:req:javascript.load-bytes+2]
// [spec:hfst:req:python.load]
/// Every transducer in `bytes`, in stream order. Optimized-lookup members are
/// read with the library's member reader, so a pmatch container's
/// definitions take its first transducer's alphabet as they do natively.
/// Members in a format the packages do not read are turned down, by the
/// format's name, before any reader runs.
pub fn read_transducers(bytes: &[u8], package: &Package) -> Result<Vec<Transducer>, Error> {
    if bytes.is_empty() {
        return Err(stream::empty());
    }
    let mut rest = bytes;
    let mut first: Option<Format> = None;
    let mut container: Option<ContainerAlphabet> = None;
    let mut transducers = Vec::new();
    while !rest.is_empty() {
        let member = stream::member(rest, package)?;
        let format = *first.get_or_insert(member.format);
        if member.format != format {
            return Err(stream::mixed(format, member.format));
        }
        if !format.is_read() {
            return Err(stream::unsupported(
                &format!("a transducer in the {} format", format.name()),
                package,
            ));
        }
        let mut cursor = member.payload;
        let ol = OlMember::read_from(&mut cursor, container.as_ref())
            .map_err(|error| ol_error(error, cursor.is_empty()))?;
        rest = cursor;
        if container.is_none() && !rest.is_empty() {
            container = Some(ol.container_alphabet());
        }
        let machine = match ol {
            OlMember::Weighted(t) => Machine::Weighted(t),
            OlMember::Unweighted(t) => Machine::Unweighted(t),
        };
        transducers.push(Transducer {
            machine,
            properties: member.properties,
        });
    }
    Ok(transducers)
}

/// The error for an optimized-lookup payload that did not read. The library
/// reports a short read as the transducer having the wrong type, so a read
/// that used up the bytes is called what it is.
fn ol_error(error: hfst::error::Error, used_up: bool) -> Error {
    if used_up {
        return Error::new(
            Code::InvalidFormat,
            format!(
                "the stream ends inside an optimized-lookup transducer, so the bytes are truncated ({error})"
            ),
        );
    }
    stream::read_error(error)
}
