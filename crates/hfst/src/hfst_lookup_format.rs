//! The shared engine of the lookup-family CLI tools (hfst-lookup and
//! hfst-flookup), lifted out of the two tools which carried near-verbatim
//! copies of it: the predefined xerox/cg/apertium output templates, the
//! %-template output renderer, the lookup input-line parser (spaced / text /
//! apertium formats) and the multi-transducer cascade application. The tools
//! keep only their I/O plumbing (option parsing, writers, the stdin loop) and
//! genuinely tool-specific lookup behaviour.
//!
//! Nothing here touches process stdin/stdout or exits: every printer takes a
//! `&mut dyn Write`, every knob is passed in explicitly, and fallible paths
//! return a `Result`.

use std::io::Write;

use crate::error::Result;
use crate::hfst_data_types::{HfstOneLevelPath, HfstOneLevelPaths, StringPairVector};
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use crate::hfst_symbol_defs::{StringSet, is_epsilon};

/// How the lookup tools parse each input line.
// [spec:hfst:def:hfst-lookup.lookup-input-format]
// [spec:hfst:def:hfst-flookup.lookup-input-format]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LookupInputFormat {
    Utf8TokenInput,
    SpaceSeparatedTokenInput,
    ApertiumInput,
}

/// Which predefined output template family a lookup run prints with.
// [spec:hfst:def:hfst-lookup.lookup-output-format]
// [spec:hfst:def:hfst-flookup.lookup-output-format]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LookupOutputFormat {
    XeroxOutput,
    CgOutput,
    ApertiumOutput,
}

/// The twelve %-template format strings driving a lookup run's output.
///
/// The formats for lookup cases go like so:
///  BEGIN LOOKUP LOOKUP LOOKUP... END
/// with a (begin, lookup, end) triple for the standard case of more than 0 and
/// less than infinite results, for 0 results, for 0 results on a token the
/// analyser cannot even tokenize, and for infinite results.
#[derive(Clone, Debug)]
pub struct LookupFormats {
    pub begin_setf: String,
    pub lookupf: String,
    pub end_setf: String,
    pub empty_begin_setf: String,
    pub empty_lookupf: String,
    pub empty_end_setf: String,
    pub unknown_begin_setf: String,
    pub unknown_lookupf: String,
    pub unknown_end_setf: String,
    pub infinite_begin_setf: String,
    pub infinite_lookupf: String,
    pub infinite_end_setf: String,
}

impl LookupFormats {
    /// The predefined templates for the given output format.
    pub fn for_output_format(format: LookupOutputFormat) -> LookupFormats {
        match format {
            LookupOutputFormat::XeroxOutput => LookupFormats {
                begin_setf: "".to_string(),
                lookupf: "%i\t%l\t%w%n".to_string(),
                end_setf: "%n".to_string(),
                empty_begin_setf: "".to_string(),
                empty_lookupf: "%i\t%i+?\t%w%n".to_string(),
                empty_end_setf: "%n".to_string(),
                unknown_begin_setf: "".to_string(),
                unknown_lookupf: "%i\t%i+?\t%w%n".to_string(),
                unknown_end_setf: "%n".to_string(),
                infinite_begin_setf: "".to_string(),
                infinite_lookupf: "%i\t%l\t%w%n".to_string(),
                infinite_end_setf: "%i\t[...cyclic...]%n%n".to_string(),
            },
            LookupOutputFormat::CgOutput => LookupFormats {
                begin_setf: "\"<%i>\"%n".to_string(),
                lookupf: "\t\"%b\"%a\t%w%n".to_string(),
                end_setf: "%n".to_string(),
                empty_begin_setf: "\"<%i>\"%n".to_string(),
                empty_lookupf: "\t\"%i\" ?\tInf%n".to_string(),
                empty_end_setf: "%n".to_string(),
                unknown_begin_setf: "\"<%i>\"%n".to_string(),
                unknown_lookupf: "\t\"%i\"\t ?\tInf%n".to_string(),
                unknown_end_setf: "%n".to_string(),
                infinite_begin_setf: "\"<%i>\"%n".to_string(),
                infinite_lookupf: "\t\"%b\"%a\t%w%n".to_string(),
                infinite_end_setf: "\t\"%i\"...cyclic...%n%n".to_string(),
            },
            LookupOutputFormat::ApertiumOutput => LookupFormats {
                begin_setf: "^%i".to_string(),
                lookupf: "/%l".to_string(),
                end_setf: "$%m%n".to_string(),
                empty_begin_setf: "^%i".to_string(),
                empty_lookupf: "/*%i".to_string(),
                empty_end_setf: "$%m%n".to_string(),
                unknown_begin_setf: " ".to_string(),
                unknown_lookupf: "%i%m".to_string(),
                unknown_end_setf: " ".to_string(),
                infinite_begin_setf: "^%i".to_string(),
                infinite_lookupf: "/%l".to_string(),
                infinite_end_setf: "/...$%n".to_string(),
            },
        }
    }
}

