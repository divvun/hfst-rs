//! The compile entry points the tools and the compile surface share: AT&T
//! and Prolog text, twolc grammars, pmatch archives, and the compilers and
//! sessions the tools' options set up.

use super::lexc::{LexcOptions, LexcVerbosity};
use super::pmatch2fst::{
    ArchiveError, Definitions, Pmatch2FstOptions, compile_ruleset, has_archive, write_ruleset,
};
use super::test_support::read_all;
use super::twolc::{TwolcOptions, compile_twolc};
use super::txt2fst::{TextEvent, Txt2FstOptions, read_text};
use super::xfst::XfstOptions;
use crate::diag::Severity;
use crate::file_resolver::{FileResolver, MemoryFiles};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_output_stream::HfstOutputStream;
use crate::hfst_transducer::HfstTransducer;
use crate::xfst_compiler::XfstCompiler;
use hfst_openfst::StdVectorFst;

/// What reading `text` hands back: the names of the transducers read, the
/// negative weights with their lines, whether a negative epsilon cycle was
/// found, and the progress messages.
type Read = (Vec<String>, Vec<(f32, u32)>, bool, Vec<String>);

fn read(text: &str, options: &Txt2FstOptions) -> crate::error::Result<Read> {
    let mut read: Read = (Vec::new(), Vec::new(), false, Vec::new());
    read_text::<StdVectorFst>(&mut text.as_bytes(), "in.att", options, &mut |event| {
        match event {
            TextEvent::Progress(message) => read.3.push(message),
            TextEvent::NegativeWeight { weight, line } => read.1.push((weight, line)),
            TextEvent::NegativeEpsilonCycles => read.2 = true,
            TextEvent::Read(t) => read.0.push(t.get_name()),
        }
        Ok(())
    })?;
    Ok(read)
}

// [spec:hfst:sem:hfst-txt2fst.process-stream-fn/test]
// [spec:hfst:req:embed.tools-in-library/test]
// Each AT&T transducer is read and named after the source, its negative
// weights told with their lines; joined, they make one unnamed transducer.
#[test]
fn att_text_reads_as_txt2fst_reads_it() {
    let text = "0\t1\ta\tb\t-1.5\n1\t0\n--\n0\t1\tc\tc\n1\t-2\n";
    let (names, negatives, _, progress) = read(text, &Txt2FstOptions::default()).expect("reads");
    assert_eq!(names, ["text(in.att)", "text(in.att)"]);
    assert_eq!(negatives, [(-1.5, 1), (-2.0, 5)]);
    assert_eq!(
        progress,
        [
            "Reading transducer table...\n",
            "Reading transducer table 2...\n"
        ]
    );
    let quiet = Txt2FstOptions {
        warn_negative_weights: false,
        disjunct: true,
        ..Txt2FstOptions::default()
    };
    let (names, negatives, _, _) = read(text, &quiet).expect("reads");
    assert_eq!(names, [""]);
    assert!(negatives.is_empty());
    let cycles = Txt2FstOptions {
        check_negative_epsilon_cycles: true,
        ..Txt2FstOptions::default()
    };
    let (_, _, found, _) = read("0\t0\t@0@\t@0@\t-1\n0\t1\ta\ta\n1\n", &cycles).expect("reads");
    assert!(found);
    let (_, _, found, progress) = read("0\t1\ta\ta\n1\n", &cycles).expect("reads");
    assert!(!found);
    assert_eq!(
        progress.last().map(String::as_str),
        Some("No epsilon cycles with a negative weight detected...\n")
    );
    let prolog = Txt2FstOptions {
        prolog: true,
        ..Txt2FstOptions::default()
    };
    assert!(read("not prolog\n", &prolog).is_err());
}

// [spec:hfst:req:embed.compile-diagnostics/test]
// The AT&T reader tells of each negative weight with the line it is on.
#[test]
fn negative_att_weights_are_noted_with_their_lines() {
    let mut lines = 0;
    let mut noted = Vec::new();
    HfstBasicTransducer::read_in_att_format_noting(
        &mut "0\t1\ta\ta\n0\t2\tb\tb\t-3\n2\t-1\n".as_bytes(),
        "@0@",
        &mut lines,
        &mut |weight, line| noted.push((weight, line)),
    )
    .expect("reads");
    assert_eq!(noted, [(-3.0, 2), (-1.0, 3)]);
    assert_eq!(lines, 3);
}

fn memory_stream(files: &MemoryFiles, ty: ImplementationType) -> HfstOutputStream {
    HfstOutputStream::from_writer(files.create("out").expect("a file"), ty, true).expect("a stream")
}

