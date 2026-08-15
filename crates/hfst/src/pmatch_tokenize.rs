//! Full port of 'libhfst/src/implementations/optimized-lookup/pmatch_tokenize.{h,cc}'
//! (namespace 'hfst_ol_tokenize').
//!
//! This is a standalone module that depends on the 'pmatch' module types
//! ('PmatchContainer', 'Location', 'LocationVector', 'LocationVectorVector').
//!
//! C++ 'std::ostream &' parameters are modelled as '&mut dyn std::io::Write'.
//! C++ 'std::cerr' diagnostics become 'tracing' events.

use std::collections::BTreeSet;
use std::io::{BufRead, Write};

use crate::backend::AlgebraBackend;
use crate::convert_transducer_format::ConversionFunctions;
use crate::error::Result;
use crate::hfst_data_types::StringVector;
use crate::hfst_transducer::HfstTransducer;
use crate::pmatch::{Location, LocationVector, LocationVectorVector, PmatchContainer};
use crate::pmatch_compiler;
use crate::transducer::{INFINITE_WEIGHT, Weight};

use icu::properties::CodePointMapData;
use icu::properties::props::GeneralCategory;
use icu::segmenter::GraphemeClusterSegmenter;

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.output-format]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    tokenize,
    space_separated,
    xerox,
    cg,
    finnpos,
    giellacg,
    conllu,
    visl,
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.tokenize-settings]
#[derive(Clone)]
pub struct TokenizeSettings {
    pub output_format: OutputFormat,
    pub max_weight_classes: i32,
    pub dedupe: bool,
    pub print_weights: bool,
    pub print_all: bool,
    pub time_cutoff: f64,
    pub weight_cutoff: f32,
    pub verbose: bool,
    pub beam: f32,
    pub tokenize_multichar: bool,
    pub hack_uncompose: bool,
    /// Per-run dedup for the "skipping modifier letter" warning (was a process
    /// global 'static bool'). 'Cell' so it can be flipped through the '&self'
    /// borrow as the settings are threaded immutably through the print chain.
    pub cg_tag_modifier_warned: std::cell::Cell<bool>,
}

impl Default for TokenizeSettings {
    fn default() -> TokenizeSettings {
        TokenizeSettings {
            output_format: OutputFormat::tokenize,
            max_weight_classes: i32::MAX,
            dedupe: false,
            print_weights: false,
            print_all: false,
            time_cutoff: 0.0,
            weight_cutoff: -1.0,
            verbose: true,
            beam: -1.0,
            tokenize_multichar: false,
            hack_uncompose: false,
            cg_tag_modifier_warned: std::cell::Cell::new(false),
        }
    }
}

const subreading_separator: &str = "#";
const wtag: &str = "W"; // (C++ note) cg-conv has an argument --wtag, allow
// changing here as well?

// [spec:hfst:def:pmatch-tokenize.find-first-not-of-def-fn]
// [spec:hfst:sem:pmatch-tokenize.find-first-not-of-def-fn]
pub fn find_first_not_of_def(str: &str, c: u8, def: usize) -> usize {
    match str.as_bytes().iter().position(|&b| b != c) {
        None => def,
        Some(ret) => ret,
    }
}

// [spec:hfst:def:pmatch-tokenize.find-last-not-of-def-fn]
// [spec:hfst:sem:pmatch-tokenize.find-last-not-of-def-fn]
pub fn find_last_not_of_def(str: &str, c: u8, def: usize) -> usize {
    match str.as_bytes().iter().rposition(|&b| b != c) {
        None => def,
        Some(ret) => ret,
    }
}

/// Weight carried by a reading that segments a token without analysing it —
/// the naive tokenizer's `others` fallback.
///
/// PORT DIVERGENCE. Upstream marks that fallback with
/// `std::numeric_limits<float>::max()`, but `PmatchTransducer::get_analyses`
/// abandons any walk whose running weight exceeds the cutoff handed to
/// `locate`, and every call site here passes `INFINITE_WEIGHT` — which is
/// `NO_TABLE_INDEX as f32`, about 4.29e9, twenty-nine orders of magnitude
/// below `f32::MAX`. The fallback branch is therefore unreachable upstream and
/// dictionary-external input is discarded. `INFINITE_WEIGHT` is the largest
/// weight the runtime admits (its cutoff test is a strict `>`), so it keeps
/// the fallback worse than any realistic dictionary path while staying
/// reachable.
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.unanalysed-weight]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.unanalysed-weight]
pub const UNANALYSED_WEIGHT: Weight = INFINITE_WEIGHT;

/// Whether a location segments a token without analysing it: an empty output,
/// the ` ??` unknown marker a pmatch script can emit, or the naive tokenizer's
/// fallback weight.
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn]
pub fn is_unanalysed(loc: &Location) -> bool {
    loc.output.is_empty() || loc.output.contains(" ??") || loc.weight >= UNANALYSED_WEIGHT
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-escaping-backslashes-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-escaping-backslashes-fn]
pub fn print_escaping_backslashes(str: &str, outstream: &mut dyn Write) {
    // (C++ note) inline?
    let bytes = str.as_bytes();
    let mut i: usize = 0;
    let mut j: usize;
    while let Some(rel) = str[i..].find('\\') {
        j = i + rel;
        let _ = outstream.write_all(&bytes[i..j]);
        let _ = outstream.write_all(b"\\\\");
        i = j + 1;
    }
    // mirror C++ substr(i, npos): the rest of the string from i
    let _ = outstream.write_all(&bytes[i..]);
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-no-output-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-no-output-fn]
pub fn print_no_output(input: &str, outstream: &mut dyn Write, s: &TokenizeSettings) {
    if s.output_format == OutputFormat::tokenize || s.output_format == OutputFormat::space_separated
    {
        let _ = write!(outstream, "{}", input);
    } else if s.output_format == OutputFormat::xerox {
        // [#500] hfst-lookup renders an unanalysed form as `%i\t%i+?\t%w` with
        // an infinite weight, i.e. `input<TAB>input+?<TAB>inf`; match that
        // three-field layout instead of dropping the weight column.
        let _ = write!(outstream, "{}\t{}+?\tinf", input, input);
    } else if s.output_format == OutputFormat::cg || s.output_format == OutputFormat::giellacg {
        let _ = write!(outstream, "\"<");
        print_escaping_backslashes(input, outstream);
        let _ = write!(outstream, ">\"\n\t\"");
        print_escaping_backslashes(input, outstream);
        let _ = write!(outstream, "\" ?");
    }
    //    std::cerr << "from print_no_output\n";
    let _ = write!(outstream, "\n\n");
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-escaping-newlines-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-escaping-newlines-fn]
pub fn print_escaping_newlines(str: &str, outstream: &mut dyn Write) {
    // (C++ note) inline?
    let bytes = str.as_bytes();
    let mut i: usize = 0;
    let mut j: usize;
    while let Some(rel) = str[i..].find(['\n', '\r']) {
        j = i + rel;
        let _ = outstream.write_all(&bytes[i..j]);
        if bytes[j] == b'\n' {
            let _ = write!(outstream, "\\n");
        } else if bytes[j] == b'\r' {
            let _ = write!(outstream, "\\r");
        }
        i = j + 1;
    }
    let _ = outstream.write_all(&bytes[i..]);
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-nonmatching-sequence-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-nonmatching-sequence-fn]
pub fn print_nonmatching_sequence(str: &str, outstream: &mut dyn Write, s: &TokenizeSettings) {
    if s.output_format == OutputFormat::tokenize || s.output_format == OutputFormat::space_separated
    {
        let _ = write!(outstream, "{}", str);
    } else if s.output_format == OutputFormat::xerox {
        // [#500] Same three-field unanalysed layout as hfst-lookup:
        // `input<TAB>input+?<TAB>inf`.
        let _ = write!(outstream, "{}\t{}+?\tinf", str, str);
    } else if s.output_format == OutputFormat::cg {
        let _ = write!(outstream, "\"<");
        print_escaping_backslashes(str, outstream);
        let _ = write!(outstream, ">\"\n\t\"");
        print_escaping_backslashes(str, outstream);
        let _ = write!(outstream, "\" ?");
    } else if s.output_format == OutputFormat::giellacg {
        let _ = write!(outstream, ":");
        print_escaping_newlines(str, outstream);
    } else if s.output_format == OutputFormat::visl || s.output_format == OutputFormat::conllu {
        let _ = write!(outstream, "{}", str);
    } else if s.output_format == OutputFormat::finnpos {
        let _ = write!(outstream, "{}\t_\t_\t_\t_", str);
    }
    //    std::cerr << "from print_nonmatching_sequence\n";
    let _ = writeln!(outstream);
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.location-compare-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.location-compare-fn]
pub fn location_compare(lhs: &Location, rhs: &Location) -> bool {
    if lhs.weight == rhs.weight {
        if lhs.tag == rhs.tag {
            if lhs.start == rhs.start {
                if lhs.length == rhs.length {
                    lhs.output < rhs.output
                } else {
                    lhs.length < rhs.length
                }
            } else {
                lhs.start < rhs.start
            }
        } else {
            lhs.tag < rhs.tag
        }
    } else {
        lhs.weight < rhs.weight
    }
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.location-compare-ignoring-weights-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.location-compare-ignoring-weights-fn]
pub fn location_compare_ignoring_weights(lhs: &Location, rhs: &Location) -> bool {
    if lhs.tag == rhs.tag {
        if lhs.start == rhs.start {
            if lhs.length == rhs.length {
                lhs.output < rhs.output
            } else {
                lhs.length < rhs.length
            }
        } else {
            lhs.start < rhs.start
        }
    } else {
        lhs.tag < rhs.tag
    }
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.location-compare-using-only-weights-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.location-compare-using-only-weights-fn]
pub fn location_compare_using_only_weights(lhs: &Location, rhs: &Location) -> bool {
    lhs.weight < rhs.weight
}