/// The knobs of the %-template renderer (the tools' xfst variables and
/// -e/-b options).
#[derive(Clone, Debug)]
pub struct LookupRenderOptions {
    /// What to print for an epsilon symbol (-e).
    pub epsilon_format: String,
    /// What to print between symbols when print_space is on.
    pub space_format: String,
    /// xfst variable print-space.
    pub print_space: bool,
    /// xfst variable show-flags.
    pub show_flags: bool,
    /// xfst variable quote-special.
    pub quote_special: bool,
    /// Where the whole lookup form lands when it contains none of the
    /// '+', ' ', '<', '[' split characters: true puts it in %b with an empty
    /// %a (hfst-lookup), false puts it in %a with an empty %b (hfst-flookup).
    pub unsplit_to_base: bool,
    /// Only results whose weight is within this distance of the best result
    /// are printed; negative means no limit (-b).
    pub beam: f32,
}

/// The tools' --statistics counters.
#[derive(Clone, Copy, Debug, Default)]
pub struct LookupStats {
    pub inputs: u64,
    pub no_analyses: u64,
    pub analysed: u64,
    pub analyses: u64,
}

impl LookupStats {
    pub const fn new() -> LookupStats {
        LookupStats {
            inputs: 0,
            no_analyses: 0,
            analysed: 0,
            analyses: 0,
        }
    }

    /// The --statistics table both tools print after the input loop.
    pub fn write_statistics(&self, out: &mut dyn Write) -> std::io::Result<()> {
        write!(
            out,
            "Strings\tFound\tMissing\tResults\n{}\t{}\t{}\t{}\n",
            self.inputs, self.analysed, self.no_analyses, self.analyses
        )?;
        write!(
            out,
            "Coverage\tAmbiguity\n{:.6}\t{:.6}\n",
            self.analysed as f32 / self.inputs as f32,
            self.analyses as f32 / self.inputs as f32
        )
    }
}

/* Replace all strings str1 in symbol with str2. */
// [spec:hfst:def:hfst-lookup.replace-all-fn]
// [spec:hfst:sem:hfst-lookup.replace-all-fn]
// [spec:hfst:def:hfst-flookup.replace-all-fn]
// [spec:hfst:sem:hfst-flookup.replace-all-fn]
fn replace_all(symbol: String, str1: &str, str2: &str) -> String {
    if str1.is_empty() {
        return symbol;
    }
    symbol.replace(str1, str2)
}

/// How a symbol is printed: epsilon as the configured epsilon format, and,
/// under quote-special, backslashes, colons and spaces backslash-escaped.
// [spec:hfst:def:hfst-lookup.get-print-format-fn]
// [spec:hfst:sem:hfst-lookup.get-print-format-fn]
// [spec:hfst:def:hfst-flookup.get-print-format-fn]
// [spec:hfst:sem:hfst-flookup.get-print-format-fn]
pub fn get_print_format(s: &str, epsilon_format: &str, quote_special: bool) -> String {
    if is_epsilon(s) {
        return epsilon_format.to_string();
    }
    if quote_special {
        return replace_all(
            replace_all(replace_all(s.to_string(), "\\", "\\\\"), ":", "\\:"),
            " ",
            "\\ ",
        );
    }
    s.to_string()
}

