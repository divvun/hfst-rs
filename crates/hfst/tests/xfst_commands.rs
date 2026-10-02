// The xfst command contract: errors as values, 'source', real output from
// the print and write commands, the readers, and the ambiguity, substring and
// edit-distance commands (docs/spec/xfst-commands.md).
use hfst::xfst_compiler::{Flow, XfstCompiler};
use hfst_openfst::StdVectorFst;

// [spec:hfst:req:xfst-cmd.errors-are-values/test]
// A failing command stops the script by default, and its error comes back as
// a value pointing at that command; nothing after it runs.
#[test]
fn a_failed_command_stops_the_script() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let src = "regex a ;\npop stack\npop stack\nregex b ;\n";
    let err = c.parse(src).expect_err("popping an empty stack fails");
    let d = err.diagnostics.first().expect("the failure is described");
    assert_eq!(&src[d.span.clone()], "pop stack");
    assert!(d.message.contains("empty stack"), "{}", d.message);
    assert!(c.get_stack().is_empty(), "'regex b' ran after the failure");
}

// [spec:hfst:req:xfst-cmd.errors-are-values/test]
// With quit-on-fail OFF the failure is reported and the script goes on.
#[test]
fn quit_on_fail_off_keeps_going() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let flow = c
        .parse("set quit-on-fail OFF\npop stack\nregex b ;\n")
        .expect("the script runs to its end");
    assert_eq!(flow, Flow::Continue);
    assert_eq!(c.get_stack().len(), 1);
}

// [spec:hfst:req:xfst-cmd.errors-are-values/test]
// 'quit' is a flow value, not a flag: the commands after it do not run.
#[test]
fn quit_ends_the_run() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let flow = c
        .parse("regex a ;\nquit\nregex b ;\n")
        .expect("quit is not a failure");
    assert_eq!(flow, Flow::Quit);
    assert_eq!(c.get_stack().len(), 1);
}

// [spec:hfst:req:xfst-cmd.io-errors/test]
// A file that cannot be read is an error naming it, not an empty input.
#[test]
fn an_unreadable_file_names_the_path() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let err = c
        .parse("read lexc /nonexistent/x.lexc\n")
        .expect_err("a missing lexc file fails");
    let message = &err.diagnostics[0].message;
    assert!(message.contains("/nonexistent/x.lexc"), "{message}");
}

// [spec:hfst:req:xfst-cmd.io-errors/test]
// A file that cannot be created is an error, not a skipped write.
#[test]
fn an_unwritable_file_names_the_path() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let err = c
        .parse("regex a ;\nprint words > /nonexistent/out.txt\n")
        .expect_err("writing into a missing directory fails");
    let message = &err.diagnostics[0].message;
    assert!(message.contains("/nonexistent/out.txt"), "{message}");
}

// 'rotate stack' moves the top network to the bottom; it does not reverse.
#[test]
fn rotate_moves_the_top_to_the_bottom() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex a ;\nregex b b ;\nregex c c c ;\nrotate stack\n")
        .expect("xfst script runs");
    let states: Vec<u32> = c
        .get_stack()
        .iter()
        .map(|&id| c.net(id).number_of_states())
        .collect();
    assert_eq!(states, vec![4, 2, 3]);
}

// Universality compares one side with ?*, not with a single ?.
#[test]
fn upper_universal_compares_with_sigma_star() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex ?* ;\nassert test upper-universal\n")
        .expect("?* is upper-universal");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex ? ;\nassert test upper-universal\n")
        .expect_err("a single ? is not");
}

