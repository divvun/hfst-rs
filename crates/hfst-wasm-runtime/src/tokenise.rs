//! Pmatch tokenisers loaded from bytes, as both packages hold them.
//!
//! Loading reads the container with the pmatch runtime alone. `hfst tokenise`
//! wraps a transducer that is not a pmatch container in a naive tokeniser,
//! which it builds with the transducer algebra; that is not done here, so the
//! runtime module links no algebra code.

use hfst::hfst_data_types::Symbol;
use hfst::pmatch::{Location, LocationVectorVector, PmatchContainer};
use hfst::pmatch_tokenize::{
    OutputFormat, TokenizeInputSettings, TokenizeSettings, UNANALYSED_WEIGHT, dedupe_locations,
    is_unanalysed, keep_n_best_weight, process_input_stream,
};
use hfst::transducer::{INFINITE_WEIGHT, Weight};
use wasm_bindgen::prelude::*;

use crate::error::GlueError;
use crate::format::Format;
use crate::stream;

const INVALID_ARGUMENT: &str = "invalid-argument";

/// What `locate` reports for a stretch of text no pattern matched.
const NONMATCHING: &str = "@_NONMATCHING_@";

/// The output formats under the names of the `hfst tokenise` options that
/// choose them. `js/shared/tokenise.ts` lists the same names.
const FORMATS: [(&str, OutputFormat); 8] = [
    ("segment", OutputFormat::tokenize),
    ("space-separated", OutputFormat::space_separated),
    ("xerox", OutputFormat::xerox),
    ("cg", OutputFormat::cg),
    ("finnpos", OutputFormat::finnpos),
    ("giella-cg", OutputFormat::giellacg),
    ("conllu", OutputFormat::conllu),
    ("visl", OutputFormat::visl),
];

/// A pmatch container, with the run state its matches write to. The
/// wrapper's `Tokeniser` classes in `js/hfst-runtime/tokenise.ts` and
/// `js/hfst/tokenise.ts` own one.
#[wasm_bindgen]
pub struct Tokeniser {
    container: PmatchContainer,
}

#[wasm_bindgen]
impl Tokeniser {
    // [spec:hfst:req:javascript.tokenise]
    /// The tokens of `text`, as plain objects with the fields of the
    /// wrapper's `Token`.
    pub fn tokenise(&mut self, text: &str, options: &JsValue) -> Result<Vec<JsValue>, GlueError> {
        let (settings, cutoff) = TokeniseOptions::read(options)?.settings()?;
        let tokens = tokens(&mut self.container, text, &settings, cutoff);
        Ok(tokens.iter().map(Token::to_js).collect())
    }

    // [spec:hfst:req:javascript.tokenise]
    /// What `hfst tokenise` prints for `text` on its standard input, in the
    /// format named `format`.
    pub fn format(
        &mut self,
        text: &str,
        format: &str,
        options: &JsValue,
    ) -> Result<String, GlueError> {
        let format = output_format(format)?;
        let (settings, input) = FormatOptions::read(options)?.settings(format)?;
        print(&mut self.container, text, &settings, &input)
    }
}

// [spec:hfst:req:javascript.tokenise]
/// The tokeniser in pmatch container bytes: a weighted optimized-lookup
/// stream whose first transducer is named TOP, as `hfst pmatch2fst` writes.
#[wasm_bindgen(js_name = readTokeniser)]
pub fn read_tokeniser(bytes: &[u8]) -> Result<Tokeniser, GlueError> {
    Ok(Tokeniser {
        container: load(bytes)?,
    })
}

fn load(bytes: &[u8]) -> Result<PmatchContainer, GlueError> {
    if bytes.is_empty() {
        return Err(stream::empty());
    }
    let member = stream::member(bytes)?;
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

fn not_a_container(why: &str) -> GlueError {
    GlueError::new(
        "invalid-format",
        format!(
            "the bytes are not a pmatch container: {why} (hfst tokenise wraps any other transducer in a naive tokeniser, which this API does not make)"
        ),
    )
}

fn output_format(name: &str) -> Result<OutputFormat, GlueError> {
    FORMATS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, format)| *format)
        .ok_or_else(|| {
            GlueError::new(
                INVALID_ARGUMENT,
                format!("there is no tokenise format called {name}"),
            )
        })
}

/// One token as the wrapper's `Token` describes it, with UTF-16 offsets.
#[derive(Debug, PartialEq)]
struct Token {
    surface: String,
    start: usize,
    end: usize,
    analysed: bool,
    readings: Vec<Reading>,
}