/// Render one %-template format string (the lookup tools' lookup_printf):
/// %i is the input form, %l the whole lookup (result) form, %b/%a the lookup
/// form split at the first of '+', ' ', '<', '[', %m the apertium markup,
/// %w the weight and %n a newline.
// [spec:hfst:def:hfst-lookup.lookup-printf-fn]
// [spec:hfst:sem:hfst-lookup.lookup-printf-fn]
// [spec:hfst:def:hfst-flookup.lookup-printf-fn]
// [spec:hfst:sem:hfst-flookup.lookup-printf-fn]
pub fn print_lookup_template(
    format: &str,
    input: Option<&HfstOneLevelPath>,
    result: Option<&HfstOneLevelPath>,
    markup: Option<&str>,
    opts: &LookupRenderOptions,
    ofile: &mut dyn Write,
) -> std::io::Result<()> {
    // Build one side (input or result) of the printout.
    let render_side = |path: &HfstOneLevelPath| -> String {
        let mut p = String::new();
        let mut first = true;
        for s in path.second.iter() {
            if !first && opts.print_space {
                p.push_str(&opts.space_format);
            }
            if is_epsilon(s) {
                p.push_str(&opts.epsilon_format);
            } else if FdOperation::is_diacritic(s) {
                if opts.show_flags {
                    p.push_str(s);
                }
            } else {
                p.push_str(s);
            }
            first = false;
        }
        p
    };

    // The lookupform string (the result side).
    let lookupform: Option<String> = result.map(render_side);
    // The inputform string.
    let inputform: String = input.map(render_side).unwrap_or_default();

    // weight
    let w: f32 = match result {
        Some(r) => r.first,
        None => f32::INFINITY,
    };

    // %i, %l, %b, %a, %m substitution sources
    let i = inputform.clone();
    let (l, b, a) = match &lookupform {
        Some(lf) => {
            let l = lf.clone();
            // find the analysis split point (first of '+', ' ', '<', '[')
            let split = lf
                .find('+')
                .or_else(|| lf.find(' '))
                .or_else(|| lf.find('<'))
                .or_else(|| lf.find('['))
                .unwrap_or(if opts.unsplit_to_base { lf.len() } else { 0 });
            let b = lf[..split].to_string();
            let a = lf[split..].to_string();
            (l, b, a)
        }
        None => (String::new(), String::new(), String::new()),
    };
    let m = markup.map(|s| s.to_string()).unwrap_or_default();

    // Walk the format string, substituting %-escapes.
    let mut res = String::new();
    let mut percent = false;
    for ch in format.chars() {
        if percent {
            match ch {
                'b' => res.push_str(&b),
                'l' => res.push_str(&l),
                'i' => res.push_str(&i),
                'a' => res.push_str(&a),
                'm' => res.push_str(&m),
                'n' => res.push('\n'),
                'w' => {
                    // On non-MSC, the C++ never prints "inf" (the test is
                    // 'if (false)'), always uses %f.
                    res.push_str(&format!("{:.6}", w));
                }
                other => {
                    // unknown format, retain % as well
                    res.push('%');
                    res.push(other);
                }
            }
            percent = false;
        } else if ch == '%' {
            percent = true;
        } else {
            res.push(ch);
        }
    }

    let printed = if !opts.quote_special {
        res
    } else {
        get_print_format(&res, &opts.epsilon_format, opts.quote_special)
    };
    ofile.write_all(printed.as_bytes())
}

