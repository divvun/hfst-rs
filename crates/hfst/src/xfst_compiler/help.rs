//! The help text. `help` and `apropos` look commands up in one table of the
//! commands this compiler accepts.

/// One command in the help: the names it answers to, the first being the one
/// shown, the arguments it takes, and what it does.
struct Entry {
    names: &'static [&'static str],
    args: &'static str,
    description: &'static str,
}

const fn entry(
    names: &'static [&'static str],
    args: &'static str,
    description: &'static str,
) -> Entry {
    Entry {
        names,
        args,
        description,
    }
}

// [spec:hfst:req:xfst-cmd.help]
// A command the compiler parses but fails as not supported, or accepts and
// then ignores, is left out: alias, print aliases, cleanup net, collect
// epsilon-loops, edit properties, print file-info, print label-maps, print
// sigma-word-tally, lookup-optimize and remove-optimization.
static COMMANDS: &[Entry] = &[
    entry(
        &["ambiguous upper", "ambiguous"],
        "",
        "replaces the top network with its input strings that have more than one path",
    ),
    entry(
        &["apply down", "down"],
        "<string>",
        "applies <string> down to the top network",
    ),
    entry(
        &["apply down", "down"],
        "",
        "applies each line that follows down to the top network, until end of input (Ctrl-D)",
    ),
    entry(
        &["apply up", "up"],
        "<string>",
        "applies <string> up to the top network",
    ),
    entry(
        &["apply up", "up"],
        "",
        "applies each line that follows up to the top network, until end of input (Ctrl-D)",
    ),
    entry(
        &["apply med", "med"],
        "<string>",
        "prints the strings of the top network closest to <string> by edit distance",
    ),
    entry(
        &["apply med", "med"],
        "",
        "runs apply med on each line that follows, until end of input (Ctrl-D)",
    ),
    entry(&["apropos"], "<word>", "searches the help for <word>"),
    entry(
        &["assert"],
        "<test>",
        "runs the test command <test> and fails when its answer is 0",
    ),
    entry(
        &["clear stack", "clear"],
        "",
        "removes every network from the stack",
    ),
    entry(
        &["compact sigma"],
        "",
        "removes the symbols of the top network that ? already covers",
    ),
    entry(
        &["compile-replace lower", "com-rep lower"],
        "",
        "compiles the regular expressions on the lower side of the top network",
    ),
    entry(
        &["compile-replace upper", "com-rep upper"],
        "",
        "compiles the regular expressions on the upper side of the top network",
    ),
    entry(
        &["complete net", "complete"],
        "",
        "makes the top network complete",
    ),
    entry(
        &["compose net", "compose"],
        "",
        "composes the networks on the stack into one",
    ),
    entry(
        &["concatenate net", "concatenate"],
        "",
        "concatenates the networks on the stack into one",
    ),
    entry(
        &["crossproduct net", "crossproduct"],
        "",
        "replaces the top two automata with their cross product",
    ),
    entry(
        &["define"],
        "<name> <regex> ;",
        "defines <name> as the network of <regex>",
    ),
    entry(
        &["define"],
        "<name>(<params>) <regex> ;",
        "defines the regex function <name>",
    ),
    entry(
        &["define"],
        "<name>",
        "pops the top network and defines <name> as it",
    ),
    entry(
        &[
            "determinize net",
            "determinise net",
            "determinize",
            "determinise",
        ],
        "",
        "determinizes the top network",
    ),
    entry(&["echo"], "<text>", "prints <text>"),
    entry(
        &["eliminate flag"],
        "<name>",
        "removes the flag diacritics of feature <name> from the top network, keeping what they allowed",
    ),
    entry(
        &["eliminate flags"],
        "",
        "removes every flag diacritic from the top network, keeping what they allowed",
    ),
    entry(
        &["epsilon-remove net", "epsilon-remove"],
        "",
        "removes the epsilon arcs of the top network",
    ),
    entry(
        &["extract ambiguous"],
        "",
        "keeps the paths of the top network whose input strings have more than one path",
    ),
    entry(
        &["extract unambiguous"],
        "",
        "keeps the paths of the top network whose input strings have exactly one path",
    ),
    entry(&["help", "describe"], "", "lists every command"),
    entry(
        &["help", "describe"],
        "<name>",
        "prints the help for the commands named or starting with <name>",
    ),
    entry(
        &["ignore net", "ignore"],
        "",
        "lets the second network's strings occur anywhere in the top network's",
    ),
    entry(
        &["inspect net", "inspect"],
        "",
        "walks the top network interactively",
    ),
    entry(
        &["intersect net", "intersect", "conjunct"],
        "",
        "intersects the networks on the stack",
    ),
    entry(
        &["invert net", "invert"],
        "",
        "swaps the sides of the top network",
    ),
    entry(
        &["label net"],
        "",
        "replaces the top network with one accepting each of its symbol pairs",
    ),
    entry(
        &["list"],
        "<name> <symbols>",
        "defines the list <name> of <symbols>",
    ),
    entry(
        &["list"],
        "<name> <a>-<b>",
        "defines the list <name> of every character from <a> to <b>",
    ),
    entry(
        &["load defined", "loadd"],
        "<filename>",
        "defines the networks in a binary file under their names",
    ),
    entry(
        &["load stack", "load"],
        "<filename>",
        "pushes the networks in a binary file on the stack",
    ),
    entry(
        &["lower-side net", "lower-side"],
        "",
        "replaces the top network with its lower side",
    ),
    entry(
        &["minimize net", "minimize", "minimise"],
        "",
        "minimizes the top network",
    ),
    entry(
        &["minus net", "minus", "subtract"],
        "",
        "subtracts the second network from the top network",
    ),
    entry(
        &["name net", "name"],
        "<string>",
        "names the top network <string>",
    ),
    entry(
        &["negate net", "negate"],
        "",
        "complements the top automaton",
    ),
    entry(
        &["one-plus net", "one-plus"],
        "",
        "replaces the top network with its Kleene plus",
    ),
    entry(
        &["pop stack", "pop"],
        "",
        "removes the top network from the stack",
    ),
    entry(
        &["print arc-tally", "arc-tally"],
        "",
        "prints the number of arcs in the top network",
    ),
    entry(
        &["print defined", "pdefined"],
        "",
        "prints the defined networks and functions",
    ),
    entry(
        &["print directory", "directory"],
        "",
        "prints the files in the working directory",
    ),
    entry(
        &["print flags", "flags"],
        "",
        "prints the flag diacritics of the top network",
    ),
    entry(
        &["print labels", "labels"],
        "(<name>)",
        "prints the symbol pairs of the top network, or of the defined network <name>",
    ),
    entry(
        &["print label-tally", "label-tally"],
        "",
        "prints how many arcs each symbol pair of the top network labels",
    ),
    entry(&["print lists", "print list"], "", "prints every list"),
    entry(
        &["print longest-string", "longest-string", "pls"],
        "",
        "prints the longest string of the top network",
    ),
    entry(
        &["print longest-string-size", "longest-string-size", "plz"],
        "",
        "prints the length of the longest string of the top network",
    ),
    entry(
        &["print lower-words", "lower-words"],
        "(<count>)",
        "prints the strings of the lower side of the top network",
    ),
    entry(
        &["print name", "pname"],
        "",
        "prints the name of the top network",
    ),
    entry(
        &["print net"],
        "",
        "prints the states and arcs of the top network",
    ),
    entry(
        &["print props", "write properties"],
        "",
        "prints the properties of the top network",
    ),
    entry(
        &["print random-lower", "random-lower"],
        "(<count>)",
        "prints random strings of the lower side of the top network, 15 by default",
    ),
    entry(
        &["print random-upper", "random-upper"],
        "(<count>)",
        "prints random strings of the upper side of the top network, 15 by default",
    ),
    entry(
        &["print random-words", "random-words"],
        "(<count>)",
        "prints random paths of the top network, 15 by default",
    ),
    entry(
        &["print shortest-string", "shortest-string", "pss"],
        "",
        "prints the shortest string of the top network",
    ),
    entry(
        &[
            "print shortest-string-size",
            "print shortest-string-length",
            "shortest-string-size",
            "psz",
        ],
        "",
        "prints the length of the shortest string of the top network",
    ),
    entry(
        &["print sigma", "sigma"],
        "",
        "prints the alphabet of the top network",
    ),
    entry(
        &["print sigma-tally", "sigma-tally", "sitally"],
        "",
        "prints how many arcs each symbol of the top network labels",
    ),
    entry(
        &["print size", "size"],
        "",
        "prints the state and arc counts of the top network",
    ),
    entry(
        &["print stack", "stack"],
        "",
        "prints the size of every network on the stack",
    ),
    entry(
        &["print upper-words", "upper-words"],
        "(<count>)",
        "prints the strings of the upper side of the top network",
    ),
    entry(
        &["print words", "words"],
        "(<count>)",
        "prints the paths of the top network",
    ),
    entry(
        &["prune net", "prune"],
        "",
        "removes the states of the top network that lead to no final state",
    ),
    entry(
        &["push defined", "push"],
        "(<name>)",
        "pushes the defined network <name>, or every defined network",
    ),
    entry(
        &[
            "quit",
            "exit",
            "bye",
            "stop",
            "hyvästi",
            "au revoir",
            "näkemiin",
            "viszlát",
            "auf wiedersehen",
            "has",
        ],
        "",
        "ends the session",
    ),
    entry(
        &["read att", "att"],
        "<filename>",
        "reads a network in AT&T format and pushes it",
    ),
    entry(
        &["read lexc", "lexc"],
        "<filename>",
        "compiles a lexc file and pushes the result",
    ),
    entry(
        &["read prolog", "rpl"],
        "<filename>",
        "reads a network in Prolog format and pushes it",
    ),
    entry(
        &["read properties", "rprops"],
        "<filename>",
        "sets properties of the top network from 'name: value' lines",
    ),
    entry(
        &["read regex", "regex"],
        "<regex> ;",
        "compiles <regex> and pushes it",
    ),
    entry(
        &["read spaced-text", "rs"],
        "<filename>",
        "pushes a network of the lines in a file, symbols separated by spaces",
    ),
    entry(
        &["read text", "rt"],
        "<filename>",
        "pushes an automaton of the lines in a file, one symbol per character",
    ),
    entry(
        &["add properties", "add"],
        "<name>: <value>",
        "sets a property of the top network",
    ),
    entry(&["reverse net", "reverse"], "", "reverses the top network"),
    entry(
        &["rotate stack", "rotate"],
        "",
        "moves the top network to the bottom of the stack",
    ),
    entry(
        &["save defined", "saved", "write definitions", "wdefs"],
        "<filename>",
        "saves every defined network to a binary file",
    ),
    entry(
        &["save stack", "save", "ss"],
        "<filename>",
        "saves the stack to a binary file",
    ),
    entry(
        &["set"],
        "<variable> <value>",
        "sets a variable; show variables lists them",
    ),
    entry(
        &["show variable", "show"],
        "<name>",
        "prints the value of the variable <name>",
    ),
    entry(
        &["show variables"],
        "",
        "prints every variable, its value and what it does",
    ),
    entry(
        &["shuffle net", "shuffle"],
        "",
        "replaces the top two automata with their shuffle",
    ),
    entry(
        &["sigma net"],
        "",
        "replaces the top network with one accepting each symbol of its alphabet",
    ),
    entry(
        &["sort net", "sort"],
        "",
        "sorts the arcs of each state of the top network",
    ),
    entry(&["source"], "<filename>", "runs the commands in a file"),
    entry(
        &["substitute defined"],
        "<name> for <label>",
        "replaces each <label> arc of the top network with the defined network <name>",
    ),
    entry(
        &["substitute label"],
        "<labels> for <label>",
        "replaces each <label> arc of the top network with arcs of <labels>",
    ),
    entry(
        &["substitute symbol"],
        "<symbols> for <symbol>",
        "replaces <symbol> in the top network with <symbols>",
    ),
    entry(
        &["substring net", "substring"],
        "",
        "replaces the top network with every substring of its paths",
    ),
    entry(&["system"], "<command>", "runs <command> in the shell"),
    entry(
        &["test equivalent", "equivalent", "te"],
        "",
        "prints 1 if the top two networks are equivalent, else 0",
    ),
    entry(
        &["test functional", "functional", "tf"],
        "",
        "prints 1 if no input of the top network has two outputs, else 0",
    ),
    entry(
        &["test identity", "identity", "ti"],
        "",
        "prints 1 if the top network maps each string only to itself, else 0",
    ),
    entry(
        &[
            "test infinitely-ambiguous",
            "test infinitely ambiguous",
            "infinitely-ambiguous",
            "tia",
        ],
        "",
        "prints 1 if some input of the top network has infinitely many paths, else 0",
    ),
    entry(
        &["test lower-bounded", "lower-bounded", "tlb"],
        "",
        "prints 1 if the lower side of the top network has no epsilon cycle, else 0",
    ),
    entry(
        &["test lower-universal", "lower-universal", "tlu"],
        "",
        "prints 1 if the lower side of the top network accepts every string, else 0",
    ),
    entry(
        &["test non-null", "tnn"],
        "",
        "prints 1 if the top network accepts some string, else 0",
    ),
    entry(
        &["test null", "tnu"],
        "",
        "prints 1 if the top network accepts no string, else 0",
    ),
    entry(
        &["test overlap", "overlap", "to"],
        "",
        "prints 1 if the top two networks share a string, else 0",
    ),
    entry(
        &["test sublanguage", "sublanguage", "ts"],
        "",
        "prints 1 if the top network's language is within the second's, else 0",
    ),
    entry(
        &["test unambiguous", "tu"],
        "",
        "prints 1 if no input of the top network has two paths, else 0",
    ),
    entry(
        &["test upper-bounded", "upper-bounded", "tub"],
        "",
        "prints 1 if the upper side of the top network has no epsilon cycle, else 0",
    ),
    entry(
        &["test upper-universal", "upper-universal", "tuu"],
        "",
        "prints 1 if the upper side of the top network accepts every string, else 0",
    ),
    entry(&["turn stack", "turn"], "", "turns the stack upside down"),
    entry(
        &["twosided flag-diacritics", "tfd"],
        "",
        "makes every flag diacritic arc of the top network an identity pair",
    ),
    entry(
        &["undefine"],
        "<names>",
        "removes the definitions of <names>",
    ),
    entry(
        &["union net", "union", "disjunct"],
        "",
        "unions the networks on the stack",
    ),
    entry(&["unlist"], "<name>", "removes the list <name>"),
    entry(
        &["upper-side net", "upper-side"],
        "",
        "replaces the top network with its upper side",
    ),
    entry(
        &["view net", "view"],
        "",
        "draws the top network with Graphviz and opens the picture",
    ),
    entry(
        &["write att"],
        "(<filename>)",
        "writes the top network in AT&T format, to standard output by default",
    ),
    entry(
        &["write definition", "wdef"],
        "<name> > <filename>",
        "saves the defined network <name> to a binary file",
    ),
    entry(
        &["write dot", "wdot", "dot"],
        "<filename>",
        "writes the top network in Graphviz dot format",
    ),
    entry(
        &["write prolog", "wpl"],
        "<filename>",
        "writes the top network in Prolog format",
    ),
    entry(
        &["write spaced-text", "wspaced-text"],
        "<filename>",
        "writes the paths of the top network, symbols separated by spaces",
    ),
    entry(
        &["write text", "wt"],
        "<filename>",
        "writes the strings of the upper side of the top network, one per line",
    ),
    entry(
        &["zero-plus net", "zero-plus"],
        "",
        "replaces the top network with its Kleene star",
    ),
];