// Helper: insert 'loc' into a vector that models a 'std::set<Location, cmp>',
// keeping it sorted by the strict-weak-ordering 'cmp' and dropping equivalents
// (neither cmp(a,b) nor cmp(b,a)). Mirrors std::set insertion semantics.
fn set_insert(set: &mut Vec<Location>, cmp: fn(&Location, &Location) -> bool, loc: &Location) {
    let mut idx = 0usize;
    while idx < set.len() {
        if cmp(loc, &set[idx]) {
            // loc < set[idx]: insert here
            set.insert(idx, loc.clone());
            return;
        } else if cmp(&set[idx], loc) {
            // set[idx] < loc: keep scanning
            idx += 1;
        } else {
            // equivalent: already present, drop
            return;
        }
    }
    set.push(loc.clone());
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.dedupe-locations-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.dedupe-locations-fn]
pub fn dedupe_locations(locations: &LocationVector, s: &TokenizeSettings) -> LocationVector {
    if !s.dedupe {
        return locations.clone();
    }
    if s.print_weights {
        let mut ls: Vec<Location> = Vec::new();
        for loc in locations.iter() {
            set_insert(&mut ls, location_compare, loc);
        }
        let mut uniq: LocationVector = Vec::new();
        uniq.extend(ls);
        uniq
    } else {
        let mut ls: Vec<Location> = Vec::new();
        for loc in locations.iter() {
            set_insert(&mut ls, location_compare_ignoring_weights, loc);
        }
        let mut uniq: LocationVector = Vec::new();
        uniq.extend(ls);
        // std::sort with location_compare_using_only_weights; emulate with
        // sort_by mapping the bool-less comparator to Ordering.
        uniq.sort_by(|a, b| {
            if location_compare_using_only_weights(a, b) {
                std::cmp::Ordering::Less
            } else if location_compare_using_only_weights(b, a) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        uniq
    }
}

/// Keep only the max_weight_classes best weight classes
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.keep-n-best-weight-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.keep-n-best-weight-fn]
pub fn keep_n_best_weight(locations: &LocationVector, s: &TokenizeSettings) -> LocationVector {
    if locations.len() <= s.max_weight_classes as usize {
        // We know we won't trim anything, no need to copy the vector:
        return locations.clone();
    }
    let mut classes_found: i32 = -1;
    let mut last_weight_class: Weight = 0.0;
    let mut goodweight: LocationVector = Vec::new();
    for it in locations.iter() {
        // Tokenised-but-unanalysed readings are placeholders, not real
        // analyses. They must not count toward the weight classes: since the
        // unknown reading sits at the best (lowest) weight, counting it would
        // let `--weight-classes 1` keep only the unknown and drop every
        // genuine, non-zero-weight analysis (issue #562). Pass them through
        // uncounted; the giellacg printer already suppresses them whenever a
        // real reading survives.
        if is_unanalysed(it) {
            goodweight.push(it.clone());
            continue;
        }
        let current_weight: Weight = it.weight;
        if classes_found == -1
        // we're just starting
        {
            classes_found = 1;
            last_weight_class = current_weight;
        } else if last_weight_class != current_weight {
            last_weight_class = current_weight;
            classes_found += 1;
        }
        if classes_found > s.max_weight_classes {
            break;
        } else {
            goodweight.push(it.clone());
        }
    }
    goodweight
}

/// Return the size in bytes of the first complete UTF-8 codepoint in c,
/// or 0 if invalid.
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.u8-first-codepoint-size-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.u8-first-codepoint-size-fn]
pub fn u8_first_codepoint_size(c: &[u8]) -> usize {
    let b = c[0];
    if b <= 127 {
        1
    } else if (b & (128 + 64 + 32 + 16)) == (128 + 64 + 32 + 16) {
        4
    } else if (b & (128 + 64 + 32)) == (128 + 64 + 32) {
        3
    } else if (b & (128 + 64)) == (128 + 64) {
        2
    } else {
        0
    }
}

/// We define tags (non-lemmas) as being exactly the Multichar_symbols.
/// Since non-Multichar_symbols may still be multi*byte*, we check that
/// the symbol is strictly longer than the size of the first
/// possibly-multi-byte codepoint.
///
/// If we have ICU, we check that the symbol is longer than the first
/// "character" (so characters composed of multiple codepoints are
/// treated the same as their non-composed counterparts). ICU
/// doesn't treat modifier letters of as part of the same character,
/// but we sometimes have them on the same arc – e.g. 'k̓ʷ' where 'ʷ' is
/// a modifier – so we skip following modifiers too (c.f. issue 497).
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.is-cg-tag-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.is-cg-tag-fn]
pub fn is_cg_tag(str: &str, modifier_warned: &std::cell::Cell<bool>) -> bool {
    // C++ uses an ICU UnicodeString (UTF-16) and a character BreakIterator.
    // We replicate the UTF-16 semantics: indices/lengths are in UTF-16 code
    // units, grapheme boundaries are computed over the same UTF-16 sequence.
    let utf16: Vec<u16> = str.encode_utf16().collect();
    // grapheme cluster boundary after offset 0, as a UTF-16 code-unit index
    let i_after = following_utf16(str, &utf16, 0);
    let cp_after = char32_at_utf16(&utf16, i_after);
    if u_char_type_is_modifier_letter(cp_after) {
        let is_tag = (utf16.len() as i32) > following_utf16(str, &utf16, i_after);
        if !modifier_warned.get() && !is_tag {
            tracing::warn!(
                "Skipping modifier letter for baseform letter {} (to avoid this warning, ensure Modifiers are not part of the same Multichar_symbol as their preceding Character)",
                str
            );
            modifier_warned.set(true); // warn only once (per run)
        }
        is_tag
    } else {
        (utf16.len() as i32) > i_after
    }
}