/// Print one input's result set with the appropriate template triple
/// (unknown / empty / infinite / regular), limiting results with the beam and
/// bumping the statistics counters.
// [spec:hfst:def:hfst-lookup.print-lookups-fn]
// [spec:hfst:sem:hfst-lookup.print-lookups-fn]
// [spec:hfst:def:hfst-flookup.print-lookups-fn]
// [spec:hfst:sem:hfst-flookup.print-lookups-fn]
#[allow(clippy::too_many_arguments)]
pub fn print_lookups(
    kvs: &HfstOneLevelPaths,
    kv: &HfstOneLevelPath,
    markup: Option<&str>,
    outside_sigma: bool,
    inf: bool,
    formats: &LookupFormats,
    opts: &LookupRenderOptions,
    stats: &mut LookupStats,
    ofile: &mut dyn Write,
) -> std::io::Result<()> {
    let mut lowest_weight: f32 = -1.0;

    if outside_sigma {
        print_lookup_template(
            &formats.unknown_begin_setf,
            Some(kv),
            None,
            markup,
            opts,
            ofile,
        )?;
        print_lookup_template(
            &formats.unknown_lookupf,
            Some(kv),
            None,
            markup,
            opts,
            ofile,
        )?;
        print_lookup_template(
            &formats.unknown_end_setf,
            Some(kv),
            None,
            markup,
            opts,
            ofile,
        )?;
        stats.no_analyses += 1;
    } else if kvs.is_empty() {
        print_lookup_template(
            &formats.empty_begin_setf,
            Some(kv),
            None,
            markup,
            opts,
            ofile,
        )?;
        print_lookup_template(&formats.empty_lookupf, Some(kv), None, markup, opts, ofile)?;
        print_lookup_template(&formats.empty_end_setf, Some(kv), None, markup, opts, ofile)?;
        stats.no_analyses += 1;
    } else if inf {
        stats.analysed += 1;
        print_lookup_template(
            &formats.infinite_begin_setf,
            Some(kv),
            None,
            markup,
            opts,
            ofile,
        )?;
        let mut first = true;
        for lkv in kvs.iter() {
            if first {
                lowest_weight = lkv.first;
            }
            first = false;
            if opts.beam < 0.0 || lkv.first <= (lowest_weight + opts.beam) {
                print_lookup_template(
                    &formats.infinite_lookupf,
                    Some(kv),
                    Some(lkv),
                    markup,
                    opts,
                    ofile,
                )?;
                stats.analyses += 1;
            }
        }
        print_lookup_template(
            &formats.infinite_end_setf,
            Some(kv),
            None,
            markup,
            opts,
            ofile,
        )?;
    } else {
        stats.analysed += 1;
        print_lookup_template(&formats.begin_setf, Some(kv), None, markup, opts, ofile)?;
        let mut first = true;
        for lkv in kvs.iter() {
            if first {
                lowest_weight = lkv.first;
            }
            first = false;
            if opts.beam < 0.0 || lkv.first <= (lowest_weight + opts.beam) {
                print_lookup_template(&formats.lookupf, Some(kv), Some(lkv), markup, opts, ofile)?;
                stats.analyses += 1;
            }
        }
        print_lookup_template(&formats.end_setf, Some(kv), None, markup, opts, ofile)?;
    }
    Ok(())
}

/* Add a '\' in front of ':', ' ' and '\'. */
// [spec:hfst:def:hfst-lookup.escape-special-characters-fn]
// [spec:hfst:sem:hfst-lookup.escape-special-characters-fn]
// [spec:hfst:def:hfst-flookup.escape-special-characters-fn]
// [spec:hfst:sem:hfst-flookup.escape-special-characters-fn]
pub fn escape_special_characters(s: &str) -> String {
    let mut retval = String::new();
    for ch in s.chars() {
        if ch == ':' || ch == '\\' || ch == ' ' {
            retval.push('\\');
        }
        retval.push(ch);
    }
    retval
}

