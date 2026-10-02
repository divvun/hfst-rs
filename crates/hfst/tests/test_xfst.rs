// Behavioral coverage for the XfstCompiler command interpreter and its
// transducer stack / definitions / names model. xfst_compiler.rs is otherwise
// only exercised by examples/xfst_smoke.rs; these tests lock the stack,
// binary-op, define-and-reference, and name/print_name *identity* behaviour so
// the raw-pointer -> Rc<RefCell> conversion (idiom1.parsers Task 12) is
// validated rather than blind.
use hfst::xfst_compiler::{Flow, XfstCompiler};
use hfst_openfst::StdVectorFst;

// Number of states of the transducer on top of the stack.
fn top_states(c: &XfstCompiler<StdVectorFst>) -> u32 {
    let top = *c.get_stack().last().expect("empty stack");
    c.net(top).number_of_states()
}

// Number of arcs of the transducer on top of the stack.
fn top_arcs(c: &XfstCompiler<StdVectorFst>) -> u32 {
    let top = *c.get_stack().last().expect("empty stack");
    c.net(top).number_of_arcs()
}

// `set minimal` has to reach the operations, not merely the variable table:
// [a b c | x b c] determinizes to 7 states / 6 arcs and minimizes to 4 / 4.
// Both figures are what C++ hfst-xfst 3.17.1 prints for the same script.
#[test]
fn minimal_off_leaves_the_result_unminimized() {
    let mut on = XfstCompiler::<StdVectorFst>::new();
    on.parse("set minimal ON\nregex [a b c | x b c] ;\n")
        .expect("xfst script runs");
    assert_eq!((top_states(&on), top_arcs(&on)), (4, 4));

    let mut off = XfstCompiler::<StdVectorFst>::new();
    off.parse("set minimal OFF\nregex [a b c | x b c] ;\n")
        .expect("xfst script runs");
    assert_eq!((top_states(&off), top_arcs(&off)), (7, 6));
}

// Turning it back ON has to restore minimization, not latch OFF.
#[test]
fn minimal_on_restores_minimization_after_off() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("set minimal OFF\nset minimal ON\nregex [a b c | x b c] ;\n")
        .expect("xfst script runs");
    assert_eq!((top_states(&c), top_arcs(&c)), (4, 4));
}

// Regex compilation is one half of the reach; stack operations are the other.
#[test]
fn minimal_governs_stack_operations_as_well() {
    let script = "regex [a b c] ;\nregex [x b c] ;\nunion net\n";

    let mut off = XfstCompiler::<StdVectorFst>::new();
    off.parse(&format!("set minimal OFF\n{script}"))
        .expect("xfst script runs");
    assert_eq!((top_states(&off), top_arcs(&off)), (7, 6));

    let mut on = XfstCompiler::<StdVectorFst>::new();
    on.parse(&format!("set minimal ON\n{script}"))
        .expect("xfst script runs");
    assert_eq!((top_states(&on), top_arcs(&on)), (4, 4));
}

// `verbose` is the flag that gates the per-command size reports; upstream
// recorded the variable and never consulted it.
#[test]
fn set_verbose_reaches_the_verbosity_flag() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_verbosity(true);
    c.parse("set verbose OFF\n").expect("xfst script runs");
    assert!(!c.verbose);
    c.parse("set verbose ON\n").expect("xfst script runs");
    assert!(c.verbose);
}

#[test]
fn regex_pushes_and_union_combines() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex a:b ;\nregex c:d ;\nunion net\n")
        .expect("xfst script runs");
    // two pushes then a binary stack op -> a single combined transducer.
    assert_eq!(c.get_stack().len(), 1);
    assert!(top_states(&c) >= 1);
}

#[test]
fn name_then_print_name_finds_it() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex a:b ;\n").expect("xfst script runs");
    assert_eq!(c.get_stack().len(), 1);
    // name_net aliases the stack-top transducer into names; print_name finds
    // it by identity. This is the path the conversion must preserve.
    c.name_net("foo").expect("command runs");
    let mut buf: Vec<u8> = Vec::new();
    c.print_name(&mut buf).expect("command runs");
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Name foo"), "print_name output was {out:?}");
}

