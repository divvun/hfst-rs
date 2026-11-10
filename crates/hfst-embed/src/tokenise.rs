//! Pmatch tokenisers read from bytes, the tokens they find and the text
//! `hfst tokenise` prints with them.
//!
//! Reading takes the container with the pmatch runtime alone. `hfst tokenise`
//! wraps a transducer that is not a pmatch container in a naive tokeniser,
//! which it builds with the transducer algebra; that is not done here, so
//! nothing built over this crate links algebra code.

use hfst::hfst_data_types::Symbol;
use hfst::pmatch::{Location, LocationVectorVector, PmatchContainer};
use hfst::pmatch_tokenize::{
    OutputFormat, TokenizeInputSettings, TokenizeSettings, UNANALYSED_WEIGHT, dedupe_locations,
    is_unanalysed, keep_n_best_weight, process_input_stream,
};
use hfst::transducer::{INFINITE_WEIGHT, Weight};

use crate::Package;
use crate::error::{Code, Error};
use crate::format::Format;
use crate::stream;

/// What `locate` reports for a stretch of text no pattern matched.
const NONMATCHING: &str = "@_NONMATCHING_@";

// [spec:hfst:req:javascript.tokenise]
// [spec:hfst:req:python.tokenise]
/// The output formats under the names of the `hfst tokenise` options that
/// choose them, which both packages take.
pub const FORMATS: [(&str, OutputFormat); 8] = [
    ("segment", OutputFormat::tokenize),
    ("space-separated", OutputFormat::space_separated),
    ("xerox", OutputFormat::xerox),
    ("cg", OutputFormat::cg),
    ("finnpos", OutputFormat::finnpos),
    ("giella-cg", OutputFormat::giellacg),
    ("conllu", OutputFormat::conllu),
    ("visl", OutputFormat::visl),
];

/// The format called `name` in [`FORMATS`], or `None` for a name no
/// `hfst tokenise` option has.
pub fn output_format(name: &str) -> Option<OutputFormat> {
    FORMATS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, format)| *format)
}

// [spec:hfst:req:javascript.tokenise]
// [spec:hfst:req:python.load]
/// The tokeniser in pmatch container bytes: a weighted optimized-lookup
/// stream whose first transducer is named TOP, as `hfst pmatch2fst` writes,
/// as a run over the container. A caller that wants more runs at once makes
/// them over its core with [`PmatchContainer::from_core`].
pub fn read_tokeniser(bytes: &[u8], package: &Package) -> Result<PmatchContainer, Error> {
    if bytes.is_empty() {
        return Err(stream::empty());
    }
    let member = stream::member(bytes, package)?;
    if member.format != Format::OptimizedLookupWeighted {
        return Err(not_a_container(&format!(
            "its first transducer is {}, and a pmatch container holds optimized-lookup-weighted ones",
            member.format.name()
        )));
    }
    match member.properties.get("name") {
        Some(name) if name == "TOP" => {}
        Some(name) => {
            return Err(not_a_container(&format!(
                "its first transducer is named {name}, not TOP"
            )));
        }
        None => return Err(not_a_container("its first transducer is not named TOP")),
    }
    let mut cursor = bytes;
    PmatchContainer::new_from_stream(&mut cursor).map_err(stream::read_error)
}

fn not_a_container(why: &str) -> Error {
    Error::new(
        Code::InvalidFormat,
        format!(
            "the bytes are not a pmatch container: {why} (hfst tokenise wraps any other transducer in a naive tokeniser, which this API does not make)"
        ),
    )
}

// [spec:hfst:req:javascript.tokenise]
// [spec:hfst:req:python.tokenise]
/// A token the tokeniser found, or a stretch of text no pattern matched.
#[derive(Debug, PartialEq)]
pub struct Token {
    pub surface: String,
    /// Where the token starts in the text, as [`tokens`] counts: in bytes,
    /// until [`recount`] counts it in a package's units.
    pub start: usize,
    /// Where the token ends in the text, exclusive, counted as `start` is.
    pub end: usize,
    /// Whether any reading is an analysis rather than one the tokeniser
    /// marks as unknown.
    pub analysed: bool,
    pub readings: Vec<Reading>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub analysis: String,
    pub weight: Weight,
}

