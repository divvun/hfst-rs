//! The optimised lookup: an optimized-lookup transducer read by divvunspell's
//! `divvun-fst`, and words looked up in it by that crate's lookup.

use std::fmt::Write as _;

use divvun_fst::transducer::hfst::HfstTransducer;
use divvun_fst::transducer::{Lookup, LookupResult, TransducerLoader};
use divvun_fst::vfs::bytes::{File, Filesystem};
use hfst::transducer::TransducerHeader;
use wasm_bindgen::prelude::*;

use crate::error::GlueError;
use crate::format::Format;
use crate::stream::{self, Member, Properties};
use crate::transducer::property_pairs;

/// The name the bytes go under in the in-memory file system `divvun-fst`
/// loads them from. Its errors quote it.
const NAME: &str = "transducer";

/// The magic an HFST3 header begins with, and the only header `divvun-fst`
/// reads optimized lookup after.
const HFST3: &[u8] = b"HFST\0";

/// Bytes of an index table record: input symbol and target.
const INDEX_RECORD: usize = 6;

/// Bytes of a transition table record: input, output and target, then a
/// weight in a weighted transducer.
const TRANSITION_RECORD_WEIGHTED: usize = 12;
const TRANSITION_RECORD_UNWEIGHTED: usize = 8;

// [spec:hfst:req:javascript.optimised-lookup+2]
/// An optimized-lookup transducer as `divvun-fst` reads it, with the lookup
/// tables it reuses from one word to the next and the properties of its HFST3
/// header. The wrapper's `OptimisedTransducer` class in `js/hfst/optimised.ts`
/// owns one.
#[wasm_bindgen]
pub struct OptimisedTransducer {
    transducer: HfstTransducer,
    lookup: Lookup,
    properties: Properties,
}

