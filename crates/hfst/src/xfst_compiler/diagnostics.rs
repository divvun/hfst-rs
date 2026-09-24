//! Source-anchored diagnostics: rendering failures against the script, and
//! shaping front-end parse errors into advice worth reading.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    /// Render an error about the command currently being evaluated, anchored at
    /// its span in the script source.
    ///
    /// With no retained source — a caller driving the command methods directly
    /// rather than through `parse` — this degrades to the plain one-line
    /// message it replaced, never to a caret pointing at the wrong place.
    pub(super) fn diag_error(&self, msg: &str) {
        self.diag_error_with_notes(msg, &[]);
    }

    /// As `diag_error`, with follow-up advice under the snippet.
    pub(super) fn diag_error_with_notes(&self, msg: &str, notes: &[String]) {
        crate::diag::emit_with_notes(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Error,
            msg,
            notes,
        );
    }

    /// Report a `set`/`show` against a variable this compiler does not have,
    /// suggesting the nearest one it does. The valid set is session state, so
    /// the suggestion is drawn from it rather than from a fixed list.
    pub(super) fn diag_unknown_variable(&self, name: &str) {
        let mut notes = Vec::new();
        if let Some(near) = nearest_name(name, self.variables.keys().map(String::as_str)) {
            notes.push(format!("did you mean '{}'?", near));
        }
        notes.push(String::from(
            "'show variables' lists every variable and its value",
        ));
        self.diag_error_with_notes(&format!("no such variable: '{}'", name), &notes);
    }

    /// Report a reference to a network that was never defined, suggesting the
    /// nearest name this session has defined.
    pub(super) fn diag_unknown_definition(&self, name: &str) {
        let mut notes = Vec::new();
        if let Some(near) = nearest_name(name, self.definitions.keys().map(|k| k.as_str())) {
            notes.push(format!("did you mean '{}'?", near));
        }
        notes.push(String::from("'print defined' lists the defined networks"));
        self.diag_error_with_notes(&format!("no such defined network: '{}'", name), &notes);
    }

    /// Render a warning about the command currently being evaluated, anchored
    /// at its span in the script source.
    pub(super) fn diag_warning(&self, msg: &str) {
        crate::diag::emit(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Warning,
            msg,
        );
    }

    /// As `diag_warning`, with follow-up advice under the snippet.
    pub(super) fn diag_warning_with_notes(&self, msg: &str, notes: &[String]) {
        crate::diag::emit_with_notes(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Warning,
            msg,
            notes,
        );
    }

    pub(super) fn error_message(&self, message: &str) -> &Self {
        self.diag_error(message);
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.unknown-command-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.unknown-command-fn]
    // @brief Handle unknown command \a s.
    //  @return Whether the parser should go on, 0 signifying true.
    pub fn unknown_command(&mut self, s: &str) -> i32 {
        let d = unknown_command_diagnostic(s, self.current_span.clone());
        if self.variables["quit-on-fail"] == "ON" {
            if self.verbose {
                self.diag_error_with_notes(&d.message, &d.notes);
            }
            return 1;
        }
        self.diag_error_with_notes(&d.message, &d.notes);
        self.prompt();
        0
    }
}

// ===================== source-anchored diagnostics =========================
// The xfst surface is where a user meets this compiler: a script driver and a
// REPL. A failure here has to say where it happened and, where the cause is a
// known xfst trap, what to type instead. The front-end parser reports byte
// spans; these helpers turn them into something worth reading.

/// A failure to report against xfst source: where it points, what it says, and
/// any follow-up advice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XfstDiagnostic {
    /// Byte range in the script the caret goes under.
    pub span: std::ops::Range<usize>,
    pub message: String,
    /// Advice rendered beneath the snippet; empty when there is nothing
    /// specific to suggest.
    pub notes: Vec<String>,
}

