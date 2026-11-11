//! `hfst grep`: building a matcher from a pattern, and matching lines of text
//! against it as the tool does.
//!
//! Matching composes each line, as an identity transducer, with the matcher
//! and projects the result.

// [spec:hfst:def:hfst-grep.optimise-matcher-fn]
// [spec:hfst:sem:hfst-grep.optimise-matcher-fn]
// The C++ converts the matcher to optimized lookup only behind
// HFST_OPTIMISED_LOOKUP_CAN_IDENTITY, which it never defines, so it matches
// by composition, as here.

use std::io::{BufRead, Write};

use crate::hfst_data_types::HfstTwoLevelPaths;
use crate::hfst_symbol_defs::is_epsilon;
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::HfstTransducer;
use crate::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

type Tropical = HfstTransducer<StdVectorFst>;

/// How a pattern is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// A Xerox regular expression.
    Xerox,
    /// A string of UTF-8 characters.
    FixedStrings,
}

/// What `hfst grep`'s options decide about matching and printing.
#[derive(Clone, Copy, Debug)]
pub struct GrepOptions {
    pub dialect: Dialect,
    /// Mark matches with colour codes rather than `{{{ }}}`.
    pub colour: bool,
    /// Match only between spaces.
    pub match_word: bool,
    pub match_full_line: bool,
    /// Print the lines that do not match.
    pub invert: bool,
    /// Stop a file after this many matching lines; 0 has no limit.
    pub max_count: u64,
    pub print_filenames: bool,
    /// End a printed file name with NUL rather than `: `.
    pub filename_null: bool,
    pub print_line_numbers: bool,
    /// Print no lines, only which files matched.
    pub only_filenames: bool,
    /// Flush the output after each line.
    pub flush_lines: bool,
}

/// A line matched, with the text `hfst grep` prints for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineMatch {
    /// The line with its matches marked.
    Matched(String),
    /// The line as it is.
    Unmatched(String),
}

/// Why a pattern did not compile. Each `Display`s as the message the tool
/// prints for it.
#[derive(Clone, Debug)]
pub enum PatternError {
    /// An empty Xerox expression, which XRE does not have.
    Empty,
    /// The Xerox expression does not compile; its diagnostics say why.
    NotCompiled(String),
    Operation(crate::error::Error),
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternError::Empty => write!(
                f,
                "empty pattern: XRE parsing failed (an empty Xerox regular expression is not valid)"
            ),
            PatternError::NotCompiled(pattern) => write!(f, "XRE parsing failed: {}", pattern),
            PatternError::Operation(e) => write!(f, "{}", e),
        }
    }
}

impl From<crate::error::Error> for PatternError {
    fn from(e: crate::error::Error) -> Self {
        PatternError::Operation(e)
    }
}

/// A pattern compiled for matching.
pub struct Matcher {
    matcher: Tropical,
}

/// A progress message for a tool to print when verbose.
pub type Progress<'p> = &'p mut dyn FnMut(&str);

fn marks(colour: bool) -> (&'static str, &'static str) {
    if colour {
        ("[31m", "[00m")
    } else {
        ("{{{", "}}}")
    }
}

impl Matcher {
    // [spec:hfst:def:hfst-grep.read-matcher-fn]
    // [spec:hfst:sem:hfst-grep.read-matcher-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// The input projection of `pattern`, read in `dialect`, a Xerox
    /// expression compiled with `xre`.
    pub fn compile(
        pattern: &str,
        dialect: Dialect,
        xre: &mut XreCompiler<StdVectorFst>,
        progress: Progress<'_>,
    ) -> Result<Matcher, PatternError> {
        let mut matcher = Tropical::new();
        match dialect {
            Dialect::Xerox => {
                progress(&format!(
                    "parsing {} as Xerox style regular expression...\n",
                    pattern
                ));
                // XRE has no empty expression, so nothing sound can be built
                // from an empty pattern.
                let Some(mut t) = xre.compile(pattern) else {
                    if pattern.is_empty() {
                        return Err(PatternError::Empty);
                    }
                    return Err(PatternError::NotCompiled(pattern.to_string()));
                };
                t.input_project()?;
                matcher.disjunct(&t, true)?;
            }
            Dialect::FixedStrings => {
                progress(&format!(
                    "parsing {} as fixed string of UTF-8 symbols...\n",
                    pattern
                ));
                let t = Tropical::new_tokenized_pair(pattern, pattern, &HfstTokenizer::new())?;
                matcher.disjunct(&t, true)?;
            }
        }
        progress("minimizing...\n");
        matcher.minimize()?;
        Ok(Matcher { matcher })
    }

    pub fn transducer(&self) -> &Tropical {
        &self.matcher
    }

    // [spec:hfst:def:hfst-grep.extend-matcher-with-options-fn]
    // [spec:hfst:sem:hfst-grep.extend-matcher-with-options-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Mark the matcher's matches, bound it to words, and let it match
    /// anywhere in a line repeatedly, as `options` say.
    pub fn extend(
        &mut self,
        options: &GrepOptions,
        progress: Progress<'_>,
    ) -> crate::error::Result<()> {
        if options.colour {
            progress("Adding color codes to match boundaries...\n");
        } else {
            progress("Adding brackets to match boundaries...\n");
        }
        let (start, end) = marks(options.colour);
        self.surround(
            Tropical::new_symbol_pair("@_EPSILON_SYMBOL_@", start)?,
            Tropical::new_symbol_pair("@_EPSILON_SYMBOL_@", end)?,
        )?;
        if options.match_word {
            progress("Delimiting matcher to word boundaries (currently space)...\n");
            self.surround(Tropical::new_symbol(" ")?, Tropical::new_symbol(" ")?)?;
        }
        if !options.match_full_line {
            progress("Extending matcher for repetitions and rest...\n");
            let mut any = Tropical::new_symbol("@_IDENTITY_SYMBOL_@")?;
            any.repeat_star()?;
            self.surround(any.clone(), any)?;
            self.matcher.repeat_plus()?;
        }
        progress("Minimising extended matcher...\n");
        self.matcher.minimize()?;
        Ok(())
    }

