// Exercises the XFST command interpreter: parse a small xfst script via
// nfst-xfst and run it through the ported XfstCompiler command methods (regex
// compilation via the embedded XreCompiler, then a stack op), checking the
// resulting transducer stack.
use hfst::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use hfst::xfst_compiler::XfstCompiler;

const SRC: &str = "\
regex a:b ;
regex c:d ;
union net
";

fn main() {
    let mut c = XfstCompiler::new_with_impl(TROPICAL_OPENFST_TYPE);
    c.parse(SRC);

    let stack = c.get_stack();
    assert!(!stack.is_empty(), "xfst script left an empty stack");
    let top = *stack.last().unwrap();
    assert!(!top.is_null(), "top of stack is null");
    let t = unsafe { &*top };
    let states = t.number_of_states();
    assert!(states >= 1, "expected a non-empty transducer on the stack");

    println!(
        "xfst OK: regex a:b / regex c:d / union net -> stack[{}], top {} states",
        stack.len(),
        states
    );
}
