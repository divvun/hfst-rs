// Behavioral coverage for the XfstCompiler command interpreter and its
// transducer stack / definitions / names model. xfst_compiler.rs is otherwise
// only exercised by examples/xfst_smoke.rs; these tests lock the stack,
// binary-op, define-and-reference, and name/print_name *identity* behaviour so
// the raw-pointer -> Rc<RefCell> conversion (idiom1.parsers Task 12) is
// validated rather than blind.
use hfst::xfst_compiler::XfstCompiler;
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
    let mut on = XfstCompiler::<StdVectorFst>::new_with_impl();
    on.parse("set minimal ON\nregex [a b c | x b c] ;\n");
    assert_eq!((top_states(&on), top_arcs(&on)), (4, 4));

    let mut off = XfstCompiler::<StdVectorFst>::new_with_impl();
    off.parse("set minimal OFF\nregex [a b c | x b c] ;\n");
    assert_eq!((top_states(&off), top_arcs(&off)), (7, 6));
}

// Turning it back ON has to restore minimization, not latch OFF.
#[test]
fn minimal_on_restores_minimization_after_off() {
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.parse("set minimal OFF\nset minimal ON\nregex [a b c | x b c] ;\n");
    assert_eq!((top_states(&c), top_arcs(&c)), (4, 4));
}

// Regex compilation is one half of the reach; stack operations are the other.
#[test]
fn minimal_governs_stack_operations_as_well() {
    let script = "regex [a b c] ;\nregex [x b c] ;\nunion net\n";

    let mut off = XfstCompiler::<StdVectorFst>::new_with_impl();
    off.parse(&format!("set minimal OFF\n{script}"));
    assert_eq!((top_states(&off), top_arcs(&off)), (7, 6));

    let mut on = XfstCompiler::<StdVectorFst>::new_with_impl();
    on.parse(&format!("set minimal ON\n{script}"));
    assert_eq!((top_states(&on), top_arcs(&on)), (4, 4));
}

// `verbose` is the flag that gates the per-command size reports; upstream
// recorded the variable and never consulted it.
#[test]
fn set_verbose_reaches_the_verbosity_flag() {
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.set_verbosity(true);
    c.parse("set verbose OFF\n");
    assert!(!c.verbose);
    c.parse("set verbose ON\n");
    assert!(c.verbose);
}

#[test]
fn regex_pushes_and_union_combines() {
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.parse("regex a:b ;\nregex c:d ;\nunion net\n");
    // two pushes then a binary stack op -> a single combined transducer.
    assert_eq!(c.get_stack().len(), 1);
    assert!(top_states(&c) >= 1);
}

#[test]
fn name_then_print_name_finds_it() {
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.parse("regex a:b ;\n");
    assert_eq!(c.get_stack().len(), 1);
    // name_net aliases the stack-top transducer into names; print_name finds
    // it by identity. This is the path the conversion must preserve.
    c.name_net("foo");
    let mut buf: Vec<u8> = Vec::new();
    c.print_name(&mut buf);
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Name foo"), "print_name output was {out:?}");
}

#[test]
fn define_then_reference_pushes_definition() {
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.parse("define V [ a | b | c ] ;\n");
    // referencing the definition in a later regex pushes an equivalent net.
    c.parse("regex V ;\n");
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
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.parse("define foo a ;\ndefine bar [ a b ]* ;\n");
    let mut buf: Vec<u8> = Vec::new();
    c.print_defined(&mut buf);
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
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.parse("define Concat(x, y) x y ;\nregex Concat([ a | b ], c) ;\n");
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
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.parse("define Fn(x) x xy ;\nregex Fn(a) ;\n");
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