// A scratch directory for the 'source' tests, unique to one test.
fn source_dir(test: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("hfst-xfst-source-{}-{}", test, std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

// [spec:hfst:req:xfst-cmd.source/test]
// 'source' shares the caller's compiler state: what it defines and pushes is
// visible afterwards, and what was defined before is visible inside it.
#[test]
fn source_shares_the_session() {
    let dir = source_dir("shares");
    let inner = dir.join("inner.xfst");
    std::fs::write(&inner, "regex A b ;\ndefine C c ;\n").expect("write script");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&format!(
        "define A a ;\nsource {}\nregex C ;\n",
        inner.display()
    ))
    .expect("the sourced script runs");
    assert_eq!(c.get_stack().len(), 2);
    let first = c.get_stack()[0];
    assert_eq!(
        c.net(first).number_of_states(),
        3,
        "A b is two symbols long"
    );
}

// [spec:hfst:req:xfst-cmd.source/test]
// A failure inside the sourced file fails 'source', and the calling script
// stops there.
#[test]
fn sourced_failure_stops_the_caller() {
    let dir = source_dir("fails");
    let inner = dir.join("inner.xfst");
    std::fs::write(&inner, "pop stack\n").expect("write script");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let err = c
        .parse(&format!("source {}\nregex a ;\n", inner.display()))
        .expect_err("the sourced failure propagates");
    assert!(err.diagnostics[0].message.contains("empty stack"));
    assert!(c.get_stack().is_empty());
}

// [spec:hfst:req:xfst-cmd.source/test]
// 'quit' inside a sourced file ends the whole session.
#[test]
fn quit_in_a_sourced_file_ends_the_session() {
    let dir = source_dir("quit");
    let inner = dir.join("inner.xfst");
    std::fs::write(&inner, "regex a ;\nquit\n").expect("write script");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let flow = c
        .parse(&format!("source {}\nregex b ;\n", inner.display()))
        .expect("quit is not a failure");
    assert_eq!(flow, Flow::Quit);
    assert_eq!(c.get_stack().len(), 1);
}

// [spec:hfst:req:xfst-cmd.source/test]
// A script that sources itself fails at the nesting limit.
#[test]
fn a_self_sourcing_script_fails() {
    let dir = source_dir("self");
    let inner = dir.join("loop.xfst");
    std::fs::write(&inner, format!("source {}\n", inner.display())).expect("write script");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let err = c
        .parse(&format!("source {}\n", inner.display()))
        .expect_err("the recursion is cut off");
    assert!(err.diagnostics[0].message.contains("nested"), "{:?}", err);
}

// [spec:hfst:req:xfst-cmd.source/test]
// A missing file is an error naming it.
#[test]
fn sourcing_a_missing_file_names_it() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let err = c
        .parse("source /nonexistent/script.xfst\n")
        .expect_err("a missing file fails");
    assert!(
        err.diagnostics[0]
            .message
            .contains("/nonexistent/script.xfst")
    );
}

// Whether the rule on the top of the stack maps `input` to exactly `expected`.
fn rewrites(rule: &str, input: &str, expected: &str) -> bool {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&format!(
        "regex [{{{input}}} .o. [{rule}]].l ;\nregex {{{expected}}} ;\nassert test equivalent\n"
    ))
    .is_ok()
}

// [spec:hfst:sem:xfst-cmd.replace-context-symbol/test]
// '?' in a context is one symbol of the string, never the word edge.
#[test]
fn context_any_symbol_is_not_the_word_edge() {
    assert!(rewrites("i -> u || ? _", "i", "i"));
    assert!(rewrites("i -> u || ? _", "ai", "au"));
    assert!(rewrites("i -> u || _ ?", "i", "i"));
    assert!(rewrites("i -> u || _ ?", "ia", "ua"));
    assert!(rewrites("i -> u || \\[i] _", "ii", "ii"));
    assert!(rewrites("i -> u || \\[i] _", "ai", "au"));
    assert!(rewrites("i -> u || .#. _", "i", "u"));
}

// Run `script` and return what its last command wrote to `out`.
fn printed(test: &str, script: &str) -> String {
    let dir = source_dir(test);
    let out = dir.join("out.txt");
    let script = script.replace("OUT", &out.display().to_string());
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&script).expect("xfst script runs");
    std::fs::read_to_string(&out).expect("output written")
}