// Compute the next grapheme-cluster boundary (BreakIterator::following) at or
// after the UTF-16 code-unit index 'from', returned as a UTF-16 code-unit index.
fn following_utf16(str: &str, utf16: &[u16], from: i32) -> i32 {
    // Map the UTF-16 index 'from' to a byte offset in the UTF-8 string.
    let byte_off = utf16_index_to_byte(str, from as usize);
    let segmenter = GraphemeClusterSegmenter::new();
    let mut bounds = segmenter.segment_str(&str[byte_off..]);
    let _begin = bounds.next(); // 0
    match bounds.next() {
        None => utf16.len() as i32, // UBRK_DONE-ish: no further boundary
        Some(end_byte) => {
            // end_byte is relative to byte_off; convert the absolute byte
            // boundary back to a UTF-16 index.
            byte_to_utf16_index(str, byte_off + end_byte) as i32
        }
    }
}

// char32At for a UTF-16 buffer at code-unit index i (0 if out of range).
fn char32_at_utf16(utf16: &[u16], i: i32) -> u32 {
    if i < 0 || (i as usize) >= utf16.len() {
        return 0;
    }
    let i = i as usize;
    let c = utf16[i];
    if (0xD800..=0xDBFF).contains(&c) && i + 1 < utf16.len() {
        let c2 = utf16[i + 1];
        if (0xDC00..=0xDFFF).contains(&c2) {
            return 0x10000 + (((c as u32 - 0xD800) << 10) | (c2 as u32 - 0xDC00));
        }
    }
    c as u32
}

// Byte offset in 'str' (UTF-8) of the UTF-16 code-unit index 'u16_idx'.
fn utf16_index_to_byte(str: &str, u16_idx: usize) -> usize {
    let mut units = 0usize;
    for (byte_off, ch) in str.char_indices() {
        if units >= u16_idx {
            return byte_off;
        }
        units += ch.len_utf16();
    }
    str.len()
}

// UTF-16 code-unit index corresponding to byte offset 'byte_off' in 'str'.
fn byte_to_utf16_index(str: &str, byte_off: usize) -> usize {
    let mut units = 0usize;
    for (b, ch) in str.char_indices() {
        if b >= byte_off {
            return units;
        }
        units += ch.len_utf16();
    }
    units
}

// u_charType(cp) == U_MODIFIER_LETTER
fn u_char_type_is_modifier_letter(cp: u32) -> bool {
    match char::from_u32(cp) {
        Some(ch) => {
            CodePointMapData::<GeneralCategory>::new().get(ch) == GeneralCategory::ModifierLetter
        }
        None => false,
    }
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-cg-subreading-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-cg-subreading-fn]
#[allow(clippy::too_many_arguments)]
pub fn print_cg_subreading(
    indent: usize,
    out_beg: usize,
    out_end: usize,
    out_syms: &[crate::hfst_data_types::Symbol],
    weight: Weight,
    in_beg: usize,
    in_end: usize,
    in_syms: &[crate::hfst_data_types::Symbol],
    outstream: &mut dyn Write,
    s: &TokenizeSettings,
) {
    let _ = write!(outstream, "{}", "\t".repeat(indent));
    let mut in_lemma = false;
    for it in &out_syms[out_beg..out_end] {
        if it == "@PMATCH_BACKTRACK@" {
            continue;
        }
        let is_tag = is_cg_tag(it, &s.cg_tag_modifier_warned);
        if in_lemma {
            if is_tag {
                in_lemma = false;
                let _ = write!(outstream, "\"");
            }
        } else if !is_tag {
            in_lemma = true;
            let _ = write!(outstream, "\"");
        }
        print_escaping_backslashes(it, outstream);
    }
    if in_lemma {
        let _ = write!(outstream, "\"");
    }

    if s.print_weights {
        let mut rounded = format!("{:.9}", weight);
        let bytes = rounded.as_bytes();
        let mut seendot = false;
        let mut inzeroes = true;
        let mut firstzero = rounded.len();
        let mut i = rounded.len();
        while i > 0 {
            if inzeroes && bytes[i - 1] == b'0' {
                firstzero = i; // not i-1, keep one zero
            } else {
                inzeroes = false;
            }
            if bytes[i - 1] == b'.' {
                seendot = true;
                break;
            }
            i -= 1;
        }
        if seendot {
            rounded = rounded[0..firstzero].to_string();
        }
        let _ = write!(outstream, " <{}:{}>", wtag, rounded);
    }
    if in_beg != in_end {
        let mut form = String::new();
        for sym in &in_syms[in_beg..in_end] {
            form.push_str(sym);
        }
        let _ = write!(outstream, " \"<");
        print_escaping_backslashes(&form, outstream);
        let _ = write!(outstream, ">\"");
    }
    let _ = writeln!(outstream);
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-cg-subreading-ex-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-cg-subreading-ex-fn]
#[allow(clippy::too_many_arguments)]
pub fn print_cg_subreading_ex(
    indent: usize,
    out_beg: usize,
    out_end: usize,
    out_syms: &[crate::hfst_data_types::Symbol],
    weight: Weight,
    in_beg: usize,
    in_end: usize,
    in_syms: &[crate::hfst_data_types::Symbol],
    middle: &str,
    outstream: &mut dyn Write,
    s: &TokenizeSettings,
) {
    let _ = write!(outstream, "{}", "\t".repeat(indent));
    let mut in_lemma = false;
    for it in &out_syms[out_beg..out_end] {
        if it == "@PMATCH_BACKTRACK@" {
            continue;
        }
        let is_tag = is_cg_tag(it, &s.cg_tag_modifier_warned);
        if in_lemma {
            if is_tag {
                in_lemma = false;
                let _ = write!(outstream, "\"");
            }
        } else if !is_tag {
            in_lemma = true;
            let _ = write!(outstream, "\"");
        }
        print_escaping_backslashes(it, outstream);
    }
    if in_lemma {
        let _ = write!(outstream, "\"");
    }
    if s.hack_uncompose && !middle.is_empty() {
        let _ = write!(outstream, " \"{}\"MIDTAPE", middle);
    }
    if s.print_weights {
        let mut rounded = format!("{:.9}", weight);
        let bytes = rounded.as_bytes();
        let mut seendot = false;
        let mut inzeroes = true;
        let mut firstzero = rounded.len();
        let mut i = rounded.len();
        while i > 0 {
            if inzeroes && bytes[i - 1] == b'0' {
                firstzero = i; // not i-1, keep one zero
            } else {
                inzeroes = false;
            }
            if bytes[i - 1] == b'.' {
                seendot = true;
                break;
            }
            i -= 1;
        }
        if seendot {
            rounded = rounded[0..firstzero].to_string();
        }
        let _ = write!(outstream, " <{}:{}>", wtag, rounded);
    }
    if in_beg != in_end {
        let mut form = String::new();
        for sym in &in_syms[in_beg..in_end] {
            form.push_str(sym);
        }
        let _ = write!(outstream, " \"<");
        print_escaping_backslashes(&form, outstream);
        let _ = write!(outstream, ">\"");
    }
    let _ = writeln!(outstream);
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.split-points]
pub type SplitPoints = BTreeSet<usize>;

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-reading-giellacg-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-reading-giellacg-fn]
pub fn print_reading_giellacg(
    loc: &Location,
    mut indent: usize,
    always_wftag: bool,
    outstream: &mut dyn Write,
    s: &TokenizeSettings,
) -> (SplitPoints, usize) {
    let mut bt_its: SplitPoints = BTreeSet::new();
    if loc.output.is_empty() || (is_unanalysed(loc) && indent == 1) {
        return (bt_its, indent);
    }
    // The C++ uses iterators over output_symbol_strings/input_symbol_strings;
    // we model the [beg,end) ranges as usize indices into those vectors.
    let mut out_beg: usize = 0;
    let mut out_end: usize = loc.output_symbol_strings.len();
    let mut in_beg: usize = 0;
    let mut in_end: usize = loc.input_symbol_strings.len();
    if !always_wftag {
        // don't print input wordform tag unless we've seen a subreading/input
        // mark
        in_beg = in_end;
    }
    let mut part: usize = loc.input_parts.len();
    loop {
        let mut sub_found = false;
        let mut out_part: usize = if part > 0 {
            loc.output_parts[part - 1]
        } else {
            0
        };
        while out_part > 0 && loc.output_symbol_strings[out_part - 1] == "@PMATCH_BACKTRACK@" {
            bt_its.insert(loc.input_parts[part - 1]);
            part -= 1;
            out_part = if part > 0 {
                loc.output_parts[part - 1]
            } else {
                0
            };
        }
        // for (PartIt it = out_end - 1; it > begin + out_part; --it)
        // it ranges over indices (out_end-1) down to (out_part+1) inclusive.
        let mut it = out_end as isize - 1;
        while it > (out_part as isize) {
            if subreading_separator == loc.output_symbol_strings[it as usize] {
                // Found a sub-reading mark
                out_beg = (it + 1) as usize; // ++it
                sub_found = true;
                break;
            }
            it -= 1;
        }
        if !sub_found {
            if out_part > 0 {
                // Found an input mark
                out_beg = out_part;
                in_beg = loc.input_parts[part - 1];
                part -= 1;
            } else {
                // No remaining sub-marks or input-marks to the left
                out_beg = 0;
                if in_end != loc.input_symbol_strings.len() {
                    // We've seen at least one input-mark, so we need to output
                    // the remaining input as well
                    in_beg = 0;
                }
            }
        }
        print_cg_subreading_ex(
            indent,
            out_beg,
            out_end,
            &loc.output_symbol_strings,
            loc.weight,
            in_beg,
            in_end,
            &loc.input_symbol_strings,
            &loc.middle,
            outstream,
            s,
        );
        if out_beg == 0 {
            break;
        } else {
            indent += 1;
            out_end = out_beg;
            in_end = in_beg;
            if sub_found {
                out_end -= 1; // skip the subreading separator symbol
            }
        }
    }
    if !bt_its.is_empty() {
        bt_its.insert(0);
        bt_its.insert(loc.input_symbol_strings.len());
    }
    (bt_its, indent)
}