/// Split a string into its UTF-8 code points, one String per code point.
/// An invalid lead byte yields an IncorrectUtf8Coding error whose message
/// carries the offending remainder (the tools report it against the current
/// input line).
// [spec:hfst:def:hfst-lookup.string-to-utf8-fn]
// [spec:hfst:sem:hfst-lookup.string-to-utf8-fn]
// [spec:hfst:def:hfst-flookup.string-to-utf8-fn]
// [spec:hfst:sem:hfst-flookup.string-to-utf8-fn]
pub fn string_to_utf8(p: &str) -> Result<Vec<String>> {
    let mut path: Vec<String> = Vec::new();
    let bytes = p.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        let c = bytes[idx];
        let u8len: usize = if c <= 127 {
            1
        } else if (c & (128 + 64 + 32 + 16)) == (128 + 64 + 32 + 16) {
            4
        } else if (c & (128 + 64 + 32)) == (128 + 64 + 32) {
            3
        } else if (c & (128 + 64)) == (128 + 64) {
            2
        } else {
            return Err(crate::err!(
                IncorrectUtf8Coding,
                format!("{} not valid UTF-8\n", &p[idx..])
            ));
        };
        let end = (idx + u8len).min(bytes.len());
        path.push(String::from_utf8_lossy(&bytes[idx..end]).into_owned());
        idx += u8len;
    }
    Ok(path)
}

/// Parse one lookup input line into a lookup path according to the input
/// format. For apertium input the line is rewritten to the unescaped surface
/// form and the bracketed markup is collected into markup.
// [spec:hfst:def:hfst-lookup.line-to-lookup-path-fn]
// [spec:hfst:sem:hfst-lookup.line-to-lookup-path-fn]
// [spec:hfst:def:hfst-flookup.line-to-lookup-path-fn]
// [spec:hfst:sem:hfst-flookup.line-to-lookup-path-fn]
pub fn parse_lookup_line(
    s: &mut String,
    tok: &HfstStrings2FstTokenizer,
    markup: &mut String,
    outside_sigma: &mut bool,
    optimized_lookup: bool,
    input_format: LookupInputFormat,
) -> Result<HfstOneLevelPath> {
    let mut rv = HfstOneLevelPath {
        first: 0.0,
        second: Vec::new(),
    };
    *outside_sigma = false;
    match input_format {
        LookupInputFormat::SpaceSeparatedTokenInput => {
            let escaped = escape_special_characters(s);
            let spv: StringPairVector = tok.tokenize_string_pair(&escaped, true)?;
            for it in spv.iter() {
                rv.second.push(it.0.clone());
            }
        }
        LookupInputFormat::Utf8TokenInput => {
            if optimized_lookup {
                rv.second.push(s.clone());
            } else {
                let escaped = escape_special_characters(s);
                let spv: StringPairVector = tok.tokenize_string_pair(&escaped, false)?;
                for it in spv.iter() {
                    // todo: check if symbol is known to transducer
                    rv.second.push(it.0.clone());
                }
            }
        }
        LookupInputFormat::ApertiumInput => {
            let mut real_s = String::new();
            let mut m = String::new();
            let mut inbr = false;
            let chars: Vec<char> = s.chars().collect();
            let mut p = 0usize;
            while p < chars.len() {
                let ch = chars[p];
                if inbr {
                    if ch == ']' {
                        m.push(ch);
                        inbr = false;
                    } else if ch == '\\' && p + 1 < chars.len() && chars[p + 1] == ']' {
                        p += 1;
                        m.push(chars[p]);
                    } else {
                        m.push(ch);
                    }
                } else if ch == '[' {
                    m.push(ch);
                    inbr = true;
                } else if ch == ']' {
                    m.push(ch);
                    p += 1;
                    continue;
                } else if ch == '\\' {
                    p += 1;
                    if p < chars.len() {
                        real_s.push(chars[p]);
                    }
                } else {
                    real_s.push(ch);
                }
                p += 1;
            }
            let path = string_to_utf8(&real_s)?;
            *s = real_s;
            *markup = m;
            rv.second = path;
        }
    }
    Ok(rv)
}

