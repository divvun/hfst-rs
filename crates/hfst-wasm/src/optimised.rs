//! The optimised lookup: an optimized-lookup transducer read by divvunspell's
//! `divvun-fst`, and words looked up in it by that crate's lookup.

use hfst_embed::optimised::{Lookup, OptimisedTransducer as Machine};
use wasm_bindgen::prelude::*;

use crate::transducer::property_pairs;
use hfst_wasm_shared::GlueError;
use hfst_wasm_shared::lookup::encode;

// [spec:hfst:req:javascript.optimised-lookup+2]
/// An optimized-lookup transducer as `divvun-fst` reads it, with the lookup
/// tables it reuses from one word to the next. The wrapper's
/// `OptimisedTransducer` class in `js/hfst/optimised.ts` owns one.
#[wasm_bindgen]
pub struct OptimisedTransducer {
    machine: Machine,
    tables: Lookup,
}

#[wasm_bindgen]
impl OptimisedTransducer {
    /// The optimized-lookup format `divvun-fst` read the tables in.
    pub fn format(&self) -> String {
        self.machine.format().name().to_owned()
    }

    pub fn name(&self) -> String {
        self.machine.name().to_owned()
    }

    #[wasm_bindgen(js_name = propertyPairs)]
    pub fn property_pairs(&self) -> Vec<String> {
        property_pairs(self.machine.properties())
    }

    // [spec:hfst:req:javascript.optimised-lookup+2]
    /// What `divvun-fst`'s lookup returns for `input`, the first `limit` of
    /// them when there is a limit, as the JSON text `lookup::encode` writes.
    pub fn lookup(&mut self, input: &str, limit: Option<u32>) -> String {
        let limit = limit.map(|limit| usize::try_from(limit).unwrap_or(usize::MAX));
        let results = self.machine.lookup(&mut self.tables, input, limit);
        encode(
            results
                .iter()
                .map(|result| (result.output.as_str(), &result.symbols[..], result.weight.0)),
        )
    }
}

// [spec:hfst:req:javascript.optimised-lookup+2]
// [spec:hfst:req:javascript.load-bytes+2]
/// The one optimized-lookup transducer in `bytes`, read by `divvun-fst`. The
/// bytes are copied into the module once, and handed on without another copy
/// when they begin with an HFST3 header.
#[wasm_bindgen(js_name = readOptimisedTransducer)]
pub fn read_optimised_transducer(bytes: Vec<u8>) -> Result<OptimisedTransducer, GlueError> {
    Ok(OptimisedTransducer {
        machine: Machine::read(bytes, &crate::PACKAGE)?,
        tables: Lookup::new(),
    })
}

#[cfg(test)]
mod tests {
    use hfst_embed::format::Format;

    use super::read_optimised_transducer;
    use crate::lookup::tests::stream;

    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn the_optimised_lookup_gives_its_results_as_json() {
        let bytes = stream("{cat}:{dog}::2", Format::OptimizedLookupWeighted);
        let mut t = read_optimised_transducer(bytes).expect("read");
        assert_eq!(t.format(), "optimized-lookup-weighted");
        let found = t.lookup("cat", None);
        assert!(found.contains("\"output\":\"dog\""), "{found}");
        assert!(found.contains("\"weight\":2"), "{found}");
        assert_eq!(t.lookup("dog", Some(1)), "[]");
        let refused = read_optimised_transducer(stream("a", Format::Foma)).err();
        assert_eq!(refused.map(|e| e.code()), Some("unsupported-format"));
    }
}