    fn surround(&mut self, mut before: Tropical, after: Tropical) -> crate::error::Result<()> {
        before.concatenate(&self.matcher, true)?;
        before.concatenate(&after, true)?;
        self.matcher = before;
        Ok(())
    }

    // [spec:hfst:def:hfst-grep.string-to-utf8-fn]
    // [spec:hfst:sem:hfst-grep.string-to-utf8-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Whether the matcher matches `line`, split into its UTF-8 characters,
    /// with the text `hfst grep` prints for it: the line with its matches
    /// marked, or the line as it is.
    pub fn match_line(&self, line: &str) -> crate::error::Result<LineMatch> {
        let line_t = Tropical::new_tokenized_pair(line, line, &HfstTokenizer::new())?;
        let mut results = HfstTransducer::new_copy(&line_t)?;
        results.compose(&self.matcher, true)?;
        results.output_project()?;
        if results.compare_default(&Tropical::new())? {
            return Ok(LineMatch::Unmatched(first_path_text(&line_t)?));
        }
        Ok(LineMatch::Matched(first_path_text(&results)?))
    }

    // [spec:hfst:def:hfst-grep.match-lines-fn]
    // [spec:hfst:sem:hfst-grep.match-lines-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Match each line of `input`, the file `file_name`, and print the lines
    /// `hfst grep` prints for them to `out`. True when the file counts as
    /// matching: some line matched, or with `invert`, none did.
    pub fn match_lines(
        &self,
        input: &mut dyn BufRead,
        file_name: &str,
        options: &GrepOptions,
        out: &mut dyn Write,
        progress: Progress<'_>,
    ) -> crate::error::Result<bool> {
        progress(&format!("matching against {}...\n", file_name));
        self.select_lines(
            input,
            options,
            &mut |line_number, printed| {
                // As in the C++ tool, a write that fails does not stop
                // matching.
                if let Some(text) = printed
                    && !options.only_filenames
                {
                    let _ = write_line(out, options, file_name, line_number, text);
                }
                if options.flush_lines {
                    let _ = out.flush();
                }
            },
            progress,
        )
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// Each line of `input` `hfst grep` tests, with its number from 1, to
    /// `each`, and the text the tool prints for it: the lines that match,
    /// with their matches marked, or with `invert` the lines that do not.
    /// Empty lines are passed over, and after `max_count` matching lines the
    /// rest are too. True when some line matched, or with `invert`, none did.
    pub fn select_lines(
        &self,
        input: &mut dyn BufRead,
        options: &GrepOptions,
        each: &mut dyn FnMut(u64, Option<&str>),
        progress: Progress<'_>,
    ) -> crate::error::Result<bool> {
        let mut matches: u64 = 0;
        let mut line_number: u64 = 0;
        let mut raw: Vec<u8> = Vec::new();
        loop {
            raw.clear();
            if input.read_until(b'\n', &mut raw).unwrap_or(0) == 0 {
                break;
            }
            line_number += 1;
            let mut line = String::from_utf8_lossy(&raw).into_owned();
            if let Some(newline) = line.find('\n') {
                line.truncate(newline);
            }
            progress(&format!("matching {}...\n", line));
            if line.is_empty() {
                continue;
            }
            progress("composing...\n");
            let printed = match self.match_line(&line)? {
                LineMatch::Unmatched(text) => {
                    progress("no matches\n");
                    options.invert.then_some(text)
                }
                LineMatch::Matched(marked) => {
                    progress("matches\n");
                    matches += 1;
                    (!options.invert).then_some(marked)
                }
            };
            each(line_number, printed.as_deref());
            if options.max_count > 0 && matches >= options.max_count {
                break;
            }
        }
        Ok((matches > 0) != options.invert)
    }
}

/// The input side of the first path of `t`, epsilons left out.
fn first_path_text(t: &Tropical) -> crate::error::Result<String> {
    let mut paths = HfstTwoLevelPaths::new();
    t.extract_paths(&mut paths, 1, -1)?;
    Ok(paths
        .iter()
        .next()
        .map(|path| {
            path.second
                .iter()
                .filter(|pair| !is_epsilon(&pair.0))
                .map(|pair| pair.0.as_str())
                .collect()
        })
        .unwrap_or_default())
}

// [spec:hfst:def:hfst-grep.print-match-transducer-fn]
// [spec:hfst:sem:hfst-grep.print-match-transducer-fn]
// [spec:hfst:def:hfst-grep.print-match-line-fn]
// [spec:hfst:sem:hfst-grep.print-match-line-fn]
fn write_line(
    out: &mut dyn Write,
    options: &GrepOptions,
    file_name: &str,
    line_number: u64,
    text: &str,
) -> std::io::Result<()> {
    if options.print_filenames {
        out.write_all(file_name.as_bytes())?;
        out.write_all(if options.filename_null { b"\0" } else { b": " })?;
    }
    if options.print_line_numbers {
        write!(out, "{}: ", line_number)?;
    }
    out.write_all(text.as_bytes())?;
    out.write_all(b"\n")
}
