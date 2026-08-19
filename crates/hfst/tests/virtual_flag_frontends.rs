use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
use hfst::xfst_compiler::XfstCompiler;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

const LEFT_FLAG: &str = "@U.LEFT.VALUE@";
const RIGHT_FLAG: &str = "@P.RIGHT.VALUE@";

fn flag_path<B: AlgebraBackend>(flag: Option<&str>) -> HfstTransducer<B> {
    let mut path = match flag {
        Some(flag) => HfstTransducer::new_symbol(flag).expect("valid flag symbol"),
        None => HfstTransducer::new_symbol("shared").expect("valid ordinary symbol"),
    };
    if flag.is_some() {
        let tail = HfstTransducer::new_symbol("shared").expect("valid ordinary symbol");
        path.concatenate(&tail, true)
            .expect("concatenate flag fixture");
    }
    path
}

fn eager_reference<B: AlgebraBackend>(
    right_flag: Option<&str>,
    config: &EngineConfig,
) -> HfstTransducer<B> {
    let mut left = flag_path(Some(LEFT_FLAG));
    let mut right = flag_path(right_flag);
    left.harmonize_flag_diacritics(&mut right, true)
        .expect("eager flag harmonization");
    left.compose_with_config(&right, true, config)
        .expect("eager composition");
    left
}

fn xre_result<B: AlgebraBackend>(
    right_flag: Option<&str>,
    config: &EngineConfig,
) -> HfstTransducer<B> {
    let left = flag_path(Some(LEFT_FLAG));
    let right = flag_path(right_flag);
    let mut compiler = XreCompiler::<B>::new();
    compiler.set_expand_definitions(true);
    compiler.set_flag_harmonization(true);
    compiler.set_flag_is_epsilon(config.flag_is_epsilon_in_composition);
    compiler.set_xerox_composition(config.xerox_composition);
    compiler.define_transducer("L", &left);
    compiler.define_transducer("R", &right);
    compiler.compile("L .o. R").expect("compile XRE compose")
}

fn xfst_result(right_flag: Option<&str>, config: &EngineConfig) -> HfstTransducer<StdVectorFst> {
    let right = right_flag
        .map(|flag| format!("[ \"{flag}\" shared ]"))
        .unwrap_or_else(|| "shared".to_string());
    let script = format!(
        "set minimal OFF\n\
         set harmonize-flags ON\n\
         set flag-is-epsilon {}\n\
         set xerox-composition {}\n\
         define L [ \"{LEFT_FLAG}\" shared ] ;\n\
         define R {right} ;\n\
         regex R ;\n\
         regex L ;\n\
         compose net\n",
        if config.flag_is_epsilon_in_composition {
            "ON"
        } else {
            "OFF"
        },
        if config.xerox_composition {
            "ON"
        } else {
            "OFF"
        }
    );
    let mut compiler = XfstCompiler::<StdVectorFst>::new_with_impl();
    assert_eq!(compiler.parse(&script), 0, "XFST script failed");
    assert_eq!(compiler.get_stack().len(), 1, "compose must leave one net");
    let top = *compiler.get_stack().last().expect("one XFST result");
    compiler.net(top).clone()
}

#[test]
// [spec:hfst:req:virtual-flag-algebra.frontend-compose/test]
fn xre_virtual_flags_match_eager() {
    for right_flag in [None, Some(RIGHT_FLAG)] {
        let expected = eager_reference::<StdVectorFst>(right_flag, &EngineConfig::default());
        let actual = xre_result::<StdVectorFst>(right_flag, &EngineConfig::default());
        assert!(
            actual.compare(&expected, true).expect("compare XRE result"),
            "XRE virtual composition differs for right flag {right_flag:?}"
        );
    }
}