#[wasm_bindgen]
impl OptimisedTransducer {
    /// The optimized-lookup format `divvun-fst` read the tables in.
    pub fn format(&self) -> String {
        let format = if self.transducer.is_weighted() {
            Format::OptimizedLookupWeighted
        } else {
            Format::OptimizedLookupUnweighted
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

    // [spec:hfst:req:javascript.optimised-lookup+2]
    /// What `divvun-fst`'s lookup returns for `input`, the first `limit` of
    /// them when there is a limit, as the JSON text `encode` writes. One
    /// string crosses to JavaScript, where the wrapper parses it, rather than
    /// an object built a property at a time across the boundary.
    pub fn lookup(&mut self, input: &str, limit: Option<u32>) -> String {
        let results = self.lookup.lookup(&self.transducer, input);
        let kept = limit.map_or(results.len(), |limit| {
            usize::try_from(limit).map_or(results.len(), |limit| limit.min(results.len()))
        });
        encode(&results[..kept])
    }
}

// [spec:hfst:req:javascript.optimised-lookup+2]
// [spec:hfst:req:javascript.load-bytes+2]
/// The one optimized-lookup transducer in `bytes`, read by `divvun-fst`.
/// Bytes in another format are turned down as `readTransducers` turns them
/// down; a stream holding more than one transducer is turned down with
/// `transducer-count`. The bytes are handed to `divvun-fst` as they are when
/// they begin with an HFST3 header, so the copy into the module is the only
/// one; a transducer without that header is read after an empty one.
#[wasm_bindgen(js_name = readOptimisedTransducer)]
pub fn read_optimised_transducer(bytes: Vec<u8>) -> Result<OptimisedTransducer, GlueError> {
    if bytes.is_empty() {
        return Err(stream::empty());
    }
    let Member {
        format,
        properties,
        payload,
    } = stream::member(&bytes)?;
    if !format.is_read() {
        return Err(stream::unsupported(&format!(
            "a transducer in the {} format",
            format.name()
        )));
    }
    if payload.len() > ol_extent(payload)? {
        return Err(GlueError::new(
            "transducer-count",
            "readOptimisedTransducer takes exactly one transducer, and the stream holds more",
        ));
    }
    let bytes = if bytes.starts_with(HFST3) {
        bytes
    } else {
        with_empty_header(payload)
    };
    let mut files = Filesystem::new();
    files.insert(NAME, bytes);
    let transducer = <HfstTransducer as TransducerLoader<File>>::from_path(&files, NAME)
        .map_err(|error| GlueError::new("invalid-format", error.to_string()))?;
    Ok(OptimisedTransducer {
        transducer,
        lookup: Lookup::new(),
        properties,
    })
}

/// How many bytes the optimized-lookup transducer at the start of `payload`
/// takes: its header, alphabet, index table and transition table, laid out as
/// its header says. Bytes past it are the next transducer of a stream. A
/// payload too short to hold the header or alphabet is truncated.
fn ol_extent(payload: &[u8]) -> Result<usize, GlueError> {
    let truncated = || {
        GlueError::new(
            "invalid-format",
            "the stream ends inside an optimized-lookup transducer, so the bytes are truncated",
        )
    };
    let mut head = payload
        .get(..TransducerHeader::SIZE)
        .ok_or_else(truncated)?;
    let header = TransducerHeader::read_from(&mut head).map_err(stream::read_error)?;
    let mut at = TransducerHeader::SIZE;
    for _ in 0..header.symbol_count() {
        let rest = payload.get(at..).ok_or_else(truncated)?;
        let end = rest.iter().position(|&b| b == 0).ok_or_else(truncated)?;
        at += end + 1;
    }
    let record = if header.probe_flag(hfst::transducer::HeaderFlag::Weighted) {
        TRANSITION_RECORD_WEIGHTED
    } else {
        TRANSITION_RECORD_UNWEIGHTED
    };
    let tables = (header.index_table_size() as usize)
        .checked_mul(INDEX_RECORD)
        .zip((header.target_table_size() as usize).checked_mul(record))
        .and_then(|(index, transitions)| index.checked_add(transitions))
        .and_then(|tables| tables.checked_add(at));
    tables.ok_or_else(truncated)
}

/// `payload` after an HFST3 header with no properties: `divvun-fst` skips a
/// header's properties without reading them, and refuses tables without one.
fn with_empty_header(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HFST3.len() + 3 + payload.len());
    bytes.extend_from_slice(HFST3);
    bytes.extend_from_slice(&[0, 0, 0]);
    bytes.extend_from_slice(payload);
    bytes
}

/// `results` as a JSON array of `{ output, symbols, weight }` objects. Each
/// weight is the f64 that holds the f32 exactly, written in the fewest digits
/// that read back as that f64, so `JSON.parse` gives the same number. JSON
/// has no number for a weight that is not finite, so such a weight is the
/// string `Infinity`, `-Infinity` or `NaN`, which the wrapper turns back.
fn encode(results: &[LookupResult]) -> String {
    let mut text = String::with_capacity(results.len() * 64 + 2);
    text.push('[');
    for (i, result) in results.iter().enumerate() {
        if i > 0 {
            text.push(',');
        }
        text.push_str("{\"output\":");
        push_json_string(&mut text, &result.output);
        text.push_str(",\"symbols\":[");
        for (j, symbol) in result.symbols.iter().enumerate() {
            if j > 0 {
                text.push(',');
            }
            push_json_string(&mut text, symbol);
        }
        text.push_str("],\"weight\":");
        let weight = f64::from(result.weight.0);
        if weight.is_finite() {
            write!(text, "{weight}").expect("writing to a String does not fail");
        } else if weight.is_nan() {
            text.push_str("\"NaN\"");
        } else if weight > 0.0 {
            text.push_str("\"Infinity\"");
        } else {
            text.push_str("\"-Infinity\"");
        }
        text.push('}');
    }
    text.push(']');
    text
}

/// `value` as a JSON string. Only the quote, the backslash and the control
/// characters below U+0020 need escaping.
fn push_json_string(text: &mut String, value: &str) {
    text.push('"');
    let mut plain = 0;
    for (at, c) in value.char_indices() {
        if !matches!(c, '"' | '\\' | '\0'..='\u{1f}') {
            continue;
        }
        text.push_str(&value[plain..at]);
        match c {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            control => write!(text, "\\u{:04x}", u32::from(control))
                .expect("writing to a String does not fail"),
        }
        plain = at + c.len_utf8();
    }
    text.push_str(&value[plain..]);
    text.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use divvun_fst::types::Weight;
    use hfst::file_resolver::{FileResolver, MemoryFiles};
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer as AlgebraTransducer;

    /// `cat` analysed as `cat+N` (0.75) and as `cat` (1), and `ca` as `ca+X`
    /// (2), in optimized lookup, weighted or not, with an HFST3 header or
    /// without one.
    fn cat(weighted: bool, header: bool) -> Vec<u8> {
        let att = "0\t1\tc\tc\t0\n1\t2\ta\ta\t0\n2\t3\tt\tt\t0\n3\t4\t@0@\t+N\t0.5\n\
                   4\t0.25\n3\t1\n2\t5\t@0@\t+X\t2\n5\t0\n";
        let mut lines = 0u32;
        let net =
            HfstBasicTransducer::read_in_att_format(&mut att.as_bytes(), "@0@", &mut lines, false)
                .expect("the AT&T text parses");
        let mut ol = AlgebraTransducer::<hfst_openfst::StdVectorFst>::new_from_basic(&net)
            .expect("a transducer")
            .to_ol(weighted, "")
            .expect("optimized lookup");
        let format = if weighted {
            ImplementationType::HFST_OLW_TYPE
        } else {
            ImplementationType::HFST_OL_TYPE
        };
        let files = MemoryFiles::default();
        let sink = files.create("cat").expect("memory takes a file");
        let mut stream = HfstOutputStream::from_writer(sink, format, header)
            .expect("an optimized-lookup stream");
        stream.write(&mut ol).expect("the transducer writes");
        stream.close();
        files.get("cat").expect("the transducer was written")
    }

    fn code(result: Result<OptimisedTransducer, GlueError>) -> &'static str {
        match result {
            Ok(_) => "read",
            Err(error) => error.code(),
        }
    }

    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn both_formats_read_with_and_without_a_header() {
        for weighted in [true, false] {
            for header in [true, false] {
                let mut t =
                    read_optimised_transducer(cat(weighted, header)).expect("the transducer reads");
                let what = format!("weighted {weighted}, header {header}");
                let format = if weighted {
                    "optimized-lookup-weighted"
                } else {
                    "optimized-lookup-unweighted"
                };
                assert_eq!(t.format(), format, "{what}");
                let tagged = |weight| {
                    format!(
                        "{{\"output\":\"cat+N\",\"symbols\":[\"c\",\"a\",\"t\",\"+N\"],\"weight\":{weight}}}"
                    )
                };
                let bare = |weight| {
                    format!(
                        "{{\"output\":\"cat\",\"symbols\":[\"c\",\"a\",\"t\"],\"weight\":{weight}}}"
                    )
                };
                // Unweighted, both weigh 0, so the output orders them.
                let expected = if weighted {
                    format!("[{},{}]", tagged("0.75"), bare("1"))
                } else {
                    format!("[{},{}]", bare("0"), tagged("0"))
                };
                assert_eq!(t.lookup("cat", None), expected, "{what}");
                assert_eq!(t.lookup("dog", None), "[]", "{what}");
            }
        }
    }

    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn a_limit_keeps_the_lightest_results() {
        let mut t = read_optimised_transducer(cat(true, true)).expect("the transducer reads");
        let all = t.lookup("cat", None);
        let one = t.lookup("cat", Some(1));
        assert!(
            all.starts_with(one.trim_end_matches(']')),
            "{one} begins {all}"
        );
        assert!(one.contains("cat+N") && !one.contains("\"cat\""), "{one}");
        assert_eq!(t.lookup("cat", Some(0)), "[]");
        assert_eq!(t.lookup("cat", Some(9)), all);
        assert!(t.lookup("ca", None).contains("\"ca+X\""));
    }

    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn several_or_truncated_streams_are_turned_down() {
        for header in [true, false] {
            let one = cat(true, header);
            let two = [one.clone(), one.clone()].concat();
            assert_eq!(
                code(read_optimised_transducer(two)),
                "transducer-count",
                "header {header}"
            );
            for cut in [one.len() - 1, one.len() / 2, 70] {
                assert_eq!(
                    code(read_optimised_transducer(one[..cut].to_vec())),
                    "invalid-format",
                    "header {header}, {cut} bytes"
                );
            }
        }
        assert_eq!(
            code(read_optimised_transducer(Vec::new())),
            "invalid-format"
        );
        assert_eq!(
            code(read_optimised_transducer(b"##foma-net 1.0##\n".to_vec())),
            "unsupported-format"
        );
    }