/// The settings `tokenise` and `format` share, each `None` where the caller
/// gave none. Values are the library's, checked by the package that took
/// them as `hfst tokenise` checks its options.
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection {
    pub dedupe: Option<bool>,
    /// At least 1. `i32::MAX` keeps every class, as the default does.
    pub max_weight_classes: Option<i32>,
    /// At least 0.
    pub beam: Option<f32>,
    /// Seconds, at least 0. 0 sets no limit.
    pub time_limit: Option<f64>,
    pub tokenize_multichar: Option<bool>,
    pub print_all: Option<bool>,
}

impl Selection {
    /// Writes the settings given over `settings`.
    fn apply(&self, settings: &mut TokenizeSettings) {
        if let Some(dedupe) = self.dedupe {
            settings.dedupe = dedupe;
        }
        if let Some(classes) = self.max_weight_classes {
            settings.max_weight_classes = classes;
        }
        if let Some(beam) = self.beam {
            settings.beam = beam;
        }
        if let Some(seconds) = self.time_limit {
            settings.time_cutoff = seconds;
        }
        if let Some(multichar) = self.tokenize_multichar {
            settings.tokenize_multichar = multichar;
        }
        if let Some(print_all) = self.print_all {
            settings.print_all = print_all;
        }
    }
}

/// What changes the tokens [`tokens`] returns.
#[derive(Clone, Copy, Debug, Default)]
pub struct TokeniseOptions {
    pub selection: Selection,
    /// Abandons any match heavier than this. Not NaN.
    pub weight_cutoff: Option<f32>,
}

impl TokeniseOptions {
    /// The settings and the weight cutoff [`tokens`] runs with. Readings
    /// carry their weights, so dedupe tells readings apart by weight, as
    /// `hfst tokenise --print-weights` does.
    fn settings(&self) -> (TokenizeSettings, Weight) {
        let mut settings = TokenizeSettings {
            print_weights: true,
            ..TokenizeSettings::default()
        };
        self.selection.apply(&mut settings);
        (settings, self.weight_cutoff.unwrap_or(INFINITE_WEIGHT))
    }
}

/// What changes the text [`format`] returns, each option `None` where the
/// caller gave none.
#[derive(Clone, Copy, Debug, Default)]
pub struct FormatOptions {
    pub selection: Selection,
    pub print_weights: Option<bool>,
    pub hack_uncompose: Option<bool>,
    pub newline: Option<bool>,
    pub keep_newline: Option<bool>,
    pub superblanks: Option<bool>,
}

impl FormatOptions {
    /// The settings `hfst tokenise` runs with when given the flag that
    /// chooses `format` and then a flag for each option given, so a given
    /// option overrides what the format's flag implies.
    fn settings(&self, format: OutputFormat) -> (TokenizeSettings, TokenizeInputSettings) {
        let mut settings = TokenizeSettings {
            output_format: format,
            ..TokenizeSettings::default()
        };
        match format {
            OutputFormat::giellacg => {
                settings.print_weights = true;
                settings.print_all = true;
                settings.dedupe = true;
                settings.hack_uncompose = true;
                settings.verbose = false;
                settings.max_weight_classes = 2;
            }
            OutputFormat::visl => {
                settings.print_weights = false;
                settings.print_all = true;
                settings.dedupe = true;
                settings.verbose = false;
            }
            OutputFormat::tokenize
            | OutputFormat::space_separated
            | OutputFormat::xerox
            | OutputFormat::cg
            | OutputFormat::finnpos
            | OutputFormat::conllu => {}
        }
        self.selection.apply(&mut settings);
        if let Some(print_weights) = self.print_weights {
            settings.print_weights = print_weights;
        }
        if let Some(hack_uncompose) = self.hack_uncompose {
            settings.hack_uncompose = hack_uncompose;
        }
        let mut input = TokenizeInputSettings::default();
        if self.newline == Some(true) {
            input.blankline_separated = false;
        }
        if self.keep_newline == Some(true) {
            input.keep_newlines = true;
            input.blankline_separated = false;
        }
        if let Some(superblanks) = self.superblanks {
            input.superblanks = superblanks;
        }
        (settings, input)
    }
}

/// Reads one text symbol at a time, as `hfst tokenise` does, unless the
/// caller asked for multicharacter symbols or the network reads some.
fn choose_segmentation(container: &mut PmatchContainer, settings: &TokenizeSettings) {
    let single = !settings.tokenize_multichar && !container.has_multichar_input_symbols();
    container.set_single_codepoint_tokenization(single);
}

