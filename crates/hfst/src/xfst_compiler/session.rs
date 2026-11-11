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
        // [spec:hfst:req:embed.compile-files]
        let (rv, printed) = run_shell_capturing(command);
        self.output.write_all(&printed)?;
        if rv != 0 {
            return Err(CommandError::new(format!(
                "system '{}' returned {}",
                command, rv
            )));
        }
        self.prompt();
        Ok(())
    }

    // [spec:hfst:req:xfst-cmd.help]
    /// 'apropos WORD': the commands whose names or description hold WORD.
    pub fn apropos(&mut self, text: &str) -> CmdResult {
        match help::apropos(text) {
            Some(message) => write!(self.output, "{}", message)?,
            None => writeln!(self.output, "nothing found for '{}'", text)?,
        }
        self.prompt();
        Ok(())
    }

    // [spec:hfst:req:xfst-cmd.help]
    /// 'help' and 'describe': every command, or the commands NAME names.
    pub fn describe(&mut self, text: &str) -> CmdResult {
        match help::help_for(text) {
            Some(message) => write!(self.output, "{}", message)?,
            None => writeln!(self.output, "no help found for '{}'", text)?,
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
// keep working without libc. What it prints is discarded: a session writes
// nothing to the process's standard output.
pub(super) fn run_shell(command: &str) -> i32 {
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::null())
        .status()
    {
        Ok(status) => status.code().unwrap_or(-1),
        Err(_) => -1,
    }
}

/// As [`run_shell`], returning what the command printed for the session
/// output instead.
fn run_shell_capturing(command: &str) -> (i32, Vec<u8>) {
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => (output.status.code().unwrap_or(-1), output.stdout),
        Err(_) => (-1, Vec::new()),
    }
}