#[derive(Debug, PartialEq)]
struct Reading {
    analysis: String,
    weight: Weight,
}

impl Token {
    fn to_js(&self) -> JsValue {
        let object = js_sys::Object::new();
        set(&object, "surface", &JsValue::from_str(&self.surface));
        set(&object, "start", &JsValue::from_f64(self.start as f64));
        set(&object, "end", &JsValue::from_f64(self.end as f64));
        set(&object, "analysed", &JsValue::from_bool(self.analysed));
        let readings: js_sys::Array = self.readings.iter().map(Reading::to_js).collect();
        set(&object, "readings", &readings);
        object.into()
    }
}

impl Reading {
    fn to_js(&self) -> JsValue {
        let object = js_sys::Object::new();
        set(&object, "analysis", &JsValue::from_str(&self.analysis));
        set(
            &object,
            "weight",
            &JsValue::from_f64(f64::from(self.weight)),
        );
        object.into()
    }
}

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(object, &JsValue::from_str(key), value)
        .expect("a fresh plain object takes any property");
}

/// Reads one text symbol at a time, as `hfst tokenise` does, unless the
/// caller asked for multicharacter symbols or the network reads some.
fn choose_segmentation(container: &mut PmatchContainer, settings: &TokenizeSettings) {
    let single = !settings.tokenize_multichar && !container.has_multichar_input_symbols();
    container.set_single_codepoint_tokenization(single);
}

fn tokens(
    container: &mut PmatchContainer,
    text: &str,
    settings: &TokenizeSettings,
    cutoff: Weight,
) -> Vec<Token> {
    choose_segmentation(container, settings);
    let located = container.locate(text, settings.time_cutoff, cutoff);
    select(&located, text, settings)
}

/// The tokens `match_and_print` in `hfst::pmatch_tokenize` prints from
/// `located`, the locations of `text`: each token's readings after dedupe and
/// the weight-class limit, less the naive tokeniser's fallback readings,
/// within the beam the xerox format applies. Stretches no pattern matched are
/// tokens only under `print_all`, as they are printed only then.
fn select(located: &LocationVectorVector, text: &str, settings: &TokenizeSettings) -> Vec<Token> {
    let mut cursor = Cursor::new(text);
    if located.is_empty() {
        // The library prints the whole input as unanalysed when it locates
        // nothing in it.
        if !settings.print_all || text.is_empty() {
            return Vec::new();
        }
        let end = cursor.utf16_end();
        return vec![Token {
            surface: text.to_owned(),
            start: 0,
            end,
            analysed: false,
            readings: Vec::new(),
        }];
    }
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

/// A place in a text, as a byte offset and the UTF-16 offset of the same
/// place, moving only forward through the text as the tokens do.
struct Cursor<'a> {
    text: &'a str,
    byte: usize,
    utf16: usize,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Cursor {
            text,
            byte: 0,
            utf16: 0,
        }
    }

    fn advance(&mut self, byte: usize) {
        self.utf16 += self.text[self.byte..byte].encode_utf16().count();
        self.byte = byte;
    }

    fn utf16_end(&mut self) -> usize {
        self.advance(self.text.len());
        self.utf16
    }

    /// The UTF-16 span of the text that spells `symbols`, the next input
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
            self.advance(self.byte + offset);
            start.get_or_insert(self.utf16);
            self.advance(self.byte + symbol.len());
        }
        (start.unwrap_or(self.utf16), self.utf16)
    }
}

/// What `hfst tokenise` prints with these settings when `text` is its whole
/// standard input.
fn print(
    container: &mut PmatchContainer,
    text: &str,
    settings: &TokenizeSettings,
    input: &TokenizeInputSettings,
) -> Result<String, GlueError> {
    choose_segmentation(container, settings);
    let mut printed = Vec::new();
    let status = process_input_stream(
        container,
        &mut text.as_bytes(),
        &mut printed,
        &mut std::io::sink(),
        settings,
        input,
    );
    if status != 0 {
        return Err(GlueError::new(
            "failed",
            format!("the tokeniser stopped with status {status}"),
        ));
    }
    String::from_utf8(printed).map_err(|error| {
        GlueError::new(
            "internal",
            format!("the tokeniser printed bytes that are not UTF-8 ({error})"),
        )
    })
}