// [spec:hfst:sem:xfst-cmd.print-counts/test]
// The size printers count; they never print '?'.
#[test]
fn size_printers_count() {
    let src = "regex c a t ;\nregex d o g s ;\n";
    assert_eq!(
        printed("size", &format!("{src}print size > OUT\n")),
        "5 states, 4 arcs\n"
    );
    assert_eq!(
        printed("stack", &format!("{src}print stack > OUT\n")),
        "0: 4 states, 3 arcs\n1: 5 states, 4 arcs\n"
    );
    assert_eq!(
        printed("arcs", &format!("{src}print arc-tally > OUT\n")),
        "4\n"
    );
}

// [spec:hfst:sem:xfst-cmd.print-counts/test]
// sigma-tally counts arcs per symbol on either side; flags lists the flag
// diacritics.
#[test]
fn sigma_tally_and_flags() {
    let src = "regex [c a t | c:d o g | \"@U.F.x@\" c] ;\n";
    let tally = printed("tally", &format!("{src}print sigma-tally > OUT\n"));
    assert!(tally.contains("c: 3\n"), "{tally}");
    assert!(tally.contains("d: 1\n"), "{tally}");
    assert_eq!(
        printed("flags", &format!("{src}print flags > OUT\n")),
        "@U.F.x@\n"
    );
}

// [spec:hfst:sem:xfst-cmd.write-word-lists/test]
// write spaced-text round-trips any network through read spaced-text, with
// pairs and escapes.
#[test]
fn spaced_text_round_trips() {
    let dir = source_dir("spaced");
    let file = dir.join("words.txt");
    let net = "[c a t | c:d o g | {a:b} \"x y\"]";
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&format!(
        "regex {net} ;\nwrite spaced-text {}\n",
        file.display()
    ))
    .expect("written");
    c.parse(&format!("read spaced-text {}", file.display()))
        .expect("read back");
    c.parse("assert test equivalent\n")
        .expect("the written text reads back as the same network");
    let text = std::fs::read_to_string(&file).expect("written");
    assert!(text.contains("c:d o g\n"), "{text}");
    assert!(text.contains("x\\ y"), "{text}");
}

// [spec:hfst:sem:xfst-cmd.write-word-lists/test]
// write text lists the upper side and round-trips a single-character
// automaton; a cyclic network fails.
#[test]
fn text_round_trips_and_refuses_cycles() {
    let dir = source_dir("text");
    let file = dir.join("words.txt");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&format!(
        "regex [c a t | d o g] ;\nwrite text {}\n",
        file.display()
    ))
    .expect("written");
    c.parse(&format!("read text {}", file.display()))
        .expect("read back");
    c.parse("assert test equivalent\n")
        .expect("the written text reads back as the same network");
    assert_eq!(
        std::fs::read_to_string(&file).expect("written"),
        "cat\ndog\n"
    );
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let err = c
        .parse(&format!("regex a+ ;\nwrite text {}\n", file.display()))
        .expect_err("a cyclic network has no finite word list");
    assert!(err.diagnostics[0].message.contains("cyclic"));
}

// [spec:hfst:req:xfst-cmd.no-placeholders/test]
// A command with no implementation fails and says so; it prints nothing.
#[test]
fn unsupported_commands_fail_by_name() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let err = c
        .parse("regex a ;\nprint label-maps\n")
        .expect_err("label-maps is not supported");
    assert!(
        err.diagnostics[0]
            .message
            .contains("'print label-maps' is not supported")
    );
}

// [spec:hfst:sem:xfst-cmd.read-word-lists/test]
// Inline words build the same network as the same words read from a file,
// blank lines skipped, pairs honoured.
#[test]
fn inline_and_file_word_lists_agree() {
    let dir = source_dir("wordlist");
    let file = dir.join("words.txt");
    std::fs::write(&file, "cat\n\ndog\nc:do\n").expect("write words");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&format!("read text {}", file.display()))
        .expect("file read");
    c.parse("read text\ncat\n\ndog\nc:do\n<ctrl-d>\n")
        .expect("inline read");
    c.parse("assert test equivalent\n").expect("same network");
    c.parse("regex [c a t | d o g | c:d o] ;\nassert test equivalent\n")
        .expect("and it is the word list");

    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("read spaced-text\nc a t\nd:t o g\n<ctrl-d>\n")
        .expect("inline spaced read");
    c.parse("regex [c a t | d:t o g] ;\nassert test equivalent\n")
        .expect("spaced words with a pair");
}

