//! What the native library gives for the shared fixtures of the JavaScript
//! parity suite (js/test/parity.ts): lookups in optimized-lookup transducers,
//! the tokens a pmatch tokeniser finds, DHFST acceptor lookups, and what each
//! DHFST file says about itself.
//!
//!   cargo run -p hfst --example js_parity_expected -- <fixtures> <request> <out>
//!
//! js/test/parity-fixtures.mjs writes the request, a JSON list of the
//! fixtures and inputs to run, and calls this at test-build time. Every result
//! is in the shape `@divvun/hfst` returns it, so the suite can compare the two
//! value for value. Weights are written as the f64 the package hands
//! JavaScript, which holds each f32 weight exactly.

use std::error::Error;
use std::fs;
use std::path::Path;

use hfst::dhfst::DhfstReader;
use hfst::dhfst_acceptor::{AcceptorInfo, AcceptorReader};
use hfst::dhfst_header::{DhfstType, read_type};
use hfst::hfst_data_types::HfstOneLevelPaths;
use hfst::hfst_flag_diacritics::FdOperation;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::is_epsilon;
use hfst::hfst_transducer::AnyTransducer;
use hfst::pmatch::{Location, PmatchContainer};
use hfst::pmatch_tokenize::{
    TokenizeSettings, UNANALYSED_WEIGHT, dedupe_locations, is_unanalysed, keep_n_best_weight,
};
use hfst::transducer::INFINITE_WEIGHT;
use serde::{Deserialize, Serialize};
use serde_json::Value;

type Failure = Box<dyn Error>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    lookup: Vec<LookupRequest>,
    tokens: Vec<TokensRequest>,
    acceptors: Vec<AcceptorRequest>,
    models: Vec<String>,
}

/// Every input looked up under every limit in every transducer of `file`.
/// A limit of `None` sets none.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupRequest {
    file: String,
    inputs: Vec<String>,
    limits: Vec<Option<u32>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokensRequest {
    file: String,
    name: String,
    text: String,
    options: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptorRequest {
    file: String,
    words: Vec<String>,
}

/// The `TokeniseOptions` of the package that have a native counterpart
/// here. An option outside them fails the request rather than going unread.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TokeniseOptions {
    dedupe: Option<bool>,
    max_weight_classes: Option<i32>,
    weight_cutoff: Option<f32>,
    tokenize_multichar: Option<bool>,
    print_all: Option<bool>,
}

#[derive(Serialize)]
struct Expected {
    lookup: Vec<LookupEntry>,
    tokens: Vec<TokensEntry>,
    acceptors: Vec<AcceptorEntry>,
    models: Vec<ModelEntry>,
}

