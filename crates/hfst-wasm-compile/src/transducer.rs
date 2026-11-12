//! Transducers of every type the library reads, read from bytes or made by
//! a compiler, and lookup in them.

use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use hfst_embed::Code;
use hfst_embed::compile::{self as embed, Operand, Transducer as Machine};
use hfst_wasm_shared::GlueError;
use hfst_wasm_shared::lookup::{LookupLimits, encode};
use wasm_bindgen::prelude::*;

use crate::options::format_named;

/// A transducer of any type: tropical OpenFst, foma, and optimized lookup,
/// weighted and unweighted. The wrapper's `Transducer` class in
/// `js/hfst/compile-transducer.ts` owns one.
///
/// The machine is shared, so a [`crate::TransducerList`] can name it
/// without copying it, and every method takes `&self`, so a call that names
/// one transducer twice, as `t.compose(t)` does, borrows it twice from
/// wasm-bindgen without a conflict. An operation borrows the machine to
/// change it only after it has found which of its operands are itself.
#[wasm_bindgen]
pub struct Transducer {
    pub(crate) machine: Rc<RefCell<Machine>>,
}

/// The error for a transducer a call is changing, which only a logger or a
/// files function that calls back into the module mid-call can see.
fn busy() -> GlueError {
    GlueError::new(
        Code::InvalidArgument,
        "the transducer is in use by a call that has not returned",
    )
}

impl Transducer {
    pub(crate) fn new(machine: Machine) -> Transducer {
        Transducer {
            machine: Rc::new(RefCell::new(machine)),
        }
    }

    pub(crate) fn reading(&self) -> Result<Ref<'_, Machine>, GlueError> {
        self.machine.try_borrow().map_err(|_| busy())
    }

    pub(crate) fn changing(&self) -> Result<RefMut<'_, Machine>, GlueError> {
        self.machine.try_borrow_mut().map_err(|_| busy())
    }

    pub(crate) fn is(&self, other: &Rc<RefCell<Machine>>) -> bool {
        Rc::as_ptr(&self.machine) == Rc::as_ptr(other)
    }

    /// `f` changing the transducer with `other` as its operand. When `other`
    /// is this transducer, the operation reads it as it was before the
    /// change.
    pub(crate) fn change_with<T>(
        &self,
        other: &Transducer,
        f: impl FnOnce(&mut Machine, Operand<'_>) -> Result<T, hfst_embed::Error>,
    ) -> Result<T, GlueError> {
        if self.is(&other.machine) {
            return Ok(f(&mut *self.changing()?, Operand::Itself)?);
        }
        let other = other.reading()?;
        Ok(f(&mut *self.changing()?, Operand::Other(&other))?)
    }
}

pub(crate) fn property_pairs(properties: Vec<(String, String)>) -> Vec<String> {
    properties
        .into_iter()
        .flat_map(|(key, value)| [key, value])
        .collect()
}

#[wasm_bindgen]
impl Transducer {
    // [spec:hfst:req:javascript.compile-module]
    /// The name of its type, one the wrapper lists.
    pub fn format(&self) -> Result<String, GlueError> {
        Ok(self.reading()?.format().name().to_owned())
    }

    pub fn name(&self) -> Result<String, GlueError> {
        Ok(self.reading()?.name())
    }

    /// Its other properties, flattened to key, value, key, value.
    #[wasm_bindgen(js_name = propertyPairs)]
    pub fn property_pairs(&self) -> Result<Vec<String>, GlueError> {
        Ok(property_pairs(self.reading()?.properties()))
    }

    // [spec:hfst:req:javascript.compile-module]
    /// A transducer of its own with the same contents.
    pub fn copy(&self) -> Result<Transducer, GlueError> {
        Ok(Transducer::new(self.reading()?.copy()?))
    }

    // [spec:hfst:req:javascript.compile-module]
    /// Converts the transducer to the type named `format`, as `hfst fst2fst`
    /// converts it; `quick` packs optimized-lookup tables less tightly.
    pub fn convert(&self, format: &str, quick: bool) -> Result<(), GlueError> {
        let format = format_named("convert", format)?;
        Ok(self.changing()?.convert(format, quick)?)
    }

    // [spec:hfst:req:javascript.compile-module]
    /// Every result the hfst library's lookup finds for `input` within the
    /// limits, in the JSON text the main module's lookup writes.
    pub fn lookup(
        &self,
        input: &str,
        limit: Option<u32>,
        time_limit_ms: Option<f64>,
    ) -> Result<String, GlueError> {
        let limits = LookupLimits::new(limit, time_limit_ms)?;
        let results = self
            .reading()?
            .lookup(input, limits.results, limits.seconds)?;
        Ok(encode(results.iter().map(|result| {
            (result.output.as_str(), &result.symbols[..], result.weight)
        })))
    }
}

// [spec:hfst:req:javascript.compile-module]
/// Every transducer in `bytes`, in stream order, of any type the library
/// reads.
#[wasm_bindgen(js_name = readTransducers)]
pub fn read_transducers(bytes: &[u8]) -> Result<Vec<Transducer>, GlueError> {
    Ok(embed::read_transducers(bytes)?
        .into_iter()
        .map(Transducer::new)
        .collect())
}

#[cfg(test)]
mod tests {
    use hfst_embed::format::Format;

    #[test]
    fn the_wrapper_lists_every_type() {
        let wrapper = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/compile-transducer-base.ts"
        ))
        .expect("the wrapper is in the repository");
        for format in Format::ALL {
            let name = format!("\"{}\"", format.name());
            assert!(wrapper.contains(&name), "the wrapper lists {name}");
        }
    }
}
