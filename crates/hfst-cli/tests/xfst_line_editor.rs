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

/// Type `keys` and wait for the output line `line` that follows them, then
/// for the prompt after it. Keys typed before that prompt is drawn can land
/// in the editor's buffer for the line before, which drops them.
fn answers(p: &mut PtySession, keys: &str, line: &str, prompt: &str) -> String {
    type_keys(p, keys);
    let before = p
        .exp_string(&format!("\r\n{line}\r\n"))
        .unwrap_or_else(|e| panic!("no {line:?} after {keys:?}: {e}"));
    p.exp_string(prompt)
        .unwrap_or_else(|e| panic!("no prompt {prompt:?} after {line:?}: {e}"));
    before
}

// [spec:hfst:req:xfst-cmd.shell-line-editing/test]
#[test]
fn backspace_edits_and_up_recalls_the_line() {
    let mut p = shell(&[]);
    answers(&mut p, "echo hx\x7fi\r", "hi", "hfst[0]: ");
    answers(&mut p, "\x1b[A\r", "hi", "hfst[0]: ");
}

// [spec:hfst:req:xfst-cmd.shell-line-editing/test]
// Ctrl-C on a continued line drops the whole unfinished command.
#[test]
fn ctrl_c_drops_the_unfinished_command() {
    let mut p = shell(&[]);
    type_keys(&mut p, "echo lost \\\r");
    p.exp_string("       > ").expect("the continuation prompt");
    type_keys(&mut p, "\x03");
    p.exp_string("hfst[0]: ").expect("a fresh prompt");
    let before = answers(&mut p, "echo kept\r", "kept", "hfst[0]: ");
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
    answers(&mut p, "a\r", "b", "apply down> ");
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

// [spec:hfst:req:xfst-cmd.shell-session-edges/test]
// The start line comes before the first prompt, Ctrl-D ends the session on
// a line of its own, and no timing line follows.
#[test]
fn a_terminal_session_starts_and_ends_cleanly() {
    for args in [&[][..], &["-r"][..]] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_hfst"));
        cmd.arg("xfst")
            .args(args)
            .env_remove("HFST_OPTIONS")
            .env("NO_COLOR", "1");
        let mut p = spawn_command(cmd, Some(5_000)).expect("spawn hfst xfst");
        p.exp_string("Type 'help' for the commands, Ctrl-D to quit.")
            .expect("the start line");
        p.exp_string("hfst[0]: ").expect("the first prompt");
        type_keys(&mut p, "\x04");
        let rest = p.exp_eof().expect("the session ends");
        assert!(rest.contains("\r\n"), "{args:?}: {rest:?}");
        assert!(!rest.contains("hfst-time"), "{args:?}: {rest:?}");
    }
}

// [spec:hfst:req:xfst-cmd.shell-completion/test]
#[test]
fn tab_completes_commands_and_defined_names() {
    let mut p = shell(&[]);
    answers(&mut p, "ech\thi\r", "hi", "hfst[0]: ");
    answers(&mut p, "define Foo a ;\r", "Defined 'Foo'", "hfst[0]: ");
    type_keys(&mut p, "push defined F\t\r");
    p.exp_string("hfst[1]: ").expect("Foo was pushed");
}

// [spec:hfst:req:xfst-cmd.shell-completion/test]
#[test]
fn tab_completes_a_file_name() {
    let dir = tempfile::tempdir().expect("a scratch directory");
    std::fs::write(dir.path().join("words.txt"), "cat\ndog\n").expect("write the word list");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hfst"));
    cmd.arg("xfst")
        .current_dir(dir.path())
        .env_remove("HFST_OPTIONS")
        .env("NO_COLOR", "1");
    let mut p = spawn_command(cmd, Some(5_000)).expect("spawn hfst xfst");
    p.exp_string("hfst[0]: ").expect("the first prompt");
    type_keys(&mut p, "read text wor\t\r");
    p.exp_string("hfst[1]: ").expect("the file was read");
}

// [spec:hfst:req:xfst-cmd.shell-cancel/test]
// The compose takes many seconds in a debug build. Ctrl-C stops it well
// inside the five-second wait, and the network pushed before it remains.
#[test]
fn ctrl_c_cancels_a_running_command() {
    let mut p = shell(&[]);
    type_keys(&mut p, "regex a ;\r");
    p.exp_string("hfst[1]: ").expect("prompt after regex");
    type_keys(&mut p, "regex [[a|b]^1000]* .o. [[a|b]^999]* ;\r");
    std::thread::sleep(std::time::Duration::from_millis(500));
    type_keys(&mut p, "\x03");
    p.exp_string("interrupted").expect("the command stops");
    p.exp_string("hfst[1]: ").expect("the stack is as it was");
    answers(&mut p, "print stack\r", "0: 2 states, 1 arcs", "hfst[1]: ");
}

// [spec:hfst:req:xfst-cmd.shell-cancel-foma/test]
// Under foma the same compose runs over a minute in a debug build; Ctrl-C
// stops it inside the five-second wait, as under the default backend.
#[test]
fn ctrl_c_cancels_a_command_under_foma() {
    let mut p = shell(&["-f", "foma"]);
    type_keys(&mut p, "regex a ;\r");
    p.exp_string("hfst[1]: ").expect("prompt after regex");
    type_keys(&mut p, "regex [[a|b]^1000]* .o. [[a|b]^999]* ;\r");
    std::thread::sleep(std::time::Duration::from_millis(500));
    type_keys(&mut p, "\x03");
    p.exp_string("interrupted").expect("the command stops");
    p.exp_string("hfst[1]: ").expect("the stack is as it was");
    answers(&mut p, "print stack\r", "0: 2 states, 1 arcs", "hfst[1]: ");
}