/// Whether a lookup can possibly succeed: true when the transducer contains
/// the unknown or identity symbol, or when every input symbol is in the
/// transducer's seen-symbol set.
// [spec:hfst:def:hfst-lookup.is-possible-to-get-result-fn]
// [spec:hfst:sem:hfst-lookup.is-possible-to-get-result-fn]
// [spec:hfst:def:hfst-flookup.is-possible-to-get-result-fn]
// [spec:hfst:sem:hfst-flookup.is-possible-to-get-result-fn]
pub fn is_possible_to_get_result(
    s: &HfstOneLevelPath,
    symbols_seen: &StringSet,
    unknown_or_identity_seen: bool,
) -> bool {
    if unknown_or_identity_seen {
        return true;
    }
    for it in s.second.iter() {
        if !symbols_seen.contains(it) {
            return false;
        }
    }
    true
}

/// How results from the several transducers of a cascade are combined.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CascadeVariant {
    Union,
    PriorityUnion,
    Composition,
}

/// One lookup call issued by [apply_cascade]: which transducer of the cascade,
/// whether it is the last one and, when this call is a composition step fed an
/// intermediate result, the original input path.
pub struct CascadeStep<'a> {
    pub index: usize,
    pub is_last: bool,
    pub composed_from: Option<&'a HfstOneLevelPath>,
}

/// Apply a cascade of transducers to one input, combining the per-transducer
/// results by union, priority union or composition. The actual single-
/// transducer lookup is performed by the lookup_one callback (the tools pass
/// their optimized-lookup or basic-transducer lookup); verbose receives the
/// progress messages the tools print in verbose mode.
// [spec:hfst:def:hfst-lookup.lookup-cascading-fn]
// [spec:hfst:sem:hfst-lookup.lookup-cascading-fn]
// [spec:hfst:def:hfst-flookup.lookup-cascading-fn]
// [spec:hfst:sem:hfst-flookup.lookup-cascading-fn]
pub fn apply_cascade(
    s: &HfstOneLevelPath,
    n_transducers: usize,
    variant: CascadeVariant,
    print_pairs: bool,
    verbose: &mut dyn FnMut(&str),
    lookup_one: &mut dyn FnMut(
        &HfstOneLevelPath,
        &CascadeStep,
        &mut dyn Write,
    ) -> HfstOneLevelPaths,
    out: &mut dyn Write,
) -> std::io::Result<HfstOneLevelPaths> {
    let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();

    // go through all transducers in the cascade
    for i in 0..n_transducers {
        let is_last = (i + 1) == n_transducers;
        let result: HfstOneLevelPaths;

        if variant == CascadeVariant::Composition && i != 0 {
            let mut composed: HfstOneLevelPaths = HfstOneLevelPaths::new();
            // use previous value of 'results' as input to composition
            let prev: Vec<HfstOneLevelPath> = results.iter().cloned().collect();
            for it in prev.iter() {
                // if last transducer in cascade, print results if
                // --print-pairs is requested
                let one_result = lookup_one(
                    it,
                    &CascadeStep {
                        index: i,
                        is_last,
                        composed_from: Some(s),
                    },
                    out,
                );
                for inner in one_result.iter() {
                    // add the weights
                    composed.insert(HfstOneLevelPath {
                        first: inner.first + it.first,
                        second: inner.second.clone(),
                    });
                }
            }
            // zero 'results'
            results = HfstOneLevelPaths::new();

            // cascading composition done
            if is_last && print_pairs {
                if composed.is_empty() {
                    let mut input = String::new();
                    for it in s.second.iter() {
                        input += it;
                    }
                    write!(out, "{}\t{}+?\tinf\n\n", input, input)?;
                } else {
                    out.write_all(b"\n")?;
                }
                out.flush()?;
            }
            result = composed;
        } else {
            result = lookup_one(
                s,
                &CascadeStep {
                    index: i,
                    is_last,
                    composed_from: None,
                },
                out,
            );
        }

        // (C++ tests 'if (infinity)' on the pointer — always true here.)
        verbose(&format!("Inf results @ level {}\n", i));

        for it in result.iter() {
            results.insert(it.clone());
        }

        if variant == CascadeVariant::PriorityUnion && !results.is_empty() {
            verbose(&format!(
                "results found @ level {}, skipping rest of transducers (--cascade=priority-union)\n",
                i
            ));
            break;
        }
    }
    Ok(results)
}
