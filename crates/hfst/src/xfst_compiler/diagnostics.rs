//! Source-anchored diagnostics: rendering failures against the script, and
//! shaping front-end parse errors into advice worth reading.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    /// Everything running commands has reported since the diagnostics were
    /// last taken, oldest first, failures and warnings alike. What the
    /// embedded regex and lexc compilers report joins it after each command
    /// `parse` runs.
    // [spec:hfst:req:javascript.diagnostics-as-data]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        self.diagnostics.kept()
    }

    /// Hand over the diagnostics reported so far, the embedded compilers'
    /// included, leaving none behind.
    // [spec:hfst:req:javascript.diagnostics-as-data]
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        self.absorb_nested_diagnostics();
        self.diagnostics.take()
    }

    /// Hand each later diagnostic to `sink` the moment it is produced. They
    /// are not kept, so [`Self::take_diagnostics`] returns none of them. The
    /// embedded regex and lexc compilers report to it too, as they produce
    /// their diagnostics.
    // [spec:hfst:req:javascript.diagnostics-as-data]
    pub fn set_diagnostic_sink(&mut self, sink: DiagnosticSink) {
        self.xre.set_diagnostic_sink(sink.clone());
        self.lexc.set_diagnostic_sink(sink.clone());
        self.diagnostics.set_sink(sink);
    }

    /// Move what the embedded regex and lexc compilers reported into this
    /// compiler's diagnostics.
    pub(super) fn absorb_nested_diagnostics(&mut self) {
        let from_xre = self.xre.take_diagnostics();
        self.diagnostics.extend(from_xre);
        let from_lexc = self.lexc.take_diagnostics();
        self.diagnostics.extend(from_lexc);
    }

    /// A diagnostic about the command currently being evaluated, anchored at
    /// its span in the script source.
    ///
    /// With no retained source (a caller driving the command methods directly
    /// rather than through `parse`) it renders as the plain message, never as
    /// a caret pointing at the wrong place.
    pub(super) fn command_diagnostic(
        &self,
        severity: Severity,
        msg: &str,
        notes: &[String],
    ) -> Diagnostic {
        let d = Diagnostic::new(severity, msg)
            .label(self.current_span.clone(), msg)
            .in_source(&self.source_name, self.source.clone());
        notes.iter().fold(d, |d, n| d.note(n))
    }

    /// Record an error about the command currently being evaluated.
    pub(super) fn diag_error(&mut self, msg: &str) {
        self.diag_error_with_notes(msg, &[]);
    }

    /// As `diag_error`, with follow-up advice under the snippet.
    pub(super) fn diag_error_with_notes(&mut self, msg: &str, notes: &[String]) {
        let d = self.command_diagnostic(Severity::Error, msg, notes);
        self.diagnostics.push(d);
    }

    /// A `set`/`show` against a variable this compiler does not have,
    /// suggesting the nearest one it does. The valid set is session state, so
    /// the suggestion is drawn from it rather than from a fixed list.
    pub(super) fn unknown_variable(&self, name: &str) -> CommandError {
        let mut notes = Vec::new();
        if let Some(near) = nearest_name(name, self.variables.keys().map(String::as_str)) {
            notes.push(format!("did you mean '{}'?", near));
        }
        notes.push(String::from(
            "'show variables' lists every variable and its value",
        ));
        CommandError::new(format!("no such variable: '{}'", name)).with_notes(notes)
    }

    /// A reference to a network that was never defined, suggesting the
    /// nearest name this session has defined.
    pub(super) fn unknown_definition(&self, name: &str) -> CommandError {
        let mut notes = Vec::new();
        if let Some(near) = nearest_name(name, self.definitions.keys().map(|k| k.as_str())) {
            notes.push(format!("did you mean '{}'?", near));
        }
        notes.push(String::from("'print defined' lists the defined networks"));
        CommandError::new(format!("no such defined network: '{}'", name)).with_notes(notes)
    }

    /// Record a warning about the command currently being evaluated.
    pub(super) fn diag_warning(&mut self, msg: &str) {
        self.diag_warning_with_notes(msg, &[]);
    }

    /// As `diag_warning`, with follow-up advice under the snippet.
    pub(super) fn diag_warning_with_notes(&mut self, msg: &str, notes: &[String]) {
        let d = self.command_diagnostic(Severity::Warning, msg, notes);
        self.diagnostics.push(d);
    }
}