/// The options `tokenise` and `format` share, each `None` where the caller
/// gave none.
#[derive(Clone, Copy, Debug, Default)]
struct Selection {
    dedupe: Option<bool>,
    max_weight_classes: Option<f64>,
    beam: Option<f64>,
    time_limit: Option<f64>,
    tokenize_multichar: Option<bool>,
    print_all: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default)]
struct TokeniseOptions {
    selection: Selection,
    weight_cutoff: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default)]
struct FormatOptions {
    selection: Selection,
    print_weights: Option<bool>,
    hack_uncompose: Option<bool>,
    newline: Option<bool>,
    keep_newline: Option<bool>,
    superblanks: Option<bool>,
}

impl Selection {
    fn read(options: &Options<'_>) -> Result<Self, GlueError> {
        Ok(Selection {
            dedupe: options.flag("dedupe")?,
            max_weight_classes: options.number("maxWeightClasses")?,
            beam: options.number("beam")?,
            time_limit: options.number("timeLimit")?,
            tokenize_multichar: options.flag("tokenizeMultichar")?,
            print_all: options.flag("printAll")?,
        })
    }

    /// Writes the options given over `settings`, checked as `hfst tokenise`
    /// checks the option values it parses.
    fn apply(&self, settings: &mut TokenizeSettings) -> Result<(), GlueError> {
        if let Some(dedupe) = self.dedupe {
            settings.dedupe = dedupe;
        }
        if let Some(classes) = self.max_weight_classes {
            settings.max_weight_classes = weight_classes(classes)?;
        }
        if let Some(beam) = self.beam {
            settings.beam = non_negative("beam", beam)? as f32;
        }
        if let Some(milliseconds) = self.time_limit {
            settings.time_cutoff = non_negative("timeLimit", milliseconds)? / 1000.0;
        }
        if let Some(multichar) = self.tokenize_multichar {
            settings.tokenize_multichar = multichar;
        }
        if let Some(print_all) = self.print_all {
            settings.print_all = print_all;
        }
        Ok(())
    }
}

impl TokeniseOptions {
    fn read(options: &JsValue) -> Result<Self, GlueError> {
        let options = Options::new(options)?;
        Ok(TokeniseOptions {
            selection: Selection::read(&options)?,
            weight_cutoff: options.number("weightCutoff")?,
        })
    }

    /// The settings and the weight cutoff `tokenise` runs with. Readings
    /// carry their weights, so dedupe tells readings apart by weight, as
    /// `hfst tokenise --print-weights` does.
    fn settings(&self) -> Result<(TokenizeSettings, Weight), GlueError> {
        let mut settings = TokenizeSettings {
            print_weights: true,
            ..TokenizeSettings::default()
        };
        self.selection.apply(&mut settings)?;
        let cutoff = match self.weight_cutoff {
            None => INFINITE_WEIGHT,
            Some(cutoff) if cutoff.is_nan() => {
                return Err(GlueError::new(
                    INVALID_ARGUMENT,
                    "weightCutoff is a number, and NaN is not one",
                ));
            }
            Some(cutoff) => cutoff as f32,
        };
        Ok((settings, cutoff))
    }
}

impl FormatOptions {
    fn read(options: &JsValue) -> Result<Self, GlueError> {
        let options = Options::new(options)?;
        Ok(FormatOptions {
            selection: Selection::read(&options)?,
            print_weights: options.flag("printWeights")?,
            hack_uncompose: options.flag("hackUncompose")?,
            newline: options.flag("newline")?,
            keep_newline: options.flag("keepNewline")?,
            superblanks: options.flag("superblanks")?,
        })
    }

    /// The settings `hfst tokenise` runs with when given the flag that
    /// chooses `format` and then a flag for each option given, so a given
    /// option overrides what the format's flag implies.
    fn settings(
        &self,
        format: OutputFormat,
    ) -> Result<(TokenizeSettings, TokenizeInputSettings), GlueError> {
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
        self.selection.apply(&mut settings)?;
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
        Ok((settings, input))
    }
}

/// `--weight-classes` takes a whole number of at least 1. Infinity and any
/// count past `i32::MAX` keep every class, as the setting's default does.
fn weight_classes(classes: f64) -> Result<i32, GlueError> {
    if classes >= 1.0 && (classes.fract() == 0.0 || classes.is_infinite()) {
        return Ok(classes.min(f64::from(i32::MAX)) as i32);
    }
    Err(GlueError::new(
        INVALID_ARGUMENT,
        format!("maxWeightClasses is a whole number of at least 1, not {classes}"),
    ))
}

