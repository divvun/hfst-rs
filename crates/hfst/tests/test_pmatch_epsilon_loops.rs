// Regression coverage for the pmatch-runtime epsilon-cycle termination fix
// (upstream hfst/hfst#399). The compiled pmatch net for tokenizer grammars can
// contain an epsilon-INPUT arc whose output is UNKNOWN (`?`) under Kleene star
// (e.g. `0:?*`, in a context or at top level). The C++ engine — and the
// faithful Rust port before this fix — recurses over that arc forever (100% CPU,
// non-terminating): take_epsilons re-enters get_analyses without advancing the
// input position, and the UNKNOWN output fans out over the whole sigma at each
// step, so the search tree grows exponentially in width. hfst#399 is still open
// upstream; the Rust port DELIBERATELY DIVERGES by TERMINATING via a per-search
// visited memo over plain-epsilon configurations (see pmatch.rs, PmatchContainer
// ::epsilon_path).
//
// Every grammar here is inline. The tokenizer's `morphology` net is the
// investigation's `ja.hfst` — the union [«:«] | [»:»] | [ja:ja+V] — which is
// built at test runtime from ATT, written to a binary HFST file under
// std::env::temp_dir(), and referenced via `@bin`. That is exactly how the CLI
// (`hfst-pmatch`) loads it, so the regression-lock outputs below are
// byte-identical to the CLI ground truth.
//
// The 10s-per-test nextest cap is the point: before the fix, variants A/D/G/F
// spin at 100% CPU and would trip the cap (FAIL). Each hanging-variant test
// wraps its match in a bounded worker thread as a belt-and-braces guard so a
// regression surfaces as a clear assertion failure rather than a wall-clock
// timeout.
use std::time::Duration;

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

// ATT for the investigation's ja.hfst: [«:«] | [»:»] | [ja:ja+V].
const JA_ATT: &str = "\
0\t1\t«\t@0@\t0.0
0\t2\t»\t@0@\t0.0
0\t3\tj\tj\t0.0
1\t6\t@0@\t«\t0.0
2\t6\t@0@\t»\t0.0
3\t4\ta\ta\t0.0
4\t5\t@0@\t+\t0.0
5\t6\t@0@\tV\t0.0
6\t0.0
";

// Build ja.hfst under temp_dir and return its `@bin` path. A per-caller unique
// suffix keeps parallel tests from racing on the same file.
fn write_ja(tag: &str) -> Result<String, hfst::error::Error> {
    let path = std::env::temp_dir().join(format!("i399_ja_{tag}.hfst"));
    let path_str = path.to_str().expect("temp path is valid UTF-8").to_string();
    let mut reader = std::io::BufReader::new(JA_ATT.as_bytes());
    let mut ja =
        HfstTransducer::<StdVectorFst>::read_in_att_format_file(&mut reader, "@0@", false)?;
    let mut out =
        HfstOutputStream::new_filename(&path_str, ImplementationType::TROPICAL_OPENFST_TYPE, true)?;
    out.write(&mut ja)?;
    out.close();
    Ok(path_str)
}

// Compile `src` to TOP, build a runtime container, and return the match output.
fn compile_and_match(src: &str, input: &str) -> Result<String, hfst::error::Error> {
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let defs = compiler.compile(src)?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    let top_owned = HfstTransducer::<Transducer<WeightedTables>>::new_from_basic(&top.to_basic()?)?;
    let mut container = PmatchContainer::new_from_hfst_transducers(vec![top_owned])?;
    Ok(container.do_match(input, 0.0, 0.0))
}

// Run a match on a worker thread with a hard wall-clock bound. Before the #399
// fix the hanging variants never return, so the join would block past the
// nextest cap; this converts "never terminates" into a deterministic panic well
// under the 10s cap.
fn match_within(src: String, input: &str, bound: Duration) -> String {
    let input = input.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let out = compile_and_match(&src, &input).expect("pmatch match failed");
        // If the receiver already gave up (timed out) this send just fails; the
        // thread then exits on its own.
        let _ = tx.send(out);
    });
    match rx.recv_timeout(bound) {
        Ok(out) => {
            handle.join().expect("pmatch worker thread panicked");
            out
        }
        Err(_) => panic!(
            "pmatch match did not terminate within {bound:?} — hfst#399 epsilon loop regressed"
        ),
    }
}

// A generous bound that is still far under the 10s nextest cap: a terminating
// match completes in well under a second, while the pre-fix loop never returns.
fn bound() -> Duration {
    Duration::from_secs(5)
}

// Grammar builders mirroring the investigation's variants (ja.hfst read via
// @bin at `path`). `set need-separators off` and the LC/RC contexts are exactly
// as compiled by the CLI, so behavior matches `hfst-pmatch`.
fn g_work(path: &str) -> String {
    format!(
        "set need-separators off\n\
Define morphology @bin\"{path}\" ;\n\
Define incondform      {{«}}|{{»}} ;\n\
Define blank           Whitespace | incondform ;\n\
Define incondword       morphology & [ incondform ] ;\n\
Define morphoword       morphology                   LC([blank | #]) RC([blank | # ]);\n\
Define TOP [ morphoword | incondword ] EndTag(token);\n"
    )
}

