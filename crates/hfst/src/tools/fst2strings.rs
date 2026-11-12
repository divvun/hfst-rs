//! `hfst fst2strings`: the paths of a transducer its options select, as data
//! and as the lines the tool prints.

use std::io::Write;

use crate::backend::{AlgebraBackend, LookupBackend};
use crate::hfst_data_types::{HfstTwoLevelPath, HfstTwoLevelPaths, StringPair};
use crate::hfst_extract_strings::{ExtractStringsCb, RetVal};
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::is_epsilon;
use crate::hfst_transducer::HfstTransducer;

/// Which paths `hfst fst2strings` finds.
#[derive(Clone, Debug)]
pub struct PathQuery {
    /// At most this many paths; 0 or less for all: `--max-strings`.
    pub max_strings: i32,
    /// Only this many of the lightest paths; 0 or less for all: `--nbest`.
    pub nbest: i32,
    /// This many random paths instead; 0 or less for none: `--random`.
    pub random: i32,
    /// Follow a cycle at most this many times; negative for no limit:
    /// `--cycles`.
    pub cycles: i32,
    /// Only paths within this much of the lightest; negative for all:
    /// `--beam`.
    pub beam: f32,
    /// Drop the paths the flag diacritics forbid: `--xfst obey-flags`.
    pub obey_flags: bool,
    /// Keep the flag diacritics in the paths: `--xfst print-flags`.
    pub show_flags: bool,
    /// Only paths whose input is at most this many bytes; 0 for no limit.
    pub max_in_length: u32,
    pub max_out_length: u32,
    /// Only paths whose input starts with this.
    pub in_prefix: String,
    pub out_prefix: String,
    /// Only paths whose input does not contain this, unless it is empty.
    pub in_exclude: String,
    pub out_exclude: String,
}

impl Default for PathQuery {
    fn default() -> Self {
        PathQuery {
            max_strings: 0,
            nbest: -1,
            random: -1,
            cycles: -1,
            beam: -1.0,
            obey_flags: false,
            show_flags: false,
            max_in_length: 0,
            max_out_length: 0,
            in_prefix: String::new(),
            out_prefix: String::new(),
            in_exclude: String::new(),
            out_exclude: String::new(),
        }
    }
}

/// Why paths could not be found.
#[derive(Debug)]
pub enum Fst2StringsError {
    /// The transducer is cyclic and no option bounds the search.
    Cyclic,
    /// The option, `--beam`, `--nbest` or `--random`, which optimized
    /// lookup does not do.
    NotForOptimizedLookup(&'static str),
    /// Pruning to the best path did not leave one path.
    NotOneBestPath,
    Operation(crate::error::Error),
}

impl std::fmt::Display for Fst2StringsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fst2StringsError::Cyclic => write!(
                f,
                "Transducer is cyclic. Use one or more of these options: -n, -N, -r, -l, -L, -c"
            ),
            Fst2StringsError::NotForOptimizedLookup(option) => {
                write!(
                    f,
                    "option --{} not implemented for optimized lookup format",
                    option
                )
            }
            Fst2StringsError::NotOneBestPath => write!(f, "n_best(1) produced more than one path"),
            Fst2StringsError::Operation(e) => write!(f, "{}", e),
        }
    }
}

impl From<crate::error::Error> for Fst2StringsError {
    fn from(e: crate::error::Error) -> Self {
        Fst2StringsError::Operation(e)
    }
}

/// What the search tells its caller as it goes.
pub enum Fst2StringsProgress {
    FindingBestWeight,
    Pruning(i32),
    Finding { max_strings: i32, random: i32 },
    Found { count: i32, random: bool },
}

/// The filter the tool's callback applies to each path the search reaches,
/// and the count of paths it has given.
struct Selector<'q, 'e> {
    query: &'q PathQuery,
    /// The weight of the lightest path, with `--beam`.
    best_weight: Option<f32>,
    max: i32,
    count: i32,
    each: &'e mut dyn FnMut(&HfstTwoLevelPath),
    strings: PathStrings,
}

impl<'q, 'e> Selector<'q, 'e> {
    fn new(
        query: &'q PathQuery,
        best_weight: Option<f32>,
        max: i32,
        each: &'e mut dyn FnMut(&HfstTwoLevelPath),
    ) -> Self {
        Selector {
            query,
            best_weight,
            max,
            count: 0,
            each,
            strings: PathStrings::new(),
        }
    }

