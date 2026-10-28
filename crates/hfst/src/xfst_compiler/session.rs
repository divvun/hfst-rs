//! Session plumbing: prompts, the output and input streams, line input,
//! and the shell and help commands.

use super::*;

// [spec:hfst:req:javascript.xfst-output-sink]
/// Where a session's command output goes: the caller's writer once one is
/// set, and until then a buffer the caller takes.
#[derive(Default)]
pub(super) struct Output {
    writer: Option<Box<dyn Write>>,
    buffer: Vec<u8>,
}

impl Output {
    fn take_buffered(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buffer)
    }
}

impl Write for Output {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut self.writer {
            Some(writer) => writer.write(buf),
            None => self.buffer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.writer {
            Some(writer) => writer.flush(),
            None => Ok(()),
        }
    }
}

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
    // @brief Get next line from \a file. Return NULL if end of file is reached.
    // Use \a promptstr as prompt for readline, or print it to stderr if readline is not in use.
    pub(super) fn read_prompted_line(&mut self, promptstr: &str) -> CmdResult<Option<String>> {
        // The HAVE_READLINE and WINDOWS branches are not ported; mirror the
        // generic getline path: print the prompt, then read a line of input.
        write!(self.output, "{}", promptstr)?;
        self.output.flush()?;

        let mut line = String::new();
        // getline keeps the trailing newline.
        if self.input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        Ok(Some(line))
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.remove-newline-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.remove-newline-fn]
    // @brief Remove newline ('\n' and '\r') from the end of \a str.
    pub(super) fn remove_newline(&mut self, str: String) -> String {
        // The C++ replaces every '\n'/'\r' with '\0' in place; read back as a
        // C-string the result is everything up to the first newline/return.
        match str.find(['\n', '\r']) {
            Some(idx) => str[..idx].to_string(),
            None => str,
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.current-history-index-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.current-history-index-fn]
    // @brief Get current readline history index.
    pub(super) fn current_history_index(&mut self) -> i32 {
        // HAVE_READLINE is not in use; mirror the #else branch.
        -1
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.ignore-history-after-index-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.ignore-history-after-index-fn]
    // @brief Remove all readline history after \a index.
    pub(super) fn ignore_history_after_index(&mut self, index: i32) {
        // HAVE_READLINE is not in use; the whole body is conditional on it, so
        // there is nothing to do.
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-stream-fn]
    /* A wrapper around stream objects, see flush() for more information. */
    fn get_stream(&mut self) {
        // On Unix (non-WINDOWS) get_stream is the identity on the stream passed
        // in; the print* methods here write directly to their own 'oss' writer,
        // so there is nothing to redirect. The WINDOWS console-buffering branch
        // is not ported.
    }

    // @brief Print @a text
    pub fn echo(&mut self, text: &str) -> CmdResult {
        writeln!(self.output, "{}", text)?;
        self.prompt();
        Ok(())
    }

    // @brief Print the quit message; the driver ends the run.
    pub fn quit(&mut self, message: &str) -> CmdResult {
        if self.verbose && (message == "dodongo") {
            writeln!(self.output, "dislikes smoke.")?;
        } else if self.verbose {
            writeln!(self.output, "{}.", message)?;
        }
        Ok(())
    }

    // @brief Run @a command through the shell
    pub fn system(&mut self, command: &str) -> CmdResult {
        if self.restricted_mode {
            return Err(CommandError::new(
                "system calls are disabled by restricted mode (--restricted-mode)",
            ));
        }
        let rv = run_shell(command);
        if rv != 0 {
            return Err(CommandError::new(format!(
                "system '{}' returned {}",
                command, rv
            )));
        }
        self.prompt();
        Ok(())
    }

    // @brief Search help directory
    pub fn apropos(&mut self, text: &str) -> CmdResult {
        let mut message = String::new();
        if !get_help_message(text, &mut message, HELP_MODE_APROPOS) {
            writeln!(self.output, "nothing found for '{}'", text)?;
        } else {
            write!(self.output, "{}", message)?;
        }
        self.prompt();
        Ok(())
    }

    // @brief Print help topics
    pub fn describe(&mut self, text: &str) -> CmdResult {
        let help_mode = if text.is_empty() {
            HELP_MODE_ALL_COMMANDS
        } else {
            HELP_MODE_ONE_COMMAND
        };
        let mut message = String::new();
        if !get_help_message(text, &mut message, help_mode) {
            writeln!(self.output, "no help found for '{}'", text)?;
        } else {
            write!(self.output, "{}", message)?;
        }
        self.prompt();
        Ok(())
    }

    // @brief Sekrit HFST raw command mode!
    pub fn hfst(&mut self, data: &str) {
        info!("HFST: {}", data);
        self.prompt();
    }

    /// Print the prompt when prompts are on. A failed write fails the
    /// command running, as `keep_output_failure` describes.
    pub fn prompt(&mut self) {
        if self.verbose_prompt && self.verbose {
            let prompt = self.get_prompt();
            let written = self.output.write_all(prompt.as_bytes());
            self.keep_output_failure(written.map_err(CommandError::from));
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-prompt-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-prompt-fn]
    // @brief Get the prompt string.
    pub fn get_prompt(&self) -> String {
        format!("hfst[{}]: ", self.stack.len())
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    // [spec:hfst:req:javascript.xfst-output-sink]
    /// Write command output (printed networks, words, apply results, help
    /// text and prompts) to `writer` from here on. Until a writer is set the
    /// output is buffered for `take_output`; what is already buffered stays
    /// there. Diagnostics do not go to this writer.
    pub fn set_output_stream(&mut self, writer: impl Write + 'static) -> &mut Self {
        self.output.writer = Some(Box::new(writer));
        self
    }

    /// The command output buffered while no writer was set, leaving the
    /// buffer empty.
    pub fn take_output(&mut self) -> Vec<u8> {
        self.output.take_buffered()
    }

    /// Read interactive input, the moves 'inspect net' reads and the words
    /// 'apply' reads when given none, from `reader` from here on. Until a
    /// reader is set the input is empty.
    pub fn set_input_stream(&mut self, reader: impl BufRead + 'static) -> &mut Self {
        self.input = Box::new(reader);
        self
    }

    /// The session's input, for a driver that reads its own commands from
    /// the stream the interactive commands read.
    pub fn input(&mut self) -> &mut dyn BufRead {
        &mut *self.input
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    /// Flush the output writer. A failure fails the command running, as
    /// `keep_output_failure` describes.
    pub fn flush(&mut self) {
        let flushed = self.output.flush();
        self.keep_output_failure(flushed.map_err(CommandError::from));
    }

    /// Keep the first failure of an output write made where no result can be
    /// returned (a prompt, a flush, the verbose report on a new network).
    /// The driver takes it after the command and fails the command with it.
    pub(super) fn keep_output_failure(&mut self, outcome: CmdResult) {
        if let Err(e) = outcome
            && self.output_failure.is_none()
        {
            self.output_failure = Some(e);
        }
    }

    /// Run `command` with the session output as its writer. What the session
    /// writes itself meanwhile, such as the closing prompt, is held and
    /// written after the command's own output. The order then matches one
    /// stream only while no session write comes before the command's own
    /// output, which holds for the print, save and apply commands run here.
    pub(super) fn with_session_output<T>(
        &mut self,
        command: impl FnOnce(&mut Self, &mut dyn Write) -> CmdResult<T>,
    ) -> CmdResult<T> {
        let mut out = std::mem::take(&mut self.output);
        let result = command(self, &mut out);
        let held = std::mem::replace(&mut self.output, out).take_buffered();
        let written = self.output.write_all(&held);
        let value = result?;
        written?;
        Ok(value)
    }
}

// Replacement for C system(3): run `command` through the shell and return its
// exit code (or -1 if it could not be launched), so the callers' `!= 0` checks
// keep working without libc.
pub(super) fn run_shell(command: &str) -> i32 {
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .status()
    {
        Ok(status) => status.code().unwrap_or(-1),
        Err(_) => -1,
    }
}

// Help-message support: 'xfst_help_message.h' (get_help_message and the
// HELP_MODE_* constants) was not ported, mirroring the C++ todo that 'helps
// have not been written or copied'. We reproduce the documented behaviour:
// no help is ever found.
const HELP_MODE_APROPOS: i32 = 0;
const HELP_MODE_ALL_COMMANDS: i32 = 1;
const HELP_MODE_ONE_COMMAND: i32 = 2;

fn get_help_message(_text: &str, _message: &mut String, _help_mode: i32) -> bool {
    false
}

// The following three helpers are guarded by '#ifdef FOO' in the C++ source
// (an undefined macro), so they are compiled out and unused there. They are
// ported 1:1 for completeness and stay unused here too.

// Convert 'str' to upper case.
// [spec:hfst:def:xfst-compiler.hfst.xfst.to-upper-case-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.to-upper-case-fn]
// [spec:hfst:def:xfst-help-message.hfst.xfst.to-upper-case-fn]
// [spec:hfst:sem:xfst-help-message.hfst.xfst.to-upper-case-fn]
#[allow(dead_code)]
fn to_upper_case(str: &str) -> String {
    let str_bytes = str.as_bytes();
    let mut retval = String::new();
    for &b in str_bytes {
        if (97..=122).contains(&b) {
            retval.push((b - 32) as char);
        } else {
            retval.push(b as char);
        }
    }
    retval
}

// Whether 'c' is allowed before or after a word when
// searching for the word in text.
// [spec:hfst:def:xfst-compiler.hfst.xfst.allow-char-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.allow-char-fn]
#[allow(dead_code)]
fn allow_char(c: u8) -> bool {
    let allowed_chars = b" \n\t.,;:?!-/'\"<>()|";
    for &allowed in allowed_chars {
        if allowed == c {
            return true;
        }
    }
    false
}

// Whether word 'str' is found in text 'text'.
// Punctuation characters and upper/lower case are handled in this function.
// [spec:hfst:def:xfst-compiler.hfst.xfst.string-found-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.string-found-fn]
#[allow(dead_code)]
fn string_found(str: &str, text: &str) -> bool {
    let str = to_upper_case(str);
    let text = to_upper_case(text);
    let text_bytes = text.as_bytes();
    let pos = match text.find(&str) {
        None => {
            return false;
        }
        Some(p) => p,
    };
    if (pos == 0 || allow_char(text_bytes[pos - 1]))
        && (pos + str.len() == text.len() || allow_char(text_bytes[pos + str.len()]))
    {
        return true;
    }
    false
}
