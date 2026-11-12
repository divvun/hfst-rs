//! Transducers loaded from bytes.

use hfst_embed::transducer::Transducer as Machine;
use wasm_bindgen::prelude::*;

use hfst_wasm_shared::GlueError;

/// A transducer the package read. The wrapper's `Transducer` class in
/// `js/hfst/transducer.ts` owns one.
#[wasm_bindgen]
pub struct Transducer {
    pub(crate) machine: Machine,
}

#[wasm_bindgen]
impl Transducer {
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
}

/// Properties as the wrapper reads them, flattened to key, value, key, value.
pub(crate) fn property_pairs<'a>(
    properties: impl Iterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    properties
        .flat_map(|(key, value)| [key.to_owned(), value.to_owned()])
        .collect()
}

// [spec:hfst:req:javascript.load-bytes+2]
/// Every transducer in `bytes`, in stream order.
#[wasm_bindgen(js_name = readTransducers)]
pub fn read_transducers(bytes: &[u8]) -> Result<Vec<Transducer>, GlueError> {
    let machines = hfst_embed::transducer::read_transducers(bytes, &crate::PACKAGE)?;
    Ok(machines
        .into_iter()
        .map(|machine| Transducer { machine })
        .collect())
}

#[cfg(test)]
mod tests {
    use hfst_embed::format::Format;

    use super::read_transducers;
    use crate::lookup::tests::stream;

    // [spec:hfst:req:javascript.load-bytes+2/test]
    #[test]
    fn a_transducer_says_its_type_name_and_properties() {
        let bytes = stream("a", Format::OptimizedLookupUnweighted);
        let [t] = &read_transducers(&bytes).expect("read")[..] else {
            panic!("one transducer");
        };
        assert_eq!(t.format(), "optimized-lookup-unweighted");
        assert_eq!(t.name(), "xre(?)");
        assert_eq!(t.property_pairs().len() % 2, 0, "key, value, key, value");
        let tropical = stream("a", Format::OpenFstTropical);
        let refused = read_transducers(&tropical).err().expect("refused");
        assert_eq!(refused.code(), "unsupported-format");
        let mut both = stream("a", Format::OptimizedLookupWeighted);
        both.extend(stream("b", Format::OptimizedLookupWeighted));
        assert_eq!(
            read_transducers(&both).expect("read").len(),
            2,
            "in stream order"
        );
    }

    #[test]
    fn the_wrapper_lists_exactly_the_formats_read() {
        let wrapper = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/transducer.ts"
        ))
        .expect("the wrapper is in the repository");
        for format in Format::ALL {
            let name = format!("\"{}\"", format.name());
            assert_eq!(
                wrapper.contains(&name),
                format.is_read(),
                "the wrapper lists {name} exactly when the package reads it"
            );
        }
    }
}