// [spec:hfst:req:xfst-cmd.shell-completion]
/// Every name a command goes by, for completing at the prompt.
pub(super) fn command_names() -> impl Iterator<Item = &'static str> {
    COMMANDS
        .iter()
        .flat_map(|entry| entry.names.iter().copied())
}

/// Whether command `name` takes a file name.
pub(super) fn takes_file(name: &str) -> bool {
    COMMANDS
        .iter()
        .any(|entry| entry.names.contains(&name) && entry.args.contains("<filename>"))
}

// [spec:hfst:def:xfst-help-message.hfst.xfst.append-help-message-fn]
// [spec:hfst:sem:xfst-help-message.hfst.xfst.append-help-message-fn]
/// Append `entry` to `message`: its first name and arguments, padded to 30
/// columns, then its description, and its other names on the next line.
fn append_entry(entry: &Entry, message: &mut String) {
    const NAME_AND_ARGUMENTS_WIDTH: usize = 30;
    let (first, others) = entry
        .names
        .split_first()
        .expect("every help entry has a name");
    let head = format!("{} {}", first, entry.args);
    let padding = NAME_AND_ARGUMENTS_WIDTH
        .saturating_sub(head.chars().count())
        .max(1);
    message.push_str(&head);
    message.push_str(&" ".repeat(padding));
    message.push_str(entry.description);
    message.push('\n');
    if !others.is_empty() {
        message.push('(');
        message.push_str(&others.join(", "));
        message.push_str(")\n");
    }
}

