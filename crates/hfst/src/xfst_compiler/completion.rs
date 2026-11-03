//! What Tab completes at the shell prompt. The host's line editor asks; the
//! answer comes from the help table and the session's defined names.

use super::help;

/// What Tab completes at a cursor.
#[derive(Debug, PartialEq)]
pub enum Completion {
    /// Replace the text from `start` to the cursor with one of `candidates`.
    Words {
        start: usize,
        candidates: Vec<String>,
    },
    /// The cursor is in a file name, which starts at `start`.
    File {
        start: usize,
    },
    None,
}

/// The commands whose argument names a defined network.
const TAKES_DEFINITION: &[&str] = &[
    "push defined",
    "push",
    "substitute defined",
    "undefine",
    "print labels",
    "labels",
    "write definition",
    "wdef",
];

// [spec:hfst:req:xfst-cmd.shell-completion]
/// What Tab completes at byte `pos` of `line`, given the names `defined` in
/// the session. A command starts at the beginning of the line or after a
/// `;`. While its name is still being typed, the candidates are the command
/// names it can still become, each followed by a space. After a command that
/// takes a defined network, the word at the cursor completes to a defined
/// name, and after one that takes a file, to a file name.
pub fn complete(line: &str, pos: usize, defined: &[String]) -> Completion {
    let Some(before) = line.get(..pos) else {
        return Completion::None;
    };
    let after_semicolon = before.rfind(';').map_or(0, |i| i + 1);
    let typed = before[after_semicolon..].trim_start();
    let start = pos - typed.len();

    let mut commands: Vec<String> = help::command_names()
        .filter(|name| name.starts_with(typed))
        .map(|name| format!("{name} "))
        .collect();
    if !commands.is_empty() {
        commands.sort();
        commands.dedup();
        return Completion::Words {
            start,
            candidates: commands,
        };
    }

    // The longest command name the typed text opens with, then a space.
    let Some(command) = help::command_names()
        .filter(|name| {
            typed
                .strip_prefix(name)
                .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        })
        .max_by_key(|name| name.len())
    else {
        return Completion::None;
    };
    let word_start = before
        .rfind(char::is_whitespace)
        .map_or(start, |i| i + 1)
        .max(start + command.len());
    let word = &before[word_start..];
    if TAKES_DEFINITION.contains(&command) {
        let mut candidates: Vec<String> = defined
            .iter()
            .filter(|name| name.starts_with(word))
            .cloned()
            .collect();
        candidates.sort();
        return Completion::Words {
            start: word_start,
            candidates,
        };
    }
    if help::takes_file(command) {
        return Completion::File { start: word_start };
    }
    Completion::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        match complete(line, line.len(), &["Foo".into(), "Bar".into()]) {
            Completion::Words { candidates, .. } => candidates,
            other @ (Completion::File { .. } | Completion::None) => {
                panic!("{line:?} completed to {other:?}")
            }
        }
    }

    // [spec:hfst:req:xfst-cmd.shell-completion/test]
    #[test]
    fn a_command_name_completes_from_the_help_table() {
        assert_eq!(words("ech"), ["echo "]);
        assert!(words("print wo").contains(&String::from("print words ")));
        assert!(words("").len() > 100, "an empty line offers every command");
    }

    // [spec:hfst:req:xfst-cmd.shell-completion/test]
    // A command after a ';' on the same line completes too.
    #[test]
    fn a_command_completes_after_a_semicolon() {
        let line = "regex a ; minim";
        assert_eq!(
            complete(line, line.len(), &[]),
            Completion::Words {
                start: 10,
                candidates: vec![
                    "minimise ".into(),
                    "minimize ".into(),
                    "minimize net ".into()
                ]
            }
        );
    }

    // [spec:hfst:req:xfst-cmd.shell-completion/test]
    #[test]
    fn a_defined_name_completes_after_push_defined() {
        assert_eq!(words("push defined F"), ["Foo"]);
        assert_eq!(words("undefine Foo B"), ["Bar"]);
        let line = "push defined ";
        assert_eq!(
            complete(line, line.len(), &["Foo".into()]),
            Completion::Words {
                start: line.len(),
                candidates: vec!["Foo".into()]
            }
        );
    }

    // [spec:hfst:req:xfst-cmd.shell-completion/test]
    #[test]
    fn a_file_name_completes_after_a_file_command() {
        let line = "load stack lex";
        assert_eq!(
            complete(line, line.len(), &[]),
            Completion::File { start: 11 }
        );
        assert_eq!(complete("regex a", 7, &[]), Completion::None);
    }
}