#[test]
fn define_then_reference_pushes_definition() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("define V [ a | b | c ] ;\n")
        .expect("xfst script runs");
    // referencing the definition in a later regex pushes an equivalent net.
    c.parse("regex V ;\n").expect("xfst script runs");
    assert!(!c.get_stack().is_empty());
    assert!(top_states(&c) >= 1);
}

// `define NAME <body>` must record the definition's source form, not just
// compile it: `print defined` reports out of original_definitions, so a
// dispatch arm that only calls define_transducer leaves every such definition
// invisible while still printing "Defined 'NAME'". Verified against C++
// hfst-xfst, which lists both names with their bodies.
#[test]
fn print_defined_lists_definitions_made_with_a_body() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("define foo a ;\ndefine bar [ a b ]* ;\n")
        .expect("xfst script runs");
    let mut buf: Vec<u8> = Vec::new();
    c.print_defined(&mut buf).expect("command runs");
    let out = String::from_utf8(buf).expect("print_defined emits UTF-8");
    assert!(
        !out.contains("No defined symbols."),
        "two definitions exist but print_defined reported none: {out:?}"
    );
    for name in ["foo", "bar"] {
        assert!(
            out.contains(name),
            "print_defined omitted '{name}': {out:?}"
        );
    }
}

// A function's parameters must be rewritten to the placeholder symbols that
// eval_function_call binds the arguments to. C++ found them via positions its
// flex/bison scanner recorded during compilation; nfst replaces that lexer, so
// the port's position set was always empty and the body went through
// unchanged — every argument silently failed to substitute, and a compound
// argument compiled as a bare symbol (`Concat([a|b], c)` lost the union).
// Expectations verified against C++ hfst-xfst 3.17.1.
#[test]
fn function_arguments_substitute_including_compound_ones() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("define Concat(x, y) x y ;\nregex Concat([ a | b ], c) ;\n")
        .expect("xfst script runs");
    assert_eq!(c.get_stack().len(), 1);
    // [a|b] c: 3 states, 3 arcs. Substitution failure yielded 2 arcs.
    let top = *c.get_stack().last().expect("one net on the stack");
    assert_eq!(
        c.net(top).number_of_arcs(),
        3,
        "compound function argument lost material in substitution"
    );
}

// A parameter is a whole NAMETOKEN: `x` must not be substituted inside `xy`.
#[test]
fn function_argument_substitution_respects_token_boundaries() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("define Fn(x) x xy ;\nregex Fn(a) ;\n")
        .expect("xfst script runs");
    assert_eq!(c.get_stack().len(), 1);
    // a xy — two arcs, the second being the untouched symbol `xy`.
    let top = *c.get_stack().last().expect("one net on the stack");
    assert_eq!(c.net(top).number_of_arcs(), 2);
}

// ---------------------------------------------------------------------------
// Diagnostics. xfst is where a user meets this compiler, so a failure has to
// name a position in their script and, where the cause is a known xfst trap,
// what to type instead. The rendering goes to stderr; what is asserted here is
// the shaping that feeds it — the span, the wording, and the advice.
// ---------------------------------------------------------------------------

// 1-based line number a byte offset falls on.
fn line_of(src: &str, offset: usize) -> usize {
    src[..offset].matches('\n').count() + 1
}

// Every diagnostic a failing script produces, in source order.
fn diagnose(src: &str) -> Vec<hfst::xfst_compiler::XfstDiagnostic> {
    match nfst_xfst::parse(src) {
        Ok(_) => Vec::new(),
        Err(e) => hfst::xfst_compiler::parse_diagnostics(src, &e),
    }
}

