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
