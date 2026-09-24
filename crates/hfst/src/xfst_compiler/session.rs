//! Session plumbing: prompts, output streams, fail flags, line input, and
//! the shell and help commands.

use super::*;

// Output convention: the C++ output() returns *output_ (std::cout by
// default) and error() returns *error_ (std::cerr by default); get_stream()
// is the identity on non-Windows and flush() is a no-op there. We mirror that
// Unix behaviour directly: output() writes to stdout via print!, diagnostics
// go through the tracing macros, and the print* methods write their 'stream'
// content to the oss writer passed in (the C++ '*oss' target).
impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
    /* Set the stream where error messages and warnings are printed. */
    /* Get the stream where error messages and warnings are printed. */
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    /* Set the stream where output is printed. */
    /* Get the stream where output is printed. */
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fclose-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fclose-fn]
    /* A wrapper around file close function. */
    pub fn close_file(&mut self, name: &str) -> i32 {
        // The redesigned signature carries no FILE handle (file I/O is done via
        // std::fs / HfstInputStream elsewhere), so there is nothing to close;
        // mirror the success path of the C++ wrapper.
        let retval: i32 = 0;
        if retval != 0 {
            self.diag_error(&format!("could not close file '{}'", name));
            self.flush();
            self.xfst_fail();
        }
        retval
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fopen-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fopen-fn]
    /* A wrapper around file open function. */
    pub fn open_file(&mut self, path: &str, mode: &str) {
        match crate::hfst_data_types::open_file(path, mode) {
            Err(_) => {
                self.diag_error(&format!("could not open file '{}'", path));
                self.flush();
                self.xfst_fail();
            }
            Ok(f) => {
                // The redesigned signature returns no handle, so the freshly
                // opened file is closed again here (dropped).
                drop(f);
            }
        }
    }

    /* Get the output stream. */
    /* Get the error stream. */
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    /* Flush the stream. */

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fail-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fail-fn]
    // @brief Set fail flag to true if quit-on-fail is ON,
    // else do nothing.
    pub(super) fn xfst_fail(&mut self) {
        if self.variables["quit-on-fail"] == "ON" {
            self.fail_flag = true;
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-lesser-fail-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-lesser-fail-fn]
    // @brief Set fail flag to true if quit-on-fail is ON and hfst-xfst
    // is not used in interactive mode, else do nothing.
    pub(super) fn xfst_lesser_fail(&mut self) {
        if self.variables["quit-on-fail"] == "ON" && !self.read_interactive_text_from_stdin {
            self.fail_flag = true;
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
    // @brief Get next line from \a file. Return NULL if end of file is reached.
    // Use \a promptstr as prompt for readline, or print it to stderr if readline is not in use.
    pub(super) fn read_prompted_line(&mut self, promptstr: &str) -> Option<String> {
        // The HAVE_READLINE and WINDOWS branches are not ported; mirror the
        // generic getline path: print the prompt, then read a line from stdin.
        print!("{}", promptstr);
        self.flush();

        let mut line = String::new();
        // getline keeps the trailing newline; read == -1 (EOF) returns NULL.
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(line),
            Err(_) => None,
        }
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

    // @brief Print @a text to stdout
    pub fn echo(&mut self, text: &str) -> &mut Self {
        println!("{}", text);
        self.prompt();
        self
    }

    // @brief Stop parser, print quit message
    pub fn quit(&mut self, message: &str) -> &mut Self {
        if self.verbose && (message == "dodongo") {
            println!("dislikes smoke.");
        } else if self.verbose {
            println!("{}.", message);
        } else {
            // ;
        }
        self.quit_requested = true;
        self
    }

    // @brief Execute @c system()
    pub fn system(&mut self, command: &str) -> &mut Self {
        if self.restricted_mode {
            self.diag_warning("system calls are disabled by restricted mode (--restricted-mode)");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }
        let rv = run_shell(command);
        if rv != 0 {
            self.diag_warning(&format!("system '{}' returned {}", command, rv));
        }
        self.prompt();
        self
    }

    // @brief Search help directory
    // @todo helps have not been written or copied
    pub fn apropos(&mut self, text: &str) -> &mut Self {
        let mut message = String::new();
        if !get_help_message(text, &mut message, HELP_MODE_APROPOS) {
            println!("nothing found for '{}'", text);
        } else {
            print!("{}", message);
        }
        self.prompt();
        self
    }

    // @brief Print help topics
    // @todo helps have not been written or copied
    pub fn describe(&mut self, text: &str) -> &mut Self {
        let help_mode = if text.is_empty() {
            HELP_MODE_ALL_COMMANDS
        } else {
            HELP_MODE_ONE_COMMAND
        };
        let mut message = String::new();
        if !get_help_message(text, &mut message, help_mode) {
            println!("no help found for '{}'", text);
        } else {
            print!("{}", message);
        }
        self.prompt();
        self
    }

    // @brief Sekrit HFST raw command mode!
    pub fn hfst(&mut self, data: &str) -> &mut Self {
        info!("HFST: {}", data);
        self.prompt();
        self
    }

    // @brief Explicitly print the prompt to stdout.
    pub fn prompt(&mut self) -> &Self {
        if self.verbose_prompt && self.verbose {
            // On windows, prompt is always printed to console. On other platforms,
            // this has no effect.
            print!("hfst[{}]: ", self.stack.len());
        }
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-prompt-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-prompt-fn]
    // @brief Get the prompt string.
    pub fn get_prompt(&self) -> String {
        format!("hfst[{}]: ", self.stack.len())
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
    /* Set the stream where error messages and warnings are printed. */
    pub fn set_error_stream(&mut self) {
        // error_ = &os;
        // this->xre.set_error_stream(this->error_);
        // this->lexc.set_error_stream(this->error_);
    }

    /* Get the stream where error messages and warnings are printed. */
    pub fn get_error_stream(&mut self) {
        // return *error_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    /* Set the stream where output is printed. */
    pub fn set_output_stream(&mut self) {
        // output_ = &os;
    }

    /* Get the stream where output is printed. */
    pub fn get_output_stream(&mut self) {
        // return *output_;
    }

    /* Get the output stream. */
    pub fn output(&mut self) {
        // return *output_;
    }

    /* Get the error stream. */
    pub fn error(&mut self) {
        // return *error_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    /* Flush the stream. */
    pub fn flush(&mut self) {
        // On Unix and Mac this is a no-op; the WINDOWS console-buffering branch
        // is not ported.
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
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