fn non_negative(name: &str, value: f64) -> Result<f64, GlueError> {
    if value >= 0.0 {
        return Ok(value);
    }
    Err(GlueError::new(
        INVALID_ARGUMENT,
        format!("{name} is a number of at least 0, not {value}"),
    ))
}

/// An options object from the wrapper. `undefined` and `null`, for the
/// object or any field, mean the option was not given.
struct Options<'a>(Option<&'a JsValue>);

impl<'a> Options<'a> {
    fn new(options: &'a JsValue) -> Result<Self, GlueError> {
        if options.is_undefined() || options.is_null() {
            return Ok(Options(None));
        }
        if !options.is_object() {
            return Err(GlueError::new(
                INVALID_ARGUMENT,
                "the options are an object",
            ));
        }
        Ok(Options(Some(options)))
    }

    fn get(&self, name: &str) -> Result<Option<JsValue>, GlueError> {
        let Some(options) = self.0 else {
            return Ok(None);
        };
        let value = js_sys::Reflect::get(options, &JsValue::from_str(name)).map_err(|_| {
            GlueError::new(
                INVALID_ARGUMENT,
                format!("the option {name} cannot be read"),
            )
        })?;
        Ok((!value.is_undefined() && !value.is_null()).then_some(value))
    }

    fn flag(&self, name: &str) -> Result<Option<bool>, GlueError> {
        self.get(name)?
            .map(|value| {
                value.as_bool().ok_or_else(|| {
                    GlueError::new(INVALID_ARGUMENT, format!("the option {name} is a boolean"))
                })
            })
            .transpose()
    }