// [spec:hfst:sem:xfst-cmd.read-prolog/test]
// write prolog then read prolog restores the stack, top first.
#[test]
fn prolog_round_trips_the_stack() {
    let dir = source_dir("prolog");
    let file = dir.join("stack.pl");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&format!(
        "regex a b ;\nregex c:d e ;\nwrite prolog {}\nclear stack\nread prolog {}\n",
        file.display(),
        file.display()
    ))
    .expect("written and read back");
    c.parse("regex c:d e ;\nassert test equivalent\npop stack\npop stack\n")
        .expect("the top network comes back on top");
    c.parse("regex a b ;\nassert test equivalent\n")
        .expect("and the one under it below");
}

// [spec:hfst:sem:xfst-cmd.read-prolog/test]
// Text that is not prolog fails and says where.
#[test]
fn bad_prolog_names_the_line() {
    let dir = source_dir("badprolog");
    let file = dir.join("bad.pl");
    std::fs::write(&file, "network(x).\narc(x, 0, 1, \"a\").\nnonsense\n").expect("write");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    let err = c
        .parse(&format!("read prolog {}\n", file.display()))
        .expect_err("not prolog");
    assert!(err.diagnostics[0].message.contains("line 3"), "{:?}", err);
}

// [spec:hfst:sem:xfst-cmd.sort/test]
// sort net orders arcs by label and leaves the language alone.
#[test]
fn sort_net_orders_arcs_and_keeps_the_language() {
    let dir = source_dir("sort");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex [z | b | a | y] ;\nregex [z | b | a | y] ;\nsort net\nassert test equivalent\n")
        .expect("sorting keeps the language");
    let out = dir.join("net.txt");
    c.parse(&format!("print net > {}\n", out.display()))
        .expect("printed");
    let net = std::fs::read_to_string(&out).expect("written");
    let arcs = net.lines().find(|l| l.contains("->")).expect("an arc line");
    assert!(
        arcs.find('a') < arcs.find('b') && arcs.find('b') < arcs.find('y'),
        "{net}"
    );
}

// Whether `test` holds for the network `regex`.
fn holds(regex: &str, test: &str) -> bool {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&format!("regex {regex} ;\nassert test {test}\n"))
        .is_ok()
}

// [spec:hfst:sem:xfst-cmd.ambiguity/test]
#[test]
fn functional_and_unambiguous_tests() {
    assert!(holds("[a:b | c:d]", "functional"));
    assert!(!holds("[a:b | a:c]", "functional"));
    assert!(holds("[a | a b:0 | c]", "unambiguous"));
    assert!(!holds("[a 0:x | a]", "unambiguous"));
    // Two paths with the same output are still two paths.
    assert!(holds("[a:b | a 0:0 b:0]", "unambiguous"));
}

// [spec:hfst:sem:xfst-cmd.ambiguity/test]
// The extractors split a network by whether its inputs have one path.
#[test]
fn ambiguity_extractors_split_the_network() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(
        "regex [a 0:x | a | b] ;\nextract ambiguous\nregex [a 0:x | a] ;\nassert test equivalent\n",
    )
    .expect("the ambiguous part is the two paths for a");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex [a 0:x | a | b] ;\nextract unambiguous\nregex b ;\nassert test equivalent\n")
        .expect("the unambiguous part is b");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex [a 0:x | a | b] ;\nambiguous upper\nregex a ;\nassert test equivalent\n")
        .expect("a is the ambiguous input");
}

// [spec:hfst:sem:xfst-cmd.substring/test]
#[test]
fn substring_net_accepts_every_substring() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex c a t ;\nsubstring net\nregex [0 | c | a | t | c a | a t | c a t] ;\nassert test equivalent\n")
        .expect("every substring of cat, and nothing else");
}

