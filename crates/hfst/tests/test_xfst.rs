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