    fn number(&self, name: &str) -> Result<Option<f64>, GlueError> {
        self.get(name)?
            .map(|value| {
                value.as_f64().ok_or_else(|| {
                    GlueError::new(INVALID_ARGUMENT, format!("the option {name} is a number"))
                })
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::file_resolver::{FileResolver, MemoryFiles};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::pmatch_compiler::{PmatchCompiler, write_archive};

    fn read(path: &str) -> String {
        std::fs::read_to_string(format!("{}/../../js/{path}", env!("CARGO_MANIFEST_DIR")))
            .expect("the file is in the repository")
    }

    /// The JavaScript tests' tokeniser, compiled as `hfst pmatch2fst`
    /// compiles it.
    fn fixture() -> Vec<u8> {
        let mut compiler = PmatchCompiler::<hfst_openfst::StdVectorFst>::new();
        let mut definitions = compiler
            .compile(&read("test/fixtures/tokenise.pmatch"))
            .expect("the fixture compiles");
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
        TokeniseOptions::default()
            .settings()
            .expect("the defaults are valid")
            .0
    }

    /// The text `tokens` covers, by its UTF-16 offsets into `text`.
    fn covered(text: &str, token: &Token) -> String {
        let units: Vec<u16> = text.encode_utf16().collect();
        String::from_utf16(&units[token.start..token.end]).expect("a span ends between characters")
    }

    #[test]
    fn offsets_count_utf16_units_through_astral_characters() {
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
        let tokens = select(&located, text, &settings);
        let spans: Vec<(usize, usize)> = tokens.iter().map(|t| (t.start, t.end)).collect();
        assert_eq!(spans, [(0, 2), (2, 3), (3, 5), (5, 6), (6, 8)]);
        for token in &tokens {
            assert_eq!(covered(text, token), token.surface);
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
        let (giella, input) = none.settings(OutputFormat::giellacg).expect("valid");
        assert!(giella.print_weights && giella.print_all && giella.dedupe && giella.hack_uncompose);
        assert_eq!(giella.max_weight_classes, 2);
        assert!(input.blankline_separated && !input.keep_newlines && !input.superblanks);
        let (visl, _) = none.settings(OutputFormat::visl).expect("valid");
        assert!(!visl.print_weights && visl.print_all && visl.dedupe);
        let (cg, _) = none.settings(OutputFormat::cg).expect("valid");
        assert!(!cg.print_weights && !cg.print_all && cg.dedupe);
        assert_eq!(cg.max_weight_classes, i32::MAX);

        let given = FormatOptions {
            selection: Selection {
                dedupe: Some(false),
                max_weight_classes: Some(5.0),
                time_limit: Some(1500.0),
                ..Selection::default()
            },
            print_weights: Some(false),
            keep_newline: Some(true),
            ..FormatOptions::default()
        };
        let (giella, input) = given.settings(OutputFormat::giellacg).expect("valid");
        assert!(!giella.print_weights && !giella.dedupe);
        assert_eq!(giella.max_weight_classes, 5);
        assert_eq!(giella.time_cutoff, 1.5);
        assert!(input.keep_newlines && !input.blankline_separated);
    }

    #[test]
    fn option_values_are_checked_like_the_command_line() {
        let code = |selection: Selection| {
            let options = TokeniseOptions {
                selection,
                ..TokeniseOptions::default()
            };
            options.settings().map(|_| ()).map_err(|error| error.code())
        };
        for classes in [0.0, 1.5, -3.0, f64::NAN] {
            let selection = Selection {
                max_weight_classes: Some(classes),
                ..Selection::default()
            };
            assert_eq!(code(selection), Err(INVALID_ARGUMENT), "{classes}");
        }
        for (beam, time_limit) in [
            (Some(-0.5), None),
            (None, Some(-1.0)),
            (Some(f64::NAN), None),
        ] {
            let selection = Selection {
                beam,
                time_limit,
                ..Selection::default()
            };
            assert_eq!(
                code(selection),
                Err(INVALID_ARGUMENT),
                "{beam:?} {time_limit:?}"
            );
        }
        assert_eq!(weight_classes(f64::INFINITY).ok(), Some(i32::MAX));
        assert_eq!(weight_classes(1e12).ok(), Some(i32::MAX));
        let nan_cutoff = TokeniseOptions {
            weight_cutoff: Some(f64::NAN),
            ..TokeniseOptions::default()
        };
        assert!(nan_cutoff.settings().is_err());
    }

    #[test]
    fn format_names_are_the_command_line_option_names() {
        let wrapper = read("shared/tokenise.ts");
        for (name, format) in FORMATS {
            assert_eq!(output_format(name).ok(), Some(format));
            assert!(
                wrapper.contains(&format!("\"{name}\"")),
                "the wrapper lists {name}"
            );
        }
        assert!(output_format("giellacg").is_err());
    }

    #[test]
    fn non_pmatch_bytes_are_invalid_formats() {
        let code = |bytes: &[u8]| load(bytes).map(|_| ()).map_err(|error| error.code());
        assert_eq!(code(b""), Err("invalid-format"));
        assert_eq!(code(b"not a stream"), Err("invalid-format"));
        let archive = fixture();
        assert_eq!(code(&archive), Ok(()));
        let truncated = &archive[..archive.len() - 9];
        assert_eq!(code(truncated), Err("invalid-format"));
        let mut renamed = archive.clone();
        let name = renamed
            .windows(9)
            .position(|window| window == b"name\0TOP\0")
            .expect("the first header names TOP");
        renamed[name + 6] = b'I';
        assert_eq!(code(&renamed), Err("invalid-format"));
    }

    /// The cohorts `hfst tokenise --cg --print-weights` prints for tokens
    /// `tokenise` returned with `print_all` off.
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
        let (settings, cutoff) = TokeniseOptions::default().settings().expect("valid");
        let found = tokens(&mut container, text, &settings, cutoff);
        for token in &found {
            assert_eq!(covered(text, token), token.surface);
        }
        let options = FormatOptions {
            print_weights: Some(true),
            ..FormatOptions::default()
        };
        let (settings, input) = options.settings(OutputFormat::cg).expect("valid");
        let printed = print(&mut container, text, &settings, &input).expect("prints");
        assert_eq!(cg(&found), printed);
        let cat = found.iter().find(|t| t.surface == "🐱").expect("a cat");
        assert_eq!(
            cat.end - cat.start,
            2,
            "an astral character is two UTF-16 units"
        );
    }

    #[test]
    fn a_weight_cutoff_abandons_heavier_matches() {
        let mut container = load(&fixture()).expect("the fixture loads");
        let options = TokeniseOptions {
            weight_cutoff: Some(50.0),
            ..TokeniseOptions::default()
        };
        let (settings, cutoff) = options.settings().expect("valid");
        let found = tokens(&mut container, "The cat blorf", &settings, cutoff);
        let surfaces: Vec<&str> = found.iter().map(|t| t.surface.as_str()).collect();
        assert_eq!(surfaces, ["cat"]);
        assert!(found[0].readings.iter().all(|r| r.weight <= 50.0));
    }
}