/// Turn an nfst-xfst parse failure into diagnostics worth showing: front-end
/// messages rephrased in the user's vocabulary, regex-body failures re-anchored
/// from body-relative to script-absolute spans, and advice attached where the
/// cause is a known xfst trap.
///
/// Public so the shaping can be asserted directly — the rendering itself goes
/// to stderr, which a test cannot read.
pub fn parse_diagnostics(src: &str, e: &nfst_xfst::ParseError) -> Vec<XfstDiagnostic> {
    let mut out = Vec::new();
    for d in &e.diagnostics {
        let span = clamp_span(d.span.range.clone(), src.len());
        let text = src.get(span.clone()).unwrap_or("");
        if d.message.starts_with("xre body:") {
            out.extend(regex_body_diagnostics(src, span));
            continue;
        }
        if d.message.starts_with("expected command keyword") {
            out.push(unknown_command_diagnostic(text, span));
            continue;
        }
        if let Some(rest) = d.message.strip_prefix("unexpected character ") {
            out.push(unexpected_character_diagnostic(rest, span));
            continue;
        }
        // Everything else: keep the front-end wording but drop the debug dump
        // of the token it stopped on, which names Rust types, not xfst syntax.
        let message = match d.message.split_once(", got ") {
            Some((expected, _)) => expected.to_string(),
            None => d.message.clone(),
        };
        out.push(XfstDiagnostic {
            span,
            message,
            notes: d.notes.clone(),
        });
    }
    out
}

fn clamp_span(span: std::ops::Range<usize>, len: usize) -> std::ops::Range<usize> {
    let end = span.end.min(len);
    span.start.min(end)..end
}

/// Re-parse the regex body at `span` so its syntax errors carry spans into the
/// script rather than into the body alone.
///
/// nfst-xfst parses an embedded body as a standalone string and keeps only the
/// root node's script span, so the inner spans count from the body's first
/// byte. Re-parsing the body padded to its real offset — the prefix blanked to
/// whitespace, which the regex lexer skips — puts every inner span back in
/// script coordinates, and byte-for-byte blanking keeps line and column exact.
fn regex_body_diagnostics(src: &str, span: std::ops::Range<usize>) -> Vec<XfstDiagnostic> {
    let body_only = XfstDiagnostic {
        span: span.clone(),
        message: String::from("could not parse this regular expression"),
        notes: Vec::new(),
    };
    let Some(padded) = pad_to_offset(src, span.clone()) else {
        return vec![body_only];
    };
    match nfst_xre::parse(&padded) {
        // The body parsed in isolation but not as part of the script: nothing
        // more specific to say than where it is.
        Ok(_) => vec![body_only],
        Err(inner) => {
            let mut out: Vec<XfstDiagnostic> = inner
                .diagnostics
                .iter()
                .map(|d| XfstDiagnostic {
                    span: clamp_span(d.span.range.clone(), src.len()),
                    message: format!(
                        "in this regular expression: {}",
                        spell_token_names(&d.message)
                    ),
                    notes: d.notes.clone(),
                })
                .collect();
            if out.is_empty() {
                out.push(body_only);
            }
            out
        }
    }
}

/// `src[span]` preceded by as many bytes of whitespace as `src` has before it,
/// so a parser reading the result reports spans in `src` coordinates. Newlines
/// are kept so line numbers survive; every other byte becomes a space, which
/// preserves both the byte count and the column.
fn pad_to_offset(src: &str, span: std::ops::Range<usize>) -> Option<String> {
    let prefix = src.get(..span.start)?;
    let body = src.get(span.clone())?;
    let mut padded = String::with_capacity(span.end);
    for b in prefix.bytes() {
        padded.push(if b == b'\n' { '\n' } else { ' ' });
    }
    padded.push_str(body);
    Some(padded)
}

/// Replace the regex parser's Rust token names with the characters the user
/// actually typed: 'expected RightBracket' names a variant of an enum the user
/// has never seen, where 'expected ]' names a key on their keyboard.
fn spell_token_names(message: &str) -> String {
    const GLYPHS: &[(&str, &str)] = &[
        ("RightBracketDotted", ".]"),
        ("LeftBracketDotted", "[."),
        ("RightParenthesis", ")"),
        ("LeftParenthesis", "("),
        ("RightBracket", "]"),
        ("LeftBracket", "["),
        ("EndOfExpression", ";"),
        ("PairSeparator", ":"),
        ("CenterMarker", "_"),
        ("Comma", ","),
    ];
    let mut out = message.to_string();
    for (name, glyph) in GLYPHS {
        out = out.replace(name, &format!("'{}'", glyph));
    }
    out
}

fn unknown_command_diagnostic(text: &str, span: std::ops::Range<usize>) -> XfstDiagnostic {
    let word = text.trim();
    if word.is_empty() {
        return XfstDiagnostic {
            span,
            message: String::from("expected a command here"),
            notes: Vec::new(),
        };
    }
    let mut notes = Vec::new();
    if let Some(near) = nearest_command(word) {
        notes.push(format!("did you mean '{}'?", near));
    }
    notes.push(String::from(
        "'help' lists the commands this compiler accepts",
    ));
    XfstDiagnostic {
        span,
        message: format!("unknown command '{}'", word),
        notes,
    }
}