fn g_a(path: &str) -> String {
    format!(
        "set need-separators off\n\
Define morphology @bin\"{path}\" ;\n\
Define incondform      {{«}}|{{»}} ;\n\
Define blank           Whitespace | [ incondform:?* 0:?* ] ;\n\
Define incondword       morphology & [ incondform ] ;\n\
Define morphoword       morphology                   LC([blank | #]) RC([blank | # ]);\n\
Define TOP [ morphoword | incondword ] EndTag(token);\n"
    )
}

fn g_c(path: &str) -> String {
    format!(
        "set need-separators off\n\
Define morphology @bin\"{path}\" ;\n\
Define incondform      {{«}}|{{»}} ;\n\
Define blank           Whitespace | [ incondform:?* ] ;\n\
Define incondword       morphology & [ incondform ] ;\n\
Define morphoword       morphology                   LC([blank | #]) RC([blank | # ]);\n\
Define TOP [ morphoword | incondword ] EndTag(token);\n"
    )
}

fn g_d(path: &str) -> String {
    format!(
        "set need-separators off\n\
Define morphology @bin\"{path}\" ;\n\
Define blank           Whitespace | [ 0:?* ] ;\n\
Define morphoword       morphology                   LC([blank | #]) RC([blank | # ]);\n\
Define TOP [ morphoword ] EndTag(token);\n"
    )
}

fn g_f(path: &str) -> String {
    format!(
        "set need-separators off\n\
Define morphology @bin\"{path}\" ;\n\
Define blank           Whitespace | [ 0:{{x}}* ] ;\n\
Define morphoword       morphology LC([blank | #]) RC([blank | # ]);\n\
Define TOP [ morphoword ] EndTag(token);\n"
    )
}

fn g_g(path: &str) -> String {
    format!(
        "set need-separators off\n\
Define morphology @bin\"{path}\" ;\n\
Define blank           Whitespace | [ 0:?* ] ;\n\
Define TOP [ morphology | blank ] EndTag(token);\n"
    )
}

// ---- Regression locks: variants that already terminated must be unchanged. ----

// The `work` grammar (no `0:?*`) treats « and » as blanks flanking the
// morphoword `ja`, so the whole `«ja»` yields a single token with the delimiters
// kept literal. This is byte-identical to `hfst-pmatch work.pmc`.
#[test]
fn work_variant_single_token_unchanged() -> Result<(), hfst::error::Error> {
    let path = write_ja("work")?;
    let out = match_within(g_work(&path), "«ja»", bound());
    assert_eq!(out, "«<token>ja+V</token>»", "work variant output changed");
    Ok(())
}

// The `C` grammar (`incondform:?*`, no `0:?*`) splits `«ja»` into three tokens:
// « , ja+V , » . Byte-identical to `hfst-pmatch C.pmc`.
#[test]
fn c_variant_three_token_split_unchanged() -> Result<(), hfst::error::Error> {
    let path = write_ja("c")?;
    let out = match_within(g_c(&path), "«ja»", bound());
    assert_eq!(
        out, "<token>«</token><token>ja+V</token><token>»</token>",
        "C variant three-token split changed"
    );
    Ok(())
}

// ---- Termination locks: the previously-hanging variants now complete. ----

// A: `blank = Whitespace | [ incondform:?* 0:?* ]` — the `0:?*` epsilon-input /
// unknown-output star inside a context is the #399 core. Pre-fix: never
// terminates. Post-fix: terminates with a plausible tokenization of `«ja»`.
#[test]
fn a_variant_terminates() -> Result<(), hfst::error::Error> {
    let path = write_ja("a")?;
    let out = match_within(g_a(&path), "«ja»", bound());
    assert!(
        out.contains("ja+V"),
        "A variant should still analyse `ja`, got {out:?}"
    );
    Ok(())
}

// D: `blank = Whitespace | [ 0:?* ]` — the bare `0:?*` in a context. Same core.
#[test]
fn d_variant_terminates() -> Result<(), hfst::error::Error> {
    let path = write_ja("d")?;
    let out = match_within(g_d(&path), "«ja»", bound());
    assert!(
        out.contains("ja+V"),
        "D variant should still analyse `ja`, got {out:?}"
    );
    Ok(())
}

// G: `TOP = [ morphology | blank ] EndTag(token)` with `blank = ... 0:?*` — the
// epsilon/unknown star reachable at TOP level, not only inside a context.
#[test]
fn g_variant_terminates() -> Result<(), hfst::error::Error> {
    let path = write_ja("g")?;
    let out = match_within(g_g(&path), "«ja»", bound());
    assert!(
        out.contains("ja+V"),
        "G variant should still analyse `ja`, got {out:?}"
    );
    Ok(())
}

// F: `blank = Whitespace | [ 0:{x}* ]` — an epsilon-input star with a CONCRETE
// output (`x`), reachable through the same context path as D. It must terminate
// with a plausible tokenization of `«ja»` (the three ja.hfst mappings).
#[test]
fn f_variant_terminates() -> Result<(), hfst::error::Error> {
    let path = write_ja("f")?;
    let out = match_within(g_f(&path), "«ja»", bound());
    assert!(
        out.contains("ja+V"),
        "F variant should still analyse `ja`, got {out:?}"
    );
    Ok(())
}
