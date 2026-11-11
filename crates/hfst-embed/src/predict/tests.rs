//! A small North Sámi generator, grammar and model, built in the test with
//! the library's compilers and model builder, written to bytes, and driven
//! through a session.

use std::sync::{Arc, Mutex, MutexGuard};

use hfst::file_resolver::{FileResolver, MemoryFiles};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::lexc::LexcCompiler;
use hfst::prediction::model::{Ambiguity, Backoff, ModelBuilder, ModelSettings, generator_accepts};
use hfst::prediction::units::AnalysisTokenizer;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

use super::*;

/// Three nouns, three endings of different weights, a question enclitic and
/// a compound join closed by `#`.
const LEXICON: &str = r#"
Multichar_Symbols
+N +Sg +Pl +Nom +Gen +Qst +Cmp

LEXICON Root
Nouns ;

LEXICON Nouns
girji:girj N ;
guolli:guol N ;
gáhkku:gáhkk N ;

LEXICON N
+N:0 NInfl ;
+N+Cmp:0 Join ;

LEXICON Join
%#:0 Nouns ;

LEXICON NInfl
+Sg+Nom:i End ;
+Sg+Gen:a End "weight: 3" ;
+Pl+Nom:at End "weight: 4" ;

LEXICON End
# ;
+Qst:go # ;
"#;

const GRAMMAR: &str = r##"
tags = ["#"]
boundary = ["#"]
enclitic = ["+Qst"]

[labels]
"+N+Sg+Gen" = "-a"

[[class]]
name = "noun"
patterns = ["+N"]
"##;

/// Six words. The units run guolli +N+Sg+Nom three times, then girji
/// +N+Pl+Nom, guolli +N+Sg+Gen, girji +N+Pl+Nom.
const TEXT: &str = "guoli\tguolli+N+Sg+Nom\t0.000000\n\n\
                    guoli\tguolli+N+Sg+Nom\t0.000000\n\n\
                    guoli\tguolli+N+Sg+Nom\t0.000000\n\n\
                    girjat\tgirji+N+Pl+Nom\t0.000000\n\n\
                    guola\tguolli+N+Sg+Gen\t0.000000\n\n\
                    girjat\tgirji+N+Pl+Nom\t0.000000\n\n";

/// Compiling writes the tropical backend's process-wide symbol table, so
/// tests that share a process take turns.
static COMPILING: Mutex<()> = Mutex::new(());

fn compiling() -> MutexGuard<'static, ()> {
    COMPILING.lock().unwrap_or_else(|e| e.into_inner())
}

/// The transducer in optimized lookup, after an HFST3 header.
fn written(compiled: &HfstTransducer<StdVectorFst>, weighted: bool) -> Vec<u8> {
    let ol = compiled.to_ol(weighted, "").expect("optimized lookup");
    let format = if weighted {
        ImplementationType::HFST_OLW_TYPE
    } else {
        ImplementationType::HFST_OL_TYPE
    };
    let files = MemoryFiles::default();
    let sink = files.create("transducer").expect("memory takes a file");
    let mut stream =
        HfstOutputStream::from_writer(sink, format, true).expect("an optimized-lookup stream");
    stream.write(&ol).expect("the transducer writes");
    stream.close();
    files.get("transducer").expect("the transducer was written")
}

fn lexc(weighted: bool) -> Vec<u8> {
    let _turn = compiling();
    let compiled = LexcCompiler::<StdVectorFst>::new()
        .compile(LEXICON)
        .expect("the lexicon compiles");
    written(&compiled, weighted)
}

fn regex(source: &str) -> Vec<u8> {
    let _turn = compiling();
    let compiled = XreCompiler::<StdVectorFst>::new()
        .compile(source)
        .expect("the expression compiles");
    written(&compiled, true)
}

/// A model of [`TEXT`] of order 3, built with `grammar` over the weighted
/// generator in `generator`.
fn model(generator: &[u8], grammar: &str) -> Vec<u8> {
    let read = read_transducer("the generator", generator, &crate::TEST_PACKAGE);
    let Ok(Tables::Weighted(generator)) = read else {
        panic!("the generator does not read as weighted");
    };
    let grammar = UnitGrammar::from_toml(grammar.as_bytes()).expect("the grammar reads");
    let tokenizer = AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table());
    let settings = ModelSettings {
        order: 3,
        backoff: Backoff::StupidBackoff,
        ambiguity: Ambiguity::FirstReading,
        face_size: 2,
        size_limit: usize::MAX,
    };
    let mut builder = ModelBuilder::new(
        &grammar,
        &tokenizer,
        generator_accepts(&generator),
        settings,
    )
    .expect("the settings are valid");
    let report = builder.add_text(TEXT).expect("the text reads");
    assert_eq!(report.counted, 6);
    builder.write().expect("the model writes")
}

