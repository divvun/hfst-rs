// Every compiler fails with an error on input past the depth limits, rather
// than overflowing the stack and aborting. Each case runs on the stack a
// compile is given (hfst::depth::on_compile_stack), as the tools and
// hfst-embed run them.
use std::sync::Arc;

use hfst::depth::{
    COMPILE_STACK_SIZE, MAX_EXPRESSION_DEPTH, MAX_NESTING_DEPTH, MAX_TREE_DEPTH, on_compile_stack,
};
use hfst::diag::Diagnostic;
use hfst::file_resolver::MemoryFiles;
use hfst::lexc::LexcCompiler;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::twolc::TwolcCompiler;
use hfst::xfst_compiler::XfstCompiler;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

fn brackets(n: usize) -> String {
    format!("{}a{}", "[".repeat(n), "]".repeat(n))
}

fn messages(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(Diagnostic::message)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Compile `src` as a regular expression, returning whether it compiled and
/// what it said.
fn xre(src: String) -> (bool, String) {
    on_compile_stack(move || {
        let mut c = XreCompiler::<StdVectorFst>::new();
        let compiled = c.compile(&src).is_some();
        (compiled, messages(&c.take_diagnostics()))
    })
}

fn pmatch(src: String) -> Result<(), String> {
    on_compile_stack(move || {
        let mut c = PmatchCompiler::<StdVectorFst>::new();
        let compiled = c.compile(&src);
        let said = messages(&c.take_diagnostics());
        match compiled {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{e}; {said}")),
        }
    })
}

fn twolc(src: String) -> (bool, String) {
    on_compile_stack(move || {
        let mut c = TwolcCompiler::<StdVectorFst>::new();
        let compiled = c.compile(&src).is_some();
        (compiled, messages(&c.take_diagnostics()))
    })
}

fn lexc(src: String) -> (bool, String) {
    on_compile_stack(move || {
        let mut c = LexcCompiler::<StdVectorFst>::new();
        let compiled = c.compile(&src).is_some();
        (compiled, messages(&c.take_diagnostics()))
    })
}

fn xfst(src: String) -> Result<(), String> {
    on_compile_stack(move || {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        c.parse(&src).map(|_| ()).map_err(|e| e.to_string())
    })
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn xre_nesting_past_the_limit_fails_as_before() {
    assert!(xre(brackets(MAX_NESTING_DEPTH)).0);
    let (compiled, said) = xre(brackets(MAX_NESTING_DEPTH + 1));
    assert!(!compiled);
    assert_eq!(
        said, "",
        "nesting fails as an ordinary parse failure, as before"
    );
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn xre_operators_past_the_limit_fail() {
    for src in [
        format!("{}a", "~".repeat(MAX_EXPRESSION_DEPTH + 1)),
        format!("a{}", ".i".repeat(MAX_EXPRESSION_DEPTH + 1)),
    ] {
        let (compiled, said) = xre(src);
        assert!(!compiled);
        assert!(said.contains("operators are nested more than"), "{said}");
    }
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn xre_chains_are_evaluated_without_recursing() {
    // On a thread of 2 MiB, which a recursion down a chain of 3000 would
    // overflow and the drop of one does not.
    let chain = vec!["a"; 3000].join(" .o. ");
    let compiled = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || XreCompiler::<StdVectorFst>::new().compile(&chain).is_some())
        .expect("a thread")
        .join()
        .expect("no panic");
    assert!(compiled);
    let (compiled, said) = xre(vec!["a"; MAX_TREE_DEPTH + 1].join(" | "));
    assert!(!compiled);
    assert!(said.contains("chains more than"), "{said}");
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn a_regex_file_that_reads_itself_fails() {
    let (compiled, said) = on_compile_stack(|| {
        let files = MemoryFiles::default();
        files.insert("self.re", "@re\"self.re\" ;");
        let mut c = XreCompiler::<StdVectorFst>::new();
        c.set_file_resolver(Arc::new(files));
        let compiled = c.compile("@re\"self.re\" ;").is_some();
        (compiled, messages(&c.take_diagnostics()))
    });
    assert!(!compiled);
    assert!(
        said.contains("'@re' of self.re is nested more than"),
        "{said}"
    );
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn function_calls_count_towards_the_limit() {
    // Each function calls the one before inside brackets, so none calls
    // itself and each is shallow, but the calls nest deeper than evaluation
    // may go.
    let depth = 200;
    let calls = MAX_EXPRESSION_DEPTH / depth + 1;
    let mut script = String::from("define f0(x) x ;\n");
    for i in 1..=calls {
        let body = brackets(depth).replace('a', &format!("f{}(x)", i - 1));
        script.push_str(&format!("define f{i}(x) {body} ;\n"));
    }
    script.push_str(&format!("regex f{calls}(a) ;\n"));
    // The limits are measured on an optimized build, and this is the one
    // case that descends all the way to one. An unoptimized build's frames
    // are 14 to 17 times as large for these calls, so there
    // it is given a stack in proportion.
    let failed = if cfg!(debug_assertions) {
        std::thread::Builder::new()
            .stack_size(4 * COMPILE_STACK_SIZE)
            .spawn(move || {
                let mut c = XfstCompiler::<StdVectorFst>::new();
                c.parse(&script).map(|_| ()).map_err(|e| e.to_string())
            })
            .expect("a thread")
            .join()
            .expect("no panic")
    } else {
        xfst(script)
    }
    .expect_err("the calls nest too deep");
    assert!(
        failed.contains("operators are nested more than"),
        "{failed}"
    );
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_past_the_limits_fails() {
    assert_eq!(
        pmatch(format!("Define TOP {} ;", brackets(MAX_NESTING_DEPTH))),
        Ok(())
    );
    let failed = pmatch(format!("Define TOP {} ;", brackets(MAX_NESTING_DEPTH + 1)))
        .expect_err("brackets nest too deep");
    assert!(failed.contains("brackets are nested more than"), "{failed}");
    let failed = pmatch(format!(
        "Define TOP {} ;",
        vec!["a"; MAX_EXPRESSION_DEPTH + 1].join(" | ")
    ))
    .expect_err("a chain pmatch recurses down is too deep");
    assert!(
        failed.contains("operators are nested more than"),
        "{failed}"
    );
    let failed = pmatch(format!(
        "Define TOP {} ;",
        vec!["a"; MAX_EXPRESSION_DEPTH + 1].join(":")
    ))
    .expect_err("pairs nest too deep");
    assert!(
        failed.contains("operators are nested more than"),
        "{failed}"
    );
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_definitions_count_towards_the_limit() {
    // Each definition is shallow, but evaluating the last goes through all
    // of them, each a level deeper than its body.
    let depth = MAX_NESTING_DEPTH / 2;
    let defined = MAX_EXPRESSION_DEPTH / (depth + 2) + 1;
    let mut script = format!("Define A0 {} ;\n", brackets(depth));
    for i in 1..=defined {
        script.push_str(&format!(
            "Define A{i} {} ;\n",
            brackets(depth).replace('a', &format!("A{}", i - 1))
        ));
    }
    script.push_str(&format!("Define TOP A{defined} ;\n"));
    let failed = pmatch(script).expect_err("the definitions nest too deep");
    assert!(failed.contains("nests operators more than"), "{failed}");
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn pmatch_regex_files_reading_themselves_fail() {
    let failed = on_compile_stack(|| {
        let files = MemoryFiles::default();
        files.insert("self.re", "@re\"self.re\" ;");
        let mut c = PmatchCompiler::<StdVectorFst>::new();
        c.set_file_resolver(Arc::new(files));
        let compiled = c.compile("Define TOP @re\"self.re\" ;");
        let said = messages(&c.take_diagnostics());
        compiled.err().map(|e| format!("{e}; {said}"))
    })
    .expect("a file that reads itself fails");
    assert!(
        failed.contains("'@re' of self.re is nested more than"),
        "{failed}"
    );
}

fn rule(regex: &str) -> String {
    format!("Alphabet a b ;\nRules\n\"r\"\na:b <=> {regex} _ ;\n")
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn twolc_past_the_limits_fails() {
    let (compiled, said) = twolc(rule(&brackets(MAX_NESTING_DEPTH)));
    assert!(compiled, "{said}");
    let (compiled, said) = twolc(rule(&brackets(MAX_NESTING_DEPTH + 1)));
    assert!(!compiled);
    assert!(said.contains("brackets are nested more than"), "{said}");
    for regex in [
        format!("{}a", "~".repeat(MAX_EXPRESSION_DEPTH + 1)),
        vec!["a"; MAX_EXPRESSION_DEPTH + 1].join(" | "),
    ] {
        let (compiled, said) = twolc(rule(&regex));
        assert!(!compiled);
        assert!(said.contains("operators are nested more than"), "{said}");
    }
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn lexc_regular_expressions_past_the_limits_fail() {
    let (compiled, said) = lexc(format!(
        "LEXICON Root\n< {} > # ;\n",
        brackets(MAX_NESTING_DEPTH + 1)
    ));
    assert!(!compiled);
    assert!(said.contains("brackets are nested more than"), "{said}");
    let (compiled, said) = lexc(format!(
        "Definitions\nD = {}a ;\nLEXICON Root\n< D > # ;\n",
        "~".repeat(MAX_EXPRESSION_DEPTH + 1)
    ));
    assert!(!compiled);
    assert!(said.contains("operators are nested more than"), "{said}");
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn xfst_regular_expressions_past_the_limits_fail() {
    assert_eq!(
        xfst(format!("regex {} ;\n", brackets(MAX_NESTING_DEPTH))),
        Ok(())
    );
    let failed = xfst(format!("regex {} ;\n", brackets(MAX_NESTING_DEPTH + 1)))
        .expect_err("brackets nest too deep");
    assert!(failed.contains("brackets are nested more than"), "{failed}");
    let failed = xfst(format!(
        "define D {}a ;\n",
        "~".repeat(MAX_EXPRESSION_DEPTH + 1)
    ))
    .expect_err("operators nest too deep");
    assert!(
        failed.contains("operators are nested more than"),
        "{failed}"
    );
}