// [spec:hfst:req:javascript.tokenise]
// [spec:hfst:req:python.tokenise]
/// The tokens the tokeniser finds in `text`, in text order, matching the
/// whole text as one input, with their offsets in bytes.
pub fn tokens(
    container: &mut PmatchContainer,
    text: &str,
    options: &TokeniseOptions,
) -> Vec<Token> {
    let (settings, cutoff) = options.settings();
    choose_segmentation(container, &settings);
    let located = container.locate(text, settings.time_cutoff, cutoff);
    select(&located, text, &settings)
}

// [spec:hfst:req:javascript.tokenise]
// [spec:hfst:req:python.tokenise]
/// Counts each token's offsets in the units `count` measures a stretch of
/// `text` in, rather than in bytes. `tokens` are those [`tokens`] found in
/// `text`, whose offsets only go forward, so the text is walked once.
pub fn recount(text: &str, tokens: &mut [Token], count: impl Fn(&str) -> usize) {
    let mut byte = 0;
    let mut units = 0;
    let mut at = |offset: usize| {
        units += count(&text[byte..offset]);
        byte = offset;
        units
    };
    for token in tokens {
        token.start = at(token.start);
        token.end = at(token.end);
    }
}

/// The tokens `match_and_print` in `hfst::pmatch_tokenize` prints from
/// `located`, the locations of `text`: each token's readings after dedupe and
/// the weight-class limit, less the naive tokeniser's fallback readings,
/// within the beam the xerox format applies. Stretches no pattern matched are
/// tokens only under `print_all`, as they are printed only then.
fn select(located: &LocationVectorVector, text: &str, settings: &TokenizeSettings) -> Vec<Token> {
    if located.is_empty() {
        // The library prints the whole input as unanalysed when it locates
        // nothing in it.
        if !settings.print_all || text.is_empty() {
            return Vec::new();
        }
        return vec![Token {
            surface: text.to_owned(),
            start: 0,
            end: text.len(),
            analysed: false,
            readings: Vec::new(),
        }];
    }
    let mut cursor = Cursor { text, byte: 0 };
    let mut tokens = Vec::new();
    for locations in located {
        // Every location of one vector covers the same input.
        let Some(first) = locations.first() else {
            continue;
        };
        let (start, end) = cursor.span(&first.input_symbol_strings);
        if locations.len() == 1 && first.output == NONMATCHING {
            if settings.print_all {
                tokens.push(Token {
                    surface: first.input.clone(),
                    start,
                    end,
                    analysed: false,
                    readings: Vec::new(),
                });
            }
            continue;
        }
        let kept = keep_n_best_weight(&dedupe_locations(locations, settings), settings);
        let mut readings: Vec<&Location> = kept
            .iter()
            .filter(|location| location.weight < UNANALYSED_WEIGHT)
            .collect();
        if settings.beam >= 0.0 {
            let best = readings
                .iter()
                .map(|location| location.weight)
                .fold(Weight::INFINITY, Weight::min);
            readings.retain(|location| location.weight <= best + settings.beam);
        }
        tokens.push(Token {
            surface: first.input.clone(),
            start,
            end,
            analysed: readings.iter().any(|location| !is_unanalysed(location)),
            readings: readings
                .iter()
                .map(|location| Reading {
                    analysis: location.output.clone(),
                    weight: location.weight,
                })
                .collect(),
        });
    }
    tokens
}

/// A byte offset in a text, moving only forward through it as the tokens do.
struct Cursor<'a> {
    text: &'a str,
    byte: usize,
}

impl Cursor<'_> {
    /// The byte span of the text that spells `symbols`, the next input
    /// symbols the tokeniser printed. Each is looked for from the cursor on
    /// rather than at it: text the tokeniser reads without printing (a
    /// special symbol's spelling, a combining mark that one-grapheme reading
    /// folds into the symbol before it) lies between printed symbols, and
    /// belongs to the span around it. The hazard is that such text may also
    /// spell the next symbol, which is then found inside it.
    fn span(&mut self, symbols: &[Symbol]) -> (usize, usize) {
        let mut start = None;
        for symbol in symbols {
            let Some(offset) = self.text[self.byte..].find(symbol.as_str()) else {
                continue;
            };
            start.get_or_insert(self.byte + offset);
            self.byte += offset + symbol.len();
        }
        (start.unwrap_or(self.byte), self.byte)
    }
}

