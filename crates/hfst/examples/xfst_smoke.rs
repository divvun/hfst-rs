// Exercises the XFST command interpreter: parse a small xfst script via
// nfst-xfst and run it through the ported XfstCompiler command methods (regex
// compilation via the embedded XreCompiler, then a stack op), checking the
// resulting transducer stack.
use hfst::xfst_compiler::XfstCompiler;
use hfst_openfst::StdVectorFst;

const SRC: &str = "\
regex a:b ;
regex c:d ;
union net
";

fn main() {
    let mut c = XfstCompiler::<StdVectorFst>::new_with_impl();
    c.parse(SRC);

    let stack = c.get_stack();
    assert!(!stack.is_empty(), "xfst script left an empty stack");
    let stack_len = stack.len();
    let top = *stack.last().unwrap();
    let states = c.net(top).number_of_states();
    assert!(states >= 1, "expected a non-empty transducer on the stack");

    println!(
        "xfst OK: regex a:b / regex c:d / union net -> stack[{}], top {} states",
        stack_len, states
    );
}