/// The help for `text`: every command when `text` is empty, else each entry
/// with a name equal to `text` or starting with `text` and a space. `None`
/// when no entry matches.
pub(super) fn help_for(text: &str) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let named = |name: &&str| {
        name.strip_prefix(text.as_str())
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    };
    collect(|entry| text.is_empty() || entry.names.iter().any(named))
}

/// The entries whose names or description contain `word` as a whole word,
/// ignoring case. `apropos` with no word is the help for `apropos`.
pub(super) fn apropos(word: &str) -> Option<String> {
    let word = word.trim();
    if word.is_empty() {
        return help_for("apropos");
    }
    collect(|entry| {
        entry
            .names
            .iter()
            .any(|name| word_found_in_text(word, name))
            || word_found_in_text(word, entry.description)
    })
}

fn collect(matches: impl Fn(&Entry) -> bool) -> Option<String> {
    let mut message = String::new();
    for entry in COMMANDS.iter().filter(|entry| matches(entry)) {
        append_entry(entry, &mut message);
    }
    (!message.is_empty()).then_some(message)
}

// [spec:hfst:def:xfst-help-message.hfst.xfst.is-punctuation-char-fn]
// [spec:hfst:sem:xfst-help-message.hfst.xfst.is-punctuation-char-fn]
// [spec:hfst:def:xfst-compiler.hfst.xfst.allow-char-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.allow-char-fn]
/// Whether `c` may stand next to a word that `apropos` finds.
fn is_punctuation_char(c: u8) -> bool {
    b" \n\t.,;:?!-/'\"<>()|".contains(&c)
}