#[derive(Serialize)]
struct LookupEntry {
    file: String,
    /// The transducer's place in the file's stream, from 0.
    member: usize,
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

#[derive(Serialize)]
struct TokensEntry {
    file: String,
    name: String,
    text: String,
    options: Value,
    tokens: Vec<Token>,
}

/// A token without its UTF-16 offsets, which are the package's own: the
/// library counts input symbols, not code units.
#[derive(Serialize)]
struct Token {
    surface: String,
    analysed: bool,
    readings: Vec<Reading>,
}

#[derive(Serialize)]
struct Reading {
    analysis: String,
    weight: f64,
}

#[derive(Serialize)]
struct AcceptorEntry {
    file: String,
    word: String,
    weight: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelEntry {
    file: String,
    #[serde(rename = "type")]
    kind: &'static str,
    version: u8,
    symbols: Vec<String>,
    meta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    info: Option<Info>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_count: Option<u32>,
}

/// [`AcceptorInfo`] under the package's names for its fields.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    ids: u32,
    slots: u32,
    used_slots: u32,
    check_bytes: usize,
    label_max: u32,
    record_bytes: usize,
    list_records: u32,
    arcs: u64,
    free_arcs: u64,
    free_pairs: u32,
    weights: u32,
    finals: u32,
    distances: bool,
}

impl From<AcceptorInfo> for Info {
    fn from(info: AcceptorInfo) -> Self {
        Info {
            ids: info.ids,
            slots: info.slots,
            used_slots: info.used_slots,
            check_bytes: info.check_bytes,
            label_max: info.label_max,
            record_bytes: info.record_bytes,
            list_records: info.list_records,
            arcs: info.arcs,
            free_arcs: info.free_arcs,
            free_pairs: info.free_pairs,
            weights: info.weights,
            finals: info.finals,
            distances: info.distances,
        }
    }
}

fn main() -> Result<(), Failure> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [fixtures, request, out] = args.as_slice() else {
        return Err("usage: js_parity_expected <fixtures dir> <request.json> <out.json>".into());
    };
    let dir = Path::new(fixtures);
    let request: Request = serde_json::from_slice(&fs::read(request)?)?;
    let mut expected = Expected {
        lookup: Vec::new(),
        tokens: Vec::new(),
        acceptors: Vec::new(),
        models: Vec::new(),
    };
    for lookup in &request.lookup {
        expected.lookup.extend(look_up(dir, lookup)?);
    }
    for tokens in request.tokens {
        expected.tokens.push(tokenise(dir, tokens)?);
    }
    for acceptor in &request.acceptors {
        let reader = AcceptorReader::parse_owned(fs::read(dir.join(&acceptor.file))?)?;
        for word in &acceptor.words {
            expected.acceptors.push(AcceptorEntry {
                file: acceptor.file.clone(),
                word: word.clone(),
                weight: reader.lookup(word).map(f64::from),
            });
        }
    }
    for file in request.models {
        expected.models.push(describe(dir, file)?);
    }
    fs::write(out, serde_json::to_vec(&expected)?)?;
    Ok(())
}

/// Every transducer in the stream at `path`, read as `hfst lookup` reads
/// its transducer file.
fn read_members(path: &Path) -> Result<Vec<AnyTransducer>, Failure> {
    let name = path.to_str().ok_or("a fixture path is not UTF-8")?;
    let mut stream = HfstInputStream::new_filename(name)?;
    let mut members = Vec::new();
    while stream.is_good() {
        members.push(stream.read()?);
    }
    stream.close();
    Ok(members)
}

fn look_up(dir: &Path, request: &LookupRequest) -> Result<Vec<LookupEntry>, Failure> {
    let members = read_members(&dir.join(&request.file))?;
    let mut entries = Vec::new();
    for (member, transducer) in members.iter().enumerate() {
        for input in &request.inputs {
            for &limit in &request.limits {
                let cap = limit.map_or(Ok(-1), isize::try_from)?;
                // Only the optimized-lookup members are named: the other
                // variants depend on the crate's features.
                let paths = if let AnyTransducer::OlW(t) = transducer {
                    t.lookup_fd_string(input, cap, 0.0)?
                } else if let AnyTransducer::OlU(t) = transducer {
                    t.lookup_fd_string(input, cap, 0.0)?
                } else {
                    return Err(format!(
                        "{} holds a transducer that is not optimized-lookup",
                        request.file
                    )
                    .into());
                };
                entries.push(LookupEntry {
                    file: request.file.clone(),
                    member,
                    input: input.clone(),
                    limit,
                    results: lookup_results(paths),
                });
            }
        }
    }
    Ok(entries)
}

/// The paths a flag-aware lookup found, as the package's `lookup` returns
/// them (javascript.lookup): one result per path, its symbols without flag
/// diacritics and epsilons, ordered by weight, then output, then symbols.
fn lookup_results(paths: HfstOneLevelPaths) -> Vec<LookupResult> {
    let mut results: Vec<LookupResult> = paths
        .into_iter()
        .map(|path| {
            let symbols: Vec<String> = path
                .second
                .iter()
                .filter(|symbol| {
                    !symbol.is_empty() && !is_epsilon(symbol) && !FdOperation::is_diacritic(symbol)
                })
                .map(|symbol| symbol.to_string())
                .collect();
            LookupResult {
                output: symbols.concat(),
                symbols,
                weight: f64::from(path.first),
            }
        })
        .collect();
    results.sort_by(|a, b| {
        a.weight
            .total_cmp(&b.weight)
            .then_with(|| a.output.cmp(&b.output))
            .then_with(|| a.symbols.cmp(&b.symbols))
    });
    results
}

