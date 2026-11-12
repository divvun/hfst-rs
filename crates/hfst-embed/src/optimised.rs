//! The optimised lookup: an optimized-lookup transducer read by divvunspell's
//! `divvun-fst`, and words looked up in it by that crate's lookup.

use divvun_fst::transducer::TransducerLoader;
use divvun_fst::transducer::hfst::HfstTransducer;
pub use divvun_fst::transducer::{Lookup, LookupResult as OptimisedResult};
use divvun_fst::vfs::bytes::{File, Filesystem};
use hfst::transducer::TransducerHeader;

use crate::Package;
use crate::error::{Code, Error};
use crate::format::Format;
use crate::stream::{self, Member, Properties};

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
// [spec:hfst:req:python.lookup]
/// An optimized-lookup transducer as `divvun-fst` reads it, with the
/// properties of its HFST3 header. It holds no lookup tables, so threads may
/// look up in it at once, each with tables of its own.
pub struct OptimisedTransducer {
    transducer: HfstTransducer,
    properties: Properties,
}

impl OptimisedTransducer {
    // [spec:hfst:req:javascript.optimised-lookup+2]
    // [spec:hfst:req:javascript.load-bytes+2]
    // [spec:hfst:req:python.load+1]
    /// The one optimized-lookup transducer in `bytes`, read by `divvun-fst`.
    /// Bytes in another format are turned down as
    /// [`crate::transducer::read_transducers`] turns them down; a stream
    /// holding more than one transducer is turned down with
    /// `transducer-count`. The bytes are handed to `divvun-fst` as they are
    /// when they begin with an HFST3 header, so they are not copied again; a
    /// transducer without that header is read after an empty one.
    pub fn read(bytes: Vec<u8>, package: &Package) -> Result<OptimisedTransducer, Error> {
        if bytes.is_empty() {
            return Err(stream::empty());
        }
        let Member {
            format,
            properties,
            payload,
        } = stream::member(&bytes, package)?;
        if !format.is_read() {
            return Err(stream::unsupported(
                &format!("a transducer in the {} format", format.name()),
                package,
            ));
        }
        if payload.len() > ol_extent(payload)? {
            return Err(Error::new(
                Code::TransducerCount,
                format!(
                    "{} takes exactly one transducer, and the stream holds more",
                    package.read_optimised_transducer
                ),
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
            .map_err(|error| Error::new(Code::InvalidFormat, error.to_string()))?;
        Ok(OptimisedTransducer {
            transducer,
            properties,
        })
    }

    /// The optimized-lookup format `divvun-fst` read the tables in.
    pub fn format(&self) -> Format {
        if self.transducer.is_weighted() {
            Format::OptimizedLookupWeighted
        } else {
            Format::OptimizedLookupUnweighted
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

    // [spec:hfst:req:javascript.optimised-lookup+2]
    // [spec:hfst:req:python.lookup]
    /// What `divvun-fst`'s lookup returns for `input`, the first `limit` of
    /// them when there is a limit, which are the lightest. `tables` are the
    /// lookup's working tables, kept by the caller so that they keep their
    /// capacity from one lookup to the next.
    pub fn lookup(
        &self,
        tables: &mut Lookup,
        input: &str,
        limit: Option<usize>,
    ) -> Vec<OptimisedResult> {
        let mut results = tables.lookup(&self.transducer, input);
        if let Some(limit) = limit {
            results.truncate(limit);
        }
        results
    }
}

/// How many bytes the optimized-lookup transducer at the start of `payload`
/// takes: its header, alphabet, index table and transition table, laid out as
/// its header says. Bytes past it are the next transducer of a stream. A
/// payload too short to hold the header or alphabet is truncated.
fn ol_extent(payload: &[u8]) -> Result<usize, Error> {
    let truncated = || {
        Error::new(
            Code::InvalidFormat,
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

#[cfg(test)]
mod tests {
    use super::*;
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
        let ol = AlgebraTransducer::<hfst_openfst::StdVectorFst>::new_from_basic(&net)
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
        stream.write(&ol).expect("the transducer writes");
        stream.close();
        files.get("cat").expect("the transducer was written")
    }

    fn read(bytes: Vec<u8>) -> Result<OptimisedTransducer, Error> {
        OptimisedTransducer::read(bytes, &crate::TEST_PACKAGE)
    }

    fn code(result: Result<OptimisedTransducer, Error>) -> &'static str {
        match result {
            Ok(_) => "read",
            Err(error) => error.code().name(),
        }
    }

    /// Each result as its output, its symbols and its weight.
    fn summary(results: &[OptimisedResult]) -> Vec<String> {
        results
            .iter()
            .map(|r| {
                let symbols: Vec<&str> = r.symbols.iter().map(|s| s.as_str()).collect();
                format!("{} {} {}", r.output, symbols.join(" "), r.weight.0)
            })
            .collect()
    }

    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn both_formats_read_with_and_without_a_header() {
        for weighted in [true, false] {
            for header in [true, false] {
                let t = read(cat(weighted, header)).expect("the transducer reads");
                let mut tables = Lookup::new();
                let what = format!("weighted {weighted}, header {header}");
                let format = if weighted {
                    Format::OptimizedLookupWeighted
                } else {
                    Format::OptimizedLookupUnweighted
                };
                assert_eq!(t.format(), format, "{what}");
                // Unweighted, both weigh 0, so the output orders them.
                let expected = if weighted {
                    ["cat+N c a t +N 0.75", "cat c a t 1"]
                } else {
                    ["cat c a t 0", "cat+N c a t +N 0"]
                };
                assert_eq!(
                    summary(&t.lookup(&mut tables, "cat", None)),
                    expected,
                    "{what}"
                );
                assert!(t.lookup(&mut tables, "dog", None).is_empty(), "{what}");
            }
        }
    }

    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn a_limit_keeps_the_lightest_results() {
        let t = read(cat(true, true)).expect("the transducer reads");
        let mut tables = Lookup::new();
        let all = summary(&t.lookup(&mut tables, "cat", None));
        assert_eq!(summary(&t.lookup(&mut tables, "cat", Some(1))), all[..1]);
        assert!(all[0].starts_with("cat+N "), "{all:?}");
        assert!(t.lookup(&mut tables, "cat", Some(0)).is_empty());
        assert_eq!(summary(&t.lookup(&mut tables, "cat", Some(9))), all);
        assert_eq!(
            summary(&t.lookup(&mut tables, "ca", None)),
            ["ca+X c a +X 2"]
        );
    }

    // [spec:hfst:req:javascript.optimised-lookup+2/test]
    #[test]
    fn several_or_truncated_streams_are_turned_down() {
        for header in [true, false] {
            let one = cat(true, header);
            let two = [one.clone(), one.clone()].concat();
            assert_eq!(code(read(two)), "transducer-count", "header {header}");
            for cut in [one.len() - 1, one.len() / 2, 70] {
                assert_eq!(
                    code(read(one[..cut].to_vec())),
                    "invalid-format",
                    "header {header}, {cut} bytes"
                );
            }
        }
        assert_eq!(code(read(Vec::new())), "invalid-format");
        assert_eq!(
            code(read(b"##foma-net 1.0##\n".to_vec())),
            "unsupported-format"
        );
    }

    #[test]
    fn properties_leave_out_version_type_and_name() {
        let t = read(cat(true, true)).expect("the transducer reads");
        assert_eq!(t.name(), "");
        assert!(
            t.properties()
                .all(|(key, _)| !matches!(key, "version" | "type" | "name"))
        );
        let bare = read(cat(true, false)).expect("the transducer reads");
        assert_eq!(bare.properties().count(), 0);
    }
}
