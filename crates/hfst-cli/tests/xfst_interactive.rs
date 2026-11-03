//! The hfst xfst shell: one prompt before each line it reads, on the
//! terminal before it waits (hfst-rs#8).

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn shell() -> Child {
    Command::new(env!("CARGO_BIN_EXE_hfst"))
        .env_remove("HFST_OPTIONS")
        .arg("xfst")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn hfst xfst")
}

fn session(input: &str) -> String {
    let mut child = shell();
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst xfst");
    String::from_utf8(out.stdout).expect("stdout is UTF-8")
}

// [spec:hfst:req:xfst-cmd.interactive-prompt/test]
#[test]
fn every_line_read_gets_one_prompt() {
    // A command that works, a blank line, a comment, a command that fails,
    // and a regex continued over two lines, then end of input.
    assert_eq!(
        session("echo hi\n\n# c\nnosuch\nregex a \\\nb ;\n"),
        "hfst[0]: hi\nhfst[0]: hfst[0]: hfst[0]: hfst[0]: hfst[0]: 3 states, 2 arcs\nhfst[1]: \n"
    );
}

// [spec:hfst:req:xfst-cmd.interactive-prompt/test]
// The prompt after a command must arrive while the shell waits for the next
// line, not when a later write happens to flush it.
#[test]
fn the_prompt_arrives_before_the_shell_waits() {
    let mut child = shell();
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 256];
        while let Ok(n) = stdout.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    stdin.write_all(b"echo hi\n").expect("write stdin");
    stdin.flush().expect("flush stdin");

    let expected = b"hfst[0]: hi\nhfst[0]: ";
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = Vec::new();
    while seen.as_slice() != expected {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(chunk) => seen.extend(chunk),
            Err(_) => break,
        }
    }
    drop(stdin);
    child.wait().expect("wait for hfst xfst");
    assert_eq!(String::from_utf8_lossy(&seen), "hfst[0]: hi\nhfst[0]: ");
}

// [spec:hfst:req:xfst-cmd.shell-word-modes/test]
#[test]
fn apply_without_a_word_reads_the_following_lines() {
    assert_eq!(
        session("regex a:b ;\napply down\na\nc\n"),
        "hfst[0]: 2 states, 1 arcs\nhfst[1]: apply down> b\napply down> ???\napply down> \nhfst[1]: \n"
    );
}

// [spec:hfst:req:xfst-cmd.shell-word-modes/test]
// A bare 'read text' used to push an empty network and run its words as
// commands.
#[test]
fn bare_read_text_reads_the_following_lines() {
    assert_eq!(
        session("read text\nfoo\nbar\n<ctrl-d>\nprint words\n"),
        "hfst[0]: read text> read text> read text> \n6 states, 6 arcs\nhfst[1]: bar\nfoo\nhfst[1]: \n"
    );
    assert_eq!(
        session("read spaced-text\nf o\n<ctrl-d>\nprint words\n"),
        "hfst[0]: read spaced-text> read spaced-text> \n3 states, 2 arcs\nhfst[1]: fo\nhfst[1]: \n"
    );
}

// [spec:hfst:req:xfst-cmd.help/test]
// The report in hfst-rs#8: 'help' found nothing.
#[test]
fn help_at_the_prompt_lists_the_commands() {
    let out = session("help\n");
    assert!(out.starts_with("hfst[0]: ambiguous upper "), "{out}");
    assert!(out.contains("\nhelp <name> "), "{out}");
    assert!(out.ends_with("hfst[0]: \n"), "{out}");
}

// [spec:hfst:req:xfst-cmd.shell-session-edges/test]
// Away from a terminal there is no start line, and the timing line stays
// for the build logs that grep it.
#[test]
fn a_piped_session_keeps_the_timing_line() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .env_remove("HFST_OPTIONS")
        .arg("xfst")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hfst xfst");
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("wait for hfst xfst");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hfst[0]: \n");
    assert!(String::from_utf8_lossy(&out.stderr).contains("hfst-time"));
}

// [spec:hfst:req:xfst-cmd.shell-session-edges/test]
#[test]
fn help_describes_the_line_editor() {
    let out = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(["xfst", "--help"])
        .output()
        .expect("run hfst xfst --help");
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("line editor"), "{help}");
    assert!(!help.contains("if enabled when configuring"), "{help}");
}
