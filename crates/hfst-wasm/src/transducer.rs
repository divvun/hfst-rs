//! Transducers of every format the library reads and writes, as the full
//! package holds them. The runtime glue's `Transducer` holds optimized-lookup
//! tables only; this one holds the library's stream sum, so it can be
//! written, converted and printed.

use hfst::file_resolver::{FileResolver, MemoryFiles};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use hfst_wasm_runtime::{Format, GlueError, property_pairs, stream};
use wasm_bindgen::prelude::*;

/// A transducer of any format the full package carries. The wrapper's
/// `Transducer` class in `js/hfst/transducer.ts` owns one.
#[wasm_bindgen]
pub struct FullTransducer {
    any: AnyTransducer,
    format: Format,
}

impl FullTransducer {
    pub(crate) fn new(any: AnyTransducer) -> Result<Self, GlueError> {
        let ty = any.get_type();
        let format = Format::of(ty).ok_or_else(|| {
            GlueError::new(
                "internal",
                format!(
                    "the library produced a transducer of type {ty:?}, which has no format name"
                ),
            )
        })?;
        Ok(FullTransducer { any, format })
    }
}

#[wasm_bindgen]
impl FullTransducer {
    pub fn format(&self) -> String {
        self.format.name().to_owned()
    }

    pub fn name(&self) -> String {
        self.any.get_name()
    }

    #[wasm_bindgen(js_name = propertyPairs)]
    pub fn property_pairs(&self) -> Vec<String> {
        property_pairs(self.any.get_properties())
    }

    // [spec:hfst:req:javascript.load-bytes+1]
    /// This transducer as an HFST3 stream in its own format: what
    /// `HfstOutputStream` writes for it.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&mut self) -> Result<Vec<u8>, GlueError> {
        const FILE: &str = "transducer";
        let files = MemoryFiles::default();
        let sink = files
            .create(FILE)
            .map_err(|error| GlueError::new("internal", error.to_string()))?;
        let mut stream = HfstOutputStream::from_writer(sink, self.any.get_type(), true)?;
        self.any.write(&mut stream)?;
        stream.close();
        files
            .get(FILE)
            .ok_or_else(|| GlueError::new("internal", "the stream wrote no file"))
    }

    // [spec:hfst:req:javascript.load-bytes+1]
    /// A new transducer of the format named `format`, made as `hfst fst2fst`
    /// makes it. This one is left as it was.
    pub fn convert(&self, format: &str) -> Result<FullTransducer, GlueError> {
        let format = parse_format(format)?;
        FullTransducer::new(self.any.converted(format.implementation_type(), "")?)
    }

    // [spec:hfst:req:javascript.load-bytes+1]
    /// This transducer as AT&T text, as `hfst fst2txt` prints it: weights
    /// for every format but foma, which has none.
    #[wasm_bindgen(js_name = toAtt)]
    pub fn to_att(&self) -> Result<String, GlueError> {
        let weights = self.format != Format::Foma;
        let mut text = Vec::new();
        let written = match &self.any {
            AnyTransducer::Tropical(t) => t.write_in_att_format_file(&mut text, weights),
            AnyTransducer::OlW(t) => t.write_in_att_format_file(&mut text, weights),
            AnyTransducer::OlU(t) => t.write_in_att_format_file(&mut text, weights),
            AnyTransducer::Foma(t) => t.write_in_att_format_file(&mut text, weights),
            AnyTransducer::Thfst(t) => t.write_in_att_format_file(&mut text, weights),
        };
        written.map_err(|error| GlueError::new("internal", error.to_string()))?;
        Ok(String::from_utf8_lossy(&text).into_owned())
    }
}