    fn result(symbols: &[&str], weight: f32) -> LookupResult {
        LookupResult {
            output: symbols.concat().into(),
            symbols: symbols.iter().map(|&symbol| symbol.into()).collect(),
            weight: Weight(weight),
        }
    }

    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn results_encode_as_json_objects() {
        let text = encode(&[result(&["c", "a", "t", "+N"], 0.1), result(&[], 2.0)]);
        assert_eq!(
            text,
            "[{\"output\":\"cat+N\",\"symbols\":[\"c\",\"a\",\"t\",\"+N\"],\"weight\":0.10000000149011612},\
             {\"output\":\"\",\"symbols\":[],\"weight\":2}]"
        );
        assert_eq!(encode(&[]), "[]");
    }

    #[test]
    fn strings_escape_quotes_backslashes_and_control_characters() {
        let mut text = String::new();
        push_json_string(&mut text, "a\"b\\c\td\u{1}é😀");
        assert_eq!(text, "\"a\\\"b\\\\c\\u0009d\\u0001é😀\"");
    }

    #[test]
    fn weights_that_are_not_finite_are_named() {
        let text = encode(&[
            result(&["a"], f32::INFINITY),
            result(&["b"], f32::NEG_INFINITY),
            result(&["c"], f32::NAN),
        ]);
        assert!(text.contains("\"weight\":\"Infinity\""), "{text}");
        assert!(text.contains("\"weight\":\"-Infinity\""), "{text}");
        assert!(text.contains("\"weight\":\"NaN\""), "{text}");
    }
}