// [spec:hfst:sem:xfst-cmd.apply-med/test]
// apply med finds the nearest strings, cheapest first, within the limit.
#[test]
fn med_matches_cheapest_first() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex [c a t | c o t | d o g] ;\napply med cst\n")
        .expect("'apply med WORD' on one line parses and runs");
    let matches = c.med_matches("cst").expect("searched");
    assert_eq!(&matches[..2], &[("cat".into(), 1), ("cot".into(), 1)]);
    assert!(matches.len() <= 3);
    c.parse("set med-cutoff 0\n").expect("set");
    assert!(c.med_matches("cst").expect("searched").is_empty());
}

// [spec:hfst:req:xfst-cmd.variables-take-effect/test]
// A variable that would change nothing cannot be set.
#[test]
fn inert_variables_cannot_be_set() {
    for name in ["recursive-define", "sort-arcs", "use-timer", "hopcroft-min"] {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        let err = c
            .parse(&format!("set {name} ON\n"))
            .expect_err("an inert variable is not settable");
        assert!(
            err.diagnostics[0].message.contains("no such variable"),
            "{name}"
        );
    }
}

// [spec:hfst:req:xfst-cmd.variables-take-effect/test]
// 'precision' sets the decimal places of printed weights, as in C++.
#[test]
fn precision_sets_weight_decimals() {
    let src = "set print-weight ON\nregex [a::1.23456789] ;\n";
    assert_eq!(
        printed("prec5", &format!("{src}print words > OUT\n")),
        "a\t1.23457\n"
    );
    assert_eq!(
        printed(
            "prec2",
            &format!("{src}set precision 2\nprint words > OUT\n")
        ),
        "a\t1.23\n"
    );
}

// [spec:hfst:sem:xfst-cmd.list-range/test]
// A range includes both ends and works beyond ASCII.
#[test]
fn list_ranges_are_inclusive_unicode() {
    let ascii = printed("range", "list V a-e ;\nprint lists > OUT\n");
    assert!(ascii.contains("a b c d e "), "{ascii}");
    let unicode = printed("urange", "list V á-ä ;\nprint lists > OUT\n");
    assert!(unicode.contains("á â ã ä "), "{unicode}");
}

// [spec:hfst:sem:xfst-cmd.list-range/test]
// Reversed or multi-character ends fail; a lone '-' is a symbol.
#[test]
fn bad_ranges_fail_and_hyphen_is_a_symbol() {
    for bad in ["list V e-a ;\n", "list V ab-c ;\n"] {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        c.parse(bad).expect_err(bad);
    }
    let hyphen = printed("hyphen", "list V - ;\nprint lists > OUT\n");
    assert!(hyphen.contains(" - "), "{hyphen}");
}

// [spec:hfst:sem:xfst-cmd.read-word-lists/test]
// 'read text FILE' reads the file and the commands after it still run.
#[test]
fn read_text_file_leaves_later_commands_alone() {
    let dir = source_dir("readfile");
    let file = dir.join("words.txt");
    std::fs::write(&file, "cat\ndog\n").expect("write words");
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(&format!(
        "read text {}\nregex [c a t | d o g] ;\nassert test equivalent\n",
        file.display()
    ))
    .expect("the file is read and the script goes on");
}

// [spec:hfst:sem:xfst-cmd.string-escapes/test]
// Escapes in quoted strings name characters; malformed ones fail.
#[test]
fn quoted_string_escapes() {
    for (quoted, plain) in [(r#""\x41""#, "A"), (r#""\101""#, "A"), (r#""é""#, "é")] {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        c.parse(&format!(
            "regex {quoted} ;\nregex {plain} ;\nassert test equivalent\n"
        ))
        .unwrap_or_else(|e| panic!("{quoted} is not {plain}: {e}"));
    }
    let tab = printed("tab", "regex \"a\\tb\" ;\nprint words > OUT\n");
    assert_eq!(tab, "a\tb\n");
    for bad in [r#""\x4""#, r#""\u12""#, r#""\x00""#] {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        c.parse(&format!("regex {bad} ;\n")).expect_err(bad);
    }
}
