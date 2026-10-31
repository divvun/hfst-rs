//! What native divvun-fst gives for the optimised-lookup fixtures of the
//! JavaScript parity suite (js/test/parity.ts): its lookup in each
//! transducer, for each input, under each limit.
//!
//!   cargo run -p hfst-wasm --example js_optimised_expected -- <fixtures> <request> <out>
//!
//! js/test/optimised-fixtures.mjs writes the request, a JSON list of the
//! fixtures and inputs to run, and calls this at test-build time. divvun-fst
//! reads optimized lookup only after an HFST3 header, so each request names
//! the headed file to read beside the file the package loads, which is the
//! same transducer with or without its header. Results are in the shape
//! `@divvun/hfst` returns them, with each weight as the f64 that holds its f32
//! exactly, so the suite can compare the two value for value.

use std::error::Error;
use std::fs;
use std::path::Path;

use divvun_fst::transducer::Lookup;
use divvun_fst::transducer::hfst::HfstTransducer;
use serde::{Deserialize, Serialize};

type Failure = Box<dyn Error>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    optimised: Vec<LookupRequest>,
}

/// Every input looked up under every limit in `reference`, which the
/// package's results for `file` are compared with. A limit of `None` sets
/// none.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupRequest {
    file: String,
    reference: String,
    inputs: Vec<String>,
    limits: Vec<Option<u32>>,
}

#[derive(Serialize)]
struct Expected {
    optimised: Vec<LookupEntry>,
}

#[derive(Serialize)]
struct LookupEntry {
    file: String,
    input: String,
    limit: Option<u32>,
    results: Vec<LookupResult>,
}

#[derive(Serialize)]
struct LookupResult {
    output: String,
    symbols: Vec<String>,
    weight: f64,
}

fn main() -> Result<(), Failure> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [fixtures, request, out] = args.as_slice() else {
        return Err("usage: js_optimised_expected <fixtures dir> <request.json> <out.json>".into());
    };
    let dir = Path::new(fixtures);
    let request: Request = serde_json::from_slice(&fs::read(request)?)?;
    let mut expected = Expected {
        optimised: Vec::new(),
    };
    for lookup in &request.optimised {
        let path = dir.join(&lookup.reference);
        let transducer = HfstTransducer::from_bytes(&fs::read(&path)?, path)?;
        expected.optimised.extend(entries(&transducer, lookup)?);
    }
    fs::write(out, serde_json::to_vec(&expected)?)?;
    Ok(())
}

/// What divvun-fst's lookup gives in `transducer` for each input and limit
/// of `lookup`, the first `limit` results of each when there is a limit.
fn entries(
    transducer: &HfstTransducer,
    lookup: &LookupRequest,
) -> Result<Vec<LookupEntry>, Failure> {
    // One Lookup for the whole file, as the package keeps one per
    // transducer, so a table it fails to reset shows as a difference.
    let mut tables = Lookup::new();
    let mut entries = Vec::new();
    for input in &lookup.inputs {
        let results = tables.lookup(transducer, input);
        for &limit in &lookup.limits {
            let kept = limit.map_or(Ok(results.len()), usize::try_from)?;
            entries.push(LookupEntry {
                file: lookup.file.clone(),
                input: input.clone(),
                limit,
                results: results
                    .iter()
                    .take(kept)
                    .map(|result| LookupResult {
                        output: result.output.to_string(),
                        symbols: result.symbols.iter().map(|s| s.to_string()).collect(),
                        weight: f64::from(result.weight.0),
                    })
                    .collect(),
            });
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::file_resolver::{FileResolver, MemoryFiles};
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;

    /// `ab` read as `ax` (0.5), `ay` (0.25) and `az` (0.1), as the weighted
    /// optimized-lookup file `hfst fst2fst -w` writes, header and all.
    fn three_ways() -> HfstTransducer {
        let att = "0\t1\ta\ta\t0\n1\t2\tb\tx\t0.5\n1\t2\tb\ty\t0.25\n1\t2\tb\tz\t0.1\n2\t0\n";
        let mut lines = 0;
        let basic =
            HfstBasicTransducer::read_in_att_format(&mut att.as_bytes(), "@0@", &mut lines, false)
                .expect("AT&T text");
        let tropical =
            hfst::hfst_transducer::HfstTransducer::<hfst_openfst::StdVectorFst>::new_from_basic(
                &basic,
            )
            .expect("tropical");
        let mut olw = tropical.to_ol(true, "").expect("weighted optimized lookup");
        let memory = MemoryFiles::default();
        let mut out = HfstOutputStream::from_writer(
            memory.create("olw").expect("a file in memory"),
            ImplementationType::HFST_OLW_TYPE,
            true,
        )
        .expect("a stream");
        out.write(&mut olw).expect("written");
        out.close();
        let bytes = memory.get("olw").expect("the bytes");
        HfstTransducer::from_bytes(&bytes, "olw").expect("divvun-fst reads it")
    }

    #[test]
    fn limits_keep_the_lightest_and_weights_stay_exact() {
        let request = LookupRequest {
            file: "three.hfst".into(),
            reference: "three.hfst".into(),
            inputs: vec!["ab".into(), "a".into()],
            limits: vec![None, Some(1), Some(0)],
        };
        let found = entries(&three_ways(), &request).expect("entries");
        let summary: Vec<String> = found
            .iter()
            .map(|entry| {
                let results: Vec<String> = entry
                    .results
                    .iter()
                    .map(|r| format!("{} {}", r.output, r.weight))
                    .collect();
                format!("{} {:?}: {}", entry.input, entry.limit, results.join(", "))
            })
            .collect();
        // The f64 that holds the f32 0.1 exactly, not the f64 0.1.
        let tenth = f64::from(0.1f32);
        assert_eq!(
            summary,
            [
                format!("ab None: az {tenth}, ay 0.25, ax 0.5"),
                format!("ab Some(1): az {tenth}"),
                "ab Some(0): ".to_owned(),
                "a None: ".to_owned(),
                "a Some(1): ".to_owned(),
                "a Some(0): ".to_owned(),
            ]
        );
    }
}