fn unexpected_character_diagnostic(quoted: &str, span: std::ops::Range<usize>) -> XfstDiagnostic {
    // The front-end quotes the byte it stopped on; recover it to decide whether
    // this is the quoting trap or an ordinary stray character.
    let ch = quoted.trim().trim_matches('\'').chars().next();
    let mut notes = Vec::new();
    if matches!(ch, Some('"') | Some('\'')) {
        notes.push(String::from(
            "xfst has no quoted strings: a word ends at the first space or quote",
        ));
        notes.push(String::from(
            "escape each special character with '%', e.g. 'Acme% Corp' for the value 'Acme Corp'",
        ));
    }
    XfstDiagnostic {
        span,
        message: match ch {
            Some(c) => format!("unexpected character '{}' in a word", c),
            None => String::from("unexpected character"),
        },
        notes,
    }
}

/// The xfst command keyword within one edit of `word`, if there is one.
///
/// The keyword table lives in the front-end lexer and is not exported, so the
/// candidates are probed against that lexer rather than matched against a
/// second copy of the table here — a copy would drift the first time a command
/// is added. A probe is a command exactly when the lexer's first token spans
/// the whole probe.
fn nearest_command(word: &str) -> Option<String> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz- ";
    // Long enough that no keyword is within one edit; also bounds the probing.
    if word.is_empty() || word.len() > 32 || !word.is_ascii() {
        return None;
    }
    let lower = word.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut best: Option<String> = None;
    let mut consider = |cand: String| {
        if !is_command_keyword(&cand) {
            return;
        }
        let better = match &best {
            None => true,
            Some(b) => (cand.len(), &cand) < (b.len(), b),
        };
        if better {
            best = Some(cand);
        }
    };
    for i in 0..bytes.len() {
        let mut del = lower.clone();
        del.remove(i);
        consider(del);
        if i + 1 < bytes.len() {
            let mut swap = bytes.to_vec();
            swap.swap(i, i + 1);
            if let Ok(s) = String::from_utf8(swap) {
                consider(s);
            }
        }
        for &c in ALPHABET {
            if c == bytes[i] {
                continue;
            }
            let mut sub = bytes.to_vec();
            sub[i] = c;
            if let Ok(s) = String::from_utf8(sub) {
                consider(s);
            }
        }
    }
    for i in 0..=bytes.len() {
        for &c in ALPHABET {
            let mut ins = bytes.to_vec();
            ins.insert(i, c);
            if let Ok(s) = String::from_utf8(ins) {
                consider(s);
            }
        }
    }
    best
}

/// Whether `word` is exactly one xfst command keyword, asked of the front-end
/// lexer so there is nothing to keep in sync.
fn is_command_keyword(word: &str) -> bool {
    let trimmed = word.trim();
    if trimmed.len() != word.len() || trimmed.is_empty() {
        return false;
    }
    match nfst_xfst::tokenize(word) {
        Ok(tokens) => match tokens.first() {
            Some((nfst_xfst::Token::Command(_), span)) => {
                span.start() == 0 && span.end() == word.len()
            }
            _ => false,
        },
        Err(_) => false,
    }
}

/// The closest name in `known` to `word`, when one is close enough to be worth
/// suggesting. Used for the runtime lookups — variables, defined networks —
/// whose valid names are session state rather than grammar.
fn nearest_name<'a>(word: &str, known: impl Iterator<Item = &'a str>) -> Option<String> {
    // A third of the word may differ, at least one edit and at most three: a
    // suggestion further away than that is noise.
    let budget = (word.chars().count() / 3).clamp(1, 3);
    let mut best: Option<(usize, String)> = None;
    for name in known {
        let d = edit_distance(word, name, budget);
        if d > budget {
            continue;
        }
        let better = match &best {
            None => true,
            Some((bd, bn)) => (d, name) < (*bd, bn.as_str()),
        };
        if better {
            best = Some((d, name.to_string()));
        }
    }
    best.map(|(_, name)| name)
}

/// Levenshtein distance, abandoned once it is known to exceed `budget`.
fn edit_distance(a: &str, b: &str, budget: usize) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > budget {
        return budget + 1;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        let mut row_best = cur[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            row_best = row_best.min(cur[j]);
        }
        if row_best > budget {
            return budget + 1;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