// `set copyright-owner "Acme Corp"` is the canonical xfst trap: a NAMETOKEN
// ends at the first space or quote, so the value has to be %-escaped. Upstream
// rejects the whole line with no position and no reason.
#[test]
fn quoted_value_points_at_the_quote() {
    let src = "set copyright-owner \"Acme Corp\"\n";
    let ds = diagnose(src);
    let first = ds.first().expect("the quote is rejected");
    assert_eq!(&src[first.span.clone()], "\"");
    assert!(
        first.notes.iter().any(|n| n.contains('%')),
        "no escaping advice in {:?}",
        first.notes
    );
}

// A regex with no `regex` in front used to be blamed on its quotes, with the
// false claim that xfst has no quoted strings. Quotes are fine in a regex; the
// missing command is the fault, and it is reported once, not once per quote.
#[test]
fn bare_quoted_regex_asks_for_the_regex_command() {
    let src = "\"+Err/Orth\":0 || _ .#. ;\n";
    let ds = diagnose(src);
    assert_eq!(ds.len(), 1, "one report per line, got {:?}", ds);
    assert_eq!(&src[ds[0].span.clone()], src.trim_end());
    assert!(
        ds[0].notes.iter().any(|n| n.contains("'regex'")),
        "no advice to add 'regex' in {:?}",
        ds[0].notes
    );
    assert!(diagnose("regex \"+Err/Orth\":0 ;\n").is_empty());
}

// A quoted percent sign is the one-character string `%`. It must not escape
// the closing quote and swallow the next command into the regex.
#[test]
fn quoted_percent_does_not_swallow_the_next_define() {
    assert!(diagnose("define A \"%\";\ndefine B b;\nregex A;\n").is_empty());
}

// A mistyped command names itself and the command it was probably meant to be.
#[test]
fn mistyped_command_suggests_the_real_one() {
    let src = "regex a ;\ndetrminize net ;\n";
    let ds = diagnose(src);
    let first = ds.first().expect("the typo is rejected");
    assert_eq!(&src[first.span.clone()], "detrminize");
    assert_eq!(first.message, "unknown command 'detrminize'");
    assert!(
        first.notes.iter().any(|n| n.contains("determinize")),
        "no suggestion in {:?}",
        first.notes
    );
}

// A regex body is parsed as a standalone string by the front end, so its error
// spans count from the body rather than the script. They have to be rebased or
// the caret lands on unrelated text — here, line 4 of the script.
#[test]
fn regex_body_error_is_anchored_in_the_script() {
    let src = "regex a ;\nregex b ;\n\ndefine Broken [ a | b ;\n";
    let ds = diagnose(src);
    let first = ds.first().expect("the unclosed bracket is rejected");
    assert!(
        first.span.start >= src.find("Broken").expect("body is on line 4"),
        "span {:?} points before the offending line",
        first.span
    );
    assert_eq!(line_of(src, first.span.start), 4);
    // The Rust token name the regex parser reports is spelled as the character
    // the user did not type.
    assert!(
        first.message.contains("']'"),
        "token name left unspelled in {:?}",
        first.message
    );
}

// The whole point of retaining the script: a failure late in a long file must
// report its own line, not the file's.
#[test]
fn late_failure_reports_its_own_line() {
    let mut src = String::new();
    for _ in 0..239 {
        src.push_str("regex a ;\n");
    }
    src.push_str("bogus command\n");
    let ds = diagnose(&src);
    let first = ds.first().expect("the unknown command is rejected");
    assert_eq!(line_of(&src, first.span.start), 240);
}

// Notes are advice, not noise: a stray character that is not a quoting mistake
// gets no lecture about escaping.
#[test]
fn ordinary_stray_character_gets_no_advice() {
    let ds = diagnose("regex a ;\n\u{7}\n");
    let first = ds.first().expect("the stray byte is rejected");
    assert!(first.notes.is_empty(), "unwanted advice {:?}", first.notes);
}

// A script that parses produces no diagnostics at all.
#[test]
fn a_valid_script_produces_no_diagnostics() {
    assert!(diagnose("define V [ a | e ] ;\nregex V ;\nprint size\n").is_empty());
}

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