// [spec:hfst:def:xfst-help-message.hfst.xfst.word-found-in-text-fn]
// [spec:hfst:sem:xfst-help-message.hfst.xfst.word-found-in-text-fn]
// [spec:hfst:def:xfst-compiler.hfst.xfst.string-found-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.string-found-fn]
// [spec:hfst:def:xfst-help-message.hfst.xfst.to-upper-case-fn]
// [spec:hfst:sem:xfst-help-message.hfst.xfst.to-upper-case-fn]
// [spec:hfst:def:xfst-compiler.hfst.xfst.to-upper-case-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.to-upper-case-fn]
/// Whether `word` occurs in `text` with punctuation or an end of `text` on
/// both sides, ASCII case folded. Every occurrence counts, not just the
/// first.
fn word_found_in_text(word: &str, text: &str) -> bool {
    let word = word.to_ascii_uppercase();
    let text = text.to_ascii_uppercase();
    let bytes = text.as_bytes();
    text.match_indices(&word).any(|(start, _)| {
        let end = start + word.len();
        (start == 0 || is_punctuation_char(bytes[start - 1]))
            && (end == bytes.len() || is_punctuation_char(bytes[end]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:hfst:req:xfst-cmd.help/test]
    // Each name in the help is one command keyword to the parser, so the
    // help cannot name a command the shell would call unknown. A command
    // that takes a body lexes an empty one after the keyword.
    #[test]
    fn every_name_is_a_command_the_parser_knows() {
        for entry in COMMANDS {
            for name in entry.names {
                let tokens = nfst_xfst::tokenize(name)
                    .unwrap_or_else(|e| panic!("'{name}' does not lex: {e:?}"));
                let keyword = matches!(
                    tokens.first(),
                    Some((nfst_xfst::Token::Command(_), span)) if span.range == (0..name.len())
                );
                let nothing_after = tokens.iter().skip(1).all(|(_, span)| span.range.is_empty());
                assert!(
                    keyword && nothing_after,
                    "'{name}' is not one command keyword: {tokens:?}"
                );
            }
        }
    }

    // [spec:hfst:req:xfst-cmd.help/test]
    #[test]
    fn an_entry_shows_name_arguments_description_aliases() {
        assert_eq!(
            help_for("union").as_deref(),
            Some(
                "union net                     unions the networks on the stack\n\
                 (union, disjunct)\n"
            )
        );
    }

    // [spec:hfst:req:xfst-cmd.help/test]
    #[test]
    fn a_group_word_finds_every_command_it_starts() {
        let apply = help_for("apply").expect("apply has help");
        assert_eq!(apply.lines().filter(|l| l.starts_with("apply ")).count(), 6);
        let tests = help_for("test").expect("test has help");
        assert_eq!(tests.lines().filter(|l| l.starts_with("test ")).count(), 13);
        assert!(
            help_for("print")
                .expect("print has help")
                .contains("print words")
        );
    }

    // [spec:hfst:req:xfst-cmd.help/test]
    #[test]
    fn a_name_matches_whole_words_only() {
        assert_eq!(help_for("prin"), None);
        assert_eq!(
            help_for("apply  up")
                .expect("apply up has help")
                .lines()
                .count(),
            4
        );
    }

    // [spec:hfst:req:xfst-cmd.help/test]
    #[test]
    fn apropos_finds_whole_words_in_any_case() {
        let found = apropos("KLEENE").expect("Kleene is in the help");
        assert!(found.contains("one-plus net"));
        assert!(found.contains("zero-plus net"));
        // 'string' as a word, here after a hyphen in a name, but not inside
        // 'strings'.
        assert!(
            apropos("string")
                .expect("string is in the help")
                .contains("print longest-string ")
        );
        assert_eq!(apropos("stri"), None);
    }

    // [spec:hfst:req:xfst-cmd.help/test]
    #[test]
    fn apropos_alone_is_the_help_for_apropos() {
        assert_eq!(apropos(""), help_for("apropos"));
    }
}
