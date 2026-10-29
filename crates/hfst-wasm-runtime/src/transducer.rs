//! Transducers loaded from bytes, as the runtime package holds them.

use std::collections::BTreeMap;

use hfst::transducer::{
    ContainerAlphabet, OlMember, Transducer as OlTransducer, UnweightedTables, WeightedTables,
};
use wasm_bindgen::prelude::*;

use crate::error::GlueError;
use crate::format::Format;
use crate::stream::{self, Properties};

pub enum Machine {
    Weighted(OlTransducer<WeightedTables>),
    Unweighted(OlTransducer<UnweightedTables>),
}

/// A transducer the runtime read, with the properties of its HFST3 header.
/// The wrapper's `Transducer` class in `js/hfst-runtime/transducer.ts` owns
/// one.
#[wasm_bindgen]
pub struct Transducer {
    machine: Machine,
    properties: Properties,
}

#[wasm_bindgen]
impl Transducer {
    pub fn format(&self) -> String {
        let format = match self.machine {
            Machine::Weighted(_) => Format::OptimizedLookupWeighted,
            Machine::Unweighted(_) => Format::OptimizedLookupUnweighted,
        };
        format.name().to_owned()
    }

    pub fn name(&self) -> String {
        self.properties.get("name").cloned().unwrap_or_default()
    }

    #[wasm_bindgen(js_name = propertyPairs)]
    pub fn property_pairs(&self) -> Vec<String> {
        property_pairs(&self.properties)
    }
}

/// A transducer's properties as the wrapper shows them, flattened to key,
/// value, key, value. `version` and `type` describe the stream, not the
/// transducer, and `name` has its own accessor, so all three are left out.
pub fn property_pairs(properties: &BTreeMap<String, String>) -> Vec<String> {
    properties
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "version" | "type" | "name"))
        .flat_map(|(key, value)| [key.clone(), value.clone()])
        .collect()
}

// [spec:hfst:req:javascript.load-bytes+1]
/// Every transducer in `bytes`, in stream order. Optimized-lookup members are
/// read with the library's member reader, so a pmatch container's
/// definitions take its first transducer's alphabet as they do natively.
/// Members in the formats only `@divvun/hfst` reads are turned down before any
/// reader runs.
#[wasm_bindgen(js_name = readTransducers)]
pub fn read_transducers(bytes: &[u8]) -> Result<Vec<Transducer>, GlueError> {
    if bytes.is_empty() {
        return Err(stream::empty());
    }
    let mut rest = bytes;
    let mut first: Option<Format> = None;
    let mut container: Option<ContainerAlphabet> = None;
    let mut transducers = Vec::new();
    while !rest.is_empty() {
        let member = stream::member(rest)?;
        let format = *first.get_or_insert(member.format);
        if member.format != format {
            return Err(stream::mixed(format, member.format));
        }
        if !format.in_runtime() {
            return Err(GlueError::new(
                "unsupported-format",
                format!(
                    "the bytes hold a {} transducer, which @divvun/hfst-runtime does not read; @divvun/hfst reads it",
                    format.name()
                ),
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
fn ol_error(error: hfst::error::Error, used_up: bool) -> GlueError {
    if used_up {
        return GlueError::new(
            "invalid-format",
            format!(
                "the stream ends inside an optimized-lookup transducer, so the bytes are truncated ({error})"
            ),
        );
    }
    stream::read_error(error)
}