#[test]
// [spec:hfst:req:virtual-flag-algebra.frontend-compose/test]
fn xfst_virtual_flags_match_eager() {
    for right_flag in [None, Some(RIGHT_FLAG)] {
        let expected = eager_reference::<StdVectorFst>(right_flag, &EngineConfig::default());
        let actual = xfst_result(right_flag, &EngineConfig::default());
        assert!(
            actual
                .compare(&expected, true)
                .expect("compare XFST result"),
            "XFST virtual composition differs for right flag {right_flag:?}"
        );
    }
}

#[test]
fn composition_chain_finalization_preserves_result() {
    let expression = "[ a:b ] .o. [ b:c ] .o. [ c:d ]";
    let expected = HfstTransducer::<StdVectorFst>::new_symbol_pair("a", "d")
        .expect("construct expected composition");

    let mut xre = XreCompiler::<StdVectorFst>::new();
    let xre_result = xre.compile(expression).expect("compile XRE chain");
    assert!(
        xre_result
            .compare(&expected, true)
            .expect("compare XRE chain")
    );

    let mut xfst = XfstCompiler::<StdVectorFst>::new_with_impl();
    assert_eq!(xfst.parse(&format!("regex {expression} ;\n")), 0);
    let top = *xfst.get_stack().last().expect("one XFST chain result");
    assert!(
        xfst.net(top)
            .compare(&expected, true)
            .expect("compare XFST chain")
    );
}

#[cfg(feature = "foma")]
#[test]
// [spec:hfst:req:virtual-flag-algebra.frontend-compose/test]
fn xre_foma_virtual_flags_match_eager() {
    use hfst::backend_foma::FomaTransducer;

    for right_flag in [None, Some(RIGHT_FLAG)] {
        let expected = eager_reference::<FomaTransducer>(right_flag, &EngineConfig::default());
        let actual = xre_result::<FomaTransducer>(right_flag, &EngineConfig::default());
        assert!(
            actual.compare(&expected, true).expect("compare Foma XRE"),
            "Foma XRE virtual composition differs for right flag {right_flag:?}"
        );
    }
}

fn special_configs() -> [EngineConfig; 3] {
    [
        EngineConfig {
            flag_is_epsilon_in_composition: true,
            ..EngineConfig::default()
        },
        EngineConfig {
            xerox_composition: true,
            ..EngineConfig::default()
        },
        EngineConfig {
            flag_is_epsilon_in_composition: true,
            xerox_composition: true,
            ..EngineConfig::default()
        },
    ]
}

#[test]
// [spec:hfst:req:virtual-flag-algebra.special-compose/test]
fn xre_special_modes_match_eager() {
    for config in special_configs() {
        let expected = eager_reference::<StdVectorFst>(Some(RIGHT_FLAG), &config);
        let actual = xre_result::<StdVectorFst>(Some(RIGHT_FLAG), &config);
        assert!(
            actual.compare(&expected, true).expect("compare XRE result"),
            "XRE special mode differs from eager reference: {config:?}"
        );
    }
}

#[test]
// [spec:hfst:req:virtual-flag-algebra.special-compose/test]
fn xfst_special_modes_match_eager() {
    for config in special_configs() {
        let expected = eager_reference::<StdVectorFst>(Some(RIGHT_FLAG), &config);
        let actual = xfst_result(Some(RIGHT_FLAG), &config);
        assert!(
            actual
                .compare(&expected, true)
                .expect("compare XFST result"),
            "XFST special mode differs from eager reference: {config:?}"
        );
    }
}

#[cfg(feature = "foma")]
#[test]
// [spec:hfst:req:virtual-flag-algebra.special-compose/test]
fn xre_foma_special_modes_match_eager() {
    use hfst::backend_foma::FomaTransducer;

    for config in special_configs() {
        let expected = eager_reference::<FomaTransducer>(Some(RIGHT_FLAG), &config);
        let actual = xre_result::<FomaTransducer>(Some(RIGHT_FLAG), &config);
        assert!(
            actual.compare(&expected, true).expect("compare Foma XRE"),
            "Foma XRE special mode differs from eager reference: {config:?}"
        );
    }
}
