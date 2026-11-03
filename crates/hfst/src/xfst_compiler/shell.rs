//! The interactive shell: the loop that reads commands a line at a time,
//! and the one place every interactive line is read.

use super::*;

/// A terminal the shell reads lines from, such as a line editor. Without
/// one the session reads its input stream and prompts on its output.
pub trait LineSource {
    /// Show `prompt` and return the next line without its line break, or
    /// `None` at end of input. A line that is not UTF-8 is an error of kind
    /// `InvalidData`; the session reports it and reads on. Ctrl-C at the
    /// prompt is an error of kind `Interrupted`; the shell drops the line and
    /// any unfinished command.
    fn read_line(&mut self, prompt: &str) -> std::io::Result<Option<String>>;
}

/// What one read at a prompt gave.
pub(super) enum Input {
    Line(String),
    /// Ctrl-C at the prompt.
    Cancelled,
    End,
}

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:req:xfst-cmd.shell-in-library]
    /// Read the shell's lines, and the lines the interactive commands read,
    /// from `source` from here on.
    pub fn set_line_source(&mut self, source: Box<dyn LineSource>) -> &mut Self {
        self.line_source = Some(source);
        self
    }

    // [spec:hfst:req:xfst-cmd.shell-in-library]
    // [spec:hfst:req:xfst-cmd.interactive-prompt]
    /// Run the interactive shell until end of input or `quit`: prompt for
    /// each line, run each command once its lines are in, and report a
    /// failure and go on. The commands print no prompts of their own while
    /// it runs. An error is a failure to read input or write output.
    pub fn interact(&mut self) -> CmdResult {
        self.verbose_prompt = false;
        self.read_interactive_text_from_stdin = true;
        let mut expression = String::new();
        loop {
            let prompt = if self.verbose {
                self.get_prompt()
            } else {
                String::new()
            };
            let line = match self.read_input(&prompt)? {
                Input::Line(line) => line,
                Input::Cancelled => {
                    expression.clear();
                    continue;
                }
                Input::End => {
                    // [spec:hfst:req:xfst-cmd.shell-session-edges]
                    // Plain input leaves the cursor after the prompt; a
                    // host's line source ends the line itself.
                    if self.verbose && self.line_source.is_none() {
                        writeln!(self.output)?;
                    }
                    return Ok(());
                }
            };
            expression.push_str(&line);
            if expression_continues(&mut expression) {
                continue;
            }
            // A failure is reported where it happens; the shell goes on.
            if let Ok(Flow::Quit) = self.parse(&format!("{}\n", expression)) {
                return Ok(());
            }
            expression.clear();
        }
    }

    // [spec:hfst:req:xfst-cmd.shell-word-modes]
    /// The word mode of `command`: hand each non-blank line read under the
    /// prompt 'COMMAND> ' to `word`, until end of input or a '<ctrl-d>' line.
    pub(super) fn read_words(
        &mut self,
        command: &str,
        mut word: impl FnMut(&mut Self, &str) -> CmdResult,
    ) -> CmdResult {
        let prompt = if self.verbose {
            format!("{}> ", command)
        } else {
            String::new()
        };
        while let Some(line) = self.read_prompted_line(&prompt)? {
            let line = line.trim_end_matches('\r');
            if line == WORDS_END {
                break;
            }
            if !line.trim().is_empty() {
                word(self, line)?;
            }
        }
        // Plain input leaves the cursor after the prompt at end of input; a
        // host's line source ends the line itself.
        if self.verbose && self.line_source.is_none() {
            writeln!(self.output)?;
        }
        Ok(())
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
    /// Show `prompt` and read the next line, as `read_input` does, skipping
    /// a line cancelled with Ctrl-C. `None` at end of input.
    pub(super) fn read_prompted_line(&mut self, prompt: &str) -> CmdResult<Option<String>> {
        loop {
            match self.read_input(prompt)? {
                Input::Line(line) => return Ok(Some(line)),
                Input::Cancelled => {}
                Input::End => return Ok(None),
            }
        }
    }

    // [spec:hfst:req:xfst-cmd.shell-in-library]
    /// Show `prompt` and read the next line, without its line break, from
    /// the line source or else the input stream. A line that is not UTF-8 is
    /// reported and skipped.
    pub(super) fn read_input(&mut self, prompt: &str) -> CmdResult<Input> {
        loop {
            let read = match &mut self.line_source {
                Some(source) => source.read_line(prompt),
                None => {
                    self.output.write_all(prompt.as_bytes())?;
                    self.output.flush()?;
                    read_input_line(&mut *self.input)
                }
            };
            match read {
                Ok(Some(line)) => return Ok(Input::Line(line)),
                Ok(None) => return Ok(Input::End),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    return Ok(Input::Cancelled);
                }
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    self.diagnostics.push(
                        Diagnostic::error("input line is not valid UTF-8; it was skipped")
                            .note("xfst reads its input as UTF-8"),
                    );
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// The line that ends a word mode before end of input, as in a script.
const WORDS_END: &str = "<ctrl-d>";

/// The next line of `input` without its line break; `None` at end of input.
fn read_input_line(input: &mut dyn BufRead) -> std::io::Result<Option<String>> {
    let mut bytes = Vec::new();
    if input.read_until(b'\n', &mut bytes)? == 0 {
        return Ok(None);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Whether `expr` goes on to the next line: it ends in `\`, which becomes a
/// line break. A trailing carriage return is dropped first.
fn expression_continues(expr: &mut String) -> bool {
    if expr.ends_with('\r') {
        expr.pop();
    }
    if expr.ends_with('\\') {
        expr.pop();
        expr.push('\n');
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst_openfst::StdVectorFst;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Lines handed out in order, with every prompt they were asked for.
    struct Scripted {
        lines: std::collections::VecDeque<std::io::Result<Option<String>>>,
        prompts: Rc<RefCell<Vec<String>>>,
    }

    impl LineSource for Scripted {
        fn read_line(&mut self, prompt: &str) -> std::io::Result<Option<String>> {
            self.prompts.borrow_mut().push(prompt.to_string());
            self.lines.pop_front().unwrap_or(Ok(None))
        }
    }

    fn session() -> XfstCompiler<StdVectorFst> {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        c.set_verbosity(true);
        c
    }

    // [spec:hfst:req:xfst-cmd.shell-in-library/test]
    // The shell, the apply word mode and inspect all read through the host's
    // line source, each with its own prompt.
    #[test]
    fn every_interactive_line_comes_through_the_line_source() {
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let lines = [
            "regex a:b ;",
            "apply down",
            "a",
            "<ctrl-d>",
            "inspect net",
            "0",
            "print words",
        ];
        let mut c = session();
        c.set_line_source(Box::new(Scripted {
            lines: lines.iter().map(|l| Ok(Some(l.to_string()))).collect(),
            prompts: Rc::clone(&prompts),
        }));
        c.interact().expect("the shell runs to end of input");
        assert_eq!(
            *prompts.borrow(),
            [
                "hfst[0]: ",
                "hfst[1]: ",
                "apply down> ",
                "apply down> ",
                "hfst[1]: ",
                "",
                "hfst[1]: ",
                "hfst[1]: ",
            ]
        );
        let out = String::from_utf8(c.take_output()).expect("UTF-8 output");
        assert!(out.contains("b\n"), "apply down a gave b: {out}");
        assert!(
            out.ends_with("a:b\n"),
            "print words ran after inspect: {out}"
        );
    }

    // [spec:hfst:req:xfst-cmd.shell-in-library/test]
    #[test]
    fn a_non_utf8_line_is_reported_and_skipped() {
        let mut c = session();
        c.set_input_stream(std::io::Cursor::new(b"echo \xff\necho after\n".to_vec()));
        c.interact().expect("the shell runs to end of input");
        let out = String::from_utf8(c.take_output()).expect("UTF-8 output");
        assert_eq!(out, "hfst[0]: hfst[0]: after\nhfst[0]: \n");
        assert!(
            c.diagnostics()
                .iter()
                .any(|d| d.message().contains("not valid UTF-8")),
            "the bad line is reported"
        );
    }

    fn scripted(lines: &[&str]) -> (XfstCompiler<StdVectorFst>, Rc<RefCell<Vec<String>>>) {
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let mut c = session();
        c.set_line_source(Box::new(Scripted {
            lines: lines.iter().map(|l| Ok(Some(l.to_string()))).collect(),
            prompts: Rc::clone(&prompts),
        }));
        (c, prompts)
    }

    // [spec:hfst:req:xfst-cmd.shell-word-modes/test]
    // The words come through the line source under the command's prompt,
    // blank lines are skipped, and the mode ends at '<ctrl-d>'.
    #[test]
    fn read_text_builds_from_the_lines_read() {
        let (mut c, prompts) = scripted(&["read text", "ab", "", "c", "<ctrl-d>", "print words"]);
        c.interact().expect("the shell runs to end of input");
        let words = &prompts.borrow()[1..5];
        assert!(words.iter().all(|p| p == "read text> "), "{words:?}");
        let out = String::from_utf8(c.take_output()).expect("UTF-8 output");
        assert!(out.ends_with("ab\nc\n"), "{out}");
    }

    // [spec:hfst:req:xfst-cmd.shell-word-modes/test]
    // End of input ends the mode; the shell then reads on and ends too.
    #[test]
    fn end_of_input_ends_a_word_mode() {
        let (mut c, prompts) = scripted(&["regex a:b ;", "apply up", "b"]);
        c.interact().expect("the shell runs to end of input");
        assert_eq!(
            *prompts.borrow(),
            [
                "hfst[0]: ",
                "hfst[1]: ",
                "apply up> ",
                "apply up> ",
                "hfst[1]: "
            ]
        );
        let out = String::from_utf8(c.take_output()).expect("UTF-8 output");
        // The host's line source ends the line at Ctrl-D, so no blank line.
        assert!(out.ends_with("2 states, 1 arcs\na\n"), "{out}");
    }

    // [spec:hfst:req:xfst-cmd.shell-line-editing/test]
    // Ctrl-C on a continued line drops what was typed before it, so the
    // half-typed echo never runs.
    #[test]
    fn ctrl_c_drops_the_unfinished_command() {
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let mut c = session();
        c.set_line_source(Box::new(Scripted {
            lines: [
                Ok(Some("echo lost \\".to_string())),
                Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
                Ok(Some("echo kept".to_string())),
            ]
            .into(),
            prompts: Rc::clone(&prompts),
        }));
        c.interact().expect("the shell runs to end of input");
        let out = String::from_utf8(c.take_output()).expect("UTF-8 output");
        assert_eq!(out, "kept\n");
        assert_eq!(prompts.borrow().len(), 4, "a fresh prompt after Ctrl-C");
    }

    // [spec:hfst:req:xfst-cmd.shell-session-edges/test]
    // Plain input ends the last prompt's line itself; a line source, such as
    // the terminal editor, already has.
    #[test]
    fn end_of_input_leaves_a_fresh_line() {
        let mut plain = session();
        plain.set_input_stream(std::io::Cursor::new(b"echo hi\n".to_vec()));
        plain.interact().expect("the shell runs to end of input");
        assert_eq!(plain.take_output(), b"hfst[0]: hi\nhfst[0]: \n");

        let (mut edited, _) = scripted(&["echo hi"]);
        edited.interact().expect("the shell runs to end of input");
        assert_eq!(edited.take_output(), b"hi\n");
    }

    // [spec:hfst:req:xfst-cmd.shell-session-edges/test]
    // A silent session prints neither prompts nor the closing line break.
    #[test]
    fn a_silent_session_prints_no_prompts() {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        c.set_verbosity(false);
        c.set_input_stream(std::io::Cursor::new(b"echo hi\n".to_vec()));
        c.interact().expect("the shell runs to end of input");
        assert_eq!(c.take_output(), b"hi\n");
    }

    // [spec:hfst:req:xfst-cmd.shell-in-library/test]
    #[test]
    fn an_input_failure_ends_the_shell() {
        let mut c = session();
        c.set_line_source(Box::new(Scripted {
            lines: [Err(std::io::Error::other("terminal went away"))].into(),
            prompts: Rc::default(),
        }));
        assert!(c.interact().is_err());
    }
}
