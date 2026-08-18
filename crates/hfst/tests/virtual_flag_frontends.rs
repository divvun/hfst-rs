use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
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

fn eager_reference<B: AlgebraBackend>(right_flag: Option<&str>) -> HfstTransducer<B> {
    let mut left = flag_path(Some(LEFT_FLAG));
    let mut right = flag_path(right_flag);
    left.harmonize_flag_diacritics(&mut right, true)
        .expect("eager flag harmonization");
    left.compose(&right, true).expect("eager composition");
    left
}

fn xre_result<B: AlgebraBackend>(right_flag: Option<&str>) -> HfstTransducer<B> {
    let left = flag_path(Some(LEFT_FLAG));
    let right = flag_path(right_flag);
    let mut compiler = XreCompiler::<B>::new();
    compiler.set_expand_definitions(true);
    compiler.set_flag_harmonization(true);
    compiler.set_xerox_composition(false);
    compiler.define_transducer("L", &left);
    compiler.define_transducer("R", &right);
    compiler.compile("L .o. R").expect("compile XRE compose")
}

fn xfst_result(right_flag: Option<&str>) -> HfstTransducer<StdVectorFst> {
    let right = right_flag
        .map(|flag| format!("[ \"{flag}\" shared ]"))
        .unwrap_or_else(|| "shared".to_string());
    let script = format!(
        "set minimal OFF\n\
         set harmonize-flags ON\n\
         set xerox-composition OFF\n\
         define L [ \"{LEFT_FLAG}\" shared ] ;\n\
         define R {right} ;\n\
         regex R ;\n\
         regex L ;\n\
         compose net\n"
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
        let expected = eager_reference::<StdVectorFst>(right_flag);
        let actual = xre_result::<StdVectorFst>(right_flag);
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
        let expected = eager_reference::<StdVectorFst>(right_flag);
        let actual = xfst_result(right_flag);
        assert!(
            actual
                .compare(&expected, true)
                .expect("compare XFST result"),
            "XFST virtual composition differs for right flag {right_flag:?}"
        );
    }
}

#[cfg(feature = "foma")]
#[test]
// [spec:hfst:req:virtual-flag-algebra.frontend-compose/test]
fn xre_foma_virtual_flags_match_eager() {
    use hfst::backend_foma::FomaTransducer;

    for right_flag in [None, Some(RIGHT_FLAG)] {
        let expected = eager_reference::<FomaTransducer>(right_flag);
        let actual = xre_result::<FomaTransducer>(right_flag);
        assert!(
            actual.compare(&expected, true).expect("compare Foma XRE"),
            "Foma XRE virtual composition differs for right flag {right_flag:?}"
        );
    }
}