// ===================== source-anchored diagnostics =========================
// The xfst surface is where a user meets this compiler: a script driver and a
// REPL. A failure here has to say where it happened and, where the cause is a
// known xfst trap, what to type instead. The front-end parser reports byte
// spans; these helpers turn them into something worth reading.

fn script_error(span: std::ops::Range<usize>, message: String, notes: Vec<String>) -> Diagnostic {
    let d = Diagnostic::error(&message).label(span, message);
    notes.into_iter().fold(d, Diagnostic::note)
}

/// Turn an nfst-xfst parse failure into diagnostics worth showing: front-end
/// messages rephrased in the user's vocabulary, regex-body failures re-anchored
/// from body-relative to script-absolute spans, and advice attached where the
/// cause is a known xfst trap. The spans index `src`; the caller attaches it
/// as their source with a name.
pub fn parse_diagnostics(src: &str, e: &nfst_xfst::ParseError) -> Vec<Diagnostic> {
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
            if let Some(line) = bare_regex_line(src, span.start) {
                // Every quote on the line trips the lexer; one report is enough.
                if !out
                    .iter()
                    .any(|o: &Diagnostic| o.primary_span().is_some_and(|s| s.start >= line.start))
                {
                    out.push(bare_regex_diagnostic(line));
                }
                continue;
            }
            out.push(unexpected_character_diagnostic(rest, span));
            continue;
        }
        // Everything else: keep the front-end wording but drop the debug dump
        // of the token it stopped on, which names Rust types, not xfst syntax.
        let message = match d.message.split_once(", got ") {
            Some((expected, _)) => expected.to_string(),
            None => d.message.clone(),
        };
        out.push(script_error(span, message, d.notes.clone()));
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
fn regex_body_diagnostics(src: &str, span: std::ops::Range<usize>) -> Vec<Diagnostic> {
    let body_only = script_error(
        span.clone(),
        String::from("could not parse this regular expression"),
        Vec::new(),
    );
    let Some(padded) = pad_to_offset(src, span.clone()) else {
        return vec![body_only];
    };
    match nfst_xre::parse(&padded) {
        // The body parsed in isolation but not as part of the script: nothing
        // more specific to say than where it is.
        Ok(_) => vec![body_only],
        Err(inner) => {
            let mut out: Vec<Diagnostic> = inner
                .diagnostics
                .iter()
                .map(|d| {
                    script_error(
                        clamp_span(d.span.range.clone(), src.len()),
                        format!(
                            "in this regular expression: {}",
                            spell_token_names(&d.message)
                        ),
                        d.notes.clone(),
                    )
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

fn unknown_command_diagnostic(text: &str, span: std::ops::Range<usize>) -> Diagnostic {
    let word = text.trim();
    if word.is_empty() {
        return script_error(span, String::from("expected a command here"), Vec::new());
    }
    let mut notes = Vec::new();
    if let Some(near) = nearest_command(word) {
        notes.push(format!("did you mean '{}'?", near));
    }
    notes.push(String::from(
        "'help' lists the commands this compiler accepts",
    ));
    script_error(span, format!("unknown command '{}'", word), notes)
}

/// The trimmed span of the line holding `pos`, when that line opens with a
/// quoted literal. No command starts with a quote, so such a line is a regex
/// written without the `regex` command in front of it.
fn bare_regex_line(src: &str, pos: usize) -> Option<std::ops::Range<usize>> {
    let line_start = src[..pos].rfind('\n').map_or(0, |i| i + 1);
    let line_end = src[pos..].find('\n').map_or(src.len(), |i| pos + i);
    let line = &src[line_start..line_end];
    let indent = line.len() - line.trim_start().len();
    if !line.trim_start().starts_with('"') {
        return None;
    }
    Some(line_start + indent..line_start + line.trim_end().len())
}

fn bare_regex_diagnostic(span: std::ops::Range<usize>) -> Diagnostic {
    script_error(
        span,
        String::from("expected a command, found a regular expression"),
        vec![
            String::from(
                "quoted strings are fine inside a regex, but the line must start with a command",
            ),
            String::from("write 'regex' before it to compile it, or 'define NAME' to name it"),
        ],
    )
}

fn unexpected_character_diagnostic(quoted: &str, span: std::ops::Range<usize>) -> Diagnostic {
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
    let message = match ch {
        Some(c) => format!("unexpected character '{}' in a word", c),
        None => String::from("unexpected character"),
    };
    script_error(span, message, notes)
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