/// Treat syms as "characters" to concatenate and split at indices
/// given by splitpoints to create a new string vector. Assumes
/// splitpoints includes both ends of syms.
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.split-at-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.split-at-fn]
pub fn split_at(syms: &StringVector, splitpoints: &SplitPoints) -> StringVector {
    let mut subs: StringVector = Vec::new();
    if splitpoints.len() < 2 {
        tracing::warn!("split_at called with ");
        return subs;
    }
    let points: Vec<usize> = splitpoints.iter().copied().collect();
    // Loop to next-to-last
    for w in points.windows(2) {
        let start = w[0];
        let next = w[1];
        let mut ss = String::new();
        // Copy the substring between this point and the next:
        for sym in &syms[start..next] {
            ss.push_str(sym);
        }
        subs.push(ss.into());
    }
    subs
}

/*
 * Look up form, filtering out empties and those that don't cover the
 * full string.
 */
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.locate-fullmatch-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.locate-fullmatch-fn]
pub fn locate_fullmatch(
    container: &mut PmatchContainer,
    form: &str,
    s: &TokenizeSettings,
) -> LocationVector {
    let sublocs: LocationVectorVector = container.locate(form, s.time_cutoff, INFINITE_WEIGHT);
    let mut loc_filtered: LocationVector = Vec::new();
    // (C++ note) Worth noticing about? Is this as safe as checking that
    // input.length != form.length? if(sublocs.size() != 1) {
    //     std::cerr << "Warning: '" << form << "' only tokenisable by further
    //     splitting."<<std::endl;
    // }
    for it in sublocs.iter() {
        if it.is_empty()
            || (it.len() == 1 && it[0].output == "@_NONMATCHING_@")
            // keep only those that cover the full form
            || it[0].input.len() != form.len()
        {
            continue;
        }
        let loc = keep_n_best_weight(&dedupe_locations(it, s), s);
        for loc_it in loc.iter() {
            if !is_unanalysed(loc_it) {
                // (C++ note) why aren't the <W:inf> excluded earlier?
                let mut loc_it = loc_it.clone();
                if s.hack_uncompose {
                    container.uncompose(&mut loc_it);
                }
                loc_filtered.push(loc_it);
            }
        }
    }
    loc_filtered
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-splitlocs-r-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-splitlocs-r-fn]
pub fn print_splitlocs_r(
    outstream: &mut dyn Write,
    splitlocs: &LocationVectorVector,
    bottom: usize,
    depth: usize,
    indent: usize,
    out: &mut Vec<Vec<u8>>,
    s: &TokenizeSettings,
) {
    // std::cerr << "bottom depth indent" << bottom << " " << depth << " "
    //           << indent << std::endl;
    let locs = splitlocs[bottom - depth].clone();
    for loc in locs.iter() {
        out[depth].clear();
        print_reading_giellacg(loc, indent, true, &mut out[depth], s);
        if depth == bottom {
            for buf in out.iter() {
                // std::cerr << "DEBUGS 9 " << it->str() << std::endl;
                let _ = outstream.write_all(buf);
            }
        } else {
            print_splitlocs_r(outstream, splitlocs, bottom, depth + 1, indent + 1, out, s);
        }
    }
    // std::cerr << "DEBUGS X " << std::endl;
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-location-vector-giellacg-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-location-vector-giellacg-fn]
pub fn print_location_vector_giellacg(
    container: &mut PmatchContainer,
    locations: &LocationVector,
    outstream: &mut dyn Write,
    s: &TokenizeSettings,
) {
    // std::cerr << "DEBUGS1 " << locations.at(0).output << std::endl;
    let _ = write!(outstream, "\"<");
    print_escaping_backslashes(&locations[0].input, outstream);
    let _ = writeln!(outstream, ">\"");
    if locations.len() == 1 && is_unanalysed(&locations[0]) {
        // Treat empty analyses as unknown-but-tokenised:
        // and ??
        let _ = write!(outstream, "\t\"");
        print_escaping_backslashes(&locations[0].input, outstream);
        let _ = writeln!(outstream, "\" ?");
        return;
    }
    // Output regular analyses first, making a note of backtracking points.
    let mut backtrack: BTreeSet<SplitPoints> = BTreeSet::new();
    for loc_it in locations.iter() {
        // Check for uncompose
        let mut hack = loc_it.clone();
        if s.hack_uncompose {
            container.uncompose(&mut hack);
        }
        let bt_points = print_reading_giellacg(&hack, 1, false, outstream, s).0;
        if !bt_points.is_empty() {
            backtrack.insert(bt_points);
        }
    }
    if backtrack.is_empty() {
        return;
    }
    // The rest of the function handles possible backtracking:
    let in_syms: StringVector = locations[0].input_symbol_strings.clone();
    // std::cerr << "DEBUGS2 " << locations.at(0).output << std::endl;
    for bt_points in backtrack.iter() {
        // First, for every set of backtrack points, we split on every
        // point in that N+1-sized set (the backtrack points include
        // start/end points), and create an N-sized vector splitlocs of
        // resulting analyses
        let mut splitlocs: LocationVectorVector = Vec::new();
        let words = split_at(&in_syms, bt_points);
        for it in words.iter() {
            // Trim left/right spaces:
            let first = find_first_not_of_def(it, b' ', 0);
            let last = 1 + find_last_not_of_def(it, b' ', it.len() - 1);
            let form = it[first..last].to_string();
            let mut loc = locate_fullmatch(container, &form, s);
            if loc.is_empty() && s.verbose {
                tracing::warn!(
                    "The analysis of \"<{}>\" has backtracking around the substring \"<{}>\", but that substring has no analyses.",
                    locations[0].input,
                    form
                );
                // but push it anyway, since we want exactly one subvector per
                // splitpoint
            }
            if form.len() != it.len() {
                // Ensure the spaces we ignored when looking up are output in the
                // form:
                let lspace: Vec<crate::hfst_data_types::Symbol> =
                    vec![crate::hfst_data_types::Symbol::new_static(" "); first];
                let rspace: Vec<crate::hfst_data_types::Symbol> =
                    vec![crate::hfst_data_types::Symbol::new_static(" "); it.len() - last];
                for lvit in loc.iter_mut() {
                    lvit.input = form.clone();
                    let syms = &mut lvit.input_symbol_strings;
                    // syms.insert(begin, lspace) then syms.insert(end, rspace)
                    let mut prefixed: Vec<crate::hfst_data_types::Symbol> = Vec::new();
                    prefixed.extend(lspace.iter().cloned());
                    prefixed.extend(syms.iter().cloned());
                    prefixed.extend(rspace.iter().cloned());
                    *syms = prefixed;
                    for ip in lvit.input_parts.iter_mut() {
                        *ip += first;
                    }
                }
            }
            splitlocs.push(loc);
        }
        if splitlocs.is_empty() {
            continue;
        }
        // Second, we reorder splitlocs so we can output as a
        // cohort of non-branching CG subreadings; first word as leaf
        // nodes. This means that splitlocs = [[A,B],[C,D]] should
        // end up as the sequence
        // (C,0),(A,1),(C,0),(B,1),(D,0),(A,1),(D,0),(B,1)
        // (where the number is the initial indentation).
        let depth: usize = 0;
        let bottom: usize = splitlocs.len() - 1;
        let mut out: Vec<Vec<u8>> = vec![Vec::new(); splitlocs.len()];
        // In CG the *last* word is the least indented, so start from
        // the end of splitlocs, indentation being 1 tab:
        //
        print_splitlocs_r(outstream, &splitlocs, bottom, depth, 1, &mut out, s);
        // (the #if 0 block in the C++ is omitted)
    }
}

