//! The hfst xfst shell on a terminal, driven through a pseudo-terminal:
//! the line editor, its history, and Ctrl-C and Ctrl-D at a prompt.
#![cfg(unix)]

use rexpect::session::{PtySession, spawn_command};
use std::process::Command;

fn shell(args: &[&str]) -> PtySession {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hfst"));
    cmd.arg("xfst")
        .args(args)
        .env_remove("HFST_OPTIONS")
        .env("NO_COLOR", "1");
    let mut p = spawn_command(cmd, Some(5_000)).expect("spawn hfst xfst on a pseudo-terminal");
    p.exp_string("hfst[0]: ").expect("the first prompt");
    p
}

fn type_keys(p: &mut PtySession, keys: &str) {
    p.send(keys).expect("type");
    p.flush().expect("flush");
}

/// Type `keys` and wait for the output line `line` that follows them.
fn answers(p: &mut PtySession, keys: &str, line: &str) -> String {
    type_keys(p, keys);
    p.exp_string(&format!("\r\n{line}\r\n"))
        .unwrap_or_else(|e| panic!("no {line:?} after {keys:?}: {e}"))
}

// [spec:hfst:req:xfst-cmd.shell-line-editing/test]
#[test]
fn backspace_edits_and_up_recalls_the_line() {
    let mut p = shell(&[]);
    answers(&mut p, "echo hx\x7fi\r", "hi");
    answers(&mut p, "\x1b[A\r", "hi");
}

// [spec:hfst:req:xfst-cmd.shell-line-editing/test]
// Ctrl-C on a continued line drops the whole unfinished command.
#[test]
fn ctrl_c_drops_the_unfinished_command() {
    let mut p = shell(&[]);
    type_keys(&mut p, "echo lost \\\r");
    p.exp_string("hfst[0]: ").expect("the continuation prompt");
    type_keys(&mut p, "\x03");
    p.exp_string("hfst[0]: ").expect("a fresh prompt");
    let before = answers(&mut p, "echo kept\r", "kept");
    assert!(!before.contains("\r\nlost"), "{before:?}");
}

// [spec:hfst:req:xfst-cmd.shell-line-editing/test]
#[test]
fn ctrl_d_ends_the_word_mode_then_session() {
    let mut p = shell(&[]);
    type_keys(&mut p, "regex a:b ;\r");
    p.exp_string("hfst[1]: ").expect("prompt after regex");
    type_keys(&mut p, "apply down\r");
    p.exp_string("apply down> ").expect("the word mode prompt");
    answers(&mut p, "a\r", "b");
    type_keys(&mut p, "\x04");
    p.exp_string("hfst[1]: ").expect("back at the shell");
    type_keys(&mut p, "\x04");
    p.exp_eof().expect("the session ends");
}

// [spec:hfst:req:xfst-cmd.shell-line-editing/test]
// With -r the terminal hands over the raw escape an arrow key sends.
#[test]
fn no_readline_reads_plain_lines() {
    let mut p = shell(&["-r"]);
    type_keys(&mut p, "\x1b[A\r");
    p.exp_string("unexpected character")
        .expect("the escape reaches the parser");
}
