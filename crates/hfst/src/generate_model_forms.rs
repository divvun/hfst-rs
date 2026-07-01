//! Port of 'tools/src/generate_model_forms.cc' and
//! 'tools/src/generate_model_forms.h'.
//!
//! Helper used by the 'hfst-guess' tool: it reads model analyses, runs a
//! guesser/generator transducer pair and assembles guessed paradigms.

use std::collections::BTreeSet;

use crate::hfst_data_types::ImplementationType::{HFST_OLW_TYPE, TROPICAL_OPENFST_TYPE};
use crate::hfst_symbol_defs::{StringSet, StringVector};
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::HfstTransducer;

// '#define MODEL_FORM_PREFIX ""'.
const MODEL_FORM_PREFIX: &str = "";

// '#define MAX_ANALYSES 5'.
pub const MAX_ANALYSES: usize = 5;

// 'guessify_fst.h': '#define CATEGORY_SYMBOL_PREFIX "[GUESS_CATEGORY="'.
// Mirrored locally because the 'guessify_fst' module is not yet ported; the
// integrator should reconcile this with 'guessify_fst::is_cathegory_symbol'
// once that module exists.
const CATEGORY_SYMBOL_PREFIX: &str = "[GUESS_CATEGORY=";

// [spec:hfst:def:generate-model-forms.string-vector-vector]
// 'typedef std::vector<StringVector> StringVectorVector'.
pub type StringVectorVector = Vec<StringVector>;

// [spec:hfst:def:generate-model-forms.string-vector-set]
// 'typedef std::set<StringVector> StringVectorSet'.
pub type StringVectorSet = BTreeSet<StringVector>;

// [spec:hfst:def:generate-model-forms.invalid-model-line]
// 'struct InvalidModelLine { std::string line; }'.
#[derive(Clone, Debug)]
pub struct InvalidModelLine {
    pub line: String,
}

impl InvalidModelLine {
    // [spec:hfst:def:generate-model-forms.invalid-model-line.invalid-model-line-fn]
    // [spec:hfst:sem:generate-model-forms.invalid-model-line.invalid-model-line-fn]
    pub fn new(line: String) -> Self {
        InvalidModelLine { line }
    }
}

// [spec:hfst:def:generate-model-forms.invalid-model-file]
// 'struct InvalidModelFile {}'.
#[derive(Clone, Debug)]
pub struct InvalidModelFile;

// Whether `t` is a guesser. hfst-guessify marks the guessers it builds with the
// "reverse input" property (set in guessify_fst::guessify_analyzer), so a
// guesser is exactly a transducer carrying that property. The C++ checked
// get_properties().count("reverse input") != 1; this was inline in hfst-guess's
// main and is lifted here.
pub fn is_guesser(t: &HfstTransducer) -> bool {
    t.get_properties().get("reverse input").is_some()
}

// Compile a generator from a guesser, for the case where the guesser file did
// not already bundle a generator: copy the guesser, convert to tropical, invert
// it (a generator maps the guesser's analyses back to surface forms), and
// convert to the optimised-lookup weighted type. Lifted verbatim from
// hfst-guess's main.
pub fn compile_generator_from_guesser(
    guesser: &HfstTransducer,
) -> crate::error::Result<HfstTransducer> {
    let mut generator = HfstTransducer::new_copy(guesser)?;
    generator.convert(TROPICAL_OPENFST_TYPE, String::new())?;
    generator.invert()?;
    generator.convert(HFST_OLW_TYPE, String::new())?;
    Ok(generator)
}

// 'guessify_fst.cc': 'bool is_cathegory_symbol(const std::string &symbol)'.
fn is_cathegory_symbol(symbol: &str) -> bool {
    symbol.starts_with(CATEGORY_SYMBOL_PREFIX)
}

// [spec:hfst:def:generate-model-forms.get-alphabet-string-tokenizer-fn]
// [spec:hfst:sem:generate-model-forms.get-alphabet-string-tokenizer-fn]
pub fn get_alphabet_string_tokenizer(
    fst: &mut HfstTransducer,
) -> crate::error::Result<HfstTokenizer> {
    // FIXME: temporary until optimized lookup transducers implement
    // get_alphabet.
    let mut temp = HfstTransducer::new_copy(fst)?;
    temp.convert(TROPICAL_OPENFST_TYPE, String::new())?;

    let alphabet: StringSet = temp.get_alphabet()?;

    let mut tokenizer = HfstTokenizer::new();

    for it in &alphabet {
        tokenizer.add_multichar_symbol(it);
    }

    Ok(tokenizer)
}

