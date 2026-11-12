//! Lookup in a transducer the package read.

use hfst_wasm_shared::GlueError;
use hfst_wasm_shared::lookup::{LookupLimits, encode};
use wasm_bindgen::prelude::*;

use crate::transducer::Transducer;

#[wasm_bindgen]
impl Transducer {
    // [spec:hfst:req:javascript.lookup]
    /// Every result the library's flag-aware lookup finds for `input` within
    /// the limits, in the JSON text `encode` writes.
    pub fn lookup(
        &self,
        input: &str,
        limit: Option<u32>,
        time_limit_ms: Option<f64>,
    ) -> Result<String, GlueError> {
        let limits = LookupLimits::new(limit, time_limit_ms)?;
        let results = self.machine.lookup(input, limits.results, limits.seconds);
        Ok(encode(results.iter().map(|result| {
            (result.output.as_str(), &result.symbols[..], result.weight)
        })))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use hfst_embed::compile::{MemoryFiles, RegexpOptions, Source, WriteOptions, compile_regexps};
    use hfst_embed::format::Format;

    use crate::transducer::read_transducers;

    /// The bytes of a stream holding the regular expression `regex` in
    /// `format`, with an HFST3 header.
    pub fn stream(regex: &str, format: Format) -> Vec<u8> {
        let source = Source {
            name: "test.xre",
            text: regex,
        };
        let files = Arc::new(MemoryFiles::default());
        let compiled = compile_regexps(source, format, &RegexpOptions::default(), files)
            .unwrap_or_else(|_| panic!("{regex} compiles"));
        let [t] = &compiled.value[..] else {
            panic!("{regex} is one transducer");
        };
        t.to_bytes(&WriteOptions::default()).expect("written")
    }

    // [spec:hfst:req:javascript.lookup/test]
    #[test]
    fn lookup_gives_each_result_as_json() {
        let bytes = stream(
            "{cat}:{dog}::0.5 | {cat}:{cow}::1",
            Format::OptimizedLookupWeighted,
        );
        let [t] = &read_transducers(&bytes).expect("read")[..] else {
            panic!("one transducer");
        };
        assert_eq!(
            t.lookup("cat", None, None).expect("looked up"),
            "[{\"output\":\"dog\",\"symbols\":[\"d\",\"o\",\"g\"],\"weight\":0.5},\
             {\"output\":\"cow\",\"symbols\":[\"c\",\"o\",\"w\"],\"weight\":1}]"
        );
        assert_eq!(t.lookup("horse", None, None).expect("looked up"), "[]");
        let limited = t.lookup("cat", Some(1), Some(0.0)).expect("looked up");
        assert_eq!(limited.matches("output").count(), 1, "{limited}");
        let refused = t.lookup("cat", None, Some(-1.0)).expect_err("refused");
        assert_eq!(refused.code(), "invalid-argument");
    }
}
