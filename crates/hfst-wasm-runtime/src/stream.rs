//! The members of a transducer stream: what each one declares itself to be,
//! read the way `HfstInputStream` reads it, without calling any reader but
//! the optimized-lookup one. Both packages describe a stream's first member
//! with [`member`], so they turn down the same bytes with the same error.

use std::collections::BTreeMap;

use hfst::error::{Error, ErrorKind};
use hfst::transducer::{HeaderFlag, TransducerHeader};

use crate::error::{GlueError, code_of};
use crate::format::Format;

const INVALID: &str = "invalid-format";
const UNSUPPORTED: &str = "unsupported-format";

pub type Properties = BTreeMap<String, String>;

/// One transducer of a stream, as far as its first bytes tell.
pub struct Member<'a> {
    /// The format the member declares: its HFST3 header's type, or for a
    /// member with no header, the format its bytes begin with.
    pub format: Format,
    /// The pairs of its HFST3 header; empty for a member with no header.
    pub properties: Properties,
    /// The bytes after the header, to the end of the stream.
    pub payload: &'a [u8],
}

// [spec:hfst:req:javascript.load-bytes+1]
pub fn empty() -> GlueError {
    GlueError::new(
        INVALID,
        "the bytes are empty, and a transducer stream holds at least one transducer",
    )
}

/// The error for a stream whose members have different types, which neither
/// package reads, as `HfstInputStream` does not.
pub fn mixed(first: Format, then: Format) -> GlueError {
    GlueError::new(
        "type-mismatch",
        format!(
            "the stream holds transducers of different types, {} and then {}; a stream holds transducers of one type",
            first.name(),
            then.name()
        ),
    )
}

/// The error for a library reader that failed partway through a stream.
/// Only a stream mixing types and a type the library lacks keep their codes;
/// anything else says the bytes are not a well-formed stream, whatever the
/// library called it. The optimized-lookup reader reports a short read and a
/// corrupt header as the transducer having the wrong type, so that error is
/// told as what it means here.
pub fn read_error(error: Error) -> GlueError {
    if matches!(error.kind, ErrorKind::TransducerTypeMismatch) {
        return GlueError::new("type-mismatch", error.to_string());
    }
    if matches!(error.kind, ErrorKind::TransducerHasWrongType) {
        return GlueError::new(
            INVALID,
            "an optimized-lookup transducer in the stream ends early or has a corrupt header",
        );
    }
    let code = match code_of(&error.kind) {
        "unsupported-format" => UNSUPPORTED,
        _ => INVALID,
    };
    GlueError::new(code, error.to_string())
}

/// Describes the member at the start of `bytes`, which are not empty.
/// Bytes in a format no package reads fail here, so the caller sees only the
/// formats [`Format`] names.
// [spec:hfst:req:javascript.load-bytes+1]
pub fn member(bytes: &[u8]) -> Result<Member<'_>, GlueError> {
    if let Some(rest) = bytes.strip_prefix(b"HFST\0") {
        return hfst3(rest);
    }
    if let Some(rest) = bytes.strip_prefix(b"HFST3\0") {
        return pre_release(rest);
    }
    if bytes.starts_with(hfst::dhfst_header::MAGIC) {
        return Err(nobody_reads(
            "DHFST, a speller format that is not a transducer stream",
        ));
    }
    if bytes.starts_with(&[0x1f, 0x8b, 0x08]) {
        return Err(nobody_reads(
            "gzip-compressed data, as a native foma file is (Divvun HFST reads foma inside an HFST3 stream, which hfst fst2fst -F writes, or uncompressed)",
        ));
    }
    let headerless = |format| Member {
        format,
        properties: Properties::new(),
        payload: bytes,
    };
    if let Some(header) = TransducerHeader::sniff(bytes) {
        return Ok(headerless(if header.probe_flag(HeaderFlag::Weighted) {
            Format::OptimizedLookupWeighted
        } else {
            Format::OptimizedLookupUnweighted
        }));
    }
    if bytes.first() == Some(&0xd6) {
        // An OpenFst header: magic, then the length-prefixed FST type
        // "vector", then the arc type, whose first letter is at offset 18.
        return match bytes.get(18) {
            Some(b's') if bytes.len() >= 26 => Ok(headerless(Format::OpenFstTropical)),
            Some(b'l') => Err(nobody_reads("a log-weight OpenFst transducer")),
            Some(_) | None => Err(not_a_stream()),
        };
    }
    if bytes.starts_with(b"##foma-net") {
        return Ok(headerless(Format::Foma));
    }
    Err(not_a_stream())
}