struct Language {
    generator: Vec<u8>,
    model: Vec<u8>,
}

fn language() -> Language {
    let generator = lexc(true);
    let model = model(&generator, GRAMMAR);
    Language { generator, model }
}

fn predictor(language: &Language, options: PredictorOptions<'_>) -> Result<Predictor, Error> {
    Predictor::new(
        &language.generator,
        GRAMMAR.as_bytes(),
        language.model.clone(),
        options,
        &crate::TEST_PACKAGE,
    )
}

fn session(language: &Language, options: PredictorOptions<'_>) -> Session {
    Session::new(Arc::new(
        predictor(language, options).expect("the predictor builds"),
    ))
}

/// Each candidate as its key, kind, label and whether it completes the word.
fn shown(offer: &Offer) -> Vec<String> {
    offer
        .candidates
        .iter()
        .map(|c| {
            let kind = kind_name(c.kind);
            format!("{} {kind} {} {}", c.key, c.label, c.completes)
        })
        .collect()
}

fn keys(session: &mut Session) -> Vec<String> {
    let offer = session.candidates(None).expect("the units on offer");
    offer.candidates.into_iter().map(|c| c.key).collect()
}

fn context(session: &Session) -> Vec<&str> {
    session.context().collect()
}

fn code<T>(result: Result<T, Error>) -> &'static str {
    match result {
        Ok(_) => "built",
        Err(error) => error.code().name(),
    }
}

#[test]
fn a_word_is_typed_ranked_picked_and_committed() {
    let mut session = session(&language(), PredictorOptions::default());
    session.type_letters("g").unwrap();
    // The model saw guolli more often than girji, and gáhkku never.
    assert_eq!(keys(&mut session), ["guolli", "girji", "gáhkku"]);
    session.type_letters("i").unwrap();
    let offer = session.candidates(None).unwrap();
    assert_eq!(shown(&offer), ["girji stem girji false"]);
    assert!(offer.stem_next);
    session.pick("girji").unwrap();
    assert_eq!(session.typed(), "");
    let offer = session.candidates(Some(2)).unwrap();
    assert_eq!(
        shown(&offer),
        [
            "+N+Pl+Nom ending girjat true",
            "+N+Sg+Nom ending girji true"
        ]
    );
    assert_eq!(offer.total, 4);
    assert!(!offer.truncated && !offer.stem_next);
    let all = session.candidates(None).unwrap();
    assert_eq!(
        shown(&all)[2..],
        ["+N+Sg+Gen ending -a true", "+N+Cmp# boundary +N+Cmp# false"]
    );
    assert_eq!(session.preview().as_deref(), Some("girjat"));
    assert!(!session.is_complete());
    assert_eq!(session.word_class(), None);
    assert!(session.face().is_empty());
    session.pick("+N+Pl+Nom").unwrap();
    assert!(session.is_complete());
    assert_eq!(session.word_class(), Some("noun"));
    assert_eq!(session.face(), ["+N+Sg+Nom", "+N+Pl+Nom"]);
    assert_eq!(
        shown(&session.candidates(None).unwrap()),
        ["+Qst enclitic +Qst true"]
    );
    assert_eq!(session.preview().as_deref(), Some("girjat"));
    assert_eq!(session.commit().unwrap(), "girjat");
    assert_eq!(context(&session), ["girji", "+N+Pl+Nom"]);
    assert!(!session.is_complete());
    assert_eq!(session.preview(), None);
}

#[test]
fn the_words_before_rank_the_next_one() {
    let mut session = session(&language(), PredictorOptions::default());
    for (letters, units, word) in [
        ("gi", ["girji", "+N+Pl+Nom"], "girjat"),
        ("gu", ["guolli", "+N+Sg+Gen"], "guola"),
    ] {
        session.type_letters(letters).unwrap();
        session.pick(units[0]).unwrap();
        if word == "guola" {
            // After girjat guolli, the model saw +N+Sg+Gen, though after
            // guolli alone it saw +N+Sg+Nom more often.
            assert_eq!(
                keys(&mut session),
                ["+N+Sg+Gen", "+N+Sg+Nom", "+N+Pl+Nom", "+N+Cmp#"]
            );
            assert_eq!(session.preview().as_deref(), Some("guola"));
        }
        session.pick(units[1]).unwrap();
        assert_eq!(session.commit().unwrap(), word);
    }
    assert_eq!(context(&session), ["guolli", "+N+Sg+Gen"]);
}