// [spec:hfst:sem:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn/test]
// [spec:hfst:req:embed.tools-in-library/test]
// A grammar's rules are written in the stream's type, with its warnings;
// one that does not compile hands back why.
#[test]
fn twolc_grammars_compile_to_the_streams_type() {
    let grammar = "Alphabet a b ;\nSets\nVow = a ;\nRules\n\"R1\"\na:b <=> vow _ # ;\n";
    for ty in [
        ImplementationType::TROPICAL_OPENFST_TYPE,
        ImplementationType::FOMA_TYPE,
    ] {
        let files = MemoryFiles::default();
        let mut out = memory_stream(&files, ty);
        let warnings = compile_twolc(
            (grammar, "g.twolc"),
            &TwolcOptions::default(),
            &mut out,
            None,
        )
        .expect("compiles");
        out.close();
        assert!(warnings.iter().any(|d| d.severity() == Severity::Warning));
        assert_eq!(warnings[0].source_name(), "g.twolc");
        let rules = read_all(&files.get("out").expect("written"));
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].get_type(), ty);
    }
    let files = MemoryFiles::default();
    let mut out = memory_stream(&files, ImplementationType::TROPICAL_OPENFST_TYPE);
    let quiet = TwolcOptions {
        quiet: true,
        ..TwolcOptions::default()
    };
    let failed = compile_twolc(
        (
            "Alphabet a b ;\nRules\n\"R1\"\na:b <=> _ \"c ;\n",
            "g.twolc",
        ),
        &quiet,
        &mut out,
        None,
    )
    .expect_err("a lex error");
    assert!(failed.iter().any(|d| d.is_error()));
}

// [spec:hfst:sem:hfst-pmatch2fst.process-stream-fn/test]
// [spec:hfst:req:embed.tools-in-library/test]
// Only a ruleset with a TOP and a symbol makes an archive, and a source of
// one byte compiles to nothing.
#[test]
fn only_a_ruleset_with_top_makes_an_archive() -> crate::error::Result<()> {
    let mut definitions = Definitions::new();
    assert!(!has_archive(&definitions));
    definitions.insert("Word".to_string(), HfstTransducer::new_symbol("a")?);
    assert!(!has_archive(&definitions), "a ruleset with no TOP");
    definitions.insert("TOP".to_string(), HfstTransducer::new_symbol("b")?);
    assert!(has_archive(&definitions));

    let options = Pmatch2FstOptions::default();
    let mut compiler = options.compiler();
    assert!(matches!(
        compile_ruleset(&mut compiler, "\n"),
        Err(ArchiveError::Empty)
    ));
    let mut compiled = compile_ruleset(&mut compiler, "Define TOP {cat} ;\n").expect("compiles");
    let files = MemoryFiles::default();
    let mut out = memory_stream(&files, ImplementationType::HFST_OLW_TYPE);
    let mut progress = Vec::new();
    write_ruleset(&mut compiled, &mut out, true, &mut progress).expect("written");
    out.close();
    assert!(!progress.is_empty());
    assert_eq!(read_all(&files.get("out").expect("written")).len(), 1);
    assert_eq!(
        ArchiveError::Empty.to_string(),
        "Empty ruleset, nothing to write"
    );
    Ok(())
}

// [spec:hfst:req:embed.tools-in-library/test]
// The compilers the options set up warn as the options say.
#[test]
fn lexc_options_set_the_compilers_warnings() {
    let source = "LEXICON Root\ncat # ;\ndog Nowhere ;\n";
    let warned = LexcOptions {
        warn_missing_lexicons: true,
        warnings_as_errors: true,
        ..LexcOptions::default()
    };
    let mut strict = warned.compiler::<StdVectorFst>();
    assert!(
        strict
            .compile_named_sources(&[("a.lexc".into(), source.into())])
            .is_err()
    );
    let quiet = LexcOptions {
        verbosity: LexcVerbosity::Quiet,
        ..warned
    };
    let lexc = quiet.compiler::<StdVectorFst>();
    assert_eq!(lexc.get_verbosity(), 0);
    let mut lenient = LexcOptions::default().compiler::<StdVectorFst>();
    let t = lenient
        .compile_named_sources(&[("a.lexc".into(), source.into())])
        .expect("compiles");
    assert_eq!(t.get_name(), "lexc(a.lexc)");
}

// [spec:hfst:req:embed.tools-in-library/test]
// An xfst session set up with print-weight prints its prompts, as the tool
// leaves it, and says so before it has a writer.
#[test]
fn xfst_sessions_are_set_as_the_tool_does() {
    let weighted = XfstOptions {
        print_weight: true,
        restricted_mode: true,
        ..XfstOptions::default()
    };
    let mut session: XfstCompiler<StdVectorFst> = weighted.session().expect("a session");
    assert!(session.verbose_prompt);
    assert!(session.restricted_mode);
    assert_eq!(session.variables["print-weight"], "ON");
    let said = String::from_utf8(session.take_output()).expect("utf8");
    assert_eq!(said, "variable print-weight = ON\n");
    let files = std::sync::Arc::new(MemoryFiles::default());
    session.set_file_resolver(files.clone() as std::sync::Arc<dyn FileResolver>);
    assert!(session.parse("regex a ;\nsystem echo hi\n").is_err());
    let silent = XfstOptions {
        silent: true,
        ..XfstOptions::default()
    };
    let session: XfstCompiler<StdVectorFst> = silent.session().expect("a session");
    assert!(!session.verbose);
    let foma: XfstCompiler<crate::backend_foma::FomaTransducer> =
        XfstOptions::default().session().expect("a foma session");
    assert!(foma.verbose);
}
