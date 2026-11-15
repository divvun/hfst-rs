// compile_depth KIND DEPTH STACK_KIB: compiles a source of KIND, DEPTH
// levels deep, on a thread of STACK_KIB KiB of stack, and prints `ok` or
// `fail`. A stack the compile overflows aborts the process instead.
//
// hfst::depth's limits come from the stack a level of each construct costs
// on the target, found by running this at growing depths on a fixed stack
// until it aborts. Built for another target, such as aarch64-apple-ios-sim,
// it runs there: `xcrun simctl spawn booted <binary> KIND DEPTH STACK_KIB`.
// The depth limits must be past the depths measured, so they do not stop a
// source before the stack would.
use hfst::lexc::LexcCompiler;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::twolc::TwolcCompiler;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

fn twolc_rule(context: &str) -> String {
    format!("Alphabet a b ;\nRules\n\"r\"\na:b <=> {context} _ ;\n")
}

/// Whether a source of `kind`, `depth` levels deep, compiles.
fn compiles(kind: &str, depth: usize) -> bool {
    let brackets = || format!("{}a{}", "[".repeat(depth), "]".repeat(depth));
    let chain = || vec!["a"; depth].join(" | ");
    match kind {
        // A parser's and an evaluator's descent into a group.
        "xre-brackets" => XreCompiler::<StdVectorFst>::new()
            .compile(&brackets())
            .is_some(),
        "pmatch-brackets" => PmatchCompiler::<StdVectorFst>::new()
            .compile(&format!("Define TOP {} ;", brackets()))
            .is_ok(),
        "twolc-brackets" => TwolcCompiler::<StdVectorFst>::new()
            .compile(&twolc_rule(&brackets()))
            .is_some(),
        // An evaluation level: a postfix operator on another.
        "xre-operators" => XreCompiler::<StdVectorFst>::new()
            .compile(&format!("a{}", ".i".repeat(depth)))
            .is_some(),
        // A chain the evaluator recurses down.
        "pmatch-chain" => PmatchCompiler::<StdVectorFst>::new()
            .compile(&format!("Define TOP {} ;", chain()))
            .is_ok(),
        "twolc-chain" => TwolcCompiler::<StdVectorFst>::new()
            .compile(&twolc_rule(&chain()))
            .is_some(),
        // A chain XRE evaluates in a loop, so only dropping its tree
        // recurses: parsed and dropped alone, as evaluating a long union
        // takes minutes.
        "xre-chain" => nfst_xre::parse(&chain()).is_ok(),
        // An acyclic path, which the tropical minimizer recurses along.
        "minimize" => XreCompiler::<StdVectorFst>::new()
            .compile(&format!("{{{}}}", "a".repeat(depth)))
            .is_some(),
        // Lexicons, each continuing to the next.
        "lexc-chain" => {
            let mut source = String::from("LEXICON Root\nL0 ;\n");
            for i in 0..depth {
                source.push_str(&format!("LEXICON L{i}\na L{} ;\n", i + 1));
            }
            source.push_str(&format!("LEXICON L{depth}\n# ;\n"));
            LexcCompiler::<StdVectorFst>::new()
                .compile(&source)
                .is_some()
        }
        _ => panic!("no kind {kind}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, kind, depth, kib] = &args[..] else {
        panic!("usage: compile_depth KIND DEPTH STACK_KIB");
    };
    let kind = kind.clone();
    let depth: usize = depth.parse().expect("a depth");
    let kib: usize = kib.parse().expect("a stack size in KiB");
    let compiled = std::thread::Builder::new()
        .stack_size(kib * 1024)
        .spawn(move || compiles(&kind, depth))
        .expect("a thread")
        .join()
        .expect("no panic");
    println!("{}", if compiled { "ok" } else { "fail" });
}