#[test]
fn backspace_removes_a_letter_then_a_unit() {
    let mut session = session(&language(), PredictorOptions::default());
    assert!(!session.backspace());
    session.type_letters("gi").unwrap();
    assert!(session.backspace());
    assert_eq!(session.typed(), "g");
    assert_eq!(keys(&mut session), ["guolli", "girji", "gáhkku"]);
    session.type_letters("i").unwrap();
    session.pick("girji").unwrap();
    session.pick("+N+Cmp#").unwrap();
    assert_eq!(session.word_class(), Some("noun"));
    session.type_letters("gu").unwrap();
    assert_eq!(keys(&mut session), ["guolli"]);
    assert!(session.backspace() && session.backspace());
    assert_eq!(session.typed(), "");
    assert!(session.backspace());
    assert_eq!(session.word_class(), None);
    assert_eq!(keys(&mut session)[0], "+N+Pl+Nom");
    assert!(session.backspace());
    assert!(!session.backspace());
    assert!(keys(&mut session).is_empty());
}

#[test]
fn units_and_letters_must_be_on_offer() {
    let mut session = session(&language(), PredictorOptions::default());
    assert_eq!(code(session.pick("+N+Sg+Nom")), "invalid-argument");
    session.type_letters("girji").unwrap();
    assert_eq!(code(session.pick("guolli")), "invalid-argument");
    session.pick("girji").unwrap();
    let refused = session
        .type_letters("x")
        .expect_err("no stem starts after one");
    assert_eq!(refused.code(), Code::InvalidArgument);
    assert!(refused.message().contains("no stem can start"), "{refused}");
    assert_eq!(session.typed(), "");
    assert_eq!(code(session.commit()), "invalid-argument");
    session.pick("+N+Cmp#").unwrap();
    session.type_letters("g").unwrap();
    assert_eq!(code(session.commit()), "invalid-argument");
    assert!(context(&session).is_empty());
}

#[test]
fn script_converts_typed_letters_and_the_committed_word() {
    let relaxation = regex("[ a | a:á | g | h | i | j | k | l | o | r | u ]*");
    let lower = regex("[ A:a | G:g | H:h | I:i | J:j | K:k | L:l | O:o | R:r | U:u ]*");
    let upper = regex("[ a:A | á:Á | g:G | h:H | i:I | j:J | k:K | o:O | r:R | t:T | u:U ]*");
    let options = PredictorOptions {
        relaxation: Some(&relaxation),
        input_transliterator: Some(&lower),
        output_transliterator: Some(&upper),
        ..PredictorOptions::default()
    };
    let mut session = session(&language(), options);
    session.type_letters("GAH").unwrap();
    assert_eq!(keys(&mut session), ["gáhkku"]);
    session.pick("gáhkku").unwrap();
    let offer = session.candidates(None).unwrap();
    assert!(shown(&offer).contains(&"+N+Sg+Nom ending gáhkki true".to_owned()));
    session.pick("+N+Sg+Nom").unwrap();
    assert_eq!(session.preview().as_deref(), Some("gáhkki"));
    assert_eq!(session.commit().unwrap(), "GÁHKKI");
    session.type_letters("Q").unwrap();
    let offer = session.candidates(None).unwrap();
    assert!(offer.candidates.is_empty() && offer.stem_next);
}

#[test]
fn an_unweighted_generator_predicts_alike() {
    let language = Language {
        generator: lexc(false),
        model: language().model,
    };
    let predictor = predictor(&language, PredictorOptions::default()).unwrap();
    assert!(matches!(predictor.generator, Tables::Unweighted(_)));
    let mut session = Session::new(Arc::new(predictor));
    session.type_letters("g").unwrap();
    assert_eq!(keys(&mut session), ["guolli", "girji", "gáhkku"]);
    session.type_letters("i").unwrap();
    session.pick("girji").unwrap();
    assert_eq!(
        keys(&mut session),
        ["+N+Pl+Nom", "+N+Sg+Nom", "+N+Sg+Gen", "+N+Cmp#"]
    );
    session.pick("+N+Pl+Nom").unwrap();
    assert_eq!(session.commit().unwrap(), "girjat");
}

