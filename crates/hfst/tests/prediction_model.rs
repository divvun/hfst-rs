//! A model built through the library's public surface: a small generator
//! compiled from lexc decides which readings count, and the kal fixture
//! grammar cuts Greenlandic readings into units.

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::lexc::LexcCompiler;
use hfst::prediction::model::{
    Ambiguity, Backoff, Model, ModelBuilder, ModelSettings, generator_accepts,
};
use hfst::prediction::units::{AnalysisTokenizer, UnitGrammar, UnitKey};
use hfst::transducer::Transducer;
use hfst_openfst::StdVectorFst;

const KAL: &[u8] = include_bytes!("fixtures/prediction/kal.toml");

const GENERATOR: &str = "Multichar_Symbols +N +V +Ind +Abs +Rel +Sg +Pl +3Sg +3Pl +NIQ \
                         +Der/vn +Gram/IV\n\
                         LEXICON Root\n\
                         nuna+N+Abs+Sg:nuna # ;\n\
                         nuna+N+Abs+Pl:nunat # ;\n\
                         nuna+N+Rel+Sg:nunap # ;\n\
                         oqar+V+Gram/IV+Ind+3Sg:oqarpoq # ;\n\
                         oqar+V+Gram/IV+Ind+3Pl:oqarput # ;\n\
                         oqar+NIQ+Der/vn+N+Abs+Sg:oqarneq # ;\n";

fn generator() -> Transducer {
    let compiled = LexcCompiler::<StdVectorFst>::new()
        .compile(GENERATOR)
        .expect("the generator compiles");
    ConversionFunctions::hfst_transducer_to_hfst_ol(&compiled)
        .expect("the generator converts to optimized lookup")
}

fn tokenizer(generator: &Transducer) -> AnalysisTokenizer {
    AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table())
}

const TEXT: &str = "nuna\tnuna+N+Abs+Sg\t0.000000\n\n\
                    oqarpoq\toqar+V+Gram/IV+Ind+3Sg\t0.000000\n\
                    oqarpoq\toqar+V+Ind+3Sg\t0.000000\n\n\
                    nunap\tnuna+N+Rel+Sg\t0.000000\n\n\
                    oqarneq\toqar+NIQ+Der/vn+N+Abs+Sg\t0.000000\n\n\
                    nunat\tnuna+N+Abs+Pl\t0.000000\n\n\
                    oqarput\toqar+V+Gram/IV+Ind+3Pl\t0.000000\n\n\
                    qqq\tqqq+?\tinf\n\n\
                    nuna\tnuna+N+Abs+Sg\t0.000000\n\n";

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn the_generator_decides_which_readings_count() {
    let generator = generator();
    let accepts = generator_accepts(&generator);
    assert!(accepts("nuna+N+Abs+Sg"));
    assert!(accepts("oqar+NIQ+Der/vn+N+Abs+Sg"));
    assert!(!accepts("oqar+V+Ind+3Sg"));
    assert!(!accepts("nuna+N+Rel+Pl"));
    assert!(!accepts("nuna"));
    assert!(!accepts(""));
    assert!(accepts("nuna+N+Abs+Sg"));
    assert!(!accepts("nuna"));
}

// [spec:hfst:req:prediction-model.input/test]
// [spec:hfst:req:prediction-model.faces/test]
// [spec:hfst:req:prediction-model.rank-only/test]
#[test]
fn a_kal_model_counts_what_the_generator_accepts() {
    let generator = generator();
    let tokenizer = tokenizer(&generator);
    let grammar = UnitGrammar::from_toml(KAL).expect("the kal grammar reads");
    let settings = ModelSettings {
        order: 3,
        backoff: Backoff::StupidBackoff,
        ambiguity: Ambiguity::EqualShares,
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
    assert_eq!(report.tokens, 8);
    assert_eq!(report.readings, 8);
    assert_eq!(report.counted, 7);
    assert_eq!(report.refused_by_generator, 1);
    assert_eq!(report.tokens_without_reading, 1);
    let bytes = builder.write().expect("the model writes");

    let model = Model::from_bytes(&bytes, &grammar).expect("the model reads");
    let key = |key: &str| UnitKey::from(key);
    assert_eq!(model.count(&[key("oqar"), key("+V+Ind+3Sg")]), 1.0);
    assert_eq!(model.count(&[key("oqar"), key("+NIQ+Der/vn")]), 1.0);
    assert_eq!(model.count(&[key("+V+Ind+3Pl"), key("nuna")]), 0.0);
    assert_eq!(model.face("noun"), [key("+N+Abs+Sg"), key("+N+Abs+Pl")]);
    assert_eq!(model.face("verb"), [key("+V+Ind+3Pl"), key("+V+Ind+3Sg")]);

    let candidates = [
        (key("+N+Ins+Sg"), 0.0),
        (key("+N+Abs+Pl"), 0.0),
        (key("+N+Abs+Sg"), 0.0),
        (key("+N+Rel+Sg"), 0.0),
    ];
    let order = model.rank(&[key("+V+Ind+3Sg"), key("nuna")], &candidates);
    assert_eq!(order, [3, 2, 1, 0]);
}