// [spec:hfst:req:javascript.tokenise]
// [spec:hfst:req:python.tokenise]
/// What `hfst tokenise` prints in `format` when `text` is its whole standard
/// input, given the flags `options` stand for.
pub fn format(
    container: &mut PmatchContainer,
    text: &str,
    format: OutputFormat,
    options: &FormatOptions,
) -> Result<String, Error> {
    let (settings, input) = options.settings(format);
    choose_segmentation(container, &settings);
    let mut printed = Vec::new();
    let status = process_input_stream(
        container,
        &mut text.as_bytes(),
        &mut printed,
        &mut std::io::sink(),
        &settings,
        &input,
    );
    if status != 0 {
        return Err(Error::new(
            Code::Failed,
            format!("the tokeniser stopped with status {status}"),
        ));
    }
    String::from_utf8(printed).map_err(|error| {
        Error::new(
            Code::Internal,
            format!("the tokeniser printed bytes that are not UTF-8 ({error})"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::file_resolver::{FileResolver, MemoryFiles};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::pmatch_compiler::{PmatchCompiler, write_archive};

    /// The JavaScript tests' tokeniser, compiled as `hfst pmatch2fst`
    /// compiles it.
    fn fixture() -> Vec<u8> {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/test/fixtures/tokenise.pmatch"
        ))
        .expect("the fixture is in the repository");
        let mut compiler = PmatchCompiler::<hfst_openfst::StdVectorFst>::new();
        let mut definitions = compiler.compile(&source).expect("the fixture compiles");
        let files = MemoryFiles::default();
        let sink = files.create("archive").expect("memory takes a file");
        let mut stream =
            HfstOutputStream::from_writer(sink, ImplementationType::HFST_OLW_TYPE, true)
                .expect("an optimized-lookup stream");
        let wrote = write_archive(&mut definitions, &mut stream, false, &mut std::io::sink())
            .expect("the archive writes");
        assert!(wrote, "the fixture defines TOP");
        stream.close();
        files.get("archive").expect("the archive was written")
    }

    fn load(bytes: &[u8]) -> Result<PmatchContainer, Error> {
        read_tokeniser(bytes, &crate::TEST_PACKAGE)
    }

    fn location(input: &[&str], output: &str, weight: Weight) -> Location {
        Location {
            input: input.concat(),
            output: output.to_owned(),
            weight,
            input_symbol_strings: input.iter().map(Symbol::new).collect(),
            ..Location::default()
        }
    }

    fn analyses(token: &Token) -> Vec<(&str, Weight)> {
        token
            .readings
            .iter()
            .map(|reading| (reading.analysis.as_str(), reading.weight))
            .collect()
    }

    fn defaults() -> TokenizeSettings {
        TokeniseOptions::default().settings().0
    }

    fn spans(tokens: &[Token]) -> Vec<(usize, usize)> {
        tokens.iter().map(|t| (t.start, t.end)).collect()
    }

    // [spec:hfst:req:javascript.tokenise/test]
    // [spec:hfst:req:python.tokenise/test]
    #[test]
    fn offsets_count_bytes_utf16_units_or_code_points() {
        let text = "🐱 Øl, x";
        let located = vec![
            vec![location(&["🐱"], "🐱+N", 0.0)],
            vec![location(&[" "], NONMATCHING, 0.0)],
            vec![location(&["Ø", "l"], "Øl+N", 0.0)],
            vec![location(&[","], ",+CLB", 0.0)],
            vec![location(&[" ", "x"], NONMATCHING, 0.0)],
        ];
        let settings = TokenizeSettings {
            print_all: true,
            ..defaults()
        };
        let mut tokens = select(&located, text, &settings);
        assert_eq!(spans(&tokens), [(0, 4), (4, 5), (5, 8), (8, 9), (9, 11)]);
        for token in &tokens {
            assert_eq!(&text[token.start..token.end], token.surface);
        }

        let mut utf16 = select(&located, text, &settings);
        recount(text, &mut utf16, |s| s.encode_utf16().count());
        assert_eq!(spans(&utf16), [(0, 2), (2, 3), (3, 5), (5, 6), (6, 8)]);

        recount(text, &mut tokens, |s| s.chars().count());
        assert_eq!(spans(&tokens), [(0, 1), (1, 2), (2, 4), (4, 5), (5, 7)]);
        let chars: Vec<char> = text.chars().collect();
        for token in &tokens {
            let covered: String = chars[token.start..token.end].iter().collect();
            assert_eq!(covered, token.surface);
        }

        let without = select(&located, text, &defaults());
        let surfaces: Vec<&str> = without.iter().map(|t| t.surface.as_str()).collect();
        assert_eq!(
            surfaces,
            ["🐱", "Øl", ","],
            "unmatched text only under print_all"
        );
    }

    #[test]
    fn unprinted_text_falls_inside_the_surrounding_span() {
        let text = "ab@P.X.Y@c d";
        let located = vec![vec![location(&["a", "b", "c"], "abc", 0.0)]];
        let tokens = select(&located, text, &defaults());
        assert_eq!((tokens[0].start, tokens[0].end), (0, 10));
    }

    #[test]
    fn dedupe_keeps_weights_apart_and_unknowns_unanalysed() {
        let text = "dog blorf";
        let located = vec![
            vec![
                location(&["d", "o", "g"], "dog+N", 0.0),
                location(&["d", "o", "g"], "dog+N", 0.0),
                location(&["d", "o", "g"], "dog+N", 2.0),
                location(&["d", "o", "g"], "dog ??", 100.0),
            ],
            vec![location(&["b", "l", "o", "r", "f"], "blorf ??", 100.0)],
        ];
        let tokens = select(&located, text, &defaults());
        assert_eq!(
            analyses(&tokens[0]),
            [("dog+N", 0.0), ("dog+N", 2.0), ("dog ??", 100.0)]
        );
        assert!(tokens[0].analysed);
        assert!(!tokens[1].analysed, "an unknown reading is not an analysis");
        assert_eq!(analyses(&tokens[1]), [("blorf ??", 100.0)]);

        let duplicates = TokenizeSettings {
            dedupe: false,
            ..defaults()
        };
        assert_eq!(select(&located, text, &duplicates)[0].readings.len(), 4);
    }

    #[test]
    fn weight_classes_skip_unknowns_and_beam_keeps_best() {
        let text = "saw";
        let located = vec![vec![
            location(&["s", "a", "w"], "see+V", 0.0),
            location(&["s", "a", "w"], "saw+N", 1.0),
            location(&["s", "a", "w"], "saw+V", 1.0),
            location(&["s", "a", "w"], "saw ??", 100.0),
            location(&["s", "a", "w"], "saw", UNANALYSED_WEIGHT),
        ]];
        let two = TokenizeSettings {
            max_weight_classes: 2,
            ..defaults()
        };
        assert_eq!(
            analyses(&select(&located, text, &two)[0]),
            [
                ("see+V", 0.0),
                ("saw+N", 1.0),
                ("saw+V", 1.0),
                ("saw ??", 100.0)
            ]
        );
        // The limit stops at the first reading past it, so an unknown reading
        // after that one goes too.
        let one = TokenizeSettings {
            max_weight_classes: 1,
            ..defaults()
        };
        assert_eq!(analyses(&select(&located, text, &one)[0]), [("see+V", 0.0)]);
        let beam = TokenizeSettings {
            beam: 0.5,
            ..defaults()
        };
        assert_eq!(
            analyses(&select(&located, text, &beam)[0]),
            [("see+V", 0.0)]
        );
    }

    #[test]
    fn a_fallback_only_token_has_no_readings() {
        let located = vec![vec![location(&["x"], "x", UNANALYSED_WEIGHT)]];
        let tokens = select(&located, "x", &defaults());
        assert!(tokens[0].readings.is_empty());
        assert!(!tokens[0].analysed);
    }

    #[test]
    fn nothing_located_is_one_unmatched_token() {
        let all = TokenizeSettings {
            print_all: true,
            ..defaults()
        };
        let tokens = select(&Vec::new(), "@BOUNDARY@", &all);
        assert_eq!((tokens[0].start, tokens[0].end), (0, 10));
        assert!(select(&Vec::new(), "@BOUNDARY@", &defaults()).is_empty());
        assert!(select(&Vec::new(), "", &all).is_empty());
    }

    #[test]
    fn given_options_override_what_a_format_implies() {
        let none = FormatOptions::default();
        let (giella, input) = none.settings(OutputFormat::giellacg);
        assert!(giella.print_weights && giella.print_all && giella.dedupe && giella.hack_uncompose);
        assert_eq!(giella.max_weight_classes, 2);
        assert!(input.blankline_separated && !input.keep_newlines && !input.superblanks);
        let (visl, _) = none.settings(OutputFormat::visl);
        assert!(!visl.print_weights && visl.print_all && visl.dedupe);
        let (cg, _) = none.settings(OutputFormat::cg);
        assert!(!cg.print_weights && !cg.print_all && cg.dedupe);
        assert_eq!(cg.max_weight_classes, i32::MAX);

        let given = FormatOptions {
            selection: Selection {
                dedupe: Some(false),
                max_weight_classes: Some(5),
                time_limit: Some(1.5),
                ..Selection::default()
            },
            print_weights: Some(false),
            keep_newline: Some(true),
            ..FormatOptions::default()
        };
        let (giella, input) = given.settings(OutputFormat::giellacg);
        assert!(!giella.print_weights && !giella.dedupe);
        assert_eq!(giella.max_weight_classes, 5);
        assert_eq!(giella.time_cutoff, 1.5);
        assert!(input.keep_newlines && !input.blankline_separated);
    }

    #[test]
    fn format_names_are_the_command_line_option_names() {
        for (name, format) in FORMATS {
            assert_eq!(output_format(name), Some(format));
        }
        assert_eq!(output_format("giellacg"), None);
    }

    #[test]
    fn non_pmatch_bytes_are_invalid_formats() {
        let code = |bytes: &[u8]| load(bytes).map(|_| ()).map_err(|error| error.code());
        assert_eq!(code(b""), Err(Code::InvalidFormat));
        assert_eq!(code(b"not a stream"), Err(Code::InvalidFormat));
        let archive = fixture();
        assert_eq!(code(&archive), Ok(()));
        let truncated = &archive[..archive.len() - 9];
        assert_eq!(code(truncated), Err(Code::InvalidFormat));
        let mut renamed = archive.clone();
        let name = renamed
            .windows(9)
            .position(|window| window == b"name\0TOP\0")
            .expect("the first header names TOP");
        renamed[name + 6] = b'I';
        assert_eq!(code(&renamed), Err(Code::InvalidFormat));
    }

    /// The cohorts `hfst tokenise --cg --print-weights` prints for tokens
    /// `tokens` returned with `print_all` off.
    fn cg(tokens: &[Token]) -> String {
        let mut printed = String::new();
        for token in tokens {
            printed.push_str(&format!("\"<{}>\"\n", token.surface));
            for reading in &token.readings {
                match reading.analysis.strip_prefix(&token.surface) {
                    Some(rest) => printed.push_str(&format!("\t\"{}\"{rest}", token.surface)),
                    None => printed.push_str(&format!("\t{}", reading.analysis)),
                }
                printed.push_str(&format!("\t{:.10}\n", reading.weight));
            }
            printed.push('\n');
        }
        printed
    }

    #[test]
    fn tokens_hold_what_the_cg_format_prints() {
        let mut container = load(&fixture()).expect("the fixture loads");
        let text = "The cat saw a dog in New York, blorf! Øl 🐱 blåbær.";
        let found = tokens(&mut container, text, &TokeniseOptions::default());
        for token in &found {
            assert_eq!(&text[token.start..token.end], token.surface);
        }
        let options = FormatOptions {
            print_weights: Some(true),
            ..FormatOptions::default()
        };
        let printed = format(&mut container, text, OutputFormat::cg, &options).expect("prints");
        assert_eq!(cg(&found), printed);
        let cat = found.iter().find(|t| t.surface == "🐱").expect("a cat");
        assert_eq!(cat.end - cat.start, 4, "an astral character is four bytes");
    }

    // [spec:hfst:req:python.threads/test]
    #[test]
    fn runs_over_one_core_agree_with_one_run() {
        let mut first = load(&fixture()).expect("the fixture loads");
        let text = "The cat saw a dog in New York, blorf! Øl 🐱 blåbær.";
        let alone = tokens(&mut first, text, &TokeniseOptions::default());
        let mut second = PmatchContainer::from_core(first.core());
        assert_eq!(
            tokens(&mut second, text, &TokeniseOptions::default()),
            alone
        );
    }

    #[test]
    fn a_weight_cutoff_abandons_heavier_matches() {
        let mut container = load(&fixture()).expect("the fixture loads");
        let options = TokeniseOptions {
            weight_cutoff: Some(50.0),
            ..TokeniseOptions::default()
        };
        let found = tokens(&mut container, "The cat blorf", &options);
        let surfaces: Vec<&str> = found.iter().map(|t| t.surface.as_str()).collect();
        assert_eq!(surfaces, ["cat"]);
        assert!(found[0].readings.iter().all(|r| r.weight <= 50.0));
    }
}