// Omorfi-specific at this time
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.fetch-and-kill-between-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.fetch-and-kill-between-fn]
pub fn fetch_and_kill_between(left: &str, right: &str, analysis: &mut String) -> String {
    let start = analysis.find(left);
    let start = match start {
        None => return String::new(),
        Some(start) => start,
    };
    // analysis.find(right, start + 1)
    let stop = analysis[start + 1..].find(right).map(|p| p + start + 1);
    let stop = match stop {
        None => return String::new(),
        Some(stop) => stop,
    };
    let retval = analysis[start + left.len()..stop].to_string();
    // analysis.erase(start, stop - start + right.size())
    analysis.replace_range(start..stop + right.len(), "");
    retval
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.fetch-and-kill-feats-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.fetch-and-kill-feats-fn]
pub fn fetch_and_kill_feats(analysis: &mut String) -> String {
    let mut retval = String::new();
    let mut tmp: String;
    tmp = fetch_and_kill_between("[ANIMACY=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Animacy={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[ASPECT=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Aspect={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[CASE=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Case={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[DEFINITE=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Definite={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[CMP=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Degree={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[GENDER=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Gender={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[MOOD=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Mood={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[NEGATIVE=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Negative={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[NUMTYPE=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Numtype={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[NUM=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Number={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[PERS=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Person={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[POSS=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Poss={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[PRONTYPE=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("PronType={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[REFLEX=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Reflex={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[TENSE=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Tense={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[VERBFORM=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("VerbForm={}|", tmp)
    } else {
        String::new()
    });
    tmp = fetch_and_kill_between("[VOICE=", "]", analysis);
    retval += &(if !tmp.is_empty() {
        format!("Voice={}|", tmp)
    } else {
        String::new()
    });
    if !retval.is_empty() {
        retval.truncate(retval.len() - 1);
    }
    retval
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.empty-to-underscore-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.empty-to-underscore-fn]
pub fn empty_to_underscore(to_test: String) -> String {
    if to_test.is_empty() {
        return "_".to_string();
    }
    to_test
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-location-vector-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-location-vector-fn]
pub fn print_location_vector(
    container: &mut PmatchContainer,
    locations: &LocationVector,
    outstream: &mut dyn Write,
    token_number: i32,
    s: &TokenizeSettings,
) {
    if s.output_format == OutputFormat::tokenize && !locations.is_empty() {
        let _ = write!(outstream, "{}", locations[0].input);
        if s.print_weights {
            let _ = write!(outstream, "\t{}", locations[0].weight);
        }
        let _ = writeln!(outstream);
        if locations[0].tag == "<Boundary=Sentence>" {
            let _ = writeln!(outstream);
        }
    } else if s.output_format == OutputFormat::space_separated && !locations.is_empty() {
        let _ = write!(outstream, "{}", locations[0].input);
        if s.print_weights {
            let _ = write!(outstream, "\t{}", locations[0].weight);
        }
        let _ = write!(outstream, " ");
        if locations[0].tag == "<Boundary=Sentence>" {
            let _ = writeln!(outstream);
        }
    } else if s.output_format == OutputFormat::cg && !locations.is_empty() {
        // Print the cg cohort header
        let _ = write!(outstream, "\"<");
        print_escaping_backslashes(&locations[0].input, outstream);
        let _ = writeln!(outstream, ">\"");
        for loc_it in locations.iter() {
            // For the most common case, eg. analysis strings that begin with
            // the original input, we try to do what cg tools expect and
            // surround the original input with double quotes. Otherwise we
            // omit the double quotes and assume the rule writer knows what
            // he's doing.
            if loc_it.output.find(&loc_it.input) == Some(0) {
                // The nice case obtains
                let _ = write!(outstream, "\t\"");
                print_escaping_backslashes(&loc_it.input, outstream);
                let _ = write!(outstream, "\"{}", &loc_it.output[loc_it.input.len()..]);
            } else {
                let _ = write!(outstream, "\t{}", loc_it.output);
            }
            if s.print_weights {
                let _ = write!(outstream, "\t{}", loc_it.weight);
            }
            let _ = writeln!(outstream);
        }
        let _ = writeln!(outstream);
    } else if (s.output_format == OutputFormat::giellacg || s.output_format == OutputFormat::visl)
        && !locations.is_empty()
    {
        print_location_vector_giellacg(container, locations, outstream, s);
    } else if s.output_format == OutputFormat::xerox {
        let mut best_weight: f32 = f32::MAX;
        for loc_it in locations.iter() {
            if best_weight > loc_it.weight {
                best_weight = loc_it.weight;
            }
        }
        let mut printed_something = false;
        for (idx, loc_it) in locations.iter().enumerate() {
            let is_last = idx + 1 == locations.len();
            if (s.beam < 0.0 || loc_it.weight <= best_weight + s.beam)
                &&
                // We don't print "plain" tokens without any analysis
                // except if they are the only one present
                (loc_it.output != loc_it.input || (is_last && !printed_something))
            {
                let _ = write!(outstream, "{}\t{}", loc_it.input, loc_it.output);
                // [#500] Match hfst-lookup's xerox layout, which always prints
                // the analysis weight as a fixed six-decimal field (its %w
                // template renders as `{:.6}`); print it here too rather than
                // gating on --print-weight and using default float formatting.
                let weight = if is_last && !printed_something {
                    best_weight
                } else {
                    loc_it.weight
                };
                let _ = write!(outstream, "\t{:.6}", weight);
                let _ = writeln!(outstream);
                printed_something = true;
            }
        }
        if locations[0].tag == "<Boundary=Sentence>" {
            let _ = writeln!(outstream);
        }
        let _ = writeln!(outstream);
    } else if s.output_format == OutputFormat::conllu && !locations.is_empty() {
        // Seeded from the first reading rather than from a default Location:
        // an unanalysed token weighs exactly INFINITE_WEIGHT, which no strict
        // `<` beats, and the default's empty input would blank the FORM column.
        let mut lowest_weight: Weight = locations[0].weight;
        let mut best_location: Location = locations[0].clone();
        for loc_it in locations.iter().skip(1) {
            if loc_it.weight < lowest_weight {
                best_location = loc_it.clone();
                lowest_weight = loc_it.weight;
            }
            //            if (loc_it->tag == "@MULTIWORD@"
            //            outstream << loc_it->input << "\t" << loc_it->output;
        }
        let _ = write!(outstream, "{}\t{}", token_number, best_location.input);
        let _ = write!(
            outstream,
            "\t{}",
            empty_to_underscore(fetch_and_kill_between(
                "[WORD_ID=",
                "]",
                &mut best_location.output
            ))
        );
        let _ = write!(
            outstream,
            "\t{}",
            empty_to_underscore(fetch_and_kill_between(
                "[UPOS=",
                "]",
                &mut best_location.output
            ))
        );
        let _ = write!(
            outstream,
            "\t{}",
            empty_to_underscore(fetch_and_kill_between(
                "[XPOS=",
                "]",
                &mut best_location.output
            ))
        );
        let _ = write!(
            outstream,
            "\t{}\t_\t_\t_",
            empty_to_underscore(fetch_and_kill_feats(&mut best_location.output)) // DEPS
        );
        let _ = write!(
            outstream,
            "\t{}",
            empty_to_underscore(best_location.output.clone())
        ); // MISC
        if s.print_weights {
            let _ = write!(outstream, "\t{}", best_location.weight);
        }
        let _ = writeln!(outstream);
    } else if s.output_format == OutputFormat::finnpos {
        let mut tags: BTreeSet<String> = BTreeSet::new();
        let mut lemmas: BTreeSet<String> = BTreeSet::new();
        for loc_it in locations.iter() {
            // Assume the last space is where the tags begin
            let tags_start_at = loc_it.output.rfind(' ');
            if let Some(tags_start_at) = tags_start_at {
                let lemma = loc_it.output[0..tags_start_at].to_string();
                if !lemma.contains(' ') {
                    // can't have spaces in lemmas
                    lemmas.insert(lemma);
                }
                let tag = loc_it.output[tags_start_at + 1..].to_string();
                if !tag.contains(' ') {
                    // or tags
                    tags.insert(tag);
                }
            }
        }
        let _ = write!(outstream, "{}\t_\t", locations[0].input);
        // the input and a blank for features
        if lemmas.is_empty() {
            let _ = write!(outstream, "_");
        } else {
            let mut accumulator = String::new();
            for it in lemmas.iter() {
                accumulator.push_str(it);
                accumulator.push(' ');
            }
            let _ = write!(outstream, "{}", &accumulator[0..accumulator.len() - 1]);
        }
        let _ = write!(outstream, "\t");
        if tags.is_empty() {
            let _ = write!(outstream, "_");
        } else {
            let mut accumulator = String::new();
            for it in tags.iter() {
                accumulator.push_str(it);
                accumulator.push(' ');
            }
            let _ = write!(outstream, "{}", &accumulator[0..accumulator.len() - 1]);
        }
        let _ = writeln!(outstream, "\t_");
        if locations[0].tag == "<Boundary=Sentence>" {
            let _ = writeln!(outstream);
        }
    }
    //    std::cerr << "from print_location_vector\n";
}

/// Print a token that was segmented but not analysed, using the marking each
/// format already reserves for unknown material rather than emitting the
/// fallback reading as if it were an analysis.
// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-unanalysed-location-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-unanalysed-location-fn]
fn print_unanalysed_location(
    container: &mut PmatchContainer,
    locations: &LocationVector,
    outstream: &mut dyn Write,
    token_number: i32,
    s: &TokenizeSettings,
) {
    match s.output_format {
        // `"<w>"\n\t"w" ?` and `w\tw+?\tinf` — the layouts print_no_output
        // already uses for an input that produced nothing at all, cohort
        // separator included.
        OutputFormat::cg | OutputFormat::xerox => {
            print_no_output(&locations[0].input, outstream, s)
        }
        // No analysis column, so the bare token line is already unambiguous;
        // the weight column, if asked for, reads `inf` as it does in xerox.
        OutputFormat::tokenize | OutputFormat::space_separated => {
            let _ = write!(outstream, "{}", locations[0].input);
            if s.print_weights {
                let _ = write!(outstream, "\tinf");
            }
            if s.output_format == OutputFormat::tokenize {
                let _ = writeln!(outstream);
            } else {
                let _ = write!(outstream, " ");
            }
        }
        // giellacg and visl route to print_location_vector_giellacg, whose
        // unknown-but-tokenised branch prints `"w" ?`; conllu and finnpos
        // render a featureless reading as underscores either way.
        OutputFormat::giellacg
        | OutputFormat::visl
        | OutputFormat::conllu
        | OutputFormat::finnpos => {
            print_location_vector(container, locations, outstream, token_number, s)
        }
    }
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.match-and-print-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.match-and-print-fn]
pub fn match_and_print(
    container: &mut PmatchContainer,
    outstream: &mut dyn Write,
    input_text: &str,
    s: &TokenizeSettings,
) {
    let locations: LocationVectorVector =
        container.locate(input_text, s.time_cutoff, INFINITE_WEIGHT);
    if locations.is_empty() && s.print_all {
        print_no_output(input_text, outstream, s);
        return;
    }
    let mut token_number: i32 = 1;
    for it in locations.iter() {
        if it.len() == 1 && it[0].output == "@_NONMATCHING_@" {
            if s.print_all {
                print_nonmatching_sequence(&it[0].input, outstream, s);
            }
            continue;
            // All nonmatching cases have been handled
        }
        let kept = keep_n_best_weight(&dedupe_locations(it, s), s);
        // The naive tokenizer's fallback covers the same span as any dictionary
        // reading of that span, so an analysed token carries the fallback
        // alongside its real analyses; report it only where it is all there is.
        //
        // Keyed on the fallback's weight alone, NOT on `is_unanalysed`. A pmatch
        // script emits its own ` ??` readings beside real analyses — lang-sma's
        // tokeniser puts `"Manne" ??` next to `manne Pron Pers Sg1 Nom` — and
        // C++ prints those in cg output. Filtering on the marker would strip
        // them, silently reshaping a stream that is not ours to reshape.
        // giellacg suppresses them separately, where upstream also does.
        let analysed: LocationVector = kept
            .iter()
            .filter(|l| l.weight < UNANALYSED_WEIGHT)
            .cloned()
            .collect();
        if analysed.is_empty() {
            print_unanalysed_location(container, &kept, outstream, token_number, s);
        } else {
            print_location_vector(container, &analysed, outstream, token_number, s);
        }
        token_number += 1;
    }
    // [#500] xerox is omitted here: unlike tokenize/finnpos (which need this
    // trailing newline as their between-input separator), print_location_vector
    // already emits one blank line after each xerox cohort, matching
    // hfst-lookup's per-record `%n` end template. Emitting it here too produced
    // a spurious second blank line.
    if s.output_format == OutputFormat::finnpos || s.output_format == OutputFormat::tokenize {
        let _ = writeln!(outstream);
    }
}

// [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.process-input-fn]
// [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.process-input-fn]
pub fn process_input(
    container: &mut PmatchContainer,
    instream: &mut crate::transducer::IStream<'_>,
    outstream: &mut dyn Write,
    s: &TokenizeSettings,
) {
    // [#367] Auto-enable multichar (longest-match) tokenization when the
    // transducer carries multichar text symbols, so tokenise matches lookup
    // without --tokenize-multichar; -m still forces it.
    let single_codepoint = if s.tokenize_multichar {
        false
    } else {
        !container.has_multichar_input_symbols()
    };
    container.set_single_codepoint_tokenization(single_codepoint);
    // C++ reads fixed-size lines (bufsize 4096) via std::istream::getline as the
    // loop condition; the IStream wrapper read_until reads up to the delimiter and
    // sets the fail flag on an immediate EOF with no bytes read.
    loop {
        let line = instream.read_until(b'\n');
        if !instream.good() {
            break;
        }
        let input_text = line;
        if !input_text.is_empty() {
            match_and_print(container, outstream, &input_text, s);
        }
    }
}

// ---------------------------------------------------------------------------
// The hfst-tokenize tool engine, lifted from tools/src/hfst-tokenize.cc: the
// naive-tokenizer construction and the input-segmentation drivers (plain
// line/blank-line input, NUL-delimited giellacg input with optional apertium
// superblanks, and VISL CG 3 input). The tool keeps only its option parsing
// and stream opening. Nothing here touches process stdin/stdout or exits:
// input is a '&mut dyn BufRead', output and verbose diagnostics are
// caller-supplied writers, and every knob arrives via 'TokenizeSettings' and
// 'TokenizeInputSettings'.
// ---------------------------------------------------------------------------

/// How the tokenize driver segments its text input — the tool-level
/// input-mode switches of hfst-tokenize (its C++ file-scope statics),
/// companion to the per-format 'TokenizeSettings'.
#[derive(Clone, Copy)]
pub struct TokenizeInputSettings {
    /// Input is apertium-style superblanks (overrides blankline_separated).
    pub superblanks: bool,
    /// Input is separated by blank lines (as opposed to single newlines).
    pub blankline_separated: bool,
    /// Keep the final newline of each chunk.
    pub keep_newlines: bool,
    /// Emit per-chunk processing diagnostics (the tool's -v).
    pub verbose: bool,
}

impl Default for TokenizeInputSettings {
    fn default() -> TokenizeInputSettings {
        TokenizeInputSettings {
            superblanks: false,
            blankline_separated: true,
            keep_newlines: false,
            verbose: false,
        }
    }
}

// [spec:hfst:def:hfst-tokenize.make-naive-tokenizer-fn]
// [spec:hfst:sem:hfst-tokenize.make-naive-tokenizer-fn]
pub fn make_naive_tokenizer<B: AlgebraBackend>(
    dictionary: &mut HfstTransducer<B>,
) -> Result<PmatchContainer> {
    let mut word_boundary: HfstTransducer<B> =
        pmatch_compiler::PmatchUtilityTransducers::make_latin1_whitespace_acceptor()?;
    let punctuation = pmatch_compiler::PmatchUtilityTransducers::make_latin1_punct_acceptor()?;
    word_boundary.disjunct(&punctuation, true)?;
    let mut others = pmatch_compiler::make_exc_list(&word_boundary)?;
    others.repeat_plus()?;
    // make the default token less likely than any dictionary token
    others.set_final_weights(UNANALYSED_WEIGHT, false)?;
    let mut word_boundary_list = pmatch_compiler::make_list(&word_boundary)?;
    // @BOUNDARY@ is pmatch's special input boundary marker
    let boundary = HfstTransducer::new_symbol("@BOUNDARY@")?;
    word_boundary_list.disjunct(&boundary, true)?;
    let mut left_context = HfstTransducer::new_symbol_pair(
        crate::hfst_symbol_defs::internal_epsilon,
        pmatch_compiler::LC_ENTRY_SYMBOL,
    )?;
    let mut right_context = HfstTransducer::new_symbol_pair(
        crate::hfst_symbol_defs::internal_epsilon,
        pmatch_compiler::RC_ENTRY_SYMBOL,
    )?;
    left_context.concatenate(&word_boundary_list, true)?;
    right_context.concatenate(&word_boundary_list, true)?;
    let left_context_exit = HfstTransducer::new_symbol_pair(
        crate::hfst_symbol_defs::internal_epsilon,
        pmatch_compiler::LC_EXIT_SYMBOL,
    )?;
    let right_context_exit = HfstTransducer::new_symbol_pair(
        crate::hfst_symbol_defs::internal_epsilon,
        pmatch_compiler::RC_EXIT_SYMBOL,
    )?;
    left_context.concatenate(&left_context_exit, true)?;
    right_context.concatenate(&right_context_exit, true)?;
    let mut dict_name = dictionary.get_name();
    if dict_name.is_empty() {
        dict_name = "unknown_pmatch_tokenized_dict".to_string();
        dictionary.set_name(&dict_name);
    }
    let dict_ins_arc =
        HfstTransducer::new_symbol(&pmatch_compiler::get_Ins_transition(&dict_name))?;
    // We now make the center of the tokenizer
    others.disjunct(&dict_ins_arc, true)?;
    // And combine it with the context conditions
    left_context.concatenate(&others, true)?;
    left_context.concatenate(&right_context, true)?;
    // Because there are context conditions we need delimiter markers
    let mut tokenizer = pmatch_compiler::add_pmatch_delimiters(&left_context)?;
    tokenizer.set_name("TOP");
    tokenizer.minimize()?;
    // Get the alphabets
    let dict_syms = dictionary.get_alphabet()?;
    let tokenizer_syms = tokenizer.get_alphabet()?;
    // What to add to the dictionary
    let tokenizer_minus_dict: Vec<crate::hfst_data_types::Symbol> =
        tokenizer_syms.difference(&dict_syms).cloned().collect();
    for it in tokenizer_minus_dict.iter() {
        dictionary.insert_to_alphabet(it.as_str())?;
    }
    let tokenizer_basic =
        ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&tokenizer)?;
    // The C++ 'hfst_basic_transducer_to_hfst_ol' takes the HfstTransducer
    // dictionary directly as its harmonizer and converts it internally; the
    // Rust port shifts that to the caller, so we first obtain the dictionary's
    // optimized-lookup backend ('hfst_transducer_to_hfst_ol' builds an owned
    // weighted ol::Transducer from the dictionary) and pass that as the
    // harmonizer. The same owned backend is used for add_rtn below.
    let dict_backend = ConversionFunctions::hfst_transducer_to_hfst_ol(dictionary)?;
    let tokenizer_ol = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
        &tokenizer_basic,
        true,                // weighted
        "",                  // no special options
        Some(&dict_backend), // harmonize with the dictionary
    )?;
    let mut retval = PmatchContainer::new_from_transducer(Box::new(tokenizer_ol))?;
    retval.add_rtn(&dict_backend, &dict_name)?;
    Ok(retval)
}

// TODO: lambda this when C++11 available everywhere
// [spec:hfst:def:hfst-tokenize.process-input-0delim-print-fn]
// [spec:hfst:sem:hfst-tokenize.process-input-0delim-print-fn]
fn process_input_0delim_print(
    container: &mut PmatchContainer,
    outstream: &mut dyn Write,
    cur: &mut Vec<u8>,
    s: &TokenizeSettings,
) {
    // 'cur' accumulates the raw input bytes (as the C++ std::string does);
    // decode to UTF-8 here so multibyte characters reach the matcher intact.
    let input_text = String::from_utf8_lossy(cur).into_owned();
    if !input_text.is_empty() {
        match_and_print(container, outstream, &input_text, s);
    }
    cur.clear();
}

// Equivalent of the C++ 'feof(inputfile)': no bytes remain on the reader.
fn is_eof(input: &mut dyn BufRead) -> bool {
    match input.fill_buf() {
        Ok(b) => b.is_empty(),
        Err(_) => true,
    }
}

// [spec:hfst:def:hfst-tokenize.trim-fn]
// [spec:hfst:sem:hfst-tokenize.trim-fn]
fn trim(s: &mut String) {
    while !s.is_empty() {
        let c = *s.as_bytes().last().expect("checked non-empty");
        if (c as char).is_ascii_whitespace() || c == 0 {
            s.pop();
        } else {
            break;
        }
    }
    while !s.is_empty() {
        let c = s.as_bytes()[0];
        if (c as char).is_ascii_whitespace() || c == 0 {
            s.remove(0);
        } else {
            break;
        }
    }
}

// [spec:hfst:def:hfst-tokenize.process-input-visl-fn]
// [spec:hfst:sem:hfst-tokenize.process-input-visl-fn]
pub fn process_input_visl(
    container: &mut PmatchContainer,
    input: &mut dyn BufRead,
    outstream: &mut dyn Write,
    s: &TokenizeSettings,
) -> i32 {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        // C: hfst_getline keeps the trailing newline; Ok(0) at EOF == len<=0.
        let n = input.read_until(b'\n', &mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        let mut line = String::from_utf8_lossy(&buf).into_owned();
        trim(&mut line);
        if !line.is_empty() {
            if line.as_bytes()[0] == b'<'
                && *line.as_bytes().last().expect("checked non-empty") == b'>'
            {
                print_nonmatching_sequence(&line, outstream, s);
            } else {
                match_and_print(container, outstream, &line, s);
            }
        } else {
            let _ = writeln!(outstream);
        }
        let _ = outstream.flush();

        // C: *buffer = 0; len = 0; — the post-loop tail then handles an empty
        // line, which trims to nothing and is a no-op. Mirrored by stopping at
        // EOF here without a separate tail block.
        if is_eof(input) {
            break;
        }
    }
    let _ = outstream.flush();

    0
}

// 'template <bool do_superblank>'
// [spec:hfst:def:hfst-tokenize.process-input-0delim-fn]
// [spec:hfst:sem:hfst-tokenize.process-input-0delim-fn]
pub fn process_input_0delim(
    container: &mut PmatchContainer,
    input: &mut dyn BufRead,
    outstream: &mut dyn Write,
    do_superblank: bool,
    s: &TokenizeSettings,
    verbose: bool,
) -> i32 {
    let mut buf: Vec<u8> = Vec::new();
    let mut in_blank = false;
    // Accumulate raw input bytes (the C++ builds a std::string byte-by-byte);
    // pushing 'ch as char' here would reinterpret each UTF-8 byte as a Latin-1
    // codepoint and corrupt multibyte characters before they reach the matcher.
    let mut cur: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        // C: hfst_getdelim keeps the trailing '\0'; Ok(0) at EOF == len<=0.
        let len = input.read_until(b'\0', &mut buf).unwrap_or(0) as isize;
        if len <= 0 {
            break;
        }
        let bytes: &[u8] = &buf;
        let mut escaped = false; // beginning of line is necessarily unescaped
        let mut i: isize = 0;
        while i < len {
            let ch = bytes[i as usize];
            if escaped {
                cur.push(ch);
                escaped = false;
                i += 1;
                continue;
            } else if do_superblank && !in_blank && ch == b'[' {
                process_input_0delim_print(container, outstream, &mut cur, s);
                cur.push(ch);
                in_blank = true;
            } else if do_superblank && in_blank && ch == b']' {
                cur.push(ch);
                if i + 1 < len && bytes[(i + 1) as usize] == b'[' {
                    // Join consecutive superblanks
                    i += 1;
                    cur.push(bytes[i as usize]);
                } else {
                    in_blank = false;
                    print_nonmatching_sequence(&String::from_utf8_lossy(&cur), outstream, s);
                    cur.clear();
                }
            } else if !in_blank && ch == b'\n' {
                cur.push(ch);
                if verbose {
                    let _ = writeln!(
                        outstream,
                        "processing: {}\\n",
                        String::from_utf8_lossy(&cur)
                    );
                }
                process_input_0delim_print(container, outstream, &mut cur, s);
            } else if ch == b'\0' {
                if verbose {
                    let _ = writeln!(
                        outstream,
                        "processing: {}\\0",
                        String::from_utf8_lossy(&cur)
                    );
                }
                process_input_0delim_print(container, outstream, &mut cur, s);
                let _ = writeln!(outstream, "<STREAMCMD:FLUSH>"); // CG format uses this instead of \0
                if outstream.flush().is_err() {
                    eprintln!("hfst-tokenize: Could not flush file");
                }
            } else {
                cur.push(ch);
            }
            escaped = ch == b'\\';
            i += 1;
        }
        if is_eof(input) {
            break;
        }
    }
    if in_blank {
        print_nonmatching_sequence(&String::from_utf8_lossy(&cur), outstream, s);
    } else {
        process_input_0delim_print(container, outstream, &mut cur, s);
    }
    0
}

// [spec:hfst:def:hfst-tokenize.maybe-erase-newline-fn]
// [spec:hfst:sem:hfst-tokenize.maybe-erase-newline-fn]
fn maybe_erase_newline(input_text: &mut String, keep_newlines: bool) {
    if !keep_newlines
        && !input_text.is_empty()
        && *input_text.as_bytes().last().expect("checked non-empty") == b'\n'
    {
        // Remove final newline
        input_text.pop();
    }
}

/// The tokenize tool's input driver: dispatch on the output format and the
/// input-mode settings and drive the matching over the whole text input,
/// writing verbose diagnostics to 'msg'.
// [spec:hfst:def:hfst-tokenize.process-input-fn]
// [spec:hfst:sem:hfst-tokenize.process-input-fn]
pub fn process_input_stream(
    container: &mut PmatchContainer,
    input: &mut dyn BufRead,
    outstream: &mut dyn Write,
    msg: &mut dyn Write,
    s: &TokenizeSettings,
    is: &TokenizeInputSettings,
) -> i32 {
    // (The C++ sets std::fixed/setprecision(10) on the stream for cg/giellacg/
    // visl; the library print functions format weights themselves, so there is
    // no stream-wide formatting flag to mirror here.)
    if s.output_format == OutputFormat::giellacg || is.superblanks {
        if is.superblanks {
            if is.verbose {
                let _ = writeln!(msg, "Processign giellacg with superblanks");
            }
            return process_input_0delim(container, input, outstream, true, s, is.verbose);
        } else {
            if is.verbose {
                let _ = writeln!(msg, "Processign giellacg without superblanks");
            }
            return process_input_0delim(container, input, outstream, false, s, is.verbose);
        }
    }
    if s.output_format == OutputFormat::visl {
        if is.verbose {
            let _ = writeln!(msg, "Processign VISL CG 3");
        }
        return process_input_visl(container, input, outstream, s);
    }
    let mut input_text = String::new();
    let mut line = String::new();
    if is.blankline_separated {
        if is.verbose {
            let _ = writeln!(msg, "Processing blankline separated input");
        }
        loop {
            line.clear();
            // C: hfst_getline keeps the trailing newline; Ok(0) at EOF.
            if input.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line.as_bytes().first() == Some(&b'\n') {
                maybe_erase_newline(&mut input_text, is.keep_newlines);
                match_and_print(container, outstream, &input_text, s);
                input_text.clear();
            } else {
                input_text.push_str(&line);
            }
        }
        if !input_text.is_empty() {
            maybe_erase_newline(&mut input_text, is.keep_newlines);
            match_and_print(container, outstream, &input_text, s);
        }
    } else {
        // newline or non-separated
        if is.verbose {
            let _ = writeln!(msg, "Processing non-separated input");
        }
        loop {
            line.clear();
            if input.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            input_text = line.clone();
            maybe_erase_newline(&mut input_text, is.keep_newlines);
            match_and_print(container, outstream, &input_text, s);
        }
    }

    0
}