// [spec:hfst:def:generate-model-forms.get-analysis-prefix-fn]
// [spec:hfst:sem:generate-model-forms.get-analysis-prefix-fn]
fn get_analysis_prefix(reversed_analysis: &StringVector) -> StringVector {
    let mut prefix: StringVector = StringVector::new();

    // We want a prefix of a reversed string, so we iterate from end to
    // beginning.
    for it in reversed_analysis.iter().rev() {
        prefix.push(it.clone());

        if is_cathegory_symbol(it) {
            // When we return, we have to reverse the result.
            return prefix.iter().rev().cloned().collect();
        }
    }

    // It should actually be impossible to get here, since valid
    // analyses contain at least one cathegory symbol.
    prefix
}

// [spec:hfst:def:generate-model-forms.join-fn]
// [spec:hfst:sem:generate-model-forms.join-fn]
fn join(mut sv1: StringVector, sv2: &StringVector) -> StringVector {
    sv1.extend(sv2.iter().cloned());
    sv1
}

// [spec:hfst:def:generate-model-forms.contains-analysis-symbols-fn]
// [spec:hfst:sem:generate-model-forms.contains-analysis-symbols-fn]
fn contains_analysis_symbols(word_form: &StringVector) -> bool {
    for symbol in word_form {
        let bytes = symbol.as_bytes();
        if bytes.len() > 1 && bytes[0] == b'[' && bytes[bytes.len() - 1] == b']' {
            return true;
        }
    }

    false
}

// [spec:hfst:def:generate-model-forms.generate-word-forms-fn]
// [spec:hfst:sem:generate-model-forms.generate-word-forms-fn]
fn generate_word_forms(
    analysis: &StringVector,
    form_generator: &mut HfstTransducer,
    max_generated_forms: usize,
    generate_threshold: f32,
) -> crate::error::Result<StringVector> {
    let word_forms = form_generator.lookup_string_vector(analysis, -1, 0.0)?;

    let mut result_set: StringVectorSet = StringVectorSet::new();

    let mut num: usize = 1;

    let mut best_weight: f32 = -1.0;

    for path in &word_forms {
        if num > max_generated_forms {
            break;
        }

        if best_weight == -1.0 {
            best_weight = path.first;
        }

        if path.first - best_weight >= generate_threshold {
            break;
        }

        let word_form = &path.second;

        if contains_analysis_symbols(word_form) {
            continue;
        }

        // The word form is reversed, so we start from the end and
        // iterate to the beginning.
        result_set.insert(word_form.iter().rev().cloned().collect());

        num += 1;
    }

    let mut results: StringVector = StringVector::new();

    let mut first_form = true;

    for it in &result_set {
        if !first_form {
            results.push(", ".to_string());
        }

        results.extend(it.iter().cloned());

        first_form = false;
    }

    if results.is_empty() {
        results.push("<no word forms>".to_string());
    }

    Ok(results)
}

// [spec:hfst:def:generate-model-forms.get-model-forms-fn]
// [spec:hfst:sem:generate-model-forms.get-model-forms-fn]
fn get_model_forms(
    reversed_analysis: &StringVector,
    model_forms: &StringVectorVector,
    form_generator: &mut HfstTransducer,
    max_generated_forms: usize,
    generate_threshold: f32,
) -> crate::error::Result<StringVectorVector> {
    let reversed_analysis_prefix = get_analysis_prefix(reversed_analysis);

    let mut results: StringVectorVector = StringVectorVector::new();

    for it in model_forms {
        let model_analysis = join(it.clone(), &reversed_analysis_prefix);

        results.push(generate_word_forms(
            &model_analysis,
            form_generator,
            max_generated_forms,
            generate_threshold,
        )?);
    }

    Ok(results)
}

// [spec:hfst:def:generate-model-forms.split-fn]
// [spec:hfst:sem:generate-model-forms.split-fn]
// Retained from the C++ source; not referenced by the rest of the helper.
#[allow(dead_code)]
fn split(line: &str, separator: &str) -> crate::hfst_symbol_defs::StringPair {
    let separator_pos = line.find(separator).unwrap_or(usize::MAX);

    let first_end = separator_pos.min(line.len());
    let second_start = separator_pos.saturating_add(1).min(line.len());

    (
        line[..first_end].to_string(),
        line[second_start..].to_string(),
    )
}