/// The tokens of the request's text: for each run of locations, the readings
/// `hfst tokenise --cg --print-weights` prints for it (`match_and_print` in
/// `hfst::pmatch_tokenize`), and, under `printAll`, each stretch no pattern
/// matched as a token with none.
fn tokenise(dir: &Path, request: TokensRequest) -> Result<TokensEntry, Failure> {
    let options: TokeniseOptions = serde_json::from_value(request.options.clone())?;
    let mut settings = TokenizeSettings {
        print_weights: true,
        ..TokenizeSettings::default()
    };
    if let Some(dedupe) = options.dedupe {
        settings.dedupe = dedupe;
    }
    if let Some(classes) = options.max_weight_classes {
        settings.max_weight_classes = classes;
    }
    if let Some(multichar) = options.tokenize_multichar {
        settings.tokenize_multichar = multichar;
    }
    if let Some(print_all) = options.print_all {
        settings.print_all = print_all;
    }
    let cutoff = options.weight_cutoff.unwrap_or(INFINITE_WEIGHT);

    let bytes = fs::read(dir.join(&request.file))?;
    let mut container = PmatchContainer::new_from_stream(&mut bytes.as_slice())?;
    let single = !settings.tokenize_multichar && !container.has_multichar_input_symbols();
    container.set_single_codepoint_tokenization(single);
    let located = container.locate(&request.text, settings.time_cutoff, cutoff);

    let unmatched = |surface: &str| Token {
        surface: surface.to_owned(),
        analysed: false,
        readings: Vec::new(),
    };
    let mut tokens = Vec::new();
    if located.is_empty() && settings.print_all && !request.text.is_empty() {
        tokens.push(unmatched(&request.text));
    }
    for locations in &located {
        let Some(first) = locations.first() else {
            continue;
        };
        if locations.len() == 1 && first.output == "@_NONMATCHING_@" {
            if settings.print_all {
                tokens.push(unmatched(&first.input));
            }
            continue;
        }
        let kept = keep_n_best_weight(&dedupe_locations(locations, &settings), &settings);
        let readings: Vec<&Location> = kept
            .iter()
            .filter(|location| location.weight < UNANALYSED_WEIGHT)
            .collect();
        tokens.push(Token {
            surface: first.input.clone(),
            analysed: readings.iter().any(|location| !is_unanalysed(location)),
            readings: readings
                .iter()
                .map(|location| Reading {
                    analysis: location.output.clone(),
                    weight: f64::from(location.weight),
                })
                .collect(),
        });
    }
    Ok(TokensEntry {
        file: request.file,
        name: request.name,
        text: request.text,
        options: request.options,
        tokens,
    })
}

/// What a DHFST file says about itself, as divvunspell's readers load it.
fn describe(dir: &Path, file: String) -> Result<ModelEntry, Failure> {
    let bytes = fs::read(dir.join(&file))?;
    Ok(match read_type(&bytes)? {
        DhfstType::Acceptor => {
            let reader = AcceptorReader::parse(&bytes)?;
            ModelEntry {
                file,
                kind: "acceptor",
                version: reader.version(),
                symbols: reader.symbol_names().to_vec(),
                meta: reader.meta().map(str::to_owned),
                info: Some(reader.info().into()),
                state_count: None,
            }
        }
        DhfstType::ErrorModel => {
            let reader = DhfstReader::parse(&bytes)?;
            ModelEntry {
                file,
                kind: "error-model",
                version: reader.version(),
                symbols: reader.symbol_names().to_vec(),
                meta: reader.meta().map(str::to_owned),
                info: None,
                state_count: Some(reader.state_count()),
            }
        }
    })
}