    /// Judge a final path that no walk led to, such as a random one.
    fn judge_alone(&mut self, path: &HfstTwoLevelPath) {
        self.strings.clear();
        self.operator_call(path, true);
    }
}

impl ExtractStringsCb for Selector<'_, '_> {
    // [spec:hfst:def:hfst-fst2strings.callback.operator-fn+1]
    // [spec:hfst:sem:hfst-fst2strings.callback.operator-fn+1]
    fn operator_call(&mut self, path: &HfstTwoLevelPath, is_final: bool) -> RetVal {
        let query = self.query;
        let whole = self.strings.follow(&path.second, query);
        let istring = self.strings.istring.as_str();
        let ostring = self.strings.ostring.as_str();
        let weight = path.first;
        let prefix = |string: &str, prefix: &str| -> Option<RetVal> {
            if prefix.is_empty() {
                None
            } else if string.len() < prefix.len() {
                Some(RetVal::new(true, true))
            } else if string.as_bytes()[..prefix.len()] != *prefix.as_bytes() {
                Some(RetVal::new(true, false))
            } else {
                None
            }
        };
        if (query.max_in_length > 0 && whole.in_len as u32 > query.max_in_length)
            || (query.max_out_length > 0 && whole.out_len as u32 > query.max_out_length)
        {
            return RetVal::new(true, false);
        }
        if let Some(verdict) = prefix(istring, &query.in_prefix) {
            return verdict;
        }
        if let Some(verdict) = prefix(ostring, &query.out_prefix) {
            return verdict;
        }
        if whole.in_excluded || whole.out_excluded {
            return RetVal::new(true, false);
        }
        // As in the C++ tool, a lightest weight below 0 lifts the beam.
        if let Some(best) = self.best_weight
            && best >= 0.0
            && weight > best + query.beam
        {
            return RetVal::new(true, false);
        }
        if is_final {
            (self.each)(path);
            self.count += 1;
        }
        RetVal::new((self.max < 1) || (self.count < self.max), true)
    }
}

/// The input and output strings of the path the selector last judged, and
/// for each of its prefixes the strings' lengths and whether each holds its
/// exclusion yet. A walk's next path extends one it gave before by one pair,
/// so following it costs that pair, where rebuilding the strings would cost
/// the whole path at every state.
struct PathStrings {
    istring: String,
    ostring: String,
    /// `prefixes[k]` describes the first `k` pairs of the path.
    prefixes: Vec<Prefix>,
}

#[derive(Clone, Copy, Default)]
struct Prefix {
    in_len: usize,
    out_len: usize,
    in_excluded: bool,
    out_excluded: bool,
}

impl PathStrings {
    fn new() -> Self {
        PathStrings {
            istring: String::new(),
            ostring: String::new(),
            prefixes: vec![Prefix::default()],
        }
    }

    /// Bring the strings to `pairs`, and describe the whole of them. As many
    /// of their first pairs as are held, up to all but the last, are taken to
    /// be the pairs held, as a walk's order promises.
    fn follow(&mut self, pairs: &[StringPair], query: &PathQuery) -> Prefix {
        let held = (self.prefixes.len() - 1).min(pairs.len().saturating_sub(1));
        self.prefixes.truncate(held + 1);
        let mut last = self.prefixes[held];
        self.istring.truncate(last.in_len);
        self.ostring.truncate(last.out_len);
        for (input, output) in &pairs[held..] {
            // Epsilons render as nothing, so the length, prefix and exclusion
            // filters leave them out too [upstream hfst/hfst#587].
            if !is_epsilon(input) {
                self.istring.push_str(input);
            }
            if !is_epsilon(output) {
                self.ostring.push_str(output);
            }
            last = Prefix {
                in_len: self.istring.len(),
                out_len: self.ostring.len(),
                in_excluded: last.in_excluded
                    || occurs_from(&self.istring, last.in_len, &query.in_exclude),
                out_excluded: last.out_excluded
                    || occurs_from(&self.ostring, last.out_len, &query.out_exclude),
            };
            self.prefixes.push(last);
        }
        last
    }

    /// Hold no path, before one that no walk led to.
    fn clear(&mut self) {
        self.prefixes.truncate(1);
    }
}