// [spec:hfst:def:generate-model-forms.read-model-form-fn]
// [spec:hfst:sem:generate-model-forms.read-model-form-fn]
fn read_model_form(
    content: &[u8],
    pos: &mut usize,
    tokenizer: &mut HfstTokenizer,
) -> Result<StringVector, InvalidModelLine> {
    let line = getline(content, pos);

    if line.find(MODEL_FORM_PREFIX) != Some(0) {
        return Err(InvalidModelLine::new(line));
    }

    let model_form = line[MODEL_FORM_PREFIX.len()..].to_string();

    let mut tokenized_model_form = tokenizer.tokenize_one_level(&model_form, false);
    tokenized_model_form.reverse();

    Ok(tokenized_model_form)
}

// [spec:hfst:def:generate-model-forms.read-model-forms-fn]
// [spec:hfst:sem:generate-model-forms.read-model-forms-fn]
pub fn read_model_forms(
    model_form_filename: &str,
    tokenizer: &mut HfstTokenizer,
) -> Result<StringVectorVector, InvalidModelLine> {
    // A failed open behaves like an empty stream ('peek() == EOF').
    let content = std::fs::read(model_form_filename).unwrap_or_default();

    let mut pos: usize = 0;

    if pos >= content.len() {
        std::panic::panic_any(InvalidModelFile);
    }

    let mut results: StringVectorVector = StringVectorVector::new();

    while pos < content.len() {
        results.push(read_model_form(&content, &mut pos, tokenizer)?);
    }

    Ok(results)
}

// [spec:hfst:def:generate-model-forms.get-guesses-fn]
// [spec:hfst:sem:generate-model-forms.get-guesses-fn]
pub fn get_guesses(
    word_form: &str,
    guesser: &mut HfstTransducer,
    number_of_guesses: usize,
    tokenizer: &mut HfstTokenizer,
) -> crate::error::Result<StringVectorVector> {
    let mut tokenized_line = tokenizer.tokenize_one_level(word_form, false);
    tokenized_line.reverse();

    let paths = guesser.lookup_fd_string_vector(&tokenized_line, -1, 0.0)?;

    let mut num: usize = 1;

    let mut results: StringVectorVector = StringVectorVector::new();

    for path in &paths {
        if num > number_of_guesses {
            break;
        }
        results.push(path.second.clone());
        num += 1;
    }

    Ok(results)
}

// [spec:hfst:def:generate-model-forms.get-paradigms-fn]
// [spec:hfst:sem:generate-model-forms.get-paradigms-fn]
pub fn get_paradigms(
    word_form: &str,
    guesses: &StringVectorVector,
    generator: &mut HfstTransducer,
    model_forms: &StringVectorVector,
    number_of_generated_forms: usize,
    generate_threshold: f32,
) -> crate::error::Result<StringVectorVector> {
    let mut paradigm_guesses: StringVectorVector = StringVectorVector::new();

    for it in guesses {
        let analysis_guess = it.clone();

        let results = get_model_forms(
            &analysis_guess,
            model_forms,
            generator,
            number_of_generated_forms,
            generate_threshold,
        )?;

        let mut paradigm: StringVector = StringVector::new();
        paradigm.push(word_form.to_string());
        paradigm.push("\t".to_string());

        let mut rev_analysis_guess = analysis_guess.clone();
        rev_analysis_guess.reverse();
        paradigm.extend(rev_analysis_guess.iter().cloned());

        for jt in &results {
            let model_form = jt;

            paradigm.push("\t".to_string());
            paradigm.extend(model_form.iter().cloned());
        }

        paradigm_guesses.push(paradigm);
    }

    Ok(paradigm_guesses)
}

// Models 'std::getline(in, line)' over a byte stream: reads up to (and
// consuming) the next '\n', returning the bytes before it as a UTF-8 line.
fn getline(content: &[u8], pos: &mut usize) -> String {
    let start = *pos;

    while *pos < content.len() && content[*pos] != b'\n' {
        *pos += 1;
    }

    let line = String::from_utf8_lossy(&content[start..*pos]).into_owned();

    if *pos < content.len() {
        // Consume the '\n'.
        *pos += 1;
    }

    line
}