fn not_a_stream() -> GlueError {
    GlueError::new(
        INVALID,
        "the bytes are not a transducer stream: they begin with neither an HFST3 header nor an optimized-lookup, OpenFst or foma transducer",
    )
}

fn nobody_reads(what: &str) -> GlueError {
    GlueError::new(
        UNSUPPORTED,
        format!("the bytes hold {what}, which Divvun HFST does not read"),
    )
}

fn malformed_header(what: &str) -> GlueError {
    GlueError::new(INVALID, format!("the HFST3 header is malformed: {what}"))
}

/// The NUL-terminated string at the start of `bytes`, and the bytes after it.
fn c_string(bytes: &[u8]) -> Option<(String, &[u8])> {
    let end = bytes.iter().position(|&b| b == 0)?;
    let text = String::from_utf8_lossy(&bytes[..end]).into_owned();
    Some((text, &bytes[end + 1..]))
}

/// An HFST3 header after its "HFST\0" magic: a little-endian length, a NUL,
/// then that many bytes of NUL-terminated key and value pairs. The first pair
/// gives the version and the second the type, as `HfstInputStream` requires.
fn hfst3(bytes: &[u8]) -> Result<Member<'_>, GlueError> {
    let [low, high, nul, rest @ ..] = bytes else {
        return Err(GlueError::new(
            INVALID,
            "the stream ends inside an HFST3 header",
        ));
    };
    if *nul != 0 {
        return Err(malformed_header("its size is not followed by a NUL"));
    }
    let size = usize::from(u16::from_le_bytes([*low, *high]));
    let (mut data, payload) = match (rest.get(..size), rest.get(size..)) {
        (Some(data), Some(payload)) => (data, payload),
        (Some(_) | None, Some(_) | None) => {
            return Err(GlueError::new(
                INVALID,
                format!(
                    "the stream ends inside an HFST3 header that claims {size} bytes of properties"
                ),
            ));
        }
    };
    let mut pairs = Vec::new();
    while !data.is_empty() {
        let pair = c_string(data)
            .and_then(|(key, after)| c_string(after).map(|(value, after)| ((key, value), after)));
        let Some((pair, after)) = pair else {
            return Err(malformed_header(
                "a property runs past the size the header gives",
            ));
        };
        pairs.push(pair);
        data = after;
    }
    let [(version_key, version), (type_key, type_name), ..] = pairs.as_slice() else {
        return Err(malformed_header("it has fewer than two properties"));
    };
    if version_key != "version" || !matches!(version.as_str(), "3.0" | "3.3") {
        return Err(malformed_header(
            "its first property is not version 3.0 or 3.3",
        ));
    }
    if type_key != "type" {
        return Err(malformed_header("its second property is not the type"));
    }
    let format = match type_name.as_str() {
        "HFST_OLW" => Format::OptimizedLookupWeighted,
        "HFST_OL" => Format::OptimizedLookupUnweighted,
        "TROPICAL_OPENFST" | "TROPICAL_OFST" => Format::OpenFstTropical,
        "FOMA" => Format::Foma,
        "SFST" => return Err(nobody_reads("an SFST transducer")),
        other => {
            return Err(malformed_header(&format!(
                "it names the type {other}, which is not a transducer type"
            )));
        }
    };
    Ok(Member {
        format,
        properties: pairs.into_iter().collect(),
        payload,
    })
}