// [spec:hfst:req:javascript.load-bytes+1]
/// Every transducer in `bytes`, in stream order, read by `HfstInputStream`.
/// The first member is described the way the runtime describes it, so bytes
/// neither package reads fail with the runtime's error.
#[wasm_bindgen(js_name = readAnyTransducers)]
pub fn read_any_transducers(bytes: &[u8]) -> Result<Vec<FullTransducer>, GlueError> {
    if bytes.is_empty() {
        return Err(stream::empty());
    }
    stream::member(bytes)?;
    let mut cursor = bytes;
    let mut input = HfstInputStream::read_from(&mut cursor).map_err(stream::read_error)?;
    let mut transducers = Vec::new();
    while input.is_good() {
        let any = input.read().map_err(stream::read_error)?;
        transducers.push(FullTransducer::new(any)?);
    }
    Ok(transducers)
}

// [spec:hfst:req:javascript.load-bytes+1]
/// Every transducer in the AT&T text `text`, in the format named `format`,
/// made as `hfst txt2fst` makes them: read with `@0@` as epsilon into a
/// tropical transducer, then converted.
#[wasm_bindgen(js_name = readAtt)]
pub fn read_att(text: &str, format: &str) -> Result<Vec<FullTransducer>, GlueError> {
    let format = parse_format(format)?;
    let mut input = text.as_bytes();
    let mut transducers = Vec::new();
    while !input.is_empty() {
        let mut lines = 0;
        let net = HfstBasicTransducer::read_in_att_format(&mut input, "@0@", &mut lines, false)?;
        let tropical = AnyTransducer::Tropical(HfstTransducer::new_from_basic(&net)?);
        let any = tropical.convert(format.implementation_type(), "")?;
        transducers.push(FullTransducer::new(any)?);
    }
    if transducers.is_empty() {
        return Err(GlueError::new(
            "invalid-format",
            "the text is empty, and AT&T text holds at least one transducer",
        ));
    }
    Ok(transducers)
}

fn parse_format(name: &str) -> Result<Format, GlueError> {
    Format::from_name(name).ok_or_else(|| {
        GlueError::new(
            "invalid-argument",
            format!("there is no transducer type called {name}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATT: &str = "0\t1\ta\tb\t0.5\n1\t2\tc\t@0@\t0\n2\t0.25\n";

    fn one(transducers: Result<Vec<FullTransducer>, GlueError>) -> FullTransducer {
        let mut all = transducers.expect("reads");
        assert_eq!(all.len(), 1);
        all.remove(0)
    }

    // Reading gives a transducer whose header had no name an empty one, as
    // the C++ reader did, so the bytes settle after the first round.
    #[test]
    fn every_format_survives_bytes_and_text() {
        for format in Format::ALL {
            let mut t = one(read_att(ATT, format.name()));
            assert_eq!(t.format(), format.name());
            let mut again = one(read_any_transducers(&t.to_bytes().expect("writes")));
            assert_eq!(again.format(), format.name());
            assert_eq!(again.to_att().expect("prints"), t.to_att().expect("prints"));
            let bytes = again.to_bytes().expect("writes again");
            let mut third = one(read_any_transducers(&bytes));
            assert_eq!(third.to_bytes().expect("writes a third time"), bytes);
        }
    }

    #[test]
    fn convert_reaches_every_format_and_leaves_the_source() {
        for from in Format::ALL {
            let mut source = one(read_att(ATT, from.name()));
            let before = source.to_bytes().expect("writes");
            for to in Format::ALL {
                let converted = source.convert(to.name()).expect("converts");
                assert_eq!(converted.format(), to.name(), "{from:?} -> {to:?}");
            }
            assert_eq!(source.to_bytes().expect("writes"), before);
        }
    }

    #[test]
    fn unknown_type_names_are_invalid_arguments() {
        let t = one(read_att(ATT, "openfst-tropical"));
        assert!(matches!(t.convert("thfst"), Err(e) if e.code() == "invalid-argument"));
        assert!(matches!(read_att(ATT, "sfst"), Err(e) if e.code() == "invalid-argument"));
    }

    #[test]
    fn several_att_transducers_read_in_order() {
        let all = read_att(&format!("{ATT}--\n0\t1\tx\tx\n1\n"), "foma").expect("reads");
        assert_eq!(all.len(), 2);
    }
}