#[test]
fn mismatched_grammars_and_models_are_refused() {
    let language = language();
    let build = |grammar: &[u8], model: Vec<u8>| {
        Predictor::new(
            &language.generator,
            grammar,
            model,
            PredictorOptions::default(),
            &crate::TEST_PACKAGE,
        )
    };
    let refused = |grammar: &[u8], model: Vec<u8>| {
        let error = build(grammar, model)
            .err()
            .expect("the predictor is refused");
        (error.code().name(), error.into_message())
    };
    let ours = || language.model.clone();
    assert_eq!(code(build(GRAMMAR.as_bytes(), ours())), "built");
    let grammars: [&[u8]; 5] = [
        b"colour = \"red\"\n",
        b"tags = [\"ab\"]\n",
        b"\xff\xfe",
        b"boundary = [\"+Der/*\"]\n",
        b"[[filter]]\nname = \"x\"\nlabel = \"x\"\nflags = [\"@U.x.y@\"]\n",
    ];
    for grammar in grammars {
        let (code, message) = refused(grammar, ours());
        assert_eq!(code, "invalid-format", "{message}");
        assert!(message.contains("unit grammar"), "{message}");
    }
    let other = model(
        &language.generator,
        &format!("{GRAMMAR}# another grammar\n"),
    );
    let (code, message) = refused(GRAMMAR.as_bytes(), other);
    assert_eq!(code, "invalid-format");
    assert!(message.contains("built with the unit grammar"), "{message}");
    for bytes in [b"not a model".to_vec(), ours()[..60].to_vec()] {
        assert_eq!(refused(GRAMMAR.as_bytes(), bytes).0, "invalid-format");
    }
}

#[test]
fn transducers_that_do_not_read_are_refused() {
    let language = language();
    let generator = &language.generator;
    let cases: [(Vec<u8>, &str); 6] = [
        (Vec::new(), "invalid-format"),
        (b"not a transducer".to_vec(), "invalid-format"),
        (generator[..generator.len() - 9].to_vec(), "invalid-format"),
        (
            [generator.clone(), generator.clone()].concat(),
            "transducer-count",
        ),
        (b"##foma-net 1.0##\n".to_vec(), "unsupported-format"),
        (b"DHFST\x01\x01\x00".to_vec(), "unsupported-format"),
    ];
    for (bytes, expected) in &cases {
        let as_generator = Language {
            generator: bytes.clone(),
            model: language.model.clone(),
        };
        let error = predictor(&as_generator, PredictorOptions::default())
            .err()
            .expect("the generator is refused");
        assert_eq!(error.code().name(), *expected, "{error}");
        assert!(error.message().starts_with("the generator"), "{error}");
        let options = PredictorOptions {
            output_transliterator: Some(bytes.as_slice()),
            ..PredictorOptions::default()
        };
        let error = predictor(&language, options)
            .err()
            .expect("the transliterator is refused");
        assert_eq!(error.code().name(), *expected, "{error}");
        assert!(
            error.message().starts_with("the output transliterator"),
            "{error}"
        );
    }
}

#[test]
fn a_predictor_is_shared_between_threads() {
    fn shared<T: Send + Sync>() {}
    shared::<Predictor>();
    shared::<Session>();
    let predictor = Arc::new(predictor(&language(), PredictorOptions::default()).unwrap());
    assert_eq!(predictor.face("noun"), ["+N+Sg+Nom", "+N+Pl+Nom"]);
    assert!(predictor.face("verb").is_empty());
    let words = std::thread::scope(|scope| {
        let runs = ["girji", "guolli"].map(|stem| {
            let predictor = Arc::clone(&predictor);
            scope.spawn(move || {
                let mut session = Session::new(predictor);
                session.type_letters(stem).unwrap();
                session.pick(stem).unwrap();
                session.pick("+N+Sg+Nom").unwrap();
                session.commit().unwrap()
            })
        });
        runs.map(|run| run.join().unwrap())
    });
    assert_eq!(words, ["girji", "guoli"]);
}

#[test]
fn every_kind_has_its_own_name() {
    let mut names: Vec<&str> = UNIT_KINDS.iter().map(|&kind| kind_name(kind)).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), UNIT_KINDS.len());
}