/// The header pre-release versions of HFST 3 wrote: after "HFST3\0", the
/// implementation type's name and a NUL, and no properties.
fn pre_release(bytes: &[u8]) -> Result<Member<'_>, GlueError> {
    let Some((type_name, payload)) = c_string(bytes) else {
        return Err(GlueError::new(
            INVALID,
            "the stream ends inside an HFST3 header",
        ));
    };
    let format = match type_name.as_str() {
        "HFST_OLW_TYPE" => Format::OptimizedLookupWeighted,
        "HFST_OL_TYPE" => Format::OptimizedLookupUnweighted,
        "TROPICAL_OPENFST_TYPE" => Format::OpenFstTropical,
        "FOMA_TYPE" => Format::Foma,
        "SFST_TYPE" => return Err(nobody_reads("an SFST transducer")),
        other => {
            return Err(malformed_header(&format!(
                "it names the type {other}, which is not a transducer type"
            )));
        }
    };
    Ok(Member {
        format,
        properties: Properties::new(),
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(pairs: &[(&str, &str)]) -> Vec<u8> {
        let mut data = Vec::new();
        for (key, value) in pairs {
            data.extend_from_slice(key.as_bytes());
            data.push(0);
            data.extend_from_slice(value.as_bytes());
            data.push(0);
        }
        let mut bytes = b"HFST\0".to_vec();
        bytes.extend_from_slice(&(data.len() as u16).to_le_bytes());
        bytes.push(0);
        bytes.extend(data);
        bytes
    }

    fn code(result: Result<Member<'_>, GlueError>) -> &'static str {
        match result {
            Ok(member) => member.format.name(),
            Err(error) => error.code(),
        }
    }

    #[test]
    fn an_hfst3_header_gives_the_format_and_properties() {
        let mut bytes = header(&[("version", "3.3"), ("type", "HFST_OL"), ("name", "x")]);
        bytes.extend_from_slice(b"payload");
        let member = member(&bytes).expect("a well-formed header");
        assert_eq!(member.format, Format::OptimizedLookupUnweighted);
        assert_eq!(member.properties.get("name").map(String::as_str), Some("x"));
        assert_eq!(member.payload, b"payload");
    }

    #[test]
    fn the_pre_release_header_names_its_type() {
        let member = member(b"HFST3\0TROPICAL_OPENFST_TYPE\0rest").expect("pre-release");
        assert_eq!(member.format, Format::OpenFstTropical);
        assert!(member.properties.is_empty());
        assert_eq!(member.payload, b"rest");
    }

    #[test]
    fn malformed_headers_are_invalid() {
        let cases: [Vec<u8>; 6] = [
            b"HFST\0".to_vec(),
            b"HFST\0\x05\x00\x01".to_vec(),
            header(&[("version", "3.3")]),
            header(&[("version", "2.0"), ("type", "HFST_OL")]),
            header(&[("version", "3.3"), ("type", "LOG_OPENFST")]),
            b"HFST\0\x09\x00\0version\x003.3".to_vec(),
        ];
        for bytes in cases {
            assert_eq!(code(member(&bytes)), INVALID, "{bytes:?}");
        }
    }

    #[test]
    fn formats_nobody_reads_are_unsupported() {
        let cases: [Vec<u8>; 4] = [
            header(&[("version", "3.0"), ("type", "SFST")]),
            b"DHFST\x01\x01\x00".to_vec(),
            vec![0x1f, 0x8b, 0x08, 0, 0],
            b"HFST3\0SFST_TYPE\0".to_vec(),
        ];
        for bytes in cases {
            assert_eq!(code(member(&bytes)), UNSUPPORTED, "{bytes:?}");
        }
    }

    #[test]
    fn headerless_transducers_are_told_by_their_first_bytes() {
        let mut openfst = vec![0xd6, 0xfd, 0xb2, 0x7e, 6, 0, 0, 0];
        openfst.extend_from_slice(b"vector\x08\0\0\0standard");
        assert_eq!(code(member(&openfst)), "openfst-tropical");
        assert_eq!(code(member(b"##foma-net 1.0##\n")), "foma");
        assert_eq!(
            code(member(&[0u8; 80])),
            INVALID,
            "zeros declare no symbols"
        );
        assert_eq!(code(member(b"just text")), INVALID);
    }
}