/// Whether `needle`, unless it is empty, occurs in `haystack` other than
/// wholly inside its first `held` bytes: the occurrences that appending past
/// `held` made. A UTF-8 needle matches bytes only at a character boundary, so
/// this agrees with `str::contains`.
fn occurs_from(haystack: &str, held: usize, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let start = held.saturating_sub(needle.len() - 1);
    haystack.as_bytes()[start..]
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

fn unbounded(query: &PathQuery) -> bool {
    query.random <= 0
        && query.max_strings <= 0
        && query.max_in_length == 0
        && query.max_out_length == 0
        && query.cycles < 0
}

// [spec:hfst:def:hfst-fst2strings.process-stream-fn]
// [spec:hfst:sem:hfst-fst2strings.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
/// The paths of `t` that `query` selects, in the order the tool prints
/// them, each given to `each`. With `nbest`, a copy of `t` is pruned to that
/// many paths first. Returns how many paths `each` heard.
pub fn extract_paths<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
    query: &PathQuery,
    each: &mut dyn FnMut(&HfstTwoLevelPath),
    progress: &mut dyn FnMut(Fst2StringsProgress),
) -> Result<i32, Fst2StringsError> {
    let mut best_weight = None;
    if query.beam >= 0.0 {
        progress(Fst2StringsProgress::FindingBestWeight);
        let mut tc = HfstTransducer::new_copy(t)?;
        tc.n_best(1)?;
        let mut best_paths = HfstTwoLevelPaths::new();
        tc.extract_paths(&mut best_paths, -1, -1)?;
        if best_paths.len() != 1 {
            return Err(Fst2StringsError::NotOneBestPath);
        }
        best_weight = best_paths.iter().next().map(|p| p.first);
    }
    let pruned;
    let t = if query.nbest > 0 {
        progress(Fst2StringsProgress::Pruning(query.nbest));
        let mut copy = HfstTransducer::new_copy(t)?;
        copy.n_best(query.nbest as u32)?;
        pruned = copy;
        &pruned
    } else if unbounded(query) && t.is_cyclic()? {
        return Err(Fst2StringsError::Cyclic);
    } else {
        t
    };
    progress(Fst2StringsProgress::Finding {
        max_strings: query.max_strings,
        random: query.random,
    });
    if query.random <= 0 {
        let mut selector = Selector::new(query, best_weight, query.max_strings, each);
        if query.obey_flags {
            t.extract_paths_fd_cb(&mut selector, query.cycles, !query.show_flags)?;
        } else {
            t.extract_paths_cb(&mut selector, query.cycles)?;
        }
        progress(Fst2StringsProgress::Found {
            count: selector.count,
            random: false,
        });
        Ok(selector.count)
    } else {
        let mut results = HfstTwoLevelPaths::new();
        if query.obey_flags {
            t.extract_random_paths_fd(&mut results, query.random, !query.show_flags)?;
        } else {
            t.extract_random_paths(&mut results, query.random)?;
        }
        let mut selector = Selector::new(query, best_weight, query.random, each);
        for it in results.iter() {
            selector.judge_alone(it);
        }
        progress(Fst2StringsProgress::Found {
            count: selector.count,
            random: true,
        });
        Ok(selector.count)
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// [`extract_paths`] for an optimized-lookup transducer, which has no
/// `--beam`, `--nbest`, or `--random` without obeying flags.
pub fn extract_paths_ol<B: LookupBackend>(
    t: &HfstTransducer<B>,
    query: &PathQuery,
    each: &mut dyn FnMut(&HfstTwoLevelPath),
    progress: &mut dyn FnMut(Fst2StringsProgress),
) -> Result<i32, Fst2StringsError> {
    if query.beam >= 0.0 {
        progress(Fst2StringsProgress::FindingBestWeight);
        return Err(Fst2StringsError::NotForOptimizedLookup("beam"));
    }
    if query.nbest > 0 {
        progress(Fst2StringsProgress::Pruning(query.nbest));
        return Err(Fst2StringsError::NotForOptimizedLookup("nbest"));
    } else if unbounded(query) && t.is_cyclic()? {
        return Err(Fst2StringsError::Cyclic);
    }
    progress(Fst2StringsProgress::Finding {
        max_strings: query.max_strings,
        random: query.random,
    });
    let max = if query.random <= 0 {
        query.max_strings
    } else {
        query.random
    };
    let mut selector = Selector::new(query, None, max, each);
    if query.random <= 0 {
        if query.obey_flags {
            t.extract_paths_fd_cb(&mut selector, query.cycles, !query.show_flags)?;
        } else {
            t.extract_paths_cb(&mut selector, query.cycles)?;
        }
    } else {
        if !query.obey_flags {
            return Err(Fst2StringsError::NotForOptimizedLookup("random"));
        }
        let mut results = HfstTwoLevelPaths::new();
        t.extract_random_paths_fd(&mut results, query.random, !query.show_flags)?;
        for it in results.iter() {
            selector.judge_alone(it);
        }
    }
    progress(Fst2StringsProgress::Found {
        count: selector.count,
        random: query.random > 0,
    });
    Ok(selector.count)
}

/// How `hfst fst2strings` prints a path.
#[derive(Clone, Debug, Default)]
pub struct PathFormat {
    /// Print the weight after a tab: `--print-weights`.
    pub print_weights: bool,
    /// Print `in:out` pairs: `--xfst print-pairs`.
    pub print_pairs: bool,
    /// Print a space between symbols: `--xfst print-space`.
    pub print_spaces: bool,
    /// Print spaces, colons and tabs as `@_SPACE_@`, `@_COLON_@` and
    /// `@_TAB_@`: `--xfst quote-special`.
    pub quote_special: bool,
    /// What to print for an epsilon: `--epsilon-format`.
    pub epsilon_format: String,
    /// Print the flag diacritics: `--xfst print-flags`.
    pub show_flags: bool,
}

// [spec:hfst:def:hfst-fst2strings.replace-all-fn]
// [spec:hfst:sem:hfst-fst2strings.replace-all-fn]
fn replace_all(symbol: String, str1: &str, str2: &str) -> String {
    let mut symbol = symbol;
    let mut pos = symbol.find(str1);
    while let Some(p) = pos {
        symbol.replace_range(p..p + str1.len(), "");
        symbol.insert_str(p, str2);
        pos = symbol[p + str2.len()..]
            .find(str1)
            .map(|rel| rel + p + str2.len());
    }
    symbol
}

// [spec:hfst:def:hfst-fst2strings.get-print-format-fn]
// [spec:hfst:sem:hfst-fst2strings.get-print-format-fn]
fn print_format(format: &PathFormat, s: &str) -> String {
    if is_epsilon(s) {
        return format.epsilon_format.clone();
    }
    if !format.quote_special {
        return s.to_string();
    }
    replace_all(
        replace_all(
            replace_all(s.to_string(), " ", "@_SPACE_@"),
            ":",
            "@_COLON_@",
        ),
        "\t",
        "@_TAB_@",
    )
}

// [spec:hfst:req:embed.tools-in-library]
/// The line `hfst fst2strings` prints for `path`, newline included.
pub fn path_line(path: &HfstTwoLevelPath, format: &PathFormat) -> String {
    let filtered = |s: &str| format.show_flags || !FdOperation::is_diacritic(s);
    let mut line = String::new();
    if format.print_pairs {
        let mut first_pair = true;
        for it in path.second.iter() {
            if filtered(&it.0) {
                if format.print_spaces && !first_pair {
                    line.push(' ');
                }
                line.push_str(&print_format(format, &it.0));
                first_pair = false;
            }
            if it.0 != it.1 && filtered(&it.1) {
                line.push(':');
                line.push_str(&print_format(format, &it.1));
            }
        }
    } else {
        let mut is_automaton = true;
        let mut first_symbol = true;
        for it in path.second.iter() {
            if filtered(&it.0) {
                if format.print_spaces && !first_symbol {
                    line.push(' ');
                }
                if it.0 != it.1 {
                    is_automaton = false;
                }
                line.push_str(&print_format(format, &it.0));
            }
            first_symbol = false;
        }
        if format.print_spaces {
            line.push(' ');
        }
        if !is_automaton {
            line.push(':');
            for it in path.second.iter() {
                if filtered(&it.1) {
                    if format.print_spaces {
                        line.push(' ');
                    }
                    line.push_str(&print_format(format, &it.1));
                }
            }
        }
    }
    if format.print_weights {
        line.push_str(&format!("\t{}", path.first));
    }
    line.push('\n');
    line
}

/// Write [`path_line`] to `out` and flush it, as the tool does per path.
pub fn write_path(out: &mut dyn Write, path: &HfstTwoLevelPath, format: &PathFormat) {
    let _ = out.write_all(path_line(path, format).as_bytes());
    let _ = out.flush();
}
